#!/usr/bin/env python3
"""Cross-run deterministic model histories; this harness contains no replay logic."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path
from typing import Any

from generate import GENERATOR, Case, cases

HERE = Path(__file__).resolve().parent
REDUCERS = HERE.parent
MANIFEST = HERE / "manifest.json"
MANIFEST_FORMAT = "theory-of-standing.adversarial-manifest.v1"
RESULT_FORMAT = "theory-of-standing.proof-result.v2"


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def commands() -> dict[str, list[str]]:
    return {
        "javascript": ["node", str(REDUCERS / "javascript" / "reducer.mjs")],
        "python": [sys.executable, str(REDUCERS / "python" / "reducer.py")],
        "rust": [
            str(REDUCERS / "rust" / "target" / "debug" / "theory-standing-rust-reducer")
        ],
    }


def run(command: list[str], path: Path, case_id: str) -> bytes:
    completed = subprocess.run(
        [*command, str(path)], cwd=REDUCERS, check=False, capture_output=True
    )
    if completed.returncode != 0 or completed.stderr:
        raise AssertionError(
            f"case_id={case_id} command={command[0]} exit={completed.returncode} "
            f"stderr={completed.stderr.decode(errors='replace')!r}"
        )
    try:
        parsed = json.loads(completed.stdout)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AssertionError(
            f"case_id={case_id} reducer emitted invalid JSON"
        ) from error
    if completed.stdout != canonical_bytes(parsed):
        raise AssertionError(f"case_id={case_id} noncanonical output from {command[0]}")
    return completed.stdout


def check_expected(case: Case, result: dict[str, Any]) -> None:
    expected = case.expected
    if result.get("format") != RESULT_FORMAT:
        raise AssertionError(f"case_id={case.id} wrong result format")
    checks = {
        "repository": result.get("repository"),
        "root": result.get("root"),
        "standing": result.get("standing"),
        "reassessment": result.get("reassessment"),
        "event_ids": [item["decision_id"] for item in result["events"]],
        "rejections": result.get("rejections"),
    }
    expected = {**expected, "repository": case.history["repository"]}
    for field, actual in checks.items():
        if actual != expected[field]:
            raise AssertionError(
                f"case_id={case.id} field={field} actual={actual!r} "
                f"expected={expected[field]!r}"
            )
    if "last_action" in expected and (
        not result["events"]
        or result["events"][-1]["action"]["kind"] != expected["last_action"]
    ):
        raise AssertionError(f"case_id={case.id} final Event action differs")
    decisions = {
        record["id"]: record
        for record in case.history["records"]
        if record["kind"] == "decision"
    }
    for event in result["events"]:
        source = decisions.get(event["decision_id"])
        if source is None:
            raise AssertionError(f"case_id={case.id} Event has no source Decision")
        exact_event = {
            "action": source["action"],
            "authority_label": source["authority_label"],
            "decision_id": source["id"],
            "performer": source["performer"],
            "repository": source["repository"],
        }
        if event != exact_event:
            raise AssertionError(
                f"case_id={case.id} admitted Event data was not retained"
            )


def check_cross_case(
    cases_by_id: dict[str, Case], results: dict[str, dict[str, Any]]
) -> None:
    dependency_ids = ["dependency_present", "dependency_absent", "dependency_unrelated"]
    stripped = []
    for identifier in dependency_ids:
        stripped.append(
            {
                key: value
                for key, value in results[identifier].items()
                if key != "reassessment"
            }
        )
    if not all(value == stripped[0] for value in stripped[1:]):
        raise AssertionError(
            "case_id=dependency_mutation descriptive data changed replay state"
        )
    if (
        results["dependency_present"]["reassessment"]
        == results["dependency_absent"]["reassessment"]
    ):
        raise AssertionError("case_id=dependency_mutation projection did not change")

    plural_left = cases_by_id["plural_authority_accept"].history["records"][:2]
    plural_right = cases_by_id["plural_authority_reject"].history["records"][:2]
    if plural_left != plural_right:
        raise AssertionError("case_id=plural_authority external evidence differs")

    required = {
        "wrong_repository",
        "unauthorized",
        "misattributed",
        "stale_root",
        "stale_read_set",
        "ineligible",
        "invalid_correction_reference",
    }
    continued = set()
    for case in cases_by_id.values():
        if "suffix_continuation" not in case.classes:
            continue
        observations = results[case.id]["rejections"]
        if not observations or not results[case.id]["events"]:
            raise AssertionError(f"case_id={case.id} continuation sentinel failed")
        continued.update(item["code"] for item in observations)
    if not required <= continued:
        raise AssertionError(
            f"suffix rejection coverage missing {sorted(required - continued)}"
        )

    multiple = results["multiple_ordered_rejections"]
    if multiple["rejections"] != [
        {"code": "stale_root", "record_index": 8},
        {"code": "unauthorized", "record_index": 9},
    ]:
        raise AssertionError("case_id=multiple_ordered_rejections order changed")


def aggregate(format_name: str, rows: list[dict[str, str]]) -> str:
    return sha256(canonical_bytes({"cases": rows, "format": format_name}))


def build_manifest(
    selected: list[Case],
    input_rows: list[dict[str, str]],
    output_rows: list[dict[str, str]],
) -> dict[str, Any]:
    class_counts = Counter(name for case in selected for name in case.classes)
    rejection_counts = Counter(
        str(len(case.expected["rejections"])) for case in selected
    )
    lean_samples = [case.id for case in selected if case.lean_sample]
    input_aggregate = aggregate("theory-of-standing.adversarial-inputs.v1", input_rows)
    output_aggregate = aggregate(
        "theory-of-standing.adversarial-outputs.v1", output_rows
    )
    reducer_paths = {
        "javascript": REDUCERS / "javascript" / "reducer.mjs",
        "python": REDUCERS / "python" / "reducer.py",
        "rust": REDUCERS / "rust" / "src" / "main.rs",
    }
    manifest = {
        "aggregate_sha256": sha256(
            canonical_bytes(
                {
                    "class_counts": dict(sorted(class_counts.items())),
                    "generator": GENERATOR,
                    "input_aggregate_sha256": input_aggregate,
                    "lean_samples": lean_samples,
                    "output_aggregate_sha256": output_aggregate,
                    "rejection_observation_count_distribution": dict(
                        sorted(rejection_counts.items())
                    ),
                }
            )
        ),
        "case_count": len(selected),
        "class_counts": dict(sorted(class_counts.items())),
        "format": MANIFEST_FORMAT,
        "generation": {
            "enumerator": "ordered bounded mutation templates",
            "generator": GENERATOR,
            "seed": None,
        },
        "input_aggregate_sha256": input_aggregate,
        "lean_sample_count": len(lean_samples),
        "lean_samples": lean_samples,
        "named_regressions": [
            "../corpus/cases/fresh-correction.json",
            "../corpus/cases/stale-root-twin.json",
            "../corpus/cases/stale-root-continuation.json",
            "../corpus/cases/multiple-rejection-continuation.json",
            "../corpus/cases/duplicate-decision-id.json",
        ],
        "output_aggregate_sha256": output_aggregate,
        "rejection_observation_count_distribution": dict(
            sorted(rejection_counts.items())
        ),
        "reducer_source_sha256": {
            name: sha256(path.read_bytes()) for name, path in reducer_paths.items()
        },
        "support_source_sha256": {
            "generator": sha256((HERE / "generate.py").read_bytes()),
            "harness": sha256(Path(__file__).read_bytes()),
        },
        "reproduction": "python3 adversarial/verify.py --case CASE_ID",
    }
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--freeze", action="store_true")
    parser.add_argument("--case", dest="case_id")
    args = parser.parse_args()
    if args.freeze and args.case_id:
        parser.error("--freeze and --case are mutually exclusive")

    selected = cases()
    if args.case_id:
        selected = [case for case in selected if case.id == args.case_id]
        if not selected:
            parser.error(f"unknown case id: {args.case_id}")
    cases_by_id = {case.id: case for case in selected}
    input_rows: list[dict[str, str]] = []
    output_rows: list[dict[str, str]] = []
    results: dict[str, dict[str, Any]] = {}
    implementations = commands()

    with tempfile.TemporaryDirectory(prefix="vela-standing-adversarial-") as directory:
        root = Path(directory)
        for case in selected:
            input_bytes = canonical_bytes(case.history)
            path = root / f"{case.id}.json"
            path.write_bytes(input_bytes)
            outputs = {
                name: run(command, path, case.id)
                for name, command in implementations.items()
            }
            if len(set(outputs.values())) != 1:
                raise AssertionError(
                    f"case_id={case.id} reducers disagree byte-for-byte"
                )
            agreed = next(iter(outputs.values()))
            result = json.loads(agreed)
            try:
                check_expected(case, result)
            except AssertionError:
                raise
            except (AttributeError, KeyError, TypeError, IndexError) as error:
                raise AssertionError(
                    f"case_id={case.id} agreed result has an invalid shape"
                ) from error
            results[case.id] = result
            input_rows.append({"id": case.id, "sha256": sha256(input_bytes)})
            output_rows.append({"id": case.id, "sha256": sha256(agreed)})

    if not args.case_id:
        check_cross_case(cases_by_id, results)
    manifest = build_manifest(selected, input_rows, output_rows)
    if args.freeze:
        MANIFEST.write_bytes(canonical_bytes(manifest))
    elif not args.case_id and json.loads(MANIFEST.read_bytes()) != manifest:
        raise AssertionError("adversarial manifest differs; run with --freeze")

    summary = {
        "aggregate_sha256": manifest["aggregate_sha256"],
        "case_count": len(selected),
        "input_aggregate_sha256": manifest["input_aggregate_sha256"],
        "lean_sample_count": manifest["lean_sample_count"],
        "output_aggregate_sha256": manifest["output_aggregate_sha256"],
        "status": "pass",
    }
    sys.stdout.buffer.write(canonical_bytes(summary))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
