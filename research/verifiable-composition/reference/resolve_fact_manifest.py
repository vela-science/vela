#!/usr/bin/env python3
"""Read-only first-party resolver for one ADR 0004 fact manifest."""

from __future__ import annotations

import sys

from fact_manifest import ManifestError, resolve_bytes
from projection_cli import arguments, emit, input_failure, read_regular


def main() -> int:
    options = arguments(
        "Resolve one exact bounded fact manifest without writing frontier state"
    )
    try:
        raw = read_regular(options.manifest)
    except ManifestError as error:
        emit(input_failure(error))
        return 1
    _, result = resolve_bytes(raw)
    emit(result)
    return 1 if result["dependency_status"] == "unresolvable" else 0


if __name__ == "__main__":
    sys.exit(main())
