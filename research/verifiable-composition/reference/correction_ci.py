#!/usr/bin/env python3
"""Removable correction-aware CI adapter over the ADR 0004 resolver."""

from __future__ import annotations

import sys

from fact_manifest import (
    ManifestError,
    correction_ci_projection,
    resolve_bytes,
)
from projection_cli import arguments, emit, input_failure, read_regular


def main() -> int:
    options = arguments("Project correction-aware CI from one exact fact manifest")
    try:
        raw = read_regular(options.manifest)
        _, resolution = resolve_bytes(raw)
    except ManifestError as error:
        resolution = input_failure(error)
    result = correction_ci_projection(resolution)
    emit(result)
    return int(result["suggested_exit_code"])


if __name__ == "__main__":
    sys.exit(main())
