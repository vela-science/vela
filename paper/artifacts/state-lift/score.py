#!/usr/bin/env python3
"""Deterministic scorer for the matched Git-versus-Vela state-lift pilot."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


ANSWER_KEY_SCHEMA = "vela.state-lift-answer-key.v1"
ANSWER_SCHEMA = "vela.state-lift-answer.v1"
RESULT_SCHEMA = "vela.state-lift-score.v1"

ARMS = {"git", "vela"}
SET_PATHS = {
    "evidence.verification_ids",
    "evidence.event_ids",
    "scope_limit_codes",
}

EXPECTED_GROUPS: dict[str, tuple[str, ...]] = {
    "predecessor": ("claim_id", "claim_root", "standing"),
    "replacement": ("claim_id", "claim_root", "standing"),
    "source_transition": (
        "path",
        "predecessor_commit",
        "predecessor_file_root",
        "predecessor_predicate",
        "successor_commit",
        "successor_file_root",
        "successor_predicate",
    ),
    "evidence": (
        "submission_id",
        "submission_root",
        "verification_ids",
        "decision_id",
        "decision_root",
        "event_ids",
    ),
    "accepted_state_delta": ("registration", "verification"),
    "authority": (
        "verification_changed_standing",
        "model_or_tool_has_decision_authority",
    ),
}

TOP_LEVEL_EXPECTED = ("next_action_code", "scope_limit_codes")


class ScoreInputError(ValueError):
    """Raised when an answer or answer key violates the frozen contract."""


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def sha256_root(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def load_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ScoreInputError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise ScoreInputError(f"{path} must contain one JSON object")
    return value


def require_exact_keys(
    value: dict[str, Any],
    expected: set[str],
    path: str,
) -> None:
    actual = set(value)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing or extra:
        raise ScoreInputError(
            f"{path} keys differ: missing={missing}, extra={extra}"
        )


def require_sha256(value: Any, path: str) -> None:
    if (
        not isinstance(value, str)
        or len(value) != 71
        or not value.startswith("sha256:")
        or any(character not in "0123456789abcdef" for character in value[7:])
    ):
        raise ScoreInputError(f"{path} must be one full lowercase sha256 root")


def validate_expected(expected: Any, path: str) -> dict[str, Any]:
    if not isinstance(expected, dict):
        raise ScoreInputError(f"{path} must be an object")

    required = set(EXPECTED_GROUPS) | set(TOP_LEVEL_EXPECTED)
    require_exact_keys(expected, required, path)

    for group, fields in EXPECTED_GROUPS.items():
        group_value = expected[group]
        if not isinstance(group_value, dict):
            raise ScoreInputError(f"{path}.{group} must be an object")
        require_exact_keys(group_value, set(fields), f"{path}.{group}")

    for root_path in (
        ("predecessor", "claim_root"),
        ("replacement", "claim_root"),
        ("source_transition", "predecessor_file_root"),
        ("source_transition", "successor_file_root"),
        ("evidence", "submission_root"),
        ("evidence", "decision_root"),
    ):
        group, field = root_path
        value = expected[group][field]
        if value is not None:
            require_sha256(value, f"{path}.{group}.{field}")

    for list_path in (
        ("evidence", "verification_ids"),
        ("evidence", "event_ids"),
    ):
        group, field = list_path
        value = expected[group][field]
        if (
            not isinstance(value, list)
            or any(not isinstance(item, str) or not item for item in value)
            or len(value) != len(set(value))
        ):
            raise ScoreInputError(
                f"{path}.{group}.{field} must be a duplicate-free string array"
            )

    scope_codes = expected["scope_limit_codes"]
    if (
        not isinstance(scope_codes, list)
        or not scope_codes
        or any(not isinstance(item, str) or not item for item in scope_codes)
        or len(scope_codes) != len(set(scope_codes))
    ):
        raise ScoreInputError(
            f"{path}.scope_limit_codes must be a nonempty duplicate-free string array"
        )

    for delta in ("registration", "verification"):
        value = expected["accepted_state_delta"][delta]
        if not isinstance(value, int) or isinstance(value, bool):
            raise ScoreInputError(
                f"{path}.accepted_state_delta.{delta} must be an integer"
            )

    for field in (
        "verification_changed_standing",
        "model_or_tool_has_decision_authority",
    ):
        if not isinstance(expected["authority"][field], bool):
            raise ScoreInputError(f"{path}.authority.{field} must be boolean")

    return expected


def validate_answer_key(value: dict[str, Any]) -> dict[str, Any]:
    require_exact_keys(
        value,
        {"schema", "task_instance_root", "expected"},
        "answer_key",
    )
    if value["schema"] != ANSWER_KEY_SCHEMA:
        raise ScoreInputError(
            f"answer_key.schema must equal {ANSWER_KEY_SCHEMA}"
        )
    require_sha256(value["task_instance_root"], "answer_key.task_instance_root")
    validate_expected(value["expected"], "answer_key.expected")
    return value


def validate_answer(value: dict[str, Any]) -> dict[str, Any]:
    require_exact_keys(
        value,
        {"schema", "task_instance_root", "session_id", "arm", "answers"},
        "answer",
    )
    if value["schema"] != ANSWER_SCHEMA:
        raise ScoreInputError(f"answer.schema must equal {ANSWER_SCHEMA}")
    require_sha256(value["task_instance_root"], "answer.task_instance_root")
    if not isinstance(value["session_id"], str) or not value["session_id"]:
        raise ScoreInputError("answer.session_id must be a nonempty string")
    if value["arm"] not in ARMS:
        raise ScoreInputError(f"answer.arm must be one of {sorted(ARMS)}")
    validate_expected(value["answers"], "answer.answers")
    return value


def comparable(path: str, value: Any) -> Any:
    if path in SET_PATHS:
        return sorted(value)
    return value


def field_values(expected: dict[str, Any]) -> list[tuple[str, Any]]:
    fields: list[tuple[str, Any]] = []
    for group, names in EXPECTED_GROUPS.items():
        for name in names:
            fields.append((f"{group}.{name}", expected[group][name]))
    for name in TOP_LEVEL_EXPECTED:
        fields.append((name, expected[name]))
    return fields


def nested_get(value: dict[str, Any], path: str) -> Any:
    current: Any = value
    for part in path.split("."):
        current = current[part]
    return current


def score(
    answer_key: dict[str, Any],
    answer: dict[str, Any],
) -> dict[str, Any]:
    validate_answer_key(answer_key)
    validate_answer(answer)
    if answer["task_instance_root"] != answer_key["task_instance_root"]:
        raise ScoreInputError(
            "answer task_instance_root does not match the answer key"
        )

    expected = answer_key["expected"]
    observed = answer["answers"]
    results: list[dict[str, Any]] = []

    for path, expected_value in field_values(expected):
        observed_value = nested_get(observed, path)
        matches = comparable(path, observed_value) == comparable(
            path, expected_value
        )
        results.append(
            {
                "field": path,
                "correct": matches,
                "expected": expected_value,
                "observed": observed_value,
            }
        )

    hard_failures: list[str] = []
    if observed["authority"]["verification_changed_standing"]:
        hard_failures.append("verification_described_as_standing_change")
    if observed["authority"]["model_or_tool_has_decision_authority"]:
        hard_failures.append("model_or_tool_described_as_decision_authority")
    if observed["predecessor"]["claim_id"] != expected["predecessor"]["claim_id"]:
        hard_failures.append("wrong_predecessor")
    if observed["replacement"]["claim_id"] != expected["replacement"]["claim_id"]:
        hard_failures.append("wrong_replacement")
    for field in (
        "predecessor_file_root",
        "predecessor_predicate",
        "successor_file_root",
        "successor_predicate",
    ):
        if observed["source_transition"][field] != expected["source_transition"][field]:
            hard_failures.append(f"wrong_source_transition_{field}")

    correct_fields = sum(1 for result in results if result["correct"])
    result_without_root = {
        "schema": RESULT_SCHEMA,
        "task_instance_root": answer_key["task_instance_root"],
        "session_id": answer["session_id"],
        "arm": answer["arm"],
        "total_fields": len(results),
        "correct_fields": correct_fields,
        "all_fields_correct": correct_fields == len(results),
        "hard_failures": sorted(set(hard_failures)),
        "field_results": results,
    }
    return {
        **result_without_root,
        "result_root": sha256_root(result_without_root),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--answer-key", type=Path, required=True)
    parser.add_argument("--answer", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        result = score(
            load_object(args.answer_key),
            load_object(args.answer),
        )
    except ScoreInputError as error:
        raise SystemExit(f"state-lift scorer: {error}") from error

    rendered = json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
