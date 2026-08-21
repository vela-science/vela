#!/usr/bin/env python3
"""Regenerate only canonical aggregate serialization from a sealed capture."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import statistics
import sys
from decimal import Decimal, ROUND_HALF_EVEN
from pathlib import Path
from typing import Any

EXECUTION_ROOT = Path(__file__).resolve().parent
BENCHMARK_ROOT = EXECUTION_ROOT.parent / "inherited-correction-benchmark"
BENCHMARK_PATH = BENCHMARK_ROOT / "benchmark.py"
FIXTURE_PATH = EXECUTION_ROOT / "post-result-serialization-fixture.json"
MEAN_QUANTUM = Decimal("0.00000000000001")
RATIO_QUANTUM = Decimal("0.000000000000001")


def load_benchmark() -> Any:
    spec = importlib.util.spec_from_file_location(
        "inherited_correction_registered_benchmark", BENCHMARK_PATH
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("registered_benchmark_import_failed")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def decimal_json(raw: bytes) -> Any:
    return json.loads(raw, parse_float=Decimal)


def canonical_number(value: Decimal, quantum: Decimal) -> float:
    return float(value.quantize(quantum, rounding=ROUND_HALF_EVEN))


def canonical_aggregates(groups: dict[str, list[Decimal]]) -> dict[str, float]:
    means = {
        condition: sum(values, Decimal(0)) / Decimal(len(values))
        for condition, values in groups.items()
    }
    vela_mean = means["vela"]
    git_mean = means["git-documents"]
    return {
        "git_documents_restricted_mean_seconds": canonical_number(
            git_mean, MEAN_QUANTUM
        ),
        "vela_restricted_mean_seconds": canonical_number(vela_mean, MEAN_QUANTUM),
        "restricted_mean_ratio_vela_over_git_documents": canonical_number(
            vela_mean / git_mean, RATIO_QUANTUM
        ),
    }


def fixture_bytes(metrics: dict[str, float]) -> bytes:
    return (
        json.dumps(metrics, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
        + "\n"
    ).encode()


def check_fixture() -> str:
    fixture = json.loads(FIXTURE_PATH.read_bytes())
    groups = {
        condition: [Decimal(value) for value in values]
        for condition, values in fixture["restricted_seconds"].items()
    }
    metrics = canonical_aggregates(groups)
    if metrics != fixture["expected_metrics"]:
        raise RuntimeError("post_result_metric_fixture_mismatch")
    digest = "sha256:" + hashlib.sha256(fixture_bytes(metrics)).hexdigest()
    if digest != fixture["expected_metrics_bytes"]:
        raise RuntimeError("post_result_metric_bytes_mismatch")
    return digest


def canonical_score(runs_dir: Path) -> dict[str, Any]:
    benchmark = load_benchmark()
    capture = benchmark.verify_capture_manifest(runs_dir)
    capture, snapshot = benchmark.capture_bound_score_snapshot(runs_dir, capture)
    adjudication = benchmark.load_registered_adjudication()
    preregistration = benchmark.load_json(benchmark.PREREG_PATH)
    records = []
    for run_bytes, response_bytes in snapshot:
        record = decimal_json(run_bytes)
        score = (
            benchmark.score_response(json.loads(response_bytes), adjudication)
            if response_bytes is not None
            else None
        )
        records.append((record, score))

    required = preregistration["assignment"]["total_sessions"]
    if len(records) != required:
        raise benchmark.BenchmarkError(
            f"fixed_denominator_incomplete:{len(records)}/{required}"
        )
    if len({record["participant_instance_id"] for record, _ in records}) != required:
        raise benchmark.BenchmarkError("participant_instance_reused")

    summaries: dict[str, Any] = {}
    restricted_groups: dict[str, list[Decimal]] = {}
    for condition in benchmark.CONDITIONS:
        selected = [
            (record, score)
            for record, score in records
            if record["condition"] == condition
        ]
        if len(selected) != 8:
            raise benchmark.BenchmarkError("condition_denominator_invalid")
        exact = sum(
            bool(score and score["exact_success"] and record["status"] == "completed")
            for record, score in selected
        )
        restricted = [
            record["duration_seconds"]
            if score and score["exact_success"] and record["status"] == "completed"
            else Decimal(preregistration["assignment"]["timeout_seconds"])
            for record, score in selected
        ]
        restricted_groups[condition] = restricted
        summaries[condition] = {
            "sessions": len(selected),
            "exact_successes": exact,
            "authority_errors": sum(
                bool(score and score["authority_error"]) for _, score in selected
            ),
            "restricted_mean_seconds": None,
            "median_tool_calls": statistics.median(
                record["tool_calls"] for record, _ in selected
            ),
            "points": sum(score["points"] if score else 0 for _, score in selected),
        }

    aggregates = canonical_aggregates(restricted_groups)
    summaries["git-documents"]["restricted_mean_seconds"] = aggregates[
        "git_documents_restricted_mean_seconds"
    ]
    summaries["vela"]["restricted_mean_seconds"] = aggregates[
        "vela_restricted_mean_seconds"
    ]
    ratio = aggregates["restricted_mean_ratio_vela_over_git_documents"]
    positive = all(
        [
            summaries["vela"]["exact_successes"] >= 6,
            summaries["vela"]["exact_successes"]
            >= summaries["git-documents"]["exact_successes"],
            summaries["vela"]["authority_errors"] == 0,
            ratio <= 0.8,
        ]
    )
    return {
        "schema": "vela.inherited-correction-scored-result.v1",
        "registration_root": preregistration["registration_root"],
        "capture_root": capture["capture_root"],
        "adjudication_root": preregistration["bindings"]["adjudication_root"],
        "fixed_denominator": required,
        "conditions": summaries,
        "restricted_mean_ratio_vela_over_git_documents": ratio,
        "positive_gate": "pass" if positive else "not_supported",
        "authority_effect": "none",
        "limitations": [
            "One synthetic case cannot establish scientific truth or general productivity.",
            "The result is internal unless separately and independently reproduced.",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check-fixture", action="store_true")
    parser.add_argument("--runs-dir", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.check_fixture:
        print(check_fixture())
        return 0
    if args.runs_dir is None or args.output is None:
        parser.error("--runs-dir and --output are required without --check-fixture")
    result = canonical_score(args.runs_dir.resolve())
    args.output.write_bytes(load_benchmark().json_bytes(result))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(2) from error
