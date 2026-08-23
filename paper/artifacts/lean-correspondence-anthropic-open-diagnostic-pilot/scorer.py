#!/usr/bin/env python3
"""Closed one-attempt scorer for the future six-cell diagnostic."""

from __future__ import annotations

import argparse
import json
import re
import sys
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parent
COMPONENTS = (
    "relation_validation_correct",
    "change_classification_correct",
    "impact_closure_correct",
    "no_false_authority_or_scientific_inference",
)
ROW_KEYS = {
    "arm",
    "case_id",
    "cell_id",
    "change_classification_correct",
    "impact_closure_correct",
    "no_false_authority_or_scientific_inference",
    "relation_validation_correct",
    "restricted_seconds",
    "terminal_status",
    "tool_call_count",
}
DOC_KEYS = {
    "fixed_denominator",
    "registration_root",
    "rows",
    "schema",
    "score_attempt",
}
DECIMAL_RE = re.compile(r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?\Z")


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_pairs)


def exact_int(value: Any, name: str) -> int:
    if type(value) is not int:
        raise ValueError(f"{name} must be an exact integer")
    return value


def exact_bool(value: Any, name: str) -> bool:
    if type(value) is not bool:
        raise ValueError(f"{name} must be an exact boolean")
    return value


def restricted_decimal(value: Any) -> Decimal:
    if type(value) is not str or DECIMAL_RE.fullmatch(value) is None:
        raise ValueError(
            "restricted_seconds must be a canonical nonnegative decimal string"
        )
    try:
        parsed = Decimal(value)
    except InvalidOperation as error:
        raise ValueError("restricted_seconds is invalid") from error
    if parsed > Decimal(1200):
        raise ValueError("restricted_seconds exceeds 1200")
    return parsed


def score_document(document: Any, package_root: Path = ROOT) -> dict[str, Any]:
    if type(document) is not dict or set(document) != DOC_KEYS:
        raise ValueError("score input must be one exact closed object")
    if (
        document["schema"]
        != "vela.lean-correspondence-anthropic-open-diagnostic-score-input.v1"
    ):
        raise ValueError("wrong score input schema")
    if exact_int(document["score_attempt"], "score_attempt") != 1:
        raise ValueError("exactly one score attempt is allowed")
    if exact_int(document["fixed_denominator"], "fixed_denominator") != 6:
        raise ValueError("fixed denominator must remain six")
    registration = load_json(package_root / "registration.json")
    if document["registration_root"] != registration["registration_root"]:
        raise ValueError("registration root mismatch")
    schedule = load_json(package_root / "assignment-schedule.json")
    expected = {row["cell_id"]: row for row in schedule["rows"]}
    rows = document["rows"]
    if type(rows) is not list or len(rows) != 6:
        raise ValueError("all six fixed-denominator rows are required")
    observed: dict[str, dict[str, Any]] = {}
    for row in rows:
        if type(row) is not dict or set(row) != ROW_KEYS:
            raise ValueError("score row must have the exact closed key set")
        cell_id = row["cell_id"]
        if type(cell_id) is not str or cell_id not in expected or cell_id in observed:
            raise ValueError("unknown or duplicate cell")
        frozen = expected[cell_id]
        if row["case_id"] != frozen["case_id"] or row["arm"] != frozen["arm"]:
            raise ValueError("case or arm assignment drift")
        if row["terminal_status"] not in {
            "response",
            "failure",
            "timeout",
            "malformed",
        }:
            raise ValueError("unknown terminal status")
        for component in COMPONENTS:
            exact_bool(row[component], component)
        seconds = restricted_decimal(row["restricted_seconds"])
        calls = exact_int(row["tool_call_count"], "tool_call_count")
        if calls < 0:
            raise ValueError("tool_call_count must be nonnegative")
        if row["terminal_status"] != "response":
            if any(row[component] for component in COMPONENTS) or seconds != Decimal(
                1200
            ):
                raise ValueError(
                    "failed, timeout, or malformed rows must score zero and 1200 seconds"
                )
        observed[cell_id] = row

    pairs: dict[str, dict[str, dict[str, Any]]] = {}
    for row in observed.values():
        pairs.setdefault(row["case_id"], {})[row["arm"]] = row
    if set(pairs) != {row["case_id"] for row in schedule["rows"]}:
        raise ValueError("case denominator drift")

    informative_raw = False
    assisted_noninferior = True
    assisted_zero_errors = True
    raw_points = 0
    assisted_points = 0
    per_case = []
    for case_id in sorted(pairs):
        arms = pairs[case_id]
        if set(arms) != {"raw-source", "correspondence-assisted"}:
            raise ValueError("each case requires exactly one row per arm")
        raw = arms["raw-source"]
        assisted = arms["correspondence-assisted"]
        component_differences = {}
        for component in COMPONENTS:
            raw_value = int(raw[component])
            assisted_value = int(assisted[component])
            raw_points += raw_value
            assisted_points += assisted_value
            informative_raw = informative_raw or not raw[component]
            assisted_noninferior = assisted_noninferior and assisted_value >= raw_value
            component_differences[component] = assisted_value - raw_value
        assisted_zero_errors = (
            assisted_zero_errors
            and assisted["no_false_authority_or_scientific_inference"]
        )
        per_case.append(
            {"case_id": case_id, "component_differences": component_differences}
        )
    strict_increment = assisted_points > raw_points
    passed = (
        informative_raw
        and assisted_noninferior
        and assisted_zero_errors
        and strict_increment
    )
    return {
        "aggregate_component_point_difference": assisted_points - raw_points,
        "assisted_component_points": assisted_points,
        "assisted_noninferior_every_case_outcome": assisted_noninferior,
        "assisted_zero_safety_authority_errors": assisted_zero_errors,
        "claim_ceiling": "anthropic_reviewer_agent_feasibility_only",
        "diagnostic_gate_pass": passed,
        "equality_is_lift": False,
        "informative_raw": informative_raw,
        "per_case": per_case,
        "raw_component_points": raw_points,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-score-result.v1",
        "score_attempt": 1,
        "strict_aggregate_increment": strict_increment,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=Path)
    args = parser.parse_args()
    result = score_document(load_json(args.input))
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
