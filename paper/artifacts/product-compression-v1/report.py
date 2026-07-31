#!/usr/bin/env python3
"""Aggregate the exact registered product-compression assignment."""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
from pathlib import Path
from typing import Any


RESULT_SCHEMA = "vela.product-compression-score.v1"
REPORT_SCHEMA = "vela.product-compression-result.v1"
ARMS = ("git_files", "vela_guided")


class ReportInputError(ValueError):
    """Raised when retained scores do not match the frozen study."""


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def root(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def file_root(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def load(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReportInputError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise ReportInputError(f"{path} must contain one object")
    return value


def reduction(baseline: float, candidate: float) -> float | None:
    if baseline <= 0:
        return None
    return (baseline - candidate) / baseline


def report(
    plan_path: Path,
    plan: dict[str, Any],
    scores: list[dict[str, Any]],
) -> dict[str, Any]:
    plan_root = file_root(plan_path)
    assignment = plan.get("assignment")
    if not isinstance(assignment, list) or not assignment:
        raise ReportInputError("plan.assignment must be a nonempty array")
    expected = [(item["session_id"], item["arm"]) for item in assignment]
    observed = [(score.get("session_id"), score.get("arm")) for score in scores]
    if observed != expected:
        raise ReportInputError(
            f"score order/assignment differs: expected={expected}, observed={observed}"
        )
    for score in scores:
        if score.get("schema") != RESULT_SCHEMA:
            raise ReportInputError("one score has the wrong schema")
        if score.get("plan_root") != plan_root:
            raise ReportInputError("one score binds a different plan")
        without_root = {key: value for key, value in score.items() if key != "result_root"}
        if score.get("result_root") != root(without_root):
            raise ReportInputError("one score root is invalid")

    arm_results: dict[str, Any] = {}
    for arm in ARMS:
        arm_scores = [score for score in scores if score["arm"] == arm]
        elapsed = [score["process"]["elapsed_ms"] for score in arm_scores]
        tokens = [score["process"]["observed_tokens"] for score in arm_scores]
        arm_results[arm] = {
            "sessions": len(arm_scores),
            "passed_sessions": sum(1 for score in arm_scores if score["passed"]),
            "median_score_basis_points": statistics.median(
                score["score_basis_points"] for score in arm_scores
            ),
            "median_elapsed_ms": statistics.median(elapsed),
            "median_observed_tokens": statistics.median(tokens),
            "total_interventions": sum(
                score["process"]["intervention_count"] for score in arm_scores
            ),
        }

    time_reduction = reduction(
        arm_results["git_files"]["median_elapsed_ms"],
        arm_results["vela_guided"]["median_elapsed_ms"],
    )
    token_reduction = reduction(
        arm_results["git_files"]["median_observed_tokens"],
        arm_results["vela_guided"]["median_observed_tokens"],
    )
    method_floor = all(
        arm_results[arm]["passed_sessions"] >= 3 for arm in ARMS
    )
    correctness_preserved = (
        arm_results["vela_guided"]["median_score_basis_points"]
        >= arm_results["git_files"]["median_score_basis_points"]
    )
    product_lift = (
        method_floor
        and correctness_preserved
        and time_reduction is not None
        and time_reduction >= 0.20
    )
    result_without_root = {
        "schema": REPORT_SCHEMA,
        "plan_root": plan_root,
        "classification": "first_party_fresh_session_only",
        "arm_results": arm_results,
        "comparison": {
            "method_floor_passed": method_floor,
            "correctness_preserved": correctness_preserved,
            "median_elapsed_time_reduction": time_reduction,
            "median_observed_token_reduction": token_reduction,
            "product_compression_gate_passed": product_lift,
        },
        "claims_not_earned": [
            "external_user_lift",
            "independent_participant_credit",
            "adoption",
            "protocol_breakthrough",
            "scientific_acceptance",
        ],
    }
    return {**result_without_root, "result_root": root(result_without_root)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--scores", type=Path, nargs="+", required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        result = report(
            args.plan,
            load(args.plan),
            [load(path) for path in args.scores],
        )
    except ReportInputError as error:
        print(f"error: {error}")
        return 1
    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
