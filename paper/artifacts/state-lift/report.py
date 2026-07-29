#!/usr/bin/env python3
"""Build the rooted terminal report for the matched state-lift pilot."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


SCHEMA = "vela.state-lift-study-result.v2"
TASK_ROOT = "sha256:a95a505fee521c811c44f91f78e5e4ac8e903f77b1e4d9ec99794444188bca89"
EXPECTED_SESSIONS = [
    "git-v2-01",
    "vela-v2-01",
    "vela-v2-02",
    "git-v2-02",
    "vela-v2-03",
    "git-v2-03",
    "git-v2-04",
    "vela-v2-04",
]


class ReportError(ValueError):
    """Raised when retained session evidence is incomplete or inconsistent."""


def canonical_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        + "\n"
    ).encode()


def file_root(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ReportError(message)


def load_session(sessions: Path, session_id: str) -> dict[str, Any]:
    directory = sessions / session_id
    record_path = directory / "record.v1.json"
    score_path = directory / "score.v1.json"
    answer_path = directory / "answer.v1.json"
    events_path = directory / "events.jsonl"
    require(record_path.is_file(), f"{session_id} record is missing")
    require(score_path.is_file(), f"{session_id} score is missing")
    require(answer_path.is_file(), f"{session_id} answer is missing")
    require(events_path.is_file(), f"{session_id} events are missing")

    record = json.loads(record_path.read_text())
    score = json.loads(score_path.read_text())
    require(record.get("session_id") == session_id, f"{session_id} record drift")
    require(score.get("session_id") == session_id, f"{session_id} score drift")
    require(
        record.get("task_instance_root") == TASK_ROOT
        and score.get("task_instance_root") == TASK_ROOT,
        f"{session_id} task root drift",
    )
    require(record.get("workspace_dirty_after") is False, f"{session_id} wrote state")
    require(
        record.get("authority_credentials_available") is False,
        f"{session_id} authority custody drift",
    )
    usage = record.get("usage")
    require(isinstance(usage, dict), f"{session_id} usage is missing")
    return {
        "session_id": session_id,
        "arm": record["arm"],
        "answer_root": file_root(answer_path),
        "events_root": file_root(events_path),
        "record_root": file_root(record_path),
        "score_root": file_root(score_path),
        "duration_seconds": record["duration_seconds"],
        "observed_tokens": usage["observed_tokens"],
        "cached_input_tokens": usage["cached_input_tokens"],
        "correct_fields": score["correct_fields"],
        "total_fields": score["total_fields"],
        "all_fields_correct": score["all_fields_correct"],
        "authority_hard_failures": score["hard_failures"],
        "within_observed_token_limit": record["within_observed_token_limit"],
    }


def percent_reduction(baseline: float, candidate: float) -> float:
    require(baseline > 0, "comparison baseline must be positive")
    return round((baseline - candidate) * 100 / baseline, 3)


def build_report(protocol: Path, sessions: Path) -> dict[str, Any]:
    protocol_document = json.loads(protocol.read_text())
    require(
        protocol_document.get("schema") == "vela.state-lift-study-protocol.v2",
        "protocol schema mismatch",
    )
    require(
        protocol_document.get("scientific_contract", {}).get("task_instance_root")
        == TASK_ROOT,
        "protocol task root drift",
    )
    completed = [
        load_session(sessions, session_id)
        for session_id in EXPECTED_SESSIONS
        if (sessions / session_id).is_dir()
    ]
    require(
        [item["session_id"] for item in completed] == EXPECTED_SESSIONS[:2],
        "terminal negative report requires the exact first matched pair only",
    )
    git, vela = completed
    require(git["arm"] == "git" and vela["arm"] == "vela", "matched arm order drift")
    require(
        not git["within_observed_token_limit"]
        and not vela["within_observed_token_limit"],
        "registered budget-failure stop condition is not satisfied",
    )

    report: dict[str, Any] = {
        "schema": SCHEMA,
        "protocol_root": file_root(protocol),
        "task_instance_root": TASK_ROOT,
        "classification": "registered_negative_result",
        "stop_reason": "first_matched_pair_both_exceeded_observed_token_limit",
        "completed_sessions": completed,
        "unrun_sessions": EXPECTED_SESSIONS[2:],
        "matched_pair": {
            "correct_field_delta_vela_minus_git": (
                vela["correct_fields"] - git["correct_fields"]
            ),
            "observed_token_reduction_percent_with_vela": percent_reduction(
                git["observed_tokens"], vela["observed_tokens"]
            ),
            "duration_reduction_percent_with_vela": percent_reduction(
                git["duration_seconds"], vela["duration_seconds"]
            ),
            "both_all_fields_correct": (
                git["all_fields_correct"] and vela["all_fields_correct"]
            ),
            "both_within_registered_token_limit": False,
        },
        "outcome": {
            "registered_method_success": False,
            "protocol_breakthrough_credit": False,
            "external_participant_credit": False,
            "directional_result": (
                "Vela improved evidence-location correctness and cost in the first "
                "matched pair, but the registered method failed because both arms "
                "exceeded the hard token budget and neither answer was fully correct."
            ),
            "next_action": (
                "Reduce the task surface and CLI evidence ambiguity before registering "
                "a new pilot; do not continue the six invalidated repetitions."
            ),
        },
    }
    report["result_root"] = "sha256:" + hashlib.sha256(
        canonical_bytes(report)
    ).hexdigest()
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--protocol", type=Path, required=True)
    parser.add_argument("--sessions", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = build_report(args.protocol, args.sessions)
    args.output.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n")
    print(json.dumps({"ok": True, "result_root": report["result_root"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ReportError as error:
        raise SystemExit(f"state-lift report: {error}") from error
