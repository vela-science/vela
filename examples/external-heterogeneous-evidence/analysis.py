#!/usr/bin/env python3
"""Compute exact descriptive facts over the retained calibration fixture."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
from decimal import Decimal
from pathlib import Path


def compute(input_path: Path) -> dict[str, object]:
    input_bytes = input_path.read_bytes()
    with input_path.open(newline="", encoding="utf-8") as stream:
        rows = list(csv.DictReader(stream))
    if not rows or set(rows[0]) != {"sample_id", "temperature_c"}:
        raise ValueError("expected sample_id and temperature_c columns")
    identifiers = [row["sample_id"] for row in rows]
    if len(identifiers) != len(set(identifiers)):
        raise ValueError("sample identifiers must be unique")
    readings = sorted(Decimal(row["temperature_c"]) for row in rows)
    if len(readings) % 2 != 1:
        raise ValueError("this bounded fixture requires an odd observation count")
    return {
        "analysis_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "count": len(readings),
        "input_sha256": hashlib.sha256(input_bytes).hexdigest(),
        "maximum_temperature_c": str(readings[-1]),
        "median_temperature_c": str(readings[len(readings) // 2]),
        "minimum_temperature_c": str(readings[0]),
        "scope": "exact descriptive statistics over the retained fixture rows only",
    }


def render(result: dict[str, object]) -> str:
    return json.dumps(result, indent=2, sort_keys=True) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=Path)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--output", type=Path)
    group.add_argument("--check", type=Path)
    arguments = parser.parse_args()
    rendered = render(compute(arguments.input))
    if arguments.output is not None:
        arguments.output.write_text(rendered, encoding="utf-8")
        print(f"wrote {arguments.output}")
        return 0
    if arguments.check.read_text(encoding="utf-8") != rendered:
        raise SystemExit(f"{arguments.check} does not match exact recomputation")
    print("heterogeneous-evidence-analysis: exact result matched")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
