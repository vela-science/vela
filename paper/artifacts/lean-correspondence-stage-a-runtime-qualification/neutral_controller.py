"""Fail-closed provider-call derivation for a future authorized neutral attempt."""

from __future__ import annotations

from typing import Any


def exact_count(value: Any, label: str) -> int:
    if type(value) is not int or value < 0:
        raise ValueError(f"{label}_not_exact_nonnegative_integer")
    return value


def derive_provider_calls(
    endpoint_write_receipts: list[dict[str, Any]],
    *,
    bridge: Any,
    runner: Any,
    terminal: Any,
    custody: Any,
) -> int:
    for index, receipt in enumerate(endpoint_write_receipts, start=1):
        if set(receipt) != {"type", "provider_calls"}:
            raise ValueError("endpoint_write_receipt_not_closed")
        if receipt["type"] != "endpoint_attempt":
            raise ValueError("endpoint_write_receipt_type")
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
