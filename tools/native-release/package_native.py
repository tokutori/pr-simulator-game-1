import argparse
import hashlib
import html
import json
import os
from pathlib import Path
import re
import shutil
import stat
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
    "docs/aircraft-json-guide.md",
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
    actual = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True, encoding="utf-8").strip()
    if actual != source_sha:
        raise ValueError(f"checkout {actual} differs from requested source {source_sha}")
    status = subprocess.check_output(
        ["git", "status", "--porcelain", "--untracked-files=normal"], cwd=root, text=True, encoding="utf-8")
    if status:
        raise ValueError("versioned Native packages require a clean fixed-source checkout")


def binary_name(target: str) -> str:
    return "birdman-game-bevy" + (".exe" if TARGETS[target][1] == "windows" else "")


def server_binary_name(target: str) -> str:
    return "birdman-game-server" + (".exe" if TARGETS[target][1] == "windows" else "")


def notice_features() -> list[str]:
    return ["birdman-game-host/server-ui", "openxr"]


def native_features(target: str) -> list[str]:
    return ["openxr"] if TARGETS[target][1] in ("windows", "linux") else []


def contract_hash(root: Path) -> str:
    generated = (root / "crates/birdman-game-runtime/src/generated_contract.rs").read_text(encoding="utf-8")
    matches = re.findall(r'pub const APPLICATION_CONTRACT_SHA256\s*:\s*&str\s*=\s*"([0-9a-f]{64})";', generated)
    if len(matches) != 1:
        raise ValueError("the canonical application contract fingerprint is missing")
    return matches[0]


def web_bundle_files(web: Path, source_sha: str, expected_contract: str) -> tuple[str, list[Path]]:
    manifest_path = web / "web-bundle-manifest.json"
    for path in (web, manifest_path):
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode) or getattr(info, "st_file_attributes", 0) & 0x400:
            raise ValueError("Web bundle root and manifest must be regular entries")
    with manifest_path.open("rb") as stream:
        manifest_bytes = stream.read(1_048_577)
    if len(manifest_bytes) > 1_048_576:
        raise ValueError("Web bundle manifest exceeds its byte limit")
    manifest = json.loads(manifest_bytes)
    if (set(manifest) != {"schema_version", "source_sha", "contract_sha256", "files"}
            or type(manifest["schema_version"]) is not int or manifest["schema_version"] != 1
            or manifest["source_sha"] != source_sha
            or manifest["contract_sha256"] != expected_contract):
        raise ValueError("Web bundle source or application contract differs")
    files, names = [manifest_path], set()
    if not 1 <= len(manifest["files"]) <= 256:
        raise ValueError("Web bundle file count is invalid")
    total = 0
    for entry in manifest["files"]:
        if set(entry) != {"path", "size", "sha256", "content_type"}:
            raise ValueError("Web file metadata is invalid")
        name = entry["path"]
        if (not isinstance(name, str) or len(name) > 240
                or any(not re.fullmatch(r"[a-zA-Z0-9_-][a-zA-Z0-9_.-]*", part) for part in name.split("/"))
                or name == "web-bundle-manifest.json" or name.startswith("api/") or name in names):
            raise ValueError("Web file path is invalid")
        path = web
        for part in name.split("/"):
            path /= part
            info = path.lstat()
            if stat.S_ISLNK(info.st_mode) or getattr(info, "st_file_attributes", 0) & 0x400:
                raise ValueError("Web bundle contains a linked file")
        if (not path.is_file() or not path.resolve().is_relative_to(web.resolve())
                or type(entry["size"]) is not int or not 0 <= entry["size"] <= 32 * 1_048_576
                or path.stat().st_size != entry["size"] or digest(path) != entry["sha256"]):
            raise ValueError("Web file bytes differ from the manifest")
        total += entry["size"]
        if total > 128 * 1_048_576:
            raise ValueError("Web bundle exceeds its byte limit")
        names.add(name)
        files.append(path)
    if "index.html" not in names:
        raise ValueError("Web bundle has no entry point")
    return hashlib.sha256(manifest_bytes).hexdigest(), files


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


