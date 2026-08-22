#!/usr/bin/env python3
"""Recompute the recursive artifact manifest for the held runtime candidate."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

PACKAGE = Path(__file__).resolve().parent
ARTIFACT_ROOT = PACKAGE / "artifact-root.json"


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical_root(value: Any) -> str:
    return digest(canonical_bytes(value))


def pretty_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def main() -> None:
    entries = []
    for path in sorted(PACKAGE.rglob("*")):
        if (
            not path.is_file()
            or path == ARTIFACT_ROOT
            or "__pycache__" in path.parts
            or path.suffix == ".pyc"
        ):
            continue
        raw = path.read_bytes()
        entries.append(
            {
                "path": path.relative_to(PACKAGE).as_posix(),
                "bytes": len(raw),
                "sha256": digest(raw),
            }
        )
    artifact = {
        "schema": "vela.lean-correspondence-stage-a-runtime-qualification-artifact-root.v2",
        "entries": entries,
    }
    artifact["artifact_root"] = canonical_root(artifact)
    ARTIFACT_ROOT.write_bytes(pretty_bytes(artifact))


if __name__ == "__main__":
    main()
