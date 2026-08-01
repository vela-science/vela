#!/usr/bin/env python3
"""Safely verify a packed Vela foreign-reference package."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path, PurePosixPath

MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_FILES = 64


def safe_members(archive: tarfile.TarFile) -> list[tarfile.TarInfo]:
    members = archive.getmembers()
    if len(members) > MAX_FILES:
        raise ValueError(f"archive has more than {MAX_FILES} entries")
    total = 0
    names: set[str] = set()
    files: list[tarfile.TarInfo] = []
    for member in members:
        path = PurePosixPath(member.name)
        if (
            path.is_absolute()
            or ".." in path.parts
            or not path.parts
            or path.parts[0] not in {"reference.v1.json", "objects"}
        ):
            raise ValueError(f"unsafe archive path: {member.name}")
        if member.name in names:
            raise ValueError(f"duplicate archive path: {member.name}")
        names.add(member.name)
        if not member.isfile():
            raise ValueError(f"archive entry is not a regular file: {member.name}")
        total += member.size
        if total > MAX_ARCHIVE_BYTES:
            raise ValueError("archive expands beyond the 64 MiB bound")
        files.append(member)
    if "reference.v1.json" not in names:
        raise ValueError("archive omits reference.v1.json")
    return files


def extract(archive_path: Path, destination: Path) -> None:
    with tarfile.open(archive_path, mode="r:gz") as archive:
        for member in safe_members(archive):
            output = destination.joinpath(*PurePosixPath(member.name).parts)
            output.parent.mkdir(parents=True, exist_ok=True)
            source = archive.extractfile(member)
            if source is None:
                raise ValueError(f"cannot read archive entry: {member.name}")
            output.write_bytes(source.read())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--expected-root")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    archive_path = args.archive.expanduser().resolve()
    verifier = Path(__file__).resolve().parent / "verify_foreign_reference.py"
    try:
        with tempfile.TemporaryDirectory(prefix="vela-foreign-reference-") as raw:
            package = Path(raw)
            extract(archive_path, package)
            command = [
                sys.executable,
                str(verifier),
                "--package-root",
                str(package),
                "--json",
            ]
            result = subprocess.run(
                command,
                check=True,
                capture_output=True,
                text=True,
            )
            assessment = json.loads(result.stdout)
            reference = json.loads(
                (package / "reference.v1.json").read_text(encoding="utf-8")
            )
            if (
                args.expected_root is not None
                and assessment["reference_root"] != args.expected_root
            ):
                raise ValueError(
                    "reference root mismatch: "
                    f"expected {args.expected_root}, "
                    f"observed {assessment['reference_root']}"
                )
            output = {
                "schema": "vela.foreign-reference-archive-verification.v1",
                "ok": True,
                "archive": str(archive_path),
                "reference_root": assessment["reference_root"],
                "object_set_root": assessment["object_set_root"],
                "object_count": len(reference["objects"]),
                "semantic_chain": "verified",
                "authority_signature": "verified",
                "local_standing_effect": assessment["local_standing_effect"],
            }
            if args.json:
                print(json.dumps(output, sort_keys=True, separators=(",", ":")))
            else:
                print(
                    "foreign-reference-archive: ok "
                    f"(root={output['reference_root']}, "
                    f"objects={output['object_count']}, "
                    "semantic_chain=verified, authority_signature=verified, "
                    "local_standing_effect=none)"
                )
        return 0
    except (
        OSError,
        ValueError,
        json.JSONDecodeError,
        subprocess.CalledProcessError,
        tarfile.TarError,
    ) as error:
        detail = (
            error.stderr.strip()
            if isinstance(error, subprocess.CalledProcessError) and error.stderr
            else str(error)
        )
        print(f"foreign-reference-archive: {detail}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