def stage_binary(root: Path, binary: Path, server_binary: Path, target: str,
                 source_sha: str, output: Path) -> None:
    require_source(root, source_sha)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="birdman-native-cli-") as directory:
        staging = Path(directory)
        _, system, architecture, _, _ = TARGETS[target]
        version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
        binaries = []
        for original, name, features in (
                (binary, binary_name(target), native_features(target)),
                (server_binary, server_binary_name(target), ["server-ui"])):
            validate_header(original, target)
            executable = staging / name
            shutil.copy2(original, executable)
            executable.chmod(0o755)
            before = {path.name for path in staging.iterdir()}
            results = {}
            for option in ("--help", "--version"):
                result = subprocess.run([str(executable), option], cwd=staging, check=True,
                                        capture_output=True, text=True, encoding="utf-8", timeout=30)
                results[option] = result.stdout.strip()
            program = name.removesuffix(".exe")
            expected_version = f"{program} {version} source {source_sha} {system} {architecture}"
            if results["--version"] != expected_version:
                raise ValueError(f"unexpected native version: {results['--version']!r}")
            if program not in results["--help"] or "--help" not in results["--help"]:
                raise ValueError("native --help did not describe the application")
            if program == "birdman-game-bevy" and (
                    ("--openxr" in results["--help"]) != ("openxr" in native_features(target))):
                raise ValueError(f"native --help does not match the release features for {target}")
            if {path.name for path in staging.iterdir()} != before:
                raise ValueError("CPU-only CLI unexpectedly created files")
            binaries.append({"name": name, "features": features, "sha256": digest(executable),
                             "cpu_cli": results})
        metadata = {
            "schema_version": 2,
            "source_sha": source_sha,
            "contract_sha256": contract_hash(root),
            "target": target,
            "binaries": binaries,
            "cargo_lock_sha256": digest(root / "Cargo.lock"),
            "rustc": subprocess.check_output(["rustc", "-vV"], text=True, encoding="utf-8").strip(),
            "workflow_run": os.environ.get("NATIVE_WORKFLOW_RUN", "local"),
        }
        write_json(staging / "BUILD-INFO.json", metadata)
        with tarfile.open(output / f"{target}.tar.gz", "w:gz") as archive:
            for path in sorted(staging.iterdir()):
                archive.add(path, arcname=path.name)


def append_source_form_notices(root: Path, source_sha: str, notice: Path, features: list[str],
                               manifest: Path, targets: tuple[str, ...]) -> list[dict]:
    """Generate MPL source locations from actual package sources, including path patches."""
    command = ["cargo", "metadata", "--format-version", "1", "--locked", "--manifest-path",
               str(manifest)]
    if features:
        command.extend(["--features", ",".join(features)])
    packages, reachable = {}, set()
    for target in targets:
        metadata = json.loads(subprocess.check_output(command + ["--filter-platform", target],
                                                     cwd=root, text=True, encoding="utf-8"))
        packages.update({package["id"]: package for package in metadata["packages"]})
        entry = next(package["id"] for package in metadata["packages"]
                     if Path(package["manifest_path"]).resolve() ==
                     manifest.resolve())
        nodes = {node["id"]: [dependency["pkg"] for dependency in node["deps"]
                 if any(kind["kind"] != "dev" for kind in dependency["dep_kinds"])]
                 for node in metadata["resolve"]["nodes"]}
        pending, seen = [entry], set()
        while pending:
            current = pending.pop()
            if current not in seen:
                seen.add(current)
                pending.extend(nodes.get(current, []))
        reachable.update(seen)
    locked = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))["package"]
    sources = []
    for package in sorted((packages[key] for key in reachable), key=lambda value: (value["name"], value["version"])):
        if "MPL-2.0" not in (package.get("license") or ""):
            continue
        source = package.get("source")
        if source == "registry+https://github.com/rust-lang/crates.io-index":
            item = next(item for item in locked if item["name"] == package["name"]
                        and item["version"] == package["version"] and item.get("source") == source)
            url = f"https://crates.io/api/v1/crates/{package['name']}/{package['version']}/download"
            checksum = item["checksum"]
        elif source is None:
            relative = Path(package["manifest_path"]).resolve().parent.relative_to(root.resolve())
            url = f"https://github.com/tokutori/pr-simulator-game-1/tree/{source_sha}/{relative.as_posix()}"
            checksum = None
        elif source.startswith("git+") and "#" in source:
            repository, revision = source[4:].rsplit("#", 1)
            url = f"{repository.split('?')[0].removesuffix('.git')}/tree/{revision}"
            checksum = None
        else:
            raise ValueError(f"MPL source location requires an explicit source mapping: {source}")
        sources.append({"name": package["name"], "version": package["version"],
                        "source_form": url, "registry_checksum": checksum})
    if sources:
        section = '<section><h2>MPL-2.0 Source Code Form</h2><p>The following sources correspond to the distributed packages. '
        section += 'MPL-2.0 applies to these source files; its full terms appear above.</p><ul>'
        for source in sources:
            label = html.escape(f"{source['name']} {source['version']}")
            section += f'<li><a href="{html.escape(source["source_form"], quote=True)}">{label}: source archive</a></li>'
        section += '</ul></section>\n'
        notice.write_text(notice.read_text(encoding="utf-8").replace('</html>', section + '</html>'),
                          encoding="utf-8", newline="\n")
    # Upstream metadata omits these inherited/table notices; retain the registered originals.
    names = {packages[key]["name"] for key in reachable}
    additions = []
    if "earcut" in names:
        original = (root / "assets/earcut.LICENSE.txt").read_text(encoding="utf-8")
        isc = "ISC License\n" + original.split("ISC License\n", 1)[1]
        additions.append("<section><h2>earcut: inherited Mapbox notice</h2><pre>" + html.escape(isc) + "</pre></section>")
    if "unicode-width" in names:
        original = (root / "assets/unicode-width.LICENSE.txt").read_text(encoding="utf-8")
        current = notice.read_text(encoding="utf-8")
        copyrights = [line for line in original.splitlines() if line.startswith("Copyright") and html.escape(line) not in current]
        if copyrights:
            additions.append("<section><h2>unicode-width: generated table copyright</h2><p>The MIT terms above apply.</p><pre>" +
                             html.escape("\n".join(copyrights)) + "</pre></section>")
    if additions:
        notice.write_text(notice.read_text(encoding="utf-8").replace("</html>", "\n".join(additions) + "\n</html>"),
                          encoding="utf-8", newline="\n")
    return sources


