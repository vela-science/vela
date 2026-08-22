#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path

PACKAGE = Path(__file__).resolve().parent
SOURCE = PACKAGE / "raw/runner-to-bridge.raw.jsonl"
TARGET = PACKAGE / "raw/actual-transmitted-body.raw.json"
PREFIX = (
    b'{"type":"provider_request","adapter":"anthropic-messages-v1",'
    b'"endpoint":"https://api.anthropic.com/v1/messages","body":'
)


class ExtractionError(RuntimeError):
    pass


def reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ExtractionError("duplicate JSON field")
        value[key] = item
    return value


def extract(raw: bytes) -> bytes:
    if not raw.startswith(PREFIX) or not raw.endswith(b"}\n") or raw.count(b"\n") != 1:
        raise ExtractionError("provider request frame lexical shape")
    body = raw[len(PREFIX) : -2]
    frame = json.loads(raw, object_pairs_hook=reject_duplicates)
    parsed_body = json.loads(body, object_pairs_hook=reject_duplicates)
    if set(frame) != {"type", "adapter", "endpoint", "body"}:
        raise ExtractionError("provider request frame fields")
    if frame["body"] != parsed_body:
        raise ExtractionError("provider request body slice mismatch")
    return body


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    body = extract(SOURCE.read_bytes())
    if arguments.check:
        if TARGET.read_bytes() != body:
            raise ExtractionError("retained actual request differs from extraction")
    else:
        TARGET.write_bytes(body)


if __name__ == "__main__":
    main()
