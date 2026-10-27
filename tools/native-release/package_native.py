import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile


TARGETS = {
    "x86_64-pc-windows-msvc": ("windows-2025", "windows", "x86_64", "pe", 0x8664),
    "aarch64-pc-windows-msvc": ("windows-11-arm", "windows", "aarch64", "pe", 0xAA64),
    "x86_64-unknown-linux-gnu": ("ubuntu-24.04", "linux", "x86_64", "elf", 62),
    "aarch64-unknown-linux-gnu": ("ubuntu-24.04-arm", "linux", "aarch64", "elf", 183),
    "x86_64-apple-darwin": ("macos-15-intel", "macos", "x86_64", "macho", 0x01000007),
    "aarch64-apple-darwin": ("macos-15", "macos", "aarch64", "macho", 0x0100000C),
}

DOCUMENTATION_FILES = (
    "docs/architecture.md",
    "docs/data-and-formats.md",
    "docs/flight-dynamics.md",
    "docs/flight-control.md",
    "docs/gameplay-and-ui.md",
    "docs/rendering-and-platforms.md",
    "docs/native-guide.md",
    "docs/verification.md",
    "docs/formal-verification.md",
)

OPENXR_NOTICES = tuple(
    f"vendor/{crate}/{name}"
    for crate in ("bevy_mod_openxr", "openxr")
    for name in ("LICENSE-MIT", "LICENSE-APACHE", "UPSTREAM.md")
)


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path: Path, value) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def require_source(root: Path, source_sha: str) -> None:
    if re.fullmatch(r"[0-9a-f]{40}", source_sha) is None:
        raise ValueError("source_sha must be a complete lowercase commit SHA")
    actual = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    if actual != source_sha:
        raise ValueError(f"checkout {actual} differs from requested source {source_sha}")


def binary_name(target: str) -> str:
    return "birdman-game-bevy" + (".exe" if TARGETS[target][1] == "windows" else "")


def native_features(target: str) -> list[str]:
    return ["openxr"] if TARGETS[target][1] in ("windows", "linux") else []


def validate_header(binary: Path, target: str) -> None:
    _, _, _, kind, expected = TARGETS[target]
    with binary.open("rb") as stream:
        header = stream.read(64)
        machine = None
        if kind == "pe" and len(header) == 64 and header[:2] == b"MZ":
            stream.seek(struct.unpack_from("<I", header, 60)[0])
            coff = stream.read(6)
            if len(coff) == 6 and coff[:4] == b"PE\0\0":
                machine = struct.unpack_from("<H", coff, 4)[0]
        elif kind == "elf" and len(header) >= 20 and header[:6] == b"\x7fELF\x02\x01":
            machine = struct.unpack_from("<H", header, 18)[0]
        elif kind == "macho" and len(header) >= 8 and header[:4] == b"\xcf\xfa\xed\xfe":
            machine = struct.unpack_from("<I", header, 4)[0]
    if machine != expected:
        raise ValueError(f"binary header does not match {target}: machine={machine}")


def stage_binary(root: Path, binary: Path, target: str, source_sha: str, output: Path) -> None:
    require_source(root, source_sha)
    validate_header(binary, target)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="birdman-native-cli-") as directory:
        staging = Path(directory)
        executable = staging / binary_name(target)
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        results = {}
        for option in ("--help", "--version"):
            result = subprocess.run([str(executable), option], cwd=staging, check=True,
                                    capture_output=True, text=True, encoding="utf-8", timeout=30)
            results[option] = result.stdout.strip()
        _, system, architecture, _, _ = TARGETS[target]
        version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
        expected_version = f"birdman-game-bevy {version} source {source_sha} {system} {architecture}"
        if results["--version"] != expected_version:
            raise ValueError(f"unexpected native version: {results['--version']!r}")
        if "birdman-game-bevy" not in results["--help"] or "--help" not in results["--help"]:
            raise ValueError("native --help did not describe the application")
        if ("--openxr" in results["--help"]) != ("openxr" in native_features(target)):
            raise ValueError(f"native --help does not match the release features for {target}")
        if sorted(path.name for path in staging.iterdir()) != [executable.name]:
            raise ValueError("CPU-only CLI unexpectedly created files")
        metadata = {
            "source_sha": source_sha,
            "target": target,
            "features": native_features(target),
            "binary_sha256": digest(executable),
            "cargo_lock_sha256": digest(root / "Cargo.lock"),
            "rustc": subprocess.check_output(["rustc", "-vV"], text=True).strip(),
            "workflow_run": os.environ.get("NATIVE_WORKFLOW_RUN", "local"),
            "cpu_cli": results,
        }
        write_json(staging / "BUILD-INFO.json", metadata)
        with tarfile.open(output / f"{target}.tar.gz", "w:gz") as archive:
            archive.add(executable, arcname=executable.name)
            archive.add(staging / "BUILD-INFO.json", arcname="BUILD-INFO.json")


