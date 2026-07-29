#!/usr/bin/env python3
"""Validate the state-lift answer schema against OpenAI's strict subset."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


FORBIDDEN = {
    "allOf",
    "dependentRequired",
    "dependentSchemas",
    "else",
    "if",
    "maxLength",
    "minLength",
    "not",
    "patternProperties",
    "then",
    "uniqueItems",
}


def fail(message: str) -> None:
    raise ValueError(message)


def validate(node: Any, path: str = "$") -> None:
    if isinstance(node, list):
        for index, item in enumerate(node):
            validate(item, f"{path}[{index}]")
        return
    if not isinstance(node, dict):
        return

    forbidden = sorted(FORBIDDEN.intersection(node))
    if forbidden:
        fail(f"{path} uses unsupported keywords: {forbidden}")
    if "const" in node and "type" not in node:
        fail(f"{path} const lacks an explicit type")
    if "enum" in node and "type" not in node:
        fail(f"{path} enum lacks an explicit type")

    if node.get("type") == "object":
        properties = node.get("properties")
        if not isinstance(properties, dict):
            fail(f"{path} object lacks properties")
        if node.get("additionalProperties") is not False:
            fail(f"{path} object must set additionalProperties=false")
        required = node.get("required")
        if not isinstance(required, list) or set(required) != set(properties):
            fail(f"{path} must require every property")

    for key, value in node.items():
        validate(value, f"{path}.{key}")


def main() -> int:
    path = (
        Path(sys.argv[1])
        if len(sys.argv) == 2
        else Path(__file__).with_name("answer.schema.json")
    )
    schema = json.loads(path.read_text(encoding="utf-8"))
    if schema.get("type") != "object":
        fail("root schema must be an object")
    validate(schema)
    print(json.dumps({"ok": True, "schema": str(path)}))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
