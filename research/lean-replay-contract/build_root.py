#!/usr/bin/env python3
"""Build the logical root of the source-local Lean replay contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import mimetypes
from pathlib import Path
import sys
from typing import Any

import rfc8785


ROOT = Path(__file__).resolve().parent
EXCLUDED_NAMES = {"root.json"}
EXCLUDED_PARTS = {"__pycache__", ".pytest_cache"}


def sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def media_type(path: Path) -> str:
    if path.suffix == ".json":
        return "application/json"
    if path.suffix == ".py":
        return "text/x-python"
    if path.suffix in {".md", ".txt"}:
        return "text/plain"
    return mimetypes.guess_type(path.name)[0] or "application/octet-stream"


def package_files(root: Path = ROOT) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*")
        if path.is_file()
        and path.name not in EXCLUDED_NAMES
        and not any(part in EXCLUDED_PARTS for part in path.relative_to(root).parts)
    )


def descriptor(root: Path = ROOT) -> dict[str, Any]:
    files = []
    for path in package_files(root):
        value = path.read_bytes()
        files.append(
            {
                "media_type": media_type(path),
                "path": path.relative_to(root).as_posix(),
                "sha256": sha256_bytes(value),
                "size": len(value),
            }
        )
    return {"schema": "vela.logical-package-root.v1", "files": files}


def build(root: Path = ROOT) -> dict[str, Any]:
    value = descriptor(root)
    jcs = rfc8785.dumps(value)
    return {
        "schema": "vela.package-root-result.v1",
        "authority_effect": "none",
        "package_id": "vela-science/lean-replay-contract",
        "package_version": "0.0.0-source-local",
        "package_root": sha256_bytes(jcs),
        "descriptor_jcs": jcs.decode("utf-8"),
        "file_count": len(value["files"]),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--directory", type=Path, default=ROOT)
    arguments = parser.parse_args()
    try:
        result = build(arguments.directory.resolve(strict=True))
    except (OSError, ValueError, rfc8785.CanonicalizationError) as error:
        print(json.dumps({"ok": False, "error": str(error)}, sort_keys=True))
        return 1
    sys.stdout.write(json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

