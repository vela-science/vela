#!/usr/bin/env python3
"""Generate the fixed Phase III P1.2 abstract-history corpus; no replay logic."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
CASES = ROOT / "cases"
FORMAT = "theory-of-standing.proof-history.v1"


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def submission(claim: int, scope: int = 7) -> dict[str, Any]:
    return {
        "authenticated": True,
        "claim": claim,
        "kind": "submission",
        "producer": 900,
        "scope": scope,
    }


def verification(
    claim: int, property_id: int, outcome: str = "pass", scope: int = 7
) -> dict[str, Any]:
    return {
        "claim": claim,
        "kind": "verification",
        "outcome": outcome,
        "property": property_id,
        "scope": scope,
    }


def decision(
    decision_id: int,
    repository: int,
    actor: int,
    expected_root: int,
    action: dict[str, Any],
    *,
    authority_label: int | None = None,
    read_set: dict[str, int] | None = None,
) -> dict[str, Any]:
    return {
        "action": action,
        "authority_label": actor if authority_label is None else authority_label,
        "expected_root": expected_root,
        "id": decision_id,
        "kind": "decision",
        "performer": actor,
        "read_set": {"0": 0} if read_set is None else read_set,
        "repository": repository,
    }


def accept(claim: int) -> dict[str, Any]:
    return {"claim": claim, "kind": "accept"}


def reject(claim: int) -> dict[str, Any]:
    return {"claim": claim, "kind": "reject"}


def correct(prior: int, predecessor: int, replacement: int) -> dict[str, Any]:
    return {
        "kind": "correct",
        "predecessor": predecessor,
        "prior_decision": prior,
        "replacement": replacement,
    }


def history(
    records: list[dict[str, Any]],
    *,
    repository: int = 1,
    authorized: list[int] | None = None,
    dependencies: list[dict[str, int]] | None = None,
) -> dict[str, Any]:
    return {
        "authorized_performers": [101] if authorized is None else authorized,
        "descriptive_dependencies": [] if dependencies is None else dependencies,
        "format": FORMAT,
        "initial_versions": {"0": 0},
        "records": records,
        "repository": repository,
    }


PREFIX = [
    submission(10),
    verification(10, 42),
    decision(1, 1, 101, 2, accept(10)),
    submission(20),
    verification(20, 43),
    decision(2, 1, 101, 5, accept(20)),
    submission(11),
    verification(11, 44),
]
FRESH = decision(3, 1, 101, 8, correct(1, 10, 11))
DEPENDENCY = [{"dependent": 20, "depends_on": 10}]


def case_rows() -> list[dict[str, Any]]:
    stale = decision(3, 1, 101, 7, correct(1, 10, 11))
    unauthorized = decision(3, 1, 404, 8, correct(1, 10, 11))
    wrong_repository = decision(3, 2, 101, 8, correct(1, 10, 11))
    misattributed = decision(3, 1, 101, 8, correct(1, 10, 11), authority_label=303)
    stale_reads = decision(3, 1, 101, 8, correct(1, 10, 11), read_set={"0": 1})
    ineligible_prefix = [*PREFIX[:-1], verification(11, 44, "fail")]
    invalid_order = [
        submission(10),
        verification(10, 42),
        submission(11),
        verification(11, 44),
        decision(3, 1, 101, 4, correct(1, 10, 11)),
        decision(4, 1, 101, 4, accept(11)),
    ]
    duplicate = decision(2, 1, 101, 8, correct(1, 10, 11))
    accept_replacement = decision(4, 1, 101, 8, accept(11))
    correct_after_stale = decision(4, 1, 101, 8, correct(1, 10, 11))
    reject_replacement = decision(4, 1, 101, 8, reject(11))
    second_rejection = decision(4, 1, 404, 8, correct(1, 10, 11))
    accept_after_two = decision(5, 1, 101, 8, accept(11))
    common_source = [submission(10), verification(10, 42)]
    rows = [
        {
            "id": "fresh-correction",
            "input": history([*PREFIX, FRESH], dependencies=DEPENDENCY),
            "expectation": "result",
            "expected_rejections": [],
            "lean_standing": ["superseded", "accepted", "accepted"],
            "lean_reassessment": "needs_reassessment",
        },
        {
            "id": "stale-root-twin",
            "input": history([*PREFIX, stale], dependencies=DEPENDENCY),
            "expectation": "result",
            "expected_rejections": [{"code": "stale_root", "record_index": 8}],
            "lean_standing": ["accepted", "unassessed", "accepted"],
        },
        {
            "id": "unauthorized",
            "input": history(
                [*PREFIX, unauthorized, accept_replacement], dependencies=DEPENDENCY
            ),
            "expectation": "result",
            "expected_event_ids": [1, 2, 4],
            "expected_rejections": [{"code": "unauthorized", "record_index": 8}],
            "expected_root": 9,
        },
        {
            "id": "wrong-repository",
            "input": history(
                [*PREFIX, wrong_repository, accept_replacement],
                dependencies=DEPENDENCY,
            ),
            "expectation": "result",
            "expected_event_ids": [1, 2, 4],
            "expected_rejections": [{"code": "wrong_repository", "record_index": 8}],
            "expected_root": 9,
        },
        {
            "id": "misattributed",
            "input": history(
                [*PREFIX, misattributed, accept_replacement], dependencies=DEPENDENCY
            ),
            "expectation": "result",
            "expected_event_ids": [1, 2, 4],
            "expected_rejections": [{"code": "misattributed", "record_index": 8}],
            "expected_root": 9,
        },
        {
            "id": "stale-read-set",
            "input": history(
                [*PREFIX, stale_reads, accept_replacement], dependencies=DEPENDENCY
            ),
            "expectation": "result",
            "expected_event_ids": [1, 2, 4],
            "expected_rejections": [{"code": "stale_read_set", "record_index": 8}],
            "expected_root": 9,
        },
        {
            "id": "ineligible",
            "input": history(
                [*ineligible_prefix, FRESH, reject_replacement],
                dependencies=DEPENDENCY,
            ),
            "expectation": "result",
            "expected_event_ids": [1, 2, 4],
            "expected_rejections": [{"code": "ineligible", "record_index": 8}],
            "expected_root": 9,
        },
        {
            "id": "invalid-correction-order",
            "input": history(invalid_order, dependencies=DEPENDENCY),
            "expectation": "result",
            "expected_event_ids": [4],
            "expected_rejections": [
                {"code": "invalid_correction_reference", "record_index": 4}
            ],
            "expected_root": 5,
        },
        {
            "id": "evidence-no-standing",
            "input": history(common_source),
            "expectation": "result",
            "expected_rejections": [],
            "lean_standing": ["unassessed"],
        },
        {
            "id": "plural-authority-accept",
            "input": history([*common_source, decision(1, 1, 101, 2, accept(10))]),
            "expectation": "result",
            "expected_rejections": [],
            "lean_standing": ["accepted"],
        },
        {
            "id": "plural-authority-reject",
            "input": history(
                [*common_source, decision(1, 2, 202, 2, reject(10))],
                repository=2,
                authorized=[202],
            ),
            "expectation": "result",
            "expected_rejections": [],
            "lean_standing": ["unassessed"],
        },
        {
            "id": "fresh-no-dependency",
            "input": history([*PREFIX, FRESH]),
            "expectation": "result",
            "expected_rejections": [],
            "lean_standing": ["superseded", "accepted", "accepted"],
            "lean_reassessment": "unaffected",
        },
        {
            "id": "duplicate-decision-id",
            "input": history([*PREFIX, duplicate], dependencies=DEPENDENCY),
            "expectation": "invalid_format",
            "code": "invalid_format",
        },
        {
            "id": "stale-root-continuation",
            "input": history(
                [*PREFIX, stale, correct_after_stale], dependencies=DEPENDENCY
            ),
            "expectation": "result",
            "expected_event_ids": [1, 2, 4],
            "expected_rejections": [{"code": "stale_root", "record_index": 8}],
            "expected_root": 9,
            "expected_standing": ["superseded", "accepted", "accepted"],
            "lean_reassessment": "needs_reassessment",
            "lean_standing": ["superseded", "accepted", "accepted"],
        },
        {
            "id": "multiple-rejection-continuation",
            "input": history(
                [*PREFIX, stale, second_rejection, accept_after_two],
                dependencies=DEPENDENCY,
            ),
            "expectation": "result",
            "expected_event_ids": [1, 2, 5],
            "expected_rejections": [
                {"code": "stale_root", "record_index": 8},
                {"code": "unauthorized", "record_index": 9},
            ],
            "expected_root": 9,
            "expected_standing": ["accepted", "accepted", "accepted"],
        },
    ]
    return rows


def main() -> None:
    CASES.mkdir(parents=True, exist_ok=True)
    manifest_cases = []
    for row in case_rows():
        path = CASES / f"{row['id']}.json"
        payload = canonical_bytes(row.pop("input"))
        path.write_bytes(payload)
        manifest_row = {
            **row,
            "input_path": f"cases/{path.name}",
            "input_sha256": hashlib.sha256(payload).hexdigest(),
            "output_path": f"outputs/{path.name}",
            "output_sha256": None,
        }
        manifest_cases.append(manifest_row)
    manifest = {
        "aggregate_sha256": None,
        "cases": manifest_cases,
        "continuation_rejection_codes": [
            "ineligible",
            "invalid_correction_reference",
            "misattributed",
            "stale_read_set",
            "stale_root",
            "unauthorized",
            "wrong_repository",
        ],
        "format": "theory-of-standing.proof-corpus-manifest.v2",
        "projection_comparisons": [
            {"left": "fresh-correction", "right": "fresh-no-dependency"}
        ],
        "source_prefix_comparisons": [
            {
                "left": "plural-authority-accept",
                "record_count": 2,
                "right": "plural-authority-reject",
            }
        ],
    }
    (ROOT / "manifest.json").write_bytes(canonical_bytes(manifest))


if __name__ == "__main__":
    main()
