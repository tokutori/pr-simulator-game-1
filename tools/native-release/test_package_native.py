import io
import hashlib
import json
from contextlib import nullcontext
from pathlib import Path
import struct
import stat
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import artifact_transfer as transfer
import package_native as packaging


SOURCE_SHA = "a" * 40
CONTRACT_SHA = "b" * 64


def fixture_contract(root: Path) -> None:
    generated = root / "crates/birdman-game-runtime/src/generated_contract.rs"
    generated.parent.mkdir(parents=True, exist_ok=True)
    generated.write_text(f'pub const APPLICATION_CONTRACT_SHA256: &str = "{CONTRACT_SHA}";\n', encoding="utf-8")


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
                 source_sha: str = SOURCE_SHA, additional: str = "", features: list[str] | None = None) -> None:
    info = {
        "schema_version": 2,
        "source_sha": source_sha,
        "contract_sha256": CONTRACT_SHA,
        "target": target,
        "binaries": [
            {"name": packaging.binary_name(target),
             "features": packaging.native_features(target) if features is None else features,
             "sha256": hashlib.sha256(binary).hexdigest()},
            {"name": packaging.server_binary_name(target), "features": ["server-ui"],
             "sha256": hashlib.sha256(binary).hexdigest()},
        ],
        "cargo_lock_sha256": packaging.digest(root / "Cargo.lock"),
    }
    entries = {packaging.binary_name(target): binary, packaging.server_binary_name(target): binary,
               "BUILD-INFO.json": json.dumps(info).encode()}
    if additional:
        entries[additional] = b"unexpected"
    with tarfile.open(directory / f"{target}.tar.gz", "w:gz") as archive:
        for name, content in entries.items():
            entry = tarfile.TarInfo(name)
            entry.size = len(content)
            archive.addfile(entry, io.BytesIO(content))