def generate_licenses(root: Path, cargo_about: Path, source_sha: str, output: Path, web: bool = False) -> None:
    require_source(root, source_sha)
    version = subprocess.check_output([str(cargo_about), "--version"], text=True, encoding="utf-8").strip()
    if version != "cargo-about 0.9.2":
        raise ValueError(f"unexpected license tool: {version}")
    output.mkdir(parents=True, exist_ok=True)
    targets = ("wasm32-unknown-unknown",) if web else tuple(TARGETS)
    features = [] if web else notice_features()
    manifest = root / ("crates/birdman-game-wasm/Cargo.toml" if web else "crates/birdman-game-bevy/Cargo.toml")
    command = [str(cargo_about), "generate", "--locked", "--fail", "--manifest-path",
               str(manifest), "--config",
               str(root / "tools/native-release/about.toml"), "--output-file",
               str(output / "THIRD-PARTY-LICENSES.html")]
    for feature in features:
        command.extend(["--features", feature])
    for target in targets:
        command.extend(["--target", target])
    command.append(str(root / "tools/native-release/about.hbs"))
    subprocess.run(command, cwd=root, check=True)
    source_forms = append_source_form_notices(root, source_sha, output / "THIRD-PARTY-LICENSES.html", features, manifest, targets)
    notice = output / "THIRD-PARTY-LICENSES.html"
    notice.write_text(notice.read_text(encoding="utf-8").replace("NOTICE_TARGETS", html.escape(", ".join(targets)))
                      .replace("NOTICE_LOCK_SHA256", digest(root / "Cargo.lock")), encoding="utf-8", newline="\n")
    write_json(output / "LICENSE-INFO.json", {
        "source_sha": source_sha,
        "cargo_lock_sha256": digest(root / "Cargo.lock"),
        "tool": version,
        "targets": list(targets),
        "features": features,
        "licenses_sha256": digest(output / "THIRD-PARTY-LICENSES.html"),
        "source_forms": source_forms,
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
        "同梱の `birdman-game-server` は管理画面からWeb配信を開始できる。"
        "表示されたアドレスを同じPCまたはLANのブラウザで開く。\n\n"
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


def validate_license_info(info, *, source_sha: str, lock_hash: str,
                          targets: list[str], features: list[str], licenses_hash: str) -> None:
    if (info.get("source_sha") != source_sha or info.get("cargo_lock_sha256") != lock_hash
            or info.get("targets") != targets or info.get("features") != features
            or info.get("licenses_sha256") != licenses_hash):
        raise ValueError("license artifact provenance does not match the source")


def assemble(root: Path, binaries: Path, licenses: Path, web: Path,
             source_sha: str, output: Path) -> None:
    require_source(root, source_sha)
    expected_contract = contract_hash(root)
    web_hash, web_files = web_bundle_files(web, source_sha, expected_contract)
    license_info = json.loads((licenses / "LICENSE-INFO.json").read_text(encoding="utf-8"))
    lock_hash = digest(root / "Cargo.lock")
    validate_license_info(license_info, source_sha=source_sha, lock_hash=lock_hash,
                          targets=list(TARGETS),
                          features=notice_features(),
                          licenses_hash=digest(licenses / "THIRD-PARTY-LICENSES.html"))
    validate_license_info(json.loads((web / "LICENSE-INFO.json").read_text(encoding="utf-8")),
                          source_sha=source_sha, lock_hash=lock_hash,
                          targets=["wasm32-unknown-unknown"], features=[],
                          licenses_hash=digest(web / "THIRD-PARTY-LICENSES.html"))
    output.mkdir(parents=True, exist_ok=True)
    packages = []
    for target in TARGETS:
        name = f"birdman-native-{target}-{source_sha[:12]}"
        with tempfile.TemporaryDirectory(prefix="birdman-native-package-") as directory:
            staging = Path(directory) / name
            staging.mkdir()
            with tarfile.open(binaries / f"{target}.tar.gz", "r:gz") as raw:
                if sorted(raw.getnames()) != sorted([binary_name(target), server_binary_name(target), "BUILD-INFO.json"]):
                    raise ValueError(f"unexpected raw artifact entries for {target}")
                for member in raw.getmembers():
                    if not member.isfile():
                        raise ValueError("raw artifact contains a non-file entry")
                    with raw.extractfile(member) as source, (staging / member.name).open("wb") as destination:
                        shutil.copyfileobj(source, destination)
            info = json.loads((staging / "BUILD-INFO.json").read_text(encoding="utf-8"))
            expected_binaries = {binary_name(target): native_features(target),
                                 server_binary_name(target): ["server-ui"]}
            actual_binaries = info.get("binaries", [])
            if (info.get("schema_version") != 2 or info["source_sha"] != source_sha
                    or info["contract_sha256"] != expected_contract or info["target"] != target
                    or info["cargo_lock_sha256"] != lock_hash
                    or len(actual_binaries) != 2
                    or {entry["name"] for entry in actual_binaries} != set(expected_binaries)):
                raise ValueError(f"binary artifact provenance differs for {target}")
            for entry in actual_binaries:
                executable = staging / entry["name"]
                if entry["features"] != expected_binaries[entry["name"]] or entry["sha256"] != digest(executable):
                    raise ValueError(f"binary artifact provenance differs for {target}")
                validate_header(executable, target)
                executable.chmod(0o755)
            copy_notices(root, licenses, staging)
            for source in web_files:
                destination = staging / "web" / source.relative_to(web)
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, destination)
            copied_web_hash, _ = web_bundle_files(staging / "web", source_sha, expected_contract)
            if copied_web_hash != web_hash:
                raise ValueError("copied Web bundle differs from the verified input")
            if TARGETS[target][1] == "windows":
                package = output / f"{name}.zip"
                with zipfile.ZipFile(package, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                    for path in sorted(staging.rglob("*")):
                        if path.is_file():
                            archive.write(path, path.relative_to(staging.parent).as_posix())
            else:
                package = output / f"{name}.tar.gz"

                def permissions(member):
                    if member.name in {f"{name}/{binary}" for binary in expected_binaries}:
                        member.mode = 0o755
                    return member

                with tarfile.open(package, "w:gz") as archive:
                    archive.add(staging, arcname=name, filter=permissions)
            packages.append({"target": target, "features": native_features(target),
                             "file": package.name, "sha256": digest(package)})
    write_json(output / "SOURCE.json", {
        "source_sha": source_sha,
        "cargo_lock_sha256": lock_hash,
        "contract_sha256": expected_contract,
        "web_manifest_sha256": web_hash,
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
            subparser.add_argument("--server-binary", type=Path, required=True)
            subparser.add_argument("--target", choices=TARGETS, required=True)
        elif command == "licenses":
            subparser.add_argument("--cargo-about", type=Path, required=True)
            subparser.add_argument("--web", action="store_true", help="Generate the actual WebAssembly dependency notice")
        else:
            subparser.add_argument("--binaries", type=Path, required=True)
            subparser.add_argument("--licenses", type=Path, required=True)
            subparser.add_argument("--web-bundle", type=Path, required=True)
    arguments = parser.parse_args()
    if arguments.command == "matrix":
        print(json.dumps({"include": [{"target": target, "runner": spec[0],
                                       "features": ",".join(native_features(target)),
                                       "notice_features": ",".join(notice_features())}
                                      for target, spec in TARGETS.items()]}))
    elif arguments.command == "stage":
        stage_binary(arguments.root, arguments.binary.resolve(), arguments.server_binary.resolve(),
                     arguments.target, arguments.source_sha, arguments.output)
    elif arguments.command == "licenses":
        generate_licenses(arguments.root, arguments.cargo_about.resolve(), arguments.source_sha, arguments.output, arguments.web)
    else:
        assemble(arguments.root, arguments.binaries, arguments.licenses, arguments.web_bundle.resolve(),
                 arguments.source_sha, arguments.output)


if __name__ == "__main__":
    main()
