"""Receipt v1 duplicate-safe parsing and RFC 8785 canonicalization.

This dependency-free implementation preserves finite decimal/exponent values
allowed by the frozen open-extension schema while rejecting exact integral
values outside the IEEE-754 safe-integer domain.

Untrusted Receipt v1 JSON is bounded to 8 MiB of encoded UTF-8, 64 container
levels (root is level one), and 131,072 JSON value nodes, matching the Rust
substrate reader.
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
        raise ValueError(f"integer {token} is outside the portable JSON range")
    return value


def _parse_decimal(token: str) -> float:
    value = float(token)
    if not math.isfinite(value):
        raise ValueError(f"non-finite JSON number {token}")
    if value.is_integer() and abs(value) > MAX_PORTABLE_JSON_INTEGER:
        raise ValueError(f"integral number {token} is outside the portable JSON range")
    return value


def _validate_resource_limits(value: Any) -> None:
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


def strict_json_loads(text: str) -> Any:
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


def strict_json_load_bytes(data: bytes) -> Any:
    if len(data) > MAX_RECEIPT_V1_BYTES:
        raise ValueError(
            f"encoded JSON is {len(data)} bytes; "
            f"limit is {MAX_RECEIPT_V1_BYTES} bytes"
        )
    return strict_json_loads(data.decode("utf-8"))


def _validate(value: Any, path: str = "$") -> None:
    if value is None or isinstance(value, (bool, str)):
        return
    if isinstance(value, int):
        if abs(value) > MAX_PORTABLE_JSON_INTEGER:
            raise ValueError(f"{path}: integer is outside the portable JSON range")
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError(f"{path}: non-finite JSON number")
        if value.is_integer() and abs(value) > MAX_PORTABLE_JSON_INTEGER:
            raise ValueError(f"{path}: integral number is outside the portable JSON range")
        return
    if isinstance(value, (list, tuple)):
        for index, item in enumerate(value):
            _validate(item, f"{path}[{index}]")
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise ValueError(f"{path}: object keys must be strings")
            _validate(item, f"{path}.{key}")
        return
    raise ValueError(f"{path}: unsupported JSON value {type(value).__name__}")


def _float(value: float) -> str:
    if value == 0:
        return "0"
    if value < 0:
        return "-" + _float(-value)
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
    first, dot, last = rendered, "", ""
    marker = rendered.find(".")
    if marker > 0:
        first, dot, last = rendered[:marker], ".", rendered[marker + 1 :]
    if last == "0":
        dot, last = "", ""
    if 0 < exponent_value < 21:
        first, last, dot, exponent = first + last, "", "", ""
        padding = exponent_value - len(first)
        while padding >= 0:
            padding -= 1
            first += "0"
    elif -7 < exponent_value < 0:
        first, dot, last, exponent = "0", ".", first + last, ""
        padding = exponent_value
        while padding < -1:
            padding += 1
            last = "0" + last
    return f"{first}{dot}{last}{exponent}"


def _write(value: Any, output: list[str]) -> None:
    if value is None:
        output.append("null")
    elif isinstance(value, bool):
        output.append("true" if value else "false")
    elif isinstance(value, int):
        output.append(str(value))
    elif isinstance(value, float):
        output.append(_float(value))
    elif isinstance(value, str):
        output.append(json.dumps(value, ensure_ascii=False, separators=(",", ":")))
    elif isinstance(value, (list, tuple)):
        output.append("[")
        for index, item in enumerate(value):
            if index:
                output.append(",")
            _write(item, output)
        output.append("]")
    elif isinstance(value, dict):
        output.append("{")
        for index, (key, item) in enumerate(
            sorted(value.items(), key=lambda pair: pair[0].encode("utf-16be"))
        ):
            if index:
                output.append(",")
            output.append(json.dumps(key, ensure_ascii=False, separators=(",", ":")))
            output.append(":")
            _write(item, output)
        output.append("}")
    else:
        raise ValueError(f"unsupported JSON value {type(value).__name__}")


def canonical_json_bytes(value: Any) -> bytes:
    _validate(value)
    output: list[str] = []
    _write(value, output)
    return "".join(output).encode("utf-8")