def generate_licenses(root: Path, cargo_about: Path, source_sha: str, output: Path) -> None:
    require_source(root, source_sha)
    version = subprocess.check_output([str(cargo_about), "--version"], text=True).strip()
    if version != "cargo-about 0.9.2":
        raise ValueError(f"unexpected license tool: {version}")
    output.mkdir(parents=True, exist_ok=True)
    features = sorted({feature for target in TARGETS for feature in native_features(target)})
    command = [str(cargo_about), "generate", "--locked", "--fail", "--manifest-path",
               str(root / "crates/birdman-game-bevy/Cargo.toml"), "--config",
               str(root / "tools/native-release/about.toml"), "--output-file",
               str(output / "THIRD-PARTY-LICENSES.html")]
    for feature in features:
        command.extend(["--features", feature])
    for target in TARGETS:
        command.extend(["--target", target])
    command.append(str(root / "tools/native-release/about.hbs"))
    subprocess.run(command, cwd=root, check=True)
    write_json(output / "LICENSE-INFO.json", {
        "source_sha": source_sha,
        "cargo_lock_sha256": digest(root / "Cargo.lock"),
        "tool": version,
        "targets": list(TARGETS),
        "features": features,
        "licenses_sha256": digest(output / "THIRD-PARTY-LICENSES.html"),
    })


def copy_notices(root: Path, licenses: Path, staging: Path) -> None:
    for relative in ("LICENSE", "THIRD_PARTY_DATA.md", *DOCUMENTATION_FILES, *OPENXR_NOTICES):
        source = root / relative
        destination = staging / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    (staging / "README.md").write_text(
        "# Native 試用版\n\n"
        "起動方法・動作環境・操作キー・保存先は "
        "[Nativeガイド](docs/native-guide.md) を参照する。\n\n"
        "- [ソースコードのライセンス](LICENSE)\n"
        "- [データ・フォントの利用条件](THIRD_PARTY_DATA.md)\n"
        "- [同梱ライブラリの利用条件](THIRD-PARTY-LICENSES.html)\n\n"
        "Windows/Linux版は `--openxr` でruntimeを初期化し、タイトル画面からHMD表示を選ぶ。"
        "前提runtimeと実機での確認範囲はNativeガイドを参照する。\n\n"
        "ビルド来歴は `BUILD-INFO.json` に記録している。"
        "配布ファイルの照合には、配布ページの `SHA256SUMS` を使用する。\n",
        encoding="utf-8", newline="\n")
    shutil.copy2(licenses / "THIRD-PARTY-LICENSES.html", staging / "THIRD-PARTY-LICENSES.html")
    registered = {asset["path"]: asset["sha256"] for asset in
                  tomllib.loads((root / "assets/manifest.toml").read_text(encoding="utf-8"))["assets"]}
    paths = subprocess.check_output(["git", "ls-files", "-z", "--", "assets"], cwd=root)
    for entry in paths.decode("utf-8").split("\0"):
        if not entry or entry.startswith("assets/test-fonts/"):
            continue
        source = root / entry
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"registered asset must be a regular file: {entry}")
        if entry != "assets/manifest.toml" and digest(source) != registered[entry]:
            raise ValueError(f"asset differs from its registered source: {entry}")
        destination = staging / entry
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    for required in ("assets/fonts/MPLUS1p-Regular.ttf", "assets/fonts/MPLUS1p-OFL.txt"):
        if not (staging / required).is_file():
            raise ValueError(f"required font or notice is missing: {required}")


