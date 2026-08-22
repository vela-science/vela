"""Fail-closed provider-call derivation for a future authorized neutral attempt."""

from __future__ import annotations

import re
from typing import Any

REQUEST_CUSTODY_KEYS = {
    "schema",
    "content_type",
    "bytes",
    "sha256",
    "payload_encoding",
    "decode_count",
    "provider_schema_bytes",
    "provider_schema_sha256",
    "provider_schema_occurrences",
    "endpoint_write_prepared",
}
SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")


def exact_count(value: Any, label: str) -> int:
    if type(value) is not int or value < 0:
        raise ValueError(f"{label}_not_exact_nonnegative_integer")
    return value


def validate_request_custody(value: Any) -> None:
    if type(value) is not dict or set(value) != REQUEST_CUSTODY_KEYS:
        raise ValueError("request_custody_not_closed")
    for key in ("bytes", "provider_schema_bytes"):
        if type(value[key]) is not int or value[key] <= 0:
            raise ValueError("request_custody_positive_integer")
    if exact_count(value["decode_count"], "request_decode_count") != 1:
        raise ValueError("request_custody_decode_count")
    if (
        exact_count(value["provider_schema_occurrences"], "request_schema_occurrences")
        != 1
    ):
        raise ValueError("request_custody_schema_occurrences")
    if (
        value["schema"] != "vela.lossless-provider-request-custody.v1"
        or value["content_type"] != "application/json"
        or value["payload_encoding"] != "base64-rfc4648-canonical"
        or type(value["sha256"]) is not str
        or SHA256.fullmatch(value["sha256"]) is None
        or type(value["provider_schema_sha256"]) is not str
        or SHA256.fullmatch(value["provider_schema_sha256"]) is None
        or value["endpoint_write_prepared"] is not True
    ):
        raise ValueError("request_custody_semantics")


def derive_provider_calls(
    endpoint_write_receipts: list[dict[str, Any]],
    *,
    bridge: Any,
    runner: Any,
    terminal: Any,
    custody: Any,
) -> int:
    for index, receipt in enumerate(endpoint_write_receipts, start=1):
        if type(receipt) is not dict or set(receipt) != {
            "type",
            "provider_calls",
            "request_custody",
        }:
            raise ValueError("endpoint_write_receipt_not_closed")
        if receipt["type"] != "endpoint_attempt":
            raise ValueError("endpoint_write_receipt_type")
        validate_request_custody(receipt["request_custody"])
        if exact_count(receipt["provider_calls"], "endpoint_receipt") != index:
            raise ValueError("endpoint_write_receipt_sequence")
    derived = len(endpoint_write_receipts)
    for label, value in (
        ("bridge", bridge),
        ("runner", runner),
        ("terminal", terminal),
        ("custody", custody),
    ):
        if exact_count(value, label) != derived:
            raise ValueError("provider_call_cross_layer_drift")
    return derived
