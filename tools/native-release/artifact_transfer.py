"""Compare Action transfers with an independently downloaded, API-digest-checked ZIP."""

import argparse
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re
import stat
import subprocess
import sys
import zipfile

from package_native import digest, require_source, validate_license_info, write_json


FILES = ("LICENSE-INFO.json", "THIRD-PARTY-LICENSES.html")
MAX_ARCHIVE_BYTES = 16 * 1024 * 1024
MAX_CONTENT_BYTES = 64 * 1024 * 1024
NAME = re.compile(r"native-licenses-([0-9a-f]{40})")


def select_artifact(pages):
    candidates = [artifact for page in pages for artifact in page["artifacts"]
                  if artifact["expired"] is False and NAME.fullmatch(artifact["name"])]
    if not candidates:
        raise ValueError("no unexpired native license artifact exists")
    return max(candidates, key=lambda artifact: (
        datetime.fromisoformat(artifact["created_at"].replace("Z", "+00:00")), artifact["id"]))


def source_identity(artifact) -> str:
    name = NAME.fullmatch(artifact["name"])
    if (artifact["expired"] is not False or name is None
            or type(artifact["id"]) is not int or artifact["id"] <= 0
            or type(artifact["workflow_run"]["id"]) is not int or artifact["workflow_run"]["id"] <= 0
            or re.fullmatch(r"sha256:[0-9a-f]{64}", artifact.get("digest") or "") is None
            or type(artifact["size_in_bytes"]) is not int
            or not 0 < artifact["size_in_bytes"] <= MAX_ARCHIVE_BYTES):
        raise ValueError("expected an unexpired bounded native license artifact with an API SHA-256 digest")
    return name[1]


def build_reference(archive: Path, artifact, source: Path):
    source_sha = source_identity(artifact)
    require_source(source, source_sha)
    if archive.stat().st_size > MAX_ARCHIVE_BYTES or "sha256:" + digest(archive) != artifact["digest"]:
        raise ValueError("REST ZIP differs from the source artifact API digest")
    # The source run may intentionally build a different commit from its workflow head.
    # Resolve the matrix from that selected source, while retaining this PR's verifier.
    matrix = json.loads(subprocess.check_output(
        [sys.executable, str(source / "tools/native-release/package_native.py"), "matrix"], text=True))
    with zipfile.ZipFile(archive) as zipped:
        entries = zipped.infolist()
        if (sorted(entry.filename for entry in entries) != list(FILES)
                or any(entry.is_dir() or stat.S_IFMT(entry.external_attr >> 16) not in (0, stat.S_IFREG)
                       or entry.flag_bits & 1 for entry in entries)
                or sum(entry.file_size for entry in entries) > MAX_CONTENT_BYTES):
            raise ValueError("source ZIP must contain exactly the two regular license files")
        contents = {entry.filename: zipped.read(entry) for entry in entries}
    hashes = {name: hashlib.sha256(content).hexdigest() for name, content in contents.items()}
    validate_license_info(json.loads(contents["LICENSE-INFO.json"]),
                          source_sha=source_sha, lock_hash=digest(source / "Cargo.lock"),
                          targets=[item["target"] for item in matrix["include"]],
                          features=sorted({feature for item in matrix["include"]
                                           for feature in item["features"].split(",") if feature}),
                          licenses_hash=hashes["THIRD-PARTY-LICENSES.html"])
    return {"artifact_id": artifact["id"], "run_id": artifact["workflow_run"]["id"],
            "source_sha": source_sha, "archive_sha256": artifact["digest"][7:], "files": hashes}


def verify_payload(payload: Path, reference) -> None:
    if payload.is_symlink() or not payload.is_dir():
        raise ValueError("transferred payload must be a regular directory")
    entries = list(payload.iterdir())
    if (sorted(path.name for path in entries) != list(FILES)
            or any(path.is_symlink() or not path.is_file() for path in entries)
            or sum(path.stat().st_size for path in entries) > MAX_CONTENT_BYTES
            or {path.name: digest(path) for path in entries} != reference["files"]):
        raise ValueError("transferred file paths or content differ from the independent source ZIP")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    select = commands.add_parser("select")
    select.add_argument("--pages", type=Path, required=True)
    select.add_argument("--output", type=Path, required=True)
    identity = commands.add_parser("identity")
    identity.add_argument("--artifact", type=Path, required=True)
    reference = commands.add_parser("reference")
    reference.add_argument("--archive", type=Path, required=True)
    reference.add_argument("--artifact", type=Path, required=True)
    reference.add_argument("--source", type=Path, required=True)
    reference.add_argument("--output", type=Path, required=True)
    verify = commands.add_parser("verify")
    verify.add_argument("--payload", type=Path, required=True)
    verify.add_argument("--reference", type=Path, required=True)
    arguments = parser.parse_args()
    read = lambda path: json.loads(path.read_text(encoding="utf-8"))
    if arguments.command == "select":
        selected = select_artifact(read(arguments.pages))
        source_identity(selected)
        write_json(arguments.output, selected)
    elif arguments.command == "identity":
        print(source_identity(read(arguments.artifact)))
    elif arguments.command == "reference":
        write_json(arguments.output, build_reference(arguments.archive, read(arguments.artifact), arguments.source))
    else:
        verify_payload(arguments.payload, read(arguments.reference))


if __name__ == "__main__":
    main()
