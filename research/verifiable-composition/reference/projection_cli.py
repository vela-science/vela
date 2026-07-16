"""Shared bounded-file plumbing for removable ADR 0004 command adapters."""

from __future__ import annotations

import argparse
import json
import stat
from pathlib import Path
from typing import Any

from fact_manifest import (
    MAX_DOCUMENT_BYTES,
    ManifestError,
    unresolvable_projection,
)


def arguments(description: str) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=description)
    parser.add_argument("--manifest", required=True, type=Path)
    return parser.parse_args()


def read_regular(path: Path) -> bytes:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise ManifestError("input:file_unavailable") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise ManifestError("input:file_not_regular")
    if metadata.st_size > MAX_DOCUMENT_BYTES:
        raise ManifestError("input:file_oversized")
    try:
        raw = path.read_bytes()
    except OSError as error:
        raise ManifestError("input:file_unreadable") from error
    if len(raw) != metadata.st_size:
        raise ManifestError("input:file_changed_during_read")
    return raw


def input_failure(error: ManifestError) -> dict[str, Any]:
    return unresolvable_projection(error)


def emit(value: dict[str, Any]) -> None:
    print(
        json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        )
    )
