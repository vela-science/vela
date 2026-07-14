"""Receipt v1 JSON parsing and RFC 8785 canonicalization.

The frozen Receipt v1 schema leaves extension objects open, so finite decimal
and exponent numbers remain valid. Canonicalization follows JCS/ECMAScript
number rendering; exact integral values outside the binary64 safe-integer
domain must be carried as strings.

The float renderer is adapted from Andrew Rundgren's Apache-2.0 licensed JCS
reference implementation and the MIT-licensed ``rfc8785`` Python package.

Untrusted Receipt v1 JSON is also bounded to the substrate parser's portable
resource envelope: 8 MiB of encoded UTF-8, 64 container levels (root is level
one), and 131,072 JSON value nodes. These limits are part of the read contract,
not only a Rust implementation detail.
"""

from __future__ import annotations

import json
import math
from typing import Any


MAX_PORTABLE_JSON_INTEGER = (1 << 53) - 1
MAX_RECEIPT_V1_BYTES = 8 * 1024 * 1024
MAX_RECEIPT_V1_DEPTH = 64
MAX_RECEIPT_V1_NODES = 131_072


def _reject_duplicate_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate object name {key!r}")
        value[key] = item
    return value


def _parse_integer(token: str) -> int:
    value = int(token)
    if abs(value) > MAX_PORTABLE_JSON_INTEGER:
        raise ValueError(
            f"integer {token} is outside the portable JSON range "
            f"-{MAX_PORTABLE_JSON_INTEGER}..={MAX_PORTABLE_JSON_INTEGER}"
        )
    return value


def _parse_decimal(token: str) -> float:
    value = float(token)
    if not math.isfinite(value):
        raise ValueError(f"non-finite JSON number {token}")
    if value.is_integer() and abs(value) > MAX_PORTABLE_JSON_INTEGER:
        raise ValueError(
            f"integral number {token} is outside the portable JSON range "
            f"-{MAX_PORTABLE_JSON_INTEGER}..={MAX_PORTABLE_JSON_INTEGER}"
        )
    return value


def _validate_resource_limits(value: Any) -> None:
    """Apply the substrate's node/depth semantics without recursive walking."""

    root_depth = 1 if isinstance(value, (dict, list)) else 0
    stack: list[tuple[Any, int]] = [(value, root_depth)]
    nodes = 0
    while stack:
        item, depth = stack.pop()
        nodes += 1
        if nodes > MAX_RECEIPT_V1_NODES:
            raise ValueError(f"JSON node budget exceeds {MAX_RECEIPT_V1_NODES}")
        if depth > MAX_RECEIPT_V1_DEPTH:
            raise ValueError(f"JSON depth is {depth}; limit is {MAX_RECEIPT_V1_DEPTH}")
        if isinstance(item, list):
            for child in reversed(item):
                child_depth = depth + (1 if isinstance(child, (dict, list)) else 0)
                stack.append((child, child_depth))
        elif isinstance(item, dict):
            for child in reversed(tuple(item.values())):
                child_depth = depth + (1 if isinstance(child, (dict, list)) else 0)
                stack.append((child, child_depth))


def strict_receipt_json_loads(text: str) -> Any:
    """Decode bounded Receipt JSON without erasing names or unsafe ints."""

    encoded_bytes = len(text.encode("utf-8"))
    if encoded_bytes > MAX_RECEIPT_V1_BYTES:
        raise ValueError(
            f"encoded JSON is {encoded_bytes} bytes; "
            f"limit is {MAX_RECEIPT_V1_BYTES} bytes"
        )

    def reject_constant(token: str) -> Any:
        raise ValueError(f"non-finite JSON number {token}")

    value = json.loads(
        text,
        object_pairs_hook=_reject_duplicate_pairs,
        parse_constant=reject_constant,
        parse_int=_parse_integer,
        parse_float=_parse_decimal,
    )
    _validate_resource_limits(value)
    return value


