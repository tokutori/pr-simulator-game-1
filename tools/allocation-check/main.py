import json
import hashlib
import os
from pathlib import Path
import subprocess


def native_library(build_output, root, target, package):
    artifacts = [
        artifact
        for line in build_output.splitlines()
        for artifact in [json.loads(line)]
        if artifact.get("reason") == "compiler-artifact"
        and artifact["target"]["name"] == package.replace("-", "_")
    ]
    if len(artifacts) != 1:
        raise RuntimeError(f"Expected exactly one native {package} compiler artifact")
    artifact = artifacts[0]
    manifest = Path(artifact["manifest_path"]).resolve(strict=True)
    if manifest != (root / "crates" / package / "Cargo.toml").resolve():
        raise RuntimeError(f"Compiler artifact does not belong to this {package} manifest")
    profile = artifact["profile"]
    if profile["opt_level"] != "0" or not profile["debug_assertions"] or profile["test"]:
        raise RuntimeError("Allocation probe requires unoptimized native debug libraries")
    libraries = [Path(filename) for filename in artifact["filenames"] if filename.endswith(".rlib")]
    if len(libraries) != 1:
        raise RuntimeError(f"Expected exactly one freshly reported {package} rlib")
    library = libraries[0].resolve(strict=True)
    if not library.is_relative_to(target.resolve()):
        raise RuntimeError("Compiler artifact is outside the dedicated probe target")
    print(
        json.dumps(
            {
                "manifest": str(manifest),
                "profile": profile,
                "rlib": str(library),
                "rlib_sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
                "fresh": artifact["fresh"],
            }
        ),
        flush=True,
    )
    return library


def main():
    root = Path(__file__).resolve().parents[2]
    probe = root / "tools" / "allocation-check" / "main.rs"
    subprocess.run(
        ["rustfmt", "--edition", "2024", "--check", str(probe)],
        cwd=root,
        check=True,
    )
    version = subprocess.run(
        ["rustc", "-vV"], cwd=root, check=True, capture_output=True, encoding="utf-8"
    )
    host = next(
        line.removeprefix("host: ")
        for line in version.stdout.splitlines()
        if line.startswith("host: ")
    )
    target = root / "target" / "hybrid-allocation-check"
    build = subprocess.run(
        [
            "cargo",
            "build",
            "-p",
            "birdman-game-core",
            "-p",
            "birdman-game-content",
            "-p",
            "birdman-game-app",
            "--locked",
            "--target",
            host,
            "--target-dir",
            str(target),
            "--message-format=json",
        ],
        cwd=root,
        capture_output=True,
        encoding="utf-8",
    )
    print(build.stderr, end="")
    build.check_returncode()
    library = native_library(build.stdout, root, target, "birdman-game-core")
    content = native_library(build.stdout, root, target, "birdman-game-content")
    application = native_library(build.stdout, root, target, "birdman-game-app")
    executable = "hybrid-allocation-probe.exe" if os.name == "nt" else "hybrid-allocation-probe"
    binary = target / host / "debug" / executable
    subprocess.run(
        [
            "rustc",
            "--edition=2024",
            "-Dwarnings",
            "-Copt-level=0",
            "-Cdebuginfo=2",
            "--extern",
            f"birdman_game_core={library}",
            "--extern",
            f"birdman_game_content={content}",
            "--extern",
            f"birdman_game_app={application}",
            "-L",
            f"dependency={library.parent / 'deps'}",
            "-L",
            f"dependency={target / 'debug' / 'deps'}",
            str(probe),
            "-o",
            str(binary),
        ],
        cwd=root,
        check=True,
    )
    print(f"native debug allocation probe: {host}", flush=True)
    subprocess.run([str(binary)], cwd=root, check=True)


if __name__ == "__main__":
    main()
