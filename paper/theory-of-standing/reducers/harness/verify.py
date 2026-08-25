#!/usr/bin/env python3
"""Orchestrate independent reducers and compare bytes; contains no replay logic."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "corpus" / "manifest.json"
MANIFEST_FORMAT = "theory-of-standing.proof-corpus-manifest.v2"
RESULT_FORMAT = "theory-of-standing.proof-result.v2"
INVALID_FORMAT = "theory-of-standing.proof-invalid.v1"
AGGREGATE_FORMAT = "theory-of-standing.proof-corpus-aggregate.v2"


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def commands() -> dict[str, list[str]]:
    return {
        "javascript": ["node", str(ROOT / "javascript" / "reducer.mjs")],
        "python": [sys.executable, str(ROOT / "python" / "reducer.py")],
        "rust": [
            str(ROOT / "rust" / "target" / "debug" / "theory-standing-rust-reducer")
        ],
    }


def run_twice(
    command: list[str], input_path: Path, expected_exit: int, allow_stderr: bool
) -> bytes:
    outputs = []
    for _ in range(2):
        completed = subprocess.run(
            [*command, str(input_path)],
            cwd=ROOT,
            check=False,
            capture_output=True,
        )
        if completed.returncode != expected_exit:
            raise AssertionError(
                f"{command[0]} {input_path.name}: exit {completed.returncode}, "
                f"expected {expected_exit}; stderr={completed.stderr.decode(errors='replace')!r}"
            )
        if completed.stderr and not allow_stderr:
            raise AssertionError(
                f"{command[0]} {input_path.name}: unexpected stderr "
                f"{completed.stderr.decode(errors='replace')!r}"
            )
        outputs.append(completed.stdout)
    if outputs[0] != outputs[1]:
        raise AssertionError(f"{command[0]} {input_path.name}: repeated outputs differ")
    parsed = json.loads(outputs[0])
    if outputs[0] != canonical_bytes(parsed):
        raise AssertionError(
            f"{command[0]} {input_path.name}: output is not canonical JSON"
        )
    return outputs[0]


def aggregate_hash(cases: list[dict[str, Any]]) -> str:
    binding = {
        "cases": [
            {
                "id": case["id"],
                "input_sha256": case["input_sha256"],
                "output_sha256": case["output_sha256"],
            }
            for case in cases
        ],
        "format": AGGREGATE_FORMAT,
    }
    return sha256(canonical_bytes(binding))


def check_lean_expectation(case: dict[str, Any], result: dict[str, Any]) -> None:
    if "lean_standing" in case:
        actual = [item["status"] for item in result["standing"]]
        if actual != case["lean_standing"]:
            raise AssertionError(
                f"{case['id']}: Standing differs from accepted Lean output"
            )
    if "lean_reassessment" in case:
        statuses = [
            item["status"] for item in result["reassessment"] if item["claim"] == 20
        ]
        if statuses != [case["lean_reassessment"]]:
            raise AssertionError(
                f"{case['id']}: reassessment differs from accepted Lean output"
            )


def check_declared_result(case: dict[str, Any], result: dict[str, Any]) -> None:
    if result.get("rejections") != case["expected_rejections"]:
        raise AssertionError(f"{case['id']}: rejection observations differ")
    if "expected_root" in case and result.get("root") != case["expected_root"]:
        raise AssertionError(f"{case['id']}: final root differs")
    if "expected_event_ids" in case:
        actual_ids = [event["decision_id"] for event in result["events"]]
        if actual_ids != case["expected_event_ids"]:
            raise AssertionError(f"{case['id']}: admitted Event ids differ")
    if "expected_standing" in case:
        actual_standing = [item["status"] for item in result["standing"]]
        if actual_standing != case["expected_standing"]:
            raise AssertionError(f"{case['id']}: final Standing differs")


def verify_cross_case_invariants(
    manifest: dict[str, Any], results: dict[str, dict[str, Any]]
) -> None:
    for comparison in manifest["projection_comparisons"]:
        left = results[comparison["left"]]
        right = results[comparison["right"]]
        left_without_projection = {k: v for k, v in left.items() if k != "reassessment"}
        right_without_projection = {
            k: v for k, v in right.items() if k != "reassessment"
        }
        if left_without_projection != right_without_projection:
            raise AssertionError("descriptive dependency mutation changed replay state")
        if left["reassessment"] == right["reassessment"]:
            raise AssertionError(
                "descriptive dependency mutation did not change projection"
            )

    cases = {case["id"]: case for case in manifest["cases"]}
    continuation_codes = set()
    for case in manifest["cases"]:
        if case.get("expected_rejections") and "expected_event_ids" in case:
            history = load_json(ROOT / "corpus" / case["input_path"])
            last_rejection = case["expected_rejections"][-1]["record_index"]
            if last_rejection >= len(history["records"]) - 1:
                raise AssertionError(f"{case['id']}: rejection has no suffix record")
            continuation_codes.update(
                observation["code"] for observation in case["expected_rejections"]
            )
    if sorted(continuation_codes) != manifest["continuation_rejection_codes"]:
        raise AssertionError("semantic rejection continuation coverage differs")

    for comparison in manifest["source_prefix_comparisons"]:
        count = comparison["record_count"]
        left_input = load_json(
            ROOT / "corpus" / cases[comparison["left"]]["input_path"]
        )
        right_input = load_json(
            ROOT / "corpus" / cases[comparison["right"]]["input_path"]
        )
        if left_input["records"][:count] != right_input["records"][:count]:
            raise AssertionError(
                "plural-authority cases do not bind the same source records"
            )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--freeze",
        action="store_true",
        help="write agreed outputs and their hashes into the frozen corpus",
    )
    args = parser.parse_args()

    manifest = load_json(MANIFEST_PATH)
    if manifest.get("format") != MANIFEST_FORMAT:
        raise AssertionError("unsupported corpus manifest")
    implementations = commands()
    agreed_results: dict[str, dict[str, Any]] = {}

    for case in manifest["cases"]:
        input_path = ROOT / "corpus" / case["input_path"]
        input_bytes = input_path.read_bytes()
        if sha256(input_bytes) != case["input_sha256"]:
            raise AssertionError(f"{case['id']}: input hash mismatch")
        if input_bytes != canonical_bytes(json.loads(input_bytes)):
            raise AssertionError(f"{case['id']}: input is not canonical JSON")
        expected_exit = 0 if case["expectation"] == "result" else 2
        outputs = {
            name: run_twice(
                command,
                input_path,
                expected_exit,
                case["expectation"] == "invalid_format",
            )
            for name, command in implementations.items()
        }
        if len(set(outputs.values())) != 1:
            raise AssertionError(f"{case['id']}: reducers disagree byte-for-byte")
        agreed = next(iter(outputs.values()))
        result = json.loads(agreed)
        expected_format = RESULT_FORMAT if expected_exit == 0 else INVALID_FORMAT
        if result.get("format") != expected_format:
            raise AssertionError(f"{case['id']}: unexpected output format")
        if expected_exit == 2:
            if result != {"code": case["code"], "format": INVALID_FORMAT}:
                raise AssertionError(f"{case['id']}: unexpected structural failure")
        else:
            check_declared_result(case, result)
            check_lean_expectation(case, result)
        agreed_results[case["id"]] = result

        output_path = ROOT / "corpus" / case["output_path"]
        output_hash = sha256(agreed)
        if args.freeze:
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_bytes(agreed)
            case["output_sha256"] = output_hash
        else:
            if output_path.read_bytes() != agreed:
                raise AssertionError(f"{case['id']}: frozen output mismatch")
            if case["output_sha256"] != output_hash:
                raise AssertionError(f"{case['id']}: output hash mismatch")

    verify_cross_case_invariants(manifest, agreed_results)
    aggregate = aggregate_hash(manifest["cases"])
    if args.freeze:
        manifest["aggregate_sha256"] = aggregate
        MANIFEST_PATH.write_bytes(canonical_bytes(manifest))
    elif manifest["aggregate_sha256"] != aggregate:
        raise AssertionError("aggregate corpus hash mismatch")

    summary = {
        "aggregate_sha256": aggregate,
        "cases": len(manifest["cases"]),
        "implementations": sorted(implementations),
        "invocations": len(manifest["cases"]) * len(implementations) * 2,
        "status": "pass",
    }
    sys.stdout.buffer.write(canonical_bytes(summary))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
