#!/usr/bin/env python3
"""Regenerate or check the deterministic ADR 0004 standards fixtures."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent
REFERENCE = ROOT / "reference"
FIXTURE = ROOT / "fixtures/standards-baseline"
sys.path.insert(0, str(REFERENCE))

from fact_manifest import build_envelope  # noqa: E402
from standards_baseline import (  # noqa: E402
    build_dsse_envelope,
    build_lock,
    build_manifest_for_inspection,
    build_statement,
    document_bytes,
    load_fixture_inspection,
    sha256_bytes,
)


def expected_files() -> dict[Path, bytes]:
    inspection = load_fixture_inspection(FIXTURE)
    manifest = build_manifest_for_inspection(inspection)
    statement = build_statement(manifest)
    envelope = build_dsse_envelope(statement)
    lock = build_lock(
        manifest,
        statement,
        envelope,
        semantics_root=sha256_bytes((FIXTURE / "semantics.md").read_bytes()),
    )
    return {
        FIXTURE / "fact-manifest.json": document_bytes(manifest),
        FIXTURE / "vela-profile.json": document_bytes(build_envelope(manifest)),
        FIXTURE / "in-toto-statement.json": document_bytes(statement),
        FIXTURE / "science.lock": document_bytes(lock),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--write",
        action="store_true",
        help="write deterministic fixtures instead of checking them",
    )
    arguments = parser.parse_args()
    drift: list[str] = []
    for path, expected in expected_files().items():
        if arguments.write:
            path.write_bytes(expected)
        elif not path.exists() or path.read_bytes() != expected:
            drift.append(path.name)
    if drift:
        print(f"standards fixture drift: {', '.join(drift)}", file=sys.stderr)
        return 1
    if arguments.write:
        print("standards fixtures regenerated")
    else:
        print("standards fixtures match deterministic builders")
    return 0


if __name__ == "__main__":
    sys.exit(main())
