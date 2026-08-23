"""Derive runner-canonical packet bytes while retaining the exact source binding."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

NUMBER = re.compile(r"-?(?:0|[1-9][0-9]*)(?:\.[0-9]*[1-9])?\Z")


@dataclass(frozen=True)
class NumberLexeme:
    value: str


def _number(value: str) -> NumberLexeme:
    if NUMBER.fullmatch(value) is None or value == "-0":
        raise ValueError("packet number is not canonical decimal JSON")
    return NumberLexeme(value)


def _object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, child in pairs:
        if key in value:
            raise ValueError(f"duplicate packet key: {key}")
        value[key] = child
    return value


def parse(raw: bytes) -> dict[str, Any]:
    try:
        text = raw.decode("utf-8")
        value = json.loads(
            text,
            object_pairs_hook=_object,
            parse_int=_number,
            parse_float=_number,
            parse_constant=lambda token: (_ for _ in ()).throw(
                ValueError(f"non-JSON packet number: {token}")
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("packet is not one JSON value") from error
    if type(value) is not dict:
        raise ValueError("packet top level must be an object")
    return value


def _string(value: str) -> bytes:
    encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    encoded = encoded.replace("\u2028", "\\u2028").replace("\u2029", "\\u2029")
    return encoded.encode("utf-8")


def canonical(value: Any) -> bytes:
    if isinstance(value, NumberLexeme):
        return value.value.encode("ascii")
    if value is None:
        return b"null"
    if type(value) is bool:
        return b"true" if value else b"false"
    if type(value) is str:
        return _string(value)
    if type(value) is list:
        return b"[" + b",".join(canonical(child) for child in value) + b"]"
    if type(value) is dict:
        return (
            b"{"
            + b",".join(
                _string(key) + b":" + canonical(value[key]) for key in sorted(value)
            )
            + b"}"
        )
    raise ValueError("unsupported packet JSON value")


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def derive(source: Path, output: Path, receipt: Path) -> dict[str, Any]:
    source_raw = source.read_bytes()
    parsed = parse(source_raw)
    execution_raw = canonical(parsed) + b"\n"
    if (
        parse(execution_raw) != parsed
        or canonical(parse(execution_raw)) + b"\n" != execution_raw
    ):
        raise ValueError("execution packet canonical round trip failed")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(execution_raw)
    value = {
        "canonicalization": "recursive_sorted_object_keys_compact_utf8_newline",
        "duplicate_keys_rejected_at_every_depth": True,
        "execution_bytes": len(execution_raw),
        "execution_packet_path": output.name,
        "execution_packet_sha256": digest(execution_raw),
        "number_lexemes": "preserved_only_if_runner_canonical_no_exponent_no_negative_zero_no_trailing_fraction_zero",
        "parsed_semantic_equality": True,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-packet-derivation.v1",
        "source_bytes": len(source_raw),
        "source_packet_path": source.name,
        "source_packet_sha256": digest(source_raw),
    }
    receipt.parent.mkdir(parents=True, exist_ok=True)
    receipt.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    derive(args.source, args.output, args.receipt)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
