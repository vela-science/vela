#!/usr/bin/env python3
"""Disposable accepted-state context projection for ADR 0004."""

from __future__ import annotations

import sys

from fact_manifest import (
    ManifestError,
    accepted_context_pack_projection,
    resolve_bytes,
)
from projection_cli import arguments, emit, input_failure, read_regular


def main() -> int:
    options = arguments(
        "Build a compact accepted-state context pack from one exact fact manifest"
    )
    try:
        raw = read_regular(options.manifest)
        envelope, resolution = resolve_bytes(raw)
    except ManifestError as error:
        envelope = None
        resolution = input_failure(error)
    result = accepted_context_pack_projection(envelope, resolution)
    emit(result)
    return 0 if result["dependency_status"] in {"satisfied", "warning"} else 1


if __name__ == "__main__":
    sys.exit(main())
