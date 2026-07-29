"""Dependency-free canonical JSON used by the Python conformance reader."""

from __future__ import annotations

import json


def canonical_bytes(value: object) -> bytes:
    """Encode the bounded vela.canonical-json/v1 value domain."""
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode("utf-8")
