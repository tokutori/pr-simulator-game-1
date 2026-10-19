import io
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import package_native as packaging


SOURCE_SHA = "a" * 40


def header(target: str) -> bytes:
    kind, machine = packaging.TARGETS[target][3:]
    data = bytearray(128)
    if kind == "pe":
        data[:2] = b"MZ"
        struct.pack_into("<I", data, 60, 64)
        data[64:68] = b"PE\0\0"
        struct.pack_into("<H", data, 68, machine)
    elif kind == "elf":
        data[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<H", data, 18, machine)
    else:
        data[:4] = b"\xcf\xfa\xed\xfe"
        struct.pack_into("<I", data, 4, machine)
    return bytes(data)


def raw_artifact(directory: Path, root: Path, target: str, binary: bytes,
                 source_sha: str = SOURCE_SHA, additional: str = "") -> None:
    info = {
        "source_sha": source_sha,
        "target": target,
        "cargo_lock_sha256": packaging.digest(root / "Cargo.lock"),
        "binary_sha256": hashlib.sha256(binary).hexdigest(),
    }
    entries = {packaging.binary_name(target): binary, "BUILD-INFO.json": json.dumps(info).encode()}
    if additional:
        entries[additional] = b"unexpected"
    with tarfile.open(directory / f"{target}.tar.gz", "w:gz") as archive:
        for name, content in entries.items():
            entry = tarfile.TarInfo(name)
            entry.size = len(content)
            archive.addfile(entry, io.BytesIO(content))


class NativePackagingTests(unittest.TestCase):
    def test_six_native_header_formats_and_architectures(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "binary"
            self.assertEqual(len(packaging.TARGETS), 6)
            self.assertEqual(len({spec[0] for spec in packaging.TARGETS.values()}), 6)
            for target in packaging.TARGETS:
                with self.subTest(target=target):
                    binary.write_bytes(header(target))
                    packaging.validate_header(binary, target)
                    other = next(candidate for candidate in packaging.TARGETS if candidate != target)
                    with self.assertRaises(ValueError):
                        packaging.validate_header(binary, other)
                    for invalid in (b"", b"exit stub", header(target)[:5]):
                        binary.write_bytes(invalid)
                        with self.assertRaises(ValueError):
                            packaging.validate_header(binary, target)

    def test_source_requires_complete_exact_sha(self) -> None:
        with patch("package_native.subprocess.check_output", return_value=SOURCE_SHA + "\n"):
            packaging.require_source(Path("."), SOURCE_SHA)
            for invalid in ("a" * 7, "b" * 40, SOURCE_SHA.upper(), "main"):
                with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                    packaging.require_source(Path("."), invalid)

    def test_cpu_cli_runs_copied_binary_without_gpu_from_external_cwd(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "source"
            root.mkdir()
            (root / "Cargo.lock").write_text("fixture lock", encoding="utf-8")
            (root / "Cargo.toml").write_text('[workspace.package]\nversion="0.1.0"\n', encoding="utf-8")
            target = "x86_64-pc-windows-msvc"
            binary = root / "original.exe"
            binary.write_bytes(header(target))
            output = root / "output"
            calls = []

            def run(command, **options):
                executable = Path(command[0])
                calls.append((command, options))
                self.assertNotEqual(executable, binary)
                self.assertNotEqual(options["cwd"], root)
                self.assertEqual(executable.read_bytes(), binary.read_bytes())
                text = ("birdman-game-bevy --help" if command[1] == "--help" else
                        f"birdman-game-bevy 0.1.0 source {SOURCE_SHA} windows x86_64")
                return subprocess.CompletedProcess(command, 0, text, "")

            with patch("package_native.require_source"), patch("package_native.subprocess.run", side_effect=run), \
                    patch("package_native.subprocess.check_output", return_value="rustc fixture"):
                packaging.stage_binary(root, binary, target, SOURCE_SHA, output)
            self.assertEqual([call[0][1] for call in calls], ["--help", "--version"])
            with tarfile.open(output / f"{target}.tar.gz") as archive:
                info = json.load(archive.extractfile("BUILD-INFO.json"))
                self.assertEqual(info["source_sha"], SOURCE_SHA)
                self.assertEqual(info["binary_sha256"], packaging.digest(binary))
                self.assertEqual(sorted(archive.getnames()), ["BUILD-INFO.json", "birdman-game-bevy.exe"])

    def fixture(self, directory: str):
        root = Path(directory) / "source"
        binaries = Path(directory) / "binaries"
        licenses = Path(directory) / "licenses"
        for path in (root, binaries, licenses):
            path.mkdir()
        (root / "Cargo.lock").write_text("fixture lock", encoding="utf-8")
        notice = licenses / "THIRD-PARTY-LICENSES.html"
        notice.write_text("license fixture", encoding="utf-8")
        packaging.write_json(licenses / "LICENSE-INFO.json", {
            "source_sha": SOURCE_SHA,
            "cargo_lock_sha256": packaging.digest(root / "Cargo.lock"),
            "targets": list(packaging.TARGETS),
            "licenses_sha256": packaging.digest(notice),
        })
        for target in packaging.TARGETS:
            raw_artifact(binaries, root, target, header(target))
        return root, binaries, licenses, Path(directory) / "output"

    def test_packages_all_six_with_matching_checksums_and_unix_executable_mode(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, binaries, licenses, output = self.fixture(directory)

            def notices(_root, _licenses, destination):
                (destination / "THIRD-PARTY-LICENSES.html").write_text("notice", encoding="utf-8")

            with patch("package_native.require_source"), patch("package_native.copy_notices", side_effect=notices):
                packaging.assemble(root, binaries, licenses, SOURCE_SHA, output)
            source = json.loads((output / "SOURCE.json").read_text(encoding="utf-8"))
            self.assertEqual(len(source["packages"]), 6)
            self.assertEqual(source["source_sha"], SOURCE_SHA)
            for package in source["packages"]:
                path = output / package["file"]
                self.assertEqual(package["sha256"], packaging.digest(path))
                if path.suffix == ".zip":
                    with zipfile.ZipFile(path) as archive:
                        self.assertEqual(len(archive.namelist()), 3)
                        self.assertTrue(any(name.endswith("/birdman-game-bevy.exe") for name in archive.namelist()))
                else:
                    with tarfile.open(path) as archive:
                        executable = next(member for member in archive if member.name.endswith("/birdman-game-bevy"))
                        self.assertEqual(executable.mode & 0o111, 0o111)
            for line in (output / "SHA256SUMS").read_text(encoding="utf-8").splitlines():
                checksum, name = line.split("  ")
                self.assertEqual(checksum, packaging.digest(output / name))

    def test_rejects_mixed_source_architecture_and_unexpected_archive_paths(self) -> None:
        for mutation in ("source", "architecture", "path", "hash", "missing"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root, binaries, licenses, output = self.fixture(directory)
                target = next(iter(packaging.TARGETS))
                if mutation == "source":
                    raw_artifact(binaries, root, target, header(target), source_sha="b" * 40)
                elif mutation == "architecture":
                    raw_artifact(binaries, root, target, header("aarch64-pc-windows-msvc"))
                elif mutation == "path":
                    raw_artifact(binaries, root, target, header(target), additional="../outside")
                elif mutation == "hash":
                    (licenses / "THIRD-PARTY-LICENSES.html").write_text("modified", encoding="utf-8")
                else:
                    (binaries / f"{target}.tar.gz").unlink()
                with patch("package_native.require_source"), patch("package_native.copy_notices"), \
                        self.assertRaises((ValueError, FileNotFoundError)):
                    packaging.assemble(root, binaries, licenses, SOURCE_SHA, output)

    def test_licenses_use_locked_native_package_and_all_target_union(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, _, licenses, _ = self.fixture(directory)
            command = []

            def run(arguments, **_options):
                command.extend(arguments)

            with patch("package_native.require_source"), \
                    patch("package_native.subprocess.check_output", return_value="cargo-about 0.9.2\n"), \
                    patch("package_native.subprocess.run", side_effect=run):
                packaging.generate_licenses(root, Path("cargo-about"), SOURCE_SHA, licenses)
            self.assertIn("--locked", command)
            self.assertIn("--fail", command)
            self.assertNotIn("--workspace", command)
            self.assertEqual(command.count("--target"), 6)
            self.assertTrue(set(packaging.TARGETS).issubset(command))

    def test_registered_assets_font_and_full_notices_are_copied_and_verified(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, _, licenses, output = self.fixture(directory)
            output.mkdir()
            for entry in ("LICENSE", "THIRD_PARTY_DATA.md", *packaging.DOCUMENTATION_FILES,
                          "assets/fonts/MPLUS1p-Regular.ttf", "assets/fonts/MPLUS1p-OFL.txt"):
                path = root / entry
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("registered fixture " + entry, encoding="utf-8")
            assets = ["assets/fonts/MPLUS1p-Regular.ttf", "assets/fonts/MPLUS1p-OFL.txt"]
            manifest = "\n".join(f'[[assets]]\npath="{entry}"\nsha256="{packaging.digest(root / entry)}"' for entry in assets)
            (root / "assets/manifest.toml").write_text(manifest, encoding="utf-8")
            tracked = ("\0".join(assets + ["assets/manifest.toml", "assets/test-fonts/not-for-distribution.ttf"]) + "\0").encode()
            with patch("package_native.subprocess.check_output", return_value=tracked):
                packaging.copy_notices(root, licenses, output)
                for entry in assets + ["LICENSE", "THIRD_PARTY_DATA.md", *packaging.DOCUMENTATION_FILES]:
                    self.assertEqual((output / entry).read_bytes(), (root / entry).read_bytes())
                self.assertEqual((output / "THIRD-PARTY-LICENSES.html").read_bytes(),
                                 (licenses / "THIRD-PARTY-LICENSES.html").read_bytes())
                guide = output / "docs/native-guide.md"
                self.assertTrue(guide.is_file())
                self.assertIn("(docs/native-guide.md)", (output / "README.md").read_text(encoding="utf-8"))
                self.assertNotEqual((output / "README.md").read_bytes(), guide.read_bytes())
                self.assertFalse((output / "assets/test-fonts").exists())
                (root / assets[0]).write_text("modified font", encoding="utf-8")
                with self.assertRaises(ValueError):
                    packaging.copy_notices(root, licenses, output)


if __name__ == "__main__":
    unittest.main()
