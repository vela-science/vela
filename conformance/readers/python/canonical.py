"""RFC 8785 canonical JSON used by the independent Python reader."""

from __future__ import annotations

import rfc8785


def canonical_bytes(value: object) -> bytes:
    """Encode one I-JSON value using the maintained RFC 8785 package."""
    return rfc8785.dumps(value)