def assemble(root: Path, binaries: Path, licenses: Path, source_sha: str, output: Path) -> None:
    require_source(root, source_sha)
    license_info = json.loads((licenses / "LICENSE-INFO.json").read_text(encoding="utf-8"))
    lock_hash = digest(root / "Cargo.lock")
    if (license_info["source_sha"] != source_sha or license_info["cargo_lock_sha256"] != lock_hash
            or license_info["targets"] != list(TARGETS)
            or license_info["features"] != sorted({feature for target in TARGETS for feature in native_features(target)})
            or license_info["licenses_sha256"] != digest(licenses / "THIRD-PARTY-LICENSES.html")):
        raise ValueError("license artifact provenance does not match the source")
    output.mkdir(parents=True, exist_ok=True)
    packages = []
    for target in TARGETS:
        name = f"birdman-native-{target}-{source_sha[:12]}"
        with tempfile.TemporaryDirectory(prefix="birdman-native-package-") as directory:
            staging = Path(directory) / name
            staging.mkdir()
            with tarfile.open(binaries / f"{target}.tar.gz", "r:gz") as raw:
                if sorted(raw.getnames()) != sorted([binary_name(target), "BUILD-INFO.json"]):
                    raise ValueError(f"unexpected raw artifact entries for {target}")
                for member in raw.getmembers():
                    if not member.isfile():
                        raise ValueError("raw artifact contains a non-file entry")
                    with raw.extractfile(member) as source, (staging / member.name).open("wb") as destination:
                        shutil.copyfileobj(source, destination)
            executable = staging / binary_name(target)
            executable.chmod(0o755)
            info = json.loads((staging / "BUILD-INFO.json").read_text(encoding="utf-8"))
            if (info["source_sha"] != source_sha or info["target"] != target
                    or info["features"] != native_features(target)
                    or info["cargo_lock_sha256"] != lock_hash
                    or info["binary_sha256"] != digest(executable)):
                raise ValueError(f"binary artifact provenance differs for {target}")
            validate_header(executable, target)
            copy_notices(root, licenses, staging)
            if TARGETS[target][1] == "windows":
                package = output / f"{name}.zip"
                with zipfile.ZipFile(package, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                    for path in sorted(staging.rglob("*")):
                        if path.is_file():
                            archive.write(path, path.relative_to(staging.parent).as_posix())
            else:
                package = output / f"{name}.tar.gz"

                def permissions(member):
                    if member.name == f"{name}/{executable.name}":
                        member.mode = 0o755
                    return member

                with tarfile.open(package, "w:gz") as archive:
                    archive.add(staging, arcname=name, filter=permissions)
            packages.append({"target": target, "features": native_features(target),
                             "file": package.name, "sha256": digest(package)})
    write_json(output / "SOURCE.json", {
        "source_sha": source_sha,
        "cargo_lock_sha256": lock_hash,
        "workflow_run": os.environ.get("NATIVE_WORKFLOW_RUN", "local"),
        "packages": packages,
        "verification": "native-host Release build, binary architecture and CPU-only CLI; GPU unverified",
    })
    sums = [f"{package['sha256']}  {package['file']}" for package in packages]
    sums.append(f"{digest(output / 'SOURCE.json')}  SOURCE.json")
    (output / "SHA256SUMS").write_text("\n".join(sums) + "\n", encoding="utf-8", newline="\n")


def main() -> None:
    parser = argparse.ArgumentParser(description="Validate and package fixed-source native preview artifacts")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("matrix")
    for command in ("stage", "licenses", "assemble"):
        subparser = commands.add_parser(command)
        subparser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
        subparser.add_argument("--source-sha", required=True)
        subparser.add_argument("--output", type=Path, required=True)
        if command == "stage":
            subparser.add_argument("--binary", type=Path, required=True)
            subparser.add_argument("--target", choices=TARGETS, required=True)
        elif command == "licenses":
            subparser.add_argument("--cargo-about", type=Path, required=True)
        else:
            subparser.add_argument("--binaries", type=Path, required=True)
            subparser.add_argument("--licenses", type=Path, required=True)
    arguments = parser.parse_args()
    if arguments.command == "matrix":
        print(json.dumps({"include": [{"target": target, "runner": spec[0],
                                       "features": ",".join(native_features(target))}
                                      for target, spec in TARGETS.items()]}))
    elif arguments.command == "stage":
        stage_binary(arguments.root, arguments.binary.resolve(), arguments.target, arguments.source_sha, arguments.output)
    elif arguments.command == "licenses":
        generate_licenses(arguments.root, arguments.cargo_about.resolve(), arguments.source_sha, arguments.output)
    else:
        assemble(arguments.root, arguments.binaries, arguments.licenses, arguments.source_sha, arguments.output)


if __name__ == "__main__":
    main()