def strict_receipt_json_load_bytes(data: bytes) -> Any:
    if len(data) > MAX_RECEIPT_V1_BYTES:
        raise ValueError(
            f"encoded JSON is {len(data)} bytes; "
            f"limit is {MAX_RECEIPT_V1_BYTES} bytes"
        )
    return strict_receipt_json_loads(data.decode("utf-8"))


def _validate_numbers(value: Any, path: str = "$") -> None:
    if value is None or isinstance(value, (bool, str)):
        return
    if isinstance(value, int):
        if abs(value) > MAX_PORTABLE_JSON_INTEGER:
            raise ValueError(
                f"{path}: integer {value} is outside the portable JSON range "
                f"-{MAX_PORTABLE_JSON_INTEGER}..={MAX_PORTABLE_JSON_INTEGER}"
            )
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError(f"{path}: non-finite JSON number")
        if value.is_integer() and abs(value) > MAX_PORTABLE_JSON_INTEGER:
            raise ValueError(
                f"{path}: integral number {value} is outside the portable JSON range "
                f"-{MAX_PORTABLE_JSON_INTEGER}..={MAX_PORTABLE_JSON_INTEGER}"
            )
        return
    if isinstance(value, (list, tuple)):
        for index, item in enumerate(value):
            _validate_numbers(item, f"{path}[{index}]")
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise ValueError(f"{path}: object keys must be strings")
            _validate_numbers(item, f"{path}.{key}")
        return
    raise ValueError(f"{path}: unsupported JSON value {type(value).__name__}")


def _jcs_float(value: float) -> str:
    if not math.isfinite(value):
        raise ValueError(f"{value} is not representable in JCS")
    if value == 0:
        return "0"
    if value < 0:
        return "-" + _jcs_float(-value)

    rendered = str(value)
    exponent = ""
    exponent_value = 0
    marker = rendered.find("e")
    if marker > 0:
        exponent = rendered[marker:]
        if exponent[2:3] == "0":
            exponent = exponent[:2] + exponent[3:]
        rendered = rendered[:marker]
        exponent_value = int(exponent[1:])

    first = rendered
    dot = ""
    last = ""
    marker = rendered.find(".")
    if marker > 0:
        dot = "."
        first = rendered[:marker]
        last = rendered[marker + 1 :]
    if last == "0":
        dot = ""
        last = ""

    if 0 < exponent_value < 21:
        first += last
        last = ""
        dot = ""
        exponent = ""
        padding = exponent_value - len(first)
        while padding >= 0:
            padding -= 1
            first += "0"
    elif -7 < exponent_value < 0:
        last = first + last
        first = "0"
        dot = "."
        exponent = ""
        padding = exponent_value
        while padding < -1:
            padding += 1
            last = "0" + last

    return f"{first}{dot}{last}{exponent}"


def _write_jcs(value: Any, output: list[str]) -> None:
    if value is None:
        output.append("null")
    elif isinstance(value, bool):
        output.append("true" if value else "false")
    elif isinstance(value, int):
        output.append(str(value))
    elif isinstance(value, float):
        output.append(_jcs_float(value))
    elif isinstance(value, str):
        output.append(json.dumps(value, ensure_ascii=False, separators=(",", ":")))
    elif isinstance(value, (list, tuple)):
        output.append("[")
        for index, item in enumerate(value):
            if index:
                output.append(",")
            _write_jcs(item, output)
        output.append("]")
    elif isinstance(value, dict):
        output.append("{")
        items = sorted(value.items(), key=lambda item: item[0].encode("utf-16be"))
        for index, (key, item) in enumerate(items):
            if index:
                output.append(",")
            output.append(json.dumps(key, ensure_ascii=False, separators=(",", ":")))
            output.append(":")
            _write_jcs(item, output)
        output.append("}")
    else:  # guarded by _validate_numbers, retained for defensive use
        raise ValueError(f"unsupported JSON value {type(value).__name__}")


def canonical_receipt_json_bytes(value: Any) -> bytes:
    _validate_numbers(value)
    output: list[str] = []
    _write_jcs(value, output)
    return "".join(output).encode("utf-8")