class NativePackagingTests(unittest.TestCase):
    def test_transfer_selects_latest_unexpired_license_across_all_pages(self) -> None:
        older = {"id": 1, "name": f"native-licenses-{SOURCE_SHA}", "expired": False,
                 "created_at": "2026-11-01T00:00:00Z"}
        latest = dict(older, id=102, created_at="2026-11-02T00:00:00Z")
        pages = [{"artifacts": [older] + [dict(older, id=i + 2, name=f"native-raw-{i}")
                                        for i in range(99)]},
                 {"artifacts": [dict(latest, id=101), latest,
                                dict(latest, id=103, expired=True, created_at="2026-11-03T00:00:00Z")]}]
        self.assertEqual(transfer.select_artifact(pages), latest)
        with self.assertRaises(ValueError):
            transfer.select_artifact([{"artifacts": [dict(latest, expired=True)]}])

    def test_initial_and_roundtrip_transfers_match_independent_zip_and_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, _, licenses, _ = self.fixture(directory)
            archive = Path(directory) / "source.zip"
            with zipfile.ZipFile(archive, "w") as zipped:
                for name in transfer.FILES:
                    zipped.write(licenses / name, name)
            artifact = {"id": 9, "name": f"native-licenses-{SOURCE_SHA}", "expired": False,
                        "workflow_run": {"id": 10, "head_sha": "b" * 40},
                        "size_in_bytes": archive.stat().st_size,
                        "digest": "sha256:" + packaging.digest(archive)}
            matrix = json.dumps({"include": [{"target": target,
                                            "features": ",".join(packaging.native_features(target)),
                                            "notice_features": ",".join(packaging.notice_features())}
                                           for target in packaging.TARGETS]})
            with patch("artifact_transfer.require_source"), \
                    patch("artifact_transfer.subprocess.check_output", return_value=matrix):
                reference = transfer.build_reference(archive, artifact, root)
                # This expected receipt exists before either transfer, independently of its payload.
                for phase in ("initial", "roundtrip"):
                    payload = Path(directory) / phase
                    payload.mkdir()
                    for name in transfer.FILES:
                        (payload / name).write_bytes((licenses / name).read_bytes())
                    transfer.verify_payload(payload, reference)
                    for mutation in ("missing", "modified", "extra"):
                        with self.subTest(phase=phase, mutation=mutation):
                            notice = payload / "THIRD-PARTY-LICENSES.html"
                            original = notice.read_bytes()
                            if mutation == "missing":
                                notice.unlink()
                            elif mutation == "modified":
                                notice.write_bytes(original + b"changed")
                            else:
                                (payload / "extra").write_bytes(b"extra")
                            with self.assertRaises(ValueError):
                                transfer.verify_payload(payload, reference)
                            notice.write_bytes(original)
                            (payload / "extra").unlink(missing_ok=True)
                for field in ("source_sha", "cargo_lock_sha256", "targets", "features", "licenses_sha256"):
                    info = json.loads((licenses / "LICENSE-INFO.json").read_text(encoding="utf-8"))
                    del info[field]
                    with self.subTest(missing_provenance=field), self.assertRaises(ValueError):
                        packaging.validate_license_info(info, source_sha=SOURCE_SHA,
                            lock_hash=packaging.digest(root / "Cargo.lock"), targets=list(packaging.TARGETS),
                            features=["openxr"], licenses_hash=packaging.digest(licenses / "THIRD-PARTY-LICENSES.html"))
                for digest in (None, "", "sha256:" + "0" * 64):
                    with self.subTest(digest=digest), self.assertRaises(ValueError):
                        transfer.build_reference(archive, dict(artifact, digest=digest), root)
                # Exact flat names reject traversal, duplicate/extra entries and all non-files.
                for invalid in ("/absolute", "../outside", "extra", "directory/", "LICENSE-INFO.json", "symlink"):
                    bad = Path(directory) / "bad.zip"
                    with zipfile.ZipFile(bad, "w") as zipped:
                        for name in transfer.FILES:
                            if invalid == "symlink" and name == "LICENSE-INFO.json":
                                entry = zipfile.ZipInfo(name)
                                entry.external_attr = (stat.S_IFLNK | 0o777) << 16
                                zipped.writestr(entry, "target")
                            else:
                                zipped.write(licenses / name, name)
                        if invalid != "symlink":
                            # A duplicate is written explicitly to exercise the central directory.
                            with self.assertWarns(UserWarning) if invalid == "LICENSE-INFO.json" else nullcontext():
                                zipped.writestr(invalid, "invalid")
                    metadata = dict(artifact, digest="sha256:" + packaging.digest(bad), size_in_bytes=bad.stat().st_size)
                    with self.subTest(entry=invalid), self.assertRaises(ValueError):
                        transfer.build_reference(bad, metadata, root)

    def test_six_native_header_formats_and_architectures(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "binary"
            self.assertEqual(len(packaging.TARGETS), 6)
            self.assertEqual(len({spec[0] for spec in packaging.TARGETS.values()}), 6)
            for target in packaging.TARGETS:
                with self.subTest(target=target):
                    self.assertEqual(packaging.native_features(target),
                                     [] if "apple-darwin" in target else ["openxr"])
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
            fixture_contract(root)
            target = "x86_64-pc-windows-msvc"
            binary = root / "original.exe"
            binary.write_bytes(header(target))
            server_binary = root / "server-original.exe"
            server_binary.write_bytes(header(target))
            output = root / "output"
            calls = []
            advertise_openxr = True

            def run(command, **options):
                executable = Path(command[0])
                calls.append((command, options))
                self.assertNotEqual(executable, binary)
                self.assertNotEqual(options["cwd"], root)
                self.assertEqual(executable.read_bytes(), binary.read_bytes())
                name = executable.stem
                text = (name + " --help" + (" --openxr" if advertise_openxr and name == "birdman-game-bevy" else "")
                        if command[1] == "--help" else f"{name} 0.1.0 source {SOURCE_SHA} windows x86_64")
                return subprocess.CompletedProcess(command, 0, text, "")

            with patch("package_native.require_source"), patch("package_native.subprocess.run", side_effect=run), \
                    patch("package_native.subprocess.check_output", return_value="rustc fixture"):
                packaging.stage_binary(root, binary, server_binary, target, SOURCE_SHA, output)
            self.assertEqual([call[0][1] for call in calls], ["--help", "--version"] * 2)
            with tarfile.open(output / f"{target}.tar.gz") as archive:
                info = json.load(archive.extractfile("BUILD-INFO.json"))
                self.assertEqual(info["source_sha"], SOURCE_SHA)
                self.assertEqual(info["schema_version"], 2)
                self.assertEqual(info["contract_sha256"], CONTRACT_SHA)
                self.assertEqual(info["binaries"][0]["features"], ["openxr"])
                self.assertEqual(info["binaries"][0]["sha256"], packaging.digest(binary))
                self.assertEqual(info["binaries"][1]["features"], ["server-ui"])
                self.assertEqual(sorted(archive.getnames()), ["BUILD-INFO.json", "birdman-game-bevy.exe", "birdman-game-server.exe"])
            advertise_openxr = False
            with patch("package_native.require_source"), patch("package_native.subprocess.run", side_effect=run), \
                    self.assertRaisesRegex(ValueError, "release features"):
                packaging.stage_binary(root, binary, server_binary, target, SOURCE_SHA, output)

    def fixture(self, directory: str):
        root = Path(directory) / "source"
        binaries = Path(directory) / "binaries"
        licenses = Path(directory) / "licenses"
        for path in (root, binaries, licenses):
            path.mkdir()
        (root / "Cargo.lock").write_text("fixture lock", encoding="utf-8")
        fixture_contract(root)
        web = root / "web"
        web.mkdir()
        (web / "index.html").write_text('<script type="module" src="./application.js"></script>', encoding="utf-8")
        (web / "application.js").write_text("export {};", encoding="utf-8")
        (web / "THIRD-PARTY-LICENSES.html").write_text("web notice", encoding="utf-8")
        packaging.write_json(web / "LICENSE-INFO.json", {
            "source_sha": SOURCE_SHA, "cargo_lock_sha256": packaging.digest(root / "Cargo.lock"),
            "targets": ["wasm32-unknown-unknown"], "features": [],
            "licenses_sha256": packaging.digest(web / "THIRD-PARTY-LICENSES.html"),
        })
        packaging.write_json(web / "web-bundle-manifest.json", {
            "schema_version": 1, "source_sha": SOURCE_SHA, "contract_sha256": CONTRACT_SHA,
            "files": [{"path": name, "size": (web / name).stat().st_size,
                       "sha256": packaging.digest(web / name), "content_type": mime}
                      for name, mime in (("index.html", "text/html; charset=utf-8"),
                                         ("application.js", "application/javascript"),
                                         ("THIRD-PARTY-LICENSES.html", "text/html; charset=utf-8"),
                                         ("LICENSE-INFO.json", "application/json"))],
        })
        notice = licenses / "THIRD-PARTY-LICENSES.html"
        notice.write_text("license fixture", encoding="utf-8")
        packaging.write_json(licenses / "LICENSE-INFO.json", {
            "source_sha": SOURCE_SHA,
            "cargo_lock_sha256": packaging.digest(root / "Cargo.lock"),
            "targets": list(packaging.TARGETS),
            "features": packaging.notice_features(),
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
                packaging.assemble(root, binaries, licenses, root / "web", SOURCE_SHA, output)
            source = json.loads((output / "SOURCE.json").read_text(encoding="utf-8"))
            self.assertEqual(len(source["packages"]), 6)
            self.assertEqual(source["source_sha"], SOURCE_SHA)
            self.assertEqual(source["contract_sha256"], CONTRACT_SHA)
            self.assertEqual(source["web_manifest_sha256"], packaging.digest(root / "web/web-bundle-manifest.json"))
            for package in source["packages"]:
                self.assertEqual(package["features"], packaging.native_features(package["target"]))
                path = output / package["file"]
                self.assertEqual(package["sha256"], packaging.digest(path))
                if path.suffix == ".zip":
                    with zipfile.ZipFile(path) as archive:
                        self.assertEqual(len(archive.namelist()), 9)
                        self.assertTrue(any(name.endswith("/birdman-game-bevy.exe") for name in archive.namelist()))
                        self.assertTrue(any(name.endswith("/birdman-game-server.exe") for name in archive.namelist()))
                        web_manifest = next(name for name in archive.namelist() if name.endswith("/web/web-bundle-manifest.json"))
                        self.assertEqual(archive.read(web_manifest), (root / "web/web-bundle-manifest.json").read_bytes())
                else:
                    with tarfile.open(path) as archive:
                        executable = next(member for member in archive if member.name.endswith("/birdman-game-bevy"))
                        self.assertEqual(executable.mode & 0o111, 0o111)
            for line in (output / "SHA256SUMS").read_text(encoding="utf-8").splitlines():
                checksum, name = line.split("  ")
                self.assertEqual(checksum, packaging.digest(output / name))

    def test_rejects_mixed_source_architecture_and_unexpected_archive_paths(self) -> None:
        for mutation in ("source", "architecture", "path", "hash", "missing", "features", "license-features",
                         "web-source", "web-contract", "web-bytes", "web-path"):
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
                elif mutation == "features":
                    raw_artifact(binaries, root, target, header(target), features=[])
                elif mutation == "license-features":
                    info = json.loads((licenses / "LICENSE-INFO.json").read_text(encoding="utf-8"))
                    info["features"] = []
                    packaging.write_json(licenses / "LICENSE-INFO.json", info)
                elif mutation.startswith("web-"):
                    manifest_path = root / "web/web-bundle-manifest.json"
                    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                    if mutation == "web-source":
                        manifest["source_sha"] = "c" * 40
                    elif mutation == "web-contract":
                        manifest["contract_sha256"] = "d" * 64
                    elif mutation == "web-path":
                        manifest["files"][0]["path"] = "../outside"
                    else:
                        (root / "web/application.js").write_text("changed", encoding="utf-8")
                    packaging.write_json(manifest_path, manifest)
                else:
                    (binaries / f"{target}.tar.gz").unlink()
                with patch("package_native.require_source"), patch("package_native.copy_notices"), \
                        self.assertRaises((ValueError, FileNotFoundError)):
                    packaging.assemble(root, binaries, licenses, root / "web", SOURCE_SHA, output)

    def test_licenses_use_locked_native_package_and_all_target_union(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, _, licenses, _ = self.fixture(directory)
            registry = "registry+https://github.com/rust-lang/crates.io-index"
            (root / "Cargo.lock").write_text(
                f'[[package]]\nname="serialport"\nversion="4.10.1"\nsource="{registry}"\nchecksum="abc"\n', encoding="utf-8")
            (licenses / "THIRD-PARTY-LICENSES.html").write_text('<html></html>', encoding="utf-8")
            packages = [
                {"id": "root", "name": "birdman-game-bevy", "version": "0.1.0", "license": "MIT",
                 "manifest_path": str(root / "crates/birdman-game-bevy/Cargo.toml"), "source": None},
                {"id": "serial", "name": "serialport", "version": "4.10.1", "license": "MPL-2.0", "source": registry},
                {"id": "patched", "name": "patched", "version": "1.0", "license": "MPL-2.0",
                 "manifest_path": str(root / "vendor/patched/Cargo.toml"), "source": None},
                {"id": "dev", "name": "dev-only", "version": "1.0", "license": "MPL-2.0", "source": "invalid"},
            ]
            metadata = {"packages": packages, "resolve": {"nodes": [{"id": "root", "deps": [
                {"pkg": "serial", "dep_kinds": [{"kind": None}]},
                {"pkg": "patched", "dep_kinds": [{"kind": "build"}]},
                {"pkg": "dev", "dep_kinds": [{"kind": "dev"}]},
            ]}]}}
            command, targets = [], []
            def checked_output(arguments, **_options):
                if "--version" in arguments:
                    return "cargo-about 0.9.2\n"
                targets.append(arguments[arguments.index("--filter-platform") + 1])
                return json.dumps(metadata)
            def run(arguments, **_options):
                command.extend(arguments)
            with patch("package_native.require_source"), \
                    patch("package_native.subprocess.check_output", side_effect=checked_output), \
                    patch("package_native.subprocess.run", side_effect=run):
                packaging.generate_licenses(root, Path("cargo-about"), SOURCE_SHA, licenses)
            self.assertIn("--locked", command)
            self.assertIn("--fail", command)
            self.assertNotIn("--workspace", command)
            self.assertEqual(command.count("--target"), 6)
            self.assertEqual(set(targets), set(packaging.TARGETS))
            self.assertTrue(set(packaging.TARGETS).issubset(command))
            self.assertEqual(command[command.index("--features") + 1], "birdman-game-host/server-ui")
            info = json.loads((licenses / "LICENSE-INFO.json").read_text(encoding="utf-8"))
            self.assertEqual(info["features"], packaging.notice_features())
            self.assertEqual(len(info["source_forms"]), 2)
            notice = (licenses / "THIRD-PARTY-LICENSES.html").read_text(encoding="utf-8")
            self.assertIn("https://crates.io/api/v1/crates/serialport/4.10.1/download", notice)
            self.assertIn(f"{SOURCE_SHA}/vendor/patched", notice)
            self.assertNotIn("dev-only", notice)

    def test_registered_assets_font_and_full_notices_are_copied_and_verified(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, _, licenses, output = self.fixture(directory)
            output.mkdir()
            for entry in ("LICENSE", "THIRD_PARTY_DATA.md", *packaging.DOCUMENTATION_FILES, *packaging.OPENXR_NOTICES,
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
                for entry in assets + ["LICENSE", "THIRD_PARTY_DATA.md", *packaging.DOCUMENTATION_FILES,
                                       *packaging.OPENXR_NOTICES]:
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
