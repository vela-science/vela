#!/usr/bin/env python3
"""Deterministically score one product-compression cold-use answer."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


ANSWER_SCHEMA = "vela.product-compression-answer.v1"
ANSWER_KEY_SCHEMA = "vela.product-compression-answer-key.v1"
RESULT_SCHEMA = "vela.product-compression-score.v1"
ARMS = {"git_files", "vela_guided"}


class ScoreInputError(ValueError):
    """Raised when a benchmark input violates the frozen contract."""


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    try:
        return sha256_bytes(path.read_bytes())
    except OSError as error:
        raise ScoreInputError(f"cannot read {path}: {error}") from error


def sha256_root(value: Any) -> str:
    return sha256_bytes(canonical_bytes(value))


def load_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ScoreInputError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise ScoreInputError(f"{path} must contain one JSON object")
    return value


def require_exact_keys(
    value: dict[str, Any], expected: set[str], path: str
) -> None:
    actual = set(value)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing or extra:
        raise ScoreInputError(
            f"{path} keys differ: missing={missing}, extra={extra}"
        )


def require_root(value: Any, path: str) -> None:
    if (
        not isinstance(value, str)
        or len(value) != 71
        or not value.startswith("sha256:")
        or any(character not in "0123456789abcdef" for character in value[7:])
    ):
        raise ScoreInputError(f"{path} must be one full lowercase sha256 root")


def validate_process(value: Any, arm: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ScoreInputError("answer.process must be an object")
    require_exact_keys(
        value,
        {
            "attempt_started",
            "attempt_start_method",
            "attempt_id",
            "authorization_root",
            "elapsed_ms",
            "observed_tokens",
            "command_count",
            "intervention_count",
            "command_log_root",
        },
        "answer.process",
    )
    for field in (
        "elapsed_ms",
        "observed_tokens",
        "command_count",
        "intervention_count",
    ):
        observed = value[field]
        if (
            not isinstance(observed, int)
            or isinstance(observed, bool)
            or observed < (1 if field == "elapsed_ms" else 0)
        ):
            raise ScoreInputError(f"answer.process.{field} is invalid")
    require_root(value["command_log_root"], "answer.process.command_log_root")

    if arm == "git_files":
        expected = {
            "attempt_started": False,
            "attempt_start_method": "manual_inspection_only",
            "attempt_id": None,
            "authorization_root": None,
        }
        for field, expected_value in expected.items():
            if value[field] != expected_value:
                raise ScoreInputError(
                    f"git_files process.{field} must equal {expected_value!r}"
                )
    else:
        if value["attempt_started"] is not True:
            raise ScoreInputError("vela_guided must retain a started Attempt")
        if value["attempt_start_method"] != "vela_start":
            raise ScoreInputError("vela_guided must use vela_start")
        attempt_id = value["attempt_id"]
        if (
            not isinstance(attempt_id, str)
            or len(attempt_id) != 68
            or not attempt_id.startswith("vat_")
            or any(
                character not in "0123456789abcdef" for character in attempt_id[4:]
            )
        ):
            raise ScoreInputError(
                "vela_guided process.attempt_id must be one full vat_ identifier"
            )
        require_root(
            value["authorization_root"], "answer.process.authorization_root"
        )
    return value


def flatten(value: Any, prefix: str = "") -> dict[str, Any]:
    if isinstance(value, dict):
        result: dict[str, Any] = {}
        for key in sorted(value):
            path = f"{prefix}.{key}" if prefix else key
            result.update(flatten(value[key], path))
        return result
    return {prefix: value}


def nested_get(value: dict[str, Any], path: str) -> Any:
    current: Any = value
    for part in path.split("."):
        if not isinstance(current, dict) or part not in current:
            raise ScoreInputError(f"missing expected answer path {path}")
        current = current[part]
    return current


def comparable(path: str, value: Any, set_paths: set[str]) -> Any:
    if path in set_paths:
        if not isinstance(value, list):
            raise ScoreInputError(f"{path} must be an array")
        encoded = [canonical_bytes(item) for item in value]
        if len(encoded) != len(set(encoded)):
            raise ScoreInputError(f"{path} must not contain duplicates")
        return sorted(encoded)
    return value


def validate_answer_key(value: dict[str, Any]) -> dict[str, Any]:
    require_exact_keys(
        value,
        {
            "schema",
            "expected",
            "rubric",
            "set_paths",
            "hard_gate_paths",
        },
        "answer_key",
    )
    if value["schema"] != ANSWER_KEY_SCHEMA:
        raise ScoreInputError(f"answer_key.schema must equal {ANSWER_KEY_SCHEMA}")
    if not isinstance(value["expected"], dict):
        raise ScoreInputError("answer_key.expected must be an object")
    expected_top = {
        "selection",
        "target",
        "attempt",
        "run",
        "inbox",
        "terminal_correction",
    }
    require_exact_keys(value["expected"], expected_top, "answer_key.expected")
    if not isinstance(value["rubric"], dict) or not value["rubric"]:
        raise ScoreInputError("answer_key.rubric must be a nonempty object")
    points = 0
    rubric_paths: list[str] = []
    for name, group in value["rubric"].items():
        if not isinstance(group, dict):
            raise ScoreInputError(f"rubric.{name} must be an object")
        require_exact_keys(group, {"points", "paths"}, f"rubric.{name}")
        if (
            not isinstance(group["points"], int)
            or isinstance(group["points"], bool)
            or group["points"] <= 0
        ):
            raise ScoreInputError(f"rubric.{name}.points must be positive")
        if (
            not isinstance(group["paths"], list)
            or not group["paths"]
            or any(not isinstance(path, str) or not path for path in group["paths"])
        ):
            raise ScoreInputError(f"rubric.{name}.paths is invalid")
        points += group["points"]
        rubric_paths.extend(group["paths"])
    if points != 100:
        raise ScoreInputError("rubric points must sum to 100")
    if sorted(rubric_paths) != sorted(expected_top):
        raise ScoreInputError("rubric must cover every expected top-level group once")
    for array_name in ("set_paths", "hard_gate_paths"):
        value_array = value[array_name]
        if (
            not isinstance(value_array, list)
            or len(value_array) != len(set(value_array))
            or any(not isinstance(item, str) or not item for item in value_array)
        ):
            raise ScoreInputError(
                f"answer_key.{array_name} must be a duplicate-free string array"
            )
    expected_flat = flatten(value["expected"])
    for path in value["set_paths"]:
        if path not in expected_flat:
            raise ScoreInputError(f"set path {path} is not an expected leaf")
    for path in value["hard_gate_paths"]:
        nested_get(value["expected"], path)
    return value


def validate_answer(value: dict[str, Any], plan_root: str) -> dict[str, Any]:
    require_exact_keys(
        value,
        {
            "schema",
            "plan_root",
            "session_id",
            "arm",
            "process",
            "answers",
        },
        "answer",
    )
    if value["schema"] != ANSWER_SCHEMA:
        raise ScoreInputError(f"answer.schema must equal {ANSWER_SCHEMA}")
    if value["plan_root"] != plan_root:
        raise ScoreInputError("answer.plan_root does not match the frozen plan")
    if (
        not isinstance(value["session_id"], str)
        or not value["session_id"]
        or any(character.isspace() for character in value["session_id"])
    ):
        raise ScoreInputError("answer.session_id must be one nonempty token")
    if value["arm"] not in ARMS:
        raise ScoreInputError(f"answer.arm must be one of {sorted(ARMS)}")
    validate_process(value["process"], value["arm"])
    if not isinstance(value["answers"], dict):
        raise ScoreInputError("answer.answers must be an object")
    return value


def score(
    plan_path: Path,
    answer_key: dict[str, Any],
    answer: dict[str, Any],
) -> dict[str, Any]:
    plan_root = sha256_file(plan_path)
    validate_answer_key(answer_key)
    validate_answer(answer, plan_root)
    expected = answer_key["expected"]
    observed = answer["answers"]
    require_exact_keys(observed, set(expected), "answer.answers")
    expected_flat = flatten(expected)
    observed_flat = flatten(observed)
    if set(expected_flat) != set(observed_flat):
        missing = sorted(set(expected_flat) - set(observed_flat))
        extra = sorted(set(observed_flat) - set(expected_flat))
        raise ScoreInputError(
            f"answer leaf paths differ: missing={missing}, extra={extra}"
        )

    set_paths = set(answer_key["set_paths"])
    field_results: list[dict[str, Any]] = []
    for path, expected_value in expected_flat.items():
        observed_value = observed_flat[path]
        correct = comparable(path, observed_value, set_paths) == comparable(
            path, expected_value, set_paths
        )
        field_results.append(
            {
                "field": path,
                "correct": correct,
                "expected": expected_value,
                "observed": observed_value,
            }
        )

    category_results: list[dict[str, Any]] = []
    score_basis_points = 0
    for name, group in answer_key["rubric"].items():
        prefixes = tuple(f"{path}." for path in group["paths"])
        members = [
            result
            for result in field_results
            if any(result["field"].startswith(prefix) for prefix in prefixes)
        ]
        correct = sum(1 for member in members if member["correct"])
        possible_basis_points = group["points"] * 100
        earned_basis_points = (
            possible_basis_points * correct // len(members)
        )
        score_basis_points += earned_basis_points
        category_results.append(
            {
                "category": name,
                "fields": len(members),
                "correct_fields": correct,
                "possible_basis_points": possible_basis_points,
                "earned_basis_points": earned_basis_points,
            }
        )

    hard_failures = [
        path
        for path in answer_key["hard_gate_paths"]
        if comparable(
            path,
            nested_get(observed, path),
            set_paths,
        )
        != comparable(
            path,
            nested_get(expected, path),
            set_paths,
        )
    ]
    passed = score_basis_points >= 9500 and not hard_failures
    result_without_root = {
        "schema": RESULT_SCHEMA,
        "plan_root": plan_root,
        "session_id": answer["session_id"],
        "arm": answer["arm"],
        "score_basis_points": score_basis_points,
        "passed": passed,
        "hard_failures": hard_failures,
        "category_results": category_results,
        "field_results": field_results,
        "process": answer["process"],
    }
    return {
        **result_without_root,
        "result_root": sha256_root(result_without_root),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--answer-key", type=Path, required=True)
    parser.add_argument("--answer", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        result = score(
            args.plan,
            load_object(args.answer_key),
            load_object(args.answer),
        )
    except ScoreInputError as error:
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
