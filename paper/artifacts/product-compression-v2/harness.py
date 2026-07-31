#!/usr/bin/env python3
"""Closed, source-only contracts for product-compression v2."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import datetime
from fractions import Fraction
from pathlib import Path
from typing import Any, Iterable, Sequence

ROOT = re.compile(r"^sha256:[0-9a-f]{64}$")
PATTERNS = {
    "frontier": re.compile(r"^vfr_[0-9a-f]{16}$"),
    "attempt": re.compile(r"^vat_[0-9a-f]{64}$"),
    "run": re.compile(r"^run_[A-Za-z0-9-]{8,128}$"),
    "submission": re.compile(r"^vsb_[0-9a-f]{16}$"),
    "proposal": re.compile(r"^vpr_[0-9a-f]{16}$"),
    "claim": re.compile(r"^vcl_[0-9a-f]{64}$"),
    "verification": re.compile(r"^vvr_[0-9a-f]{16}$"),
}
ARMS = ("git-files", "vela-guided")
HARBOR = {
    "name": "harbor",
    "version": "0.20.0",
    "source_commit": "459ff6ec99417589b7f679d14ddf3b3f0ae4f1dc",
    "package_sha256": "sha256:4b7e48223aea2384cdb8c9eff35eaebd482fc9b1ec09f8193a121c47356ff19a",
    "task_schema": "1.4",
    "trajectory_schema": "ATIF-v1.7",
}
MAYBE_TEXT = (str, type(None))
STANDING = [{"claim_id": str, "claim_root": str}]
RUN = {
    "run_number": int, "run_id": str, "receipt_root": str,
    "previous_receipt_root": MAYBE_TEXT, "evidence_root": str,
    "submission_state": str, "submission_id": MAYBE_TEXT,
    "proposal_id": MAYBE_TEXT, "claim_id": MAYBE_TEXT,
    "verification_id": MAYBE_TEXT,
}
ANSWER = {
    "schema": str,
    "work": {"frontier_id": str, "repository_root": str, "target_id": str,
             "target_index_root": str, "packet_sha256": str},
    "campaign": {
        "attempt_id": str, "authorization_root": str, "state": str,
        "completed_target_packet_sha256": str, "consequence_ceiling": str,
        "budget": {"max_runs": int, "max_submissions": int, "max_verifications": int,
                   "max_artifacts": int, "max_artifact_bytes": int},
        "usage": {"runs": int, "submissions": int, "verifications": int,
                  "artifacts": int, "artifact_bytes": int},
        "runs": [RUN], "next_action_code": str,
    },
    "review": {
        "proposal_id": str, "proposal_root": str, "source_submission_id": str,
        "target_claim_id": str, "verification_id": str, "inbox_projection_root": str,
        "inbox_entry_root": str, "protocol_gate": str, "human_decision_required": bool,
        "verification_is_acceptance": bool, "standing_transition": str,
        "accepted_before": STANDING, "accepted_if_accept": STANDING,
        "accepted_if_reject": STANDING, "staleness": str,
        "next_if_accept_code": str, "next_if_reject_code": str,
    },
    "safety": {"authority_action_performed": bool, "accepted_state_changed": bool},
}
TOOL = {"tool_contract_root": str, "interface": str, "vela_available": bool}
EXECUTOR = {
    "executor_root": str, "name": str, "version": str, "source_commit": str,
    "package_sha256": str, "task_schema": str, "trajectory_schema": str,
}
PLAN = {
    "schema": str, "plan_root": str, "fixture_root": str, "answer_key_root": str,
    "executor": EXECUTOR,
    "model": {"id": str, "config_root": str},
    "arms": {"git-files": TOOL, "vela-guided": TOOL},
    "budgets": {"elapsed_ms": int, "tool_calls": int, "observed_tokens": int,
                "per_tool_reported_output_bytes": int,
                "total_tool_reported_output_bytes": int,
                "trajectory_bytes": int, "verifier_output_bytes": int,
                "answer_bytes": int},
    "assignments": [{"pair": str, "order": [str]}],
    "publication_policy": {"publish_all_sessions": bool, "publish_failures": bool,
                           "independence_claim": str, "plan_changes_after_output": str},
}
TOOL_CALL = {
    "index": int, "item_id": str, "argv": [str], "elapsed_ms": int,
    "state": str, "exit_code": (int, type(None)),
    "reported_output_bytes": int, "reported_output_root": str,
}
PAIR = {"before": str, "after": str}
USAGE = {
    "complete": bool,
    "input_tokens": (int, type(None)),
    "cached_input_tokens": (int, type(None)),
    "output_tokens": (int, type(None)),
    "reasoning_output_tokens": (int, type(None)),
}
TRACE = {
    "instruction_root": str, "answer_schema_root": str,
    "native_trajectory_root": str, "native_trajectory_bytes": int,
    "atif_root": str, "atif_bytes": int,
    "verifier_stdout_root": str, "verifier_stdout_bytes": int,
    "verifier_stderr_root": str, "verifier_stderr_bytes": int,
    "artifacts_manifest_root": str, "artifacts_manifest_bytes": int,
}
SESSION = {
    "schema": str, "session_root": str, "plan_root": str, "fixture_root": str,
    "answer_key_root": str, "session_id": str, "arm": str, "model": str,
    "model_config_root": str, "tool_contract_root": str, "executor_root": str,
    "started_at": str,
    "completed_at": str, "elapsed_ms": int, "termination": str,
    "process": {"exit_code": (int, type(None)), "signal": (int, type(None))},
    "usage": USAGE, "tool_calls": [TOOL_CALL], "trace": TRACE,
    "semantic_interventions": [str],
    "state": {"git_status": PAIR, "repository": PAIR, "standing": PAIR},
    "violations": [str], "answer_root": MAYBE_TEXT, "answer": (dict, type(None)),
}
METRICS = {"elapsed_ms": int, "tool_call_count": int,
           "observed_tokens": (int, type(None)),
           "uncached_token_proxy": (int, type(None)),
           "semantic_intervention_count": int, "tool_reported_output_bytes": int}
SCORE = {
    "schema": str, "score_root": str, "plan_root": str, "session_root": str,
    "session_id": str, "arm": str, "passed": bool, "failure_codes": [str],
    "metrics": METRICS,
}


class ContractError(ValueError):
    """A retained document violates a closed study contract."""


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha256_root(value: bytes) -> str:
    return f"sha256:{hashlib.sha256(value).hexdigest()}"


def record_root(value: dict[str, Any], field: str) -> str:
    return sha256_root(canonical_bytes({key: item for key, item in value.items() if key != field}))


def seal(value: dict[str, Any], field: str) -> dict[str, Any]:
    value[field] = record_root(value, field)
    return value


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ContractError(f"cannot read JSON {path}: {exc}") from exc


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(canonical_bytes(value))


def shape(value: Any, contract: Any, location: str = "$") -> None:
    """Validate one explicit study shape; this is not a JSON-Schema interpreter."""
    if isinstance(contract, dict):
        if not isinstance(value, dict):
            raise ContractError(f"{location}: expected object")
        missing, extra = sorted(set(contract) - set(value)), sorted(set(value) - set(contract))
        if missing or extra:
            raise ContractError(f"{location}: missing {missing}; unexpected {extra}")
        for key, child in contract.items():
            shape(value[key], child, f"{location}.{key}")
    elif isinstance(contract, list):
        if not isinstance(value, list):
            raise ContractError(f"{location}: expected array")
        for index, child in enumerate(value):
            shape(child, contract[0], f"{location}[{index}]")
    elif not isinstance(value, contract) or (
        isinstance(value, bool)
        and (contract is int or (isinstance(contract, tuple) and int in contract))
    ):
        raise ContractError(f"{location}: wrong type")


def matches(value: Any, pattern: re.Pattern[str], location: str) -> None:
    if not isinstance(value, str) or not pattern.fullmatch(value):
        raise ContractError(f"{location}: invalid value")


def roots(value: dict[str, Any], names: Iterable[str], location: str) -> None:
    for name in names:
        matches(value[name], ROOT, f"{location}.{name}")


def nonnegative(value: dict[str, Any], location: str, *, positive: bool = False) -> None:
    minimum = 1 if positive else 0
    for key, item in value.items():
        if not isinstance(item, int) or isinstance(item, bool) or item < minimum:
            raise ContractError(f"{location}.{key}: expected integer >= {minimum}")


def rooted(value: dict[str, Any], field: str, location: str = "$") -> None:
    matches(value[field], ROOT, f"{location}.{field}")
    if value[field] != record_root(value, field):
        raise ContractError(f"{location}.{field}: root mismatch")


def validate_answer(value: Any) -> None:
    shape(value, ANSWER)
    if value["schema"] != "vela.product-compression-answer.v2":
        raise ContractError("$.schema: wrong answer schema")
    work, campaign, review = value["work"], value["campaign"], value["review"]
    matches(work["frontier_id"], PATTERNS["frontier"], "$.work.frontier_id")
    matches(work["target_id"], re.compile(r"^[A-Za-z0-9._:-]+$"), "$.work.target_id")
    roots(work, ("repository_root", "target_index_root", "packet_sha256"), "$.work")
    matches(campaign["attempt_id"], PATTERNS["attempt"], "$.campaign.attempt_id")
    roots(campaign, ("authorization_root", "completed_target_packet_sha256"), "$.campaign")
    if campaign["state"] != "completed_target_advanced":
        raise ContractError("$.campaign.state: expected completed_target_advanced")
    if campaign["consequence_ceiling"] not in {"evidence_only", "pending_review"}:
        raise ContractError("$.campaign.consequence_ceiling: invalid value")
    nonnegative(campaign["budget"], "$.campaign.budget", positive=True)
    nonnegative(campaign["usage"], "$.campaign.usage")
    for used, limit in (("runs", "max_runs"), ("submissions", "max_submissions"),
                        ("verifications", "max_verifications"), ("artifacts", "max_artifacts"),
                        ("artifact_bytes", "max_artifact_bytes")):
        if campaign["usage"][used] > campaign["budget"][limit]:
            raise ContractError(f"$.campaign.usage.{used}: exceeds {limit}")
    runs = campaign["runs"]
    if len(runs) != 2 or [run["run_number"] for run in runs] != [1, 2]:
        raise ContractError("$.campaign.runs: expected ordered Runs 1 and 2")
    for index, run in enumerate(runs):
        matches(run["run_id"], PATTERNS["run"], f"$.campaign.runs[{index}].run_id")
        roots(run, ("receipt_root", "evidence_root"), f"$.campaign.runs[{index}]")
    if runs[0]["previous_receipt_root"] is not None or runs[1]["previous_receipt_root"] != runs[0]["receipt_root"]:
        raise ContractError("$.campaign.runs: broken receipt chain")
    if sorted(run["submission_state"] for run in runs) != ["registered", "retained_corroboration"]:
        raise ContractError("$.campaign.runs: expected one registered and one retained corroboration")
    registered = next(run for run in runs if run["submission_state"] == "registered")
    corroborating = next(run for run in runs if run["submission_state"] == "retained_corroboration")
    lifecycle = (("submission_id", "submission"), ("proposal_id", "proposal"),
                 ("claim_id", "claim"), ("verification_id", "verification"))
    for field, kind in lifecycle:
        matches(registered[field], PATTERNS[kind], f"$.campaign.registered_run.{field}")
        if corroborating[field] is not None:
            raise ContractError(f"$.campaign.corroborating_run.{field}: retained Run has no separate registration")
    if campaign["usage"]["runs"] != 2 or campaign["usage"]["submissions"] != 1 or campaign["usage"]["verifications"] != 1:
        raise ContractError("$.campaign.usage: does not match Run lifecycle")
    if campaign["next_action_code"] != "start_successor_attempt":
        raise ContractError("$.campaign.next_action_code: expected start_successor_attempt")
    id_links = (("proposal_id", "proposal"), ("source_submission_id", "submission"),
                ("target_claim_id", "claim"), ("verification_id", "verification"))
    for field, kind in id_links:
        matches(review[field], PATTERNS[kind], f"$.review.{field}")
    roots(review, ("proposal_root", "inbox_projection_root", "inbox_entry_root"), "$.review")
    if tuple(review[field] for field, _ in id_links) != (registered["proposal_id"], registered["submission_id"], registered["claim_id"], registered["verification_id"]):
        raise ContractError("$.review: Proposal is not linked to registered Run Submission")
    if review["protocol_gate"] not in {"satisfied", "blocked"} or review["staleness"] not in {"current", "stale"}:
        raise ContractError("$.review: invalid readiness state")
    required_review = (True, False, "add accepted Claim", "replay_and_recompute_targets", "replay_without_standing_change")
    observed_review = (review["human_decision_required"], review["verification_is_acceptance"], review["standing_transition"], review["next_if_accept_code"], review["next_if_reject_code"])
    if observed_review != required_review:
        raise ContractError("$.review: authority or next-obligation contract is misstated")
    for field in ("accepted_before", "accepted_if_accept", "accepted_if_reject"):
        for index, standing in enumerate(review[field]):
            matches(standing["claim_id"], PATTERNS["claim"], f"$.review.{field}[{index}].claim_id")
            roots(standing, ("claim_root",), f"$.review.{field}[{index}]")
        if len({item["claim_id"] for item in review[field]}) != len(review[field]):
            raise ContractError(f"$.review.{field}: duplicate Claim")
    before, accepted, rejected = review["accepted_before"], review["accepted_if_accept"], review["accepted_if_reject"]
    additions = [item for item in accepted if item not in before]
    if rejected != before:
        raise ContractError("$.review.accepted_if_reject: rejection must preserve Standing")
    if len(accepted) != len(before) + 1 or len(additions) != 1 or additions[0]["claim_id"] != review["target_claim_id"]:
        raise ContractError("$.review.accepted_if_accept: must add exactly proposed Claim")
    if value["safety"] != {"authority_action_performed": False, "accepted_state_changed": False}:
        raise ContractError("$.safety: inspection must remain read-only")


def validate_plan(value: Any) -> None:
    shape(value, PLAN)
    if value["schema"] != "vela.product-compression-plan.v2":
        raise ContractError("$.schema: wrong plan schema")
    roots(value, ("fixture_root", "answer_key_root"), "$")
    executor = value["executor"]
    rooted(executor, "executor_root", "$.executor")
    if {key: executor[key] for key in HARBOR} != HARBOR:
        raise ContractError("$.executor: unsupported or unpinned execution harness")
    roots(value["model"], ("config_root",), "$.model")
    if not value["model"]["id"]:
        raise ContractError("$.model.id: empty")
    for arm in ARMS:
        tool = value["arms"][arm]
        expected = {
            "git-files": ("native-read-only-workspace", False),
            "vela-guided": ("native-read-only-workspace-plus-vela", True),
        }[arm]
        if (tool["interface"], tool["vela_available"]) != expected:
            raise ContractError(f"$.arms.{arm}: wrong execution interface")
        rooted(tool, "tool_contract_root", f"$.arms.{arm}")
    if value["arms"][ARMS[0]]["tool_contract_root"] == value["arms"][ARMS[1]]["tool_contract_root"]:
        raise ContractError("$.arms: tool contracts must differ")
    nonnegative(value["budgets"], "$.budgets", positive=True)
    assignments = value["assignments"]
    if len(assignments) != 2:
        raise ContractError("$.assignments: expected two pairs")
    orders, sessions = [], []
    for assignment in assignments:
        matches(assignment["pair"], re.compile(r"^[0-9]{2}$"), "$.assignments.pair")
        expected = {f"git-files-{assignment['pair']}", f"vela-guided-{assignment['pair']}"}
        if len(assignment["order"]) != 2 or set(assignment["order"]) != expected:
            raise ContractError("$.assignments.order: pair must contain both arms")
        orders.append([item.rsplit("-", 1)[0] for item in assignment["order"]])
        sessions.extend(assignment["order"])
    if len(set(sessions)) != 4 or orders[0] == orders[1]:
        raise ContractError("$.assignments: orders must be unique AB/BA pairs")
    expected_policy = {"publish_all_sessions": True, "publish_failures": True,
                       "independence_claim": "first_party_only", "plan_changes_after_output": "forbidden"}
    if value["publication_policy"] != expected_policy:
        raise ContractError("$.publication_policy: failures must remain public and first-party")
    rooted(value, "plan_root")


def validate_answer_key(value: Any) -> None:
    shape(value, {"schema": str, "answer_key_root": str, "fixture_root": str, "expected": ANSWER})
    if value["schema"] != "vela.product-compression-answer-key.v2":
        raise ContractError("$.schema: wrong answer-key schema")
    roots(value, ("fixture_root",), "$")
    validate_answer(value["expected"])
    rooted(value, "answer_key_root")


def session_arms(plan: dict[str, Any]) -> dict[str, str]:
    return {session: session.rsplit("-", 1)[0] for pair in plan["assignments"] for session in pair["order"]}


def validate_session(value: Any) -> None:
    shape(value, SESSION)
    if value["schema"] != "vela.product-compression-session.v2":
        raise ContractError("$.schema: wrong session schema")
    roots(value, ("plan_root", "fixture_root", "answer_key_root", "model_config_root", "tool_contract_root", "executor_root"), "$")
    matches(value["session_id"], re.compile(r"^(git-files|vela-guided)-[0-9]{2}$"), "$.session_id")
    if value["arm"] not in ARMS:
        raise ContractError("$.arm: invalid arm")
    try:
        start = datetime.fromisoformat(value["started_at"].replace("Z", "+00:00"))
        end = datetime.fromisoformat(value["completed_at"].replace("Z", "+00:00"))
    except ValueError as exc:
        raise ContractError("$.started_at/completed_at: invalid timestamp") from exc
    if end < start or value["elapsed_ms"] != int((end - start).total_seconds() * 1000):
        raise ContractError("$.elapsed_ms: does not match timestamps")
    if value["termination"] not in {"completed", "limit", "infrastructure_failure", "forbidden_action", "integrity_failure"}:
        raise ContractError("$.termination: invalid value")
    process = value["process"]
    if (process["exit_code"] is None) == (process["signal"] is None):
        raise ContractError("$.process: exactly one of exit_code or signal is required")
    if process["signal"] is not None and process["signal"] <= 0:
        raise ContractError("$.process.signal: expected positive signal")
    usage = value["usage"]
    token_fields = ("input_tokens", "cached_input_tokens", "output_tokens", "reasoning_output_tokens")
    if usage["complete"]:
        nonnegative({field: usage[field] for field in token_fields}, "$.usage")
        if usage["cached_input_tokens"] > usage["input_tokens"]:
            raise ContractError("$.usage.cached_input_tokens: exceeds input tokens")
        if usage["reasoning_output_tokens"] > usage["output_tokens"]:
            raise ContractError("$.usage.reasoning_output_tokens: exceeds output tokens")
    elif any(usage[field] is not None for field in token_fields):
        raise ContractError("$.usage: incomplete terminal usage must use null counters")
    item_ids: set[str] = set()
    for index, tool_call in enumerate(value["tool_calls"], start=1):
        if tool_call["index"] != index or not tool_call["item_id"] or tool_call["item_id"] in item_ids:
            raise ContractError("$.tool_calls: invalid index or duplicate item_id")
        item_ids.add(tool_call["item_id"])
        if not tool_call["argv"] or any(not item for item in tool_call["argv"]):
            raise ContractError("$.tool_calls: empty argv")
        if tool_call["state"] not in {"completed", "incomplete"}:
            raise ContractError("$.tool_calls.state: invalid value")
        if (tool_call["state"] == "completed") != (tool_call["exit_code"] is not None):
            raise ContractError("$.tool_calls.exit_code: must exist only for completed calls")
        nonnegative({key: tool_call[key] for key in ("elapsed_ms", "reported_output_bytes")}, f"$.tool_calls[{index - 1}]")
        roots(tool_call, ("reported_output_root",), f"$.tool_calls[{index - 1}]")
    trace = value["trace"]
    roots(trace, ("instruction_root", "answer_schema_root", "native_trajectory_root", "atif_root",
                  "verifier_stdout_root", "verifier_stderr_root", "artifacts_manifest_root"), "$.trace")
    nonnegative({key: trace[key] for key in ("native_trajectory_bytes", "atif_bytes",
                "verifier_stdout_bytes", "verifier_stderr_bytes", "artifacts_manifest_bytes")}, "$.trace")
    for name, pair in value["state"].items():
        roots(pair, ("before", "after"), f"$.state.{name}")
    if value["answer"] is None:
        if value["answer_root"] is not None:
            raise ContractError("$.answer_root: must be null without answer")
    else:
        validate_answer(value["answer"])
        roots(value, ("answer_root",), "$")
        if value["answer_root"] != sha256_root(canonical_bytes(value["answer"])):
            raise ContractError("$.answer_root: root mismatch")
    rooted(value, "session_root")


def validate_score(value: Any) -> None:
    shape(value, SCORE)
    if value["schema"] != "vela.product-compression-score.v2":
        raise ContractError("$.schema: wrong score schema")
    roots(value, ("plan_root", "session_root"), "$")
    if value["arm"] not in ARMS or len(set(value["failure_codes"])) != len(value["failure_codes"]):
        raise ContractError("$.arm/failure_codes: invalid")
    metrics = value["metrics"]
    nonnegative({key: metrics[key] for key in ("elapsed_ms", "tool_call_count", "semantic_intervention_count", "tool_reported_output_bytes")}, "$.metrics")
    for field in ("observed_tokens", "uncached_token_proxy"):
        if metrics[field] is not None and (isinstance(metrics[field], bool) or metrics[field] < 0):
            raise ContractError(f"$.metrics.{field}: expected nonnegative integer or null")
    if value["passed"] != (not value["failure_codes"]):
        raise ContractError("$.passed: must equal absence of failure codes")
    rooted(value, "score_root")


def score_session(plan: Any, answer_key: Any, session: Any) -> dict[str, Any]:
    validate_plan(plan)
    validate_answer_key(answer_key)
    validate_session(session)
    failures: list[str] = []
    arms = session_arms(plan)
    if arms.get(session["session_id"]) != session["arm"]:
        failures.append("assignment_mismatch")
    expected_roots = {"plan_root": plan["plan_root"], "fixture_root": plan["fixture_root"],
                      "answer_key_root": answer_key["answer_key_root"],
                      "executor_root": plan["executor"]["executor_root"]}
    for key, expected in expected_roots.items():
        if session[key] != expected:
            failures.append(f"{key}_mismatch")
    if plan["fixture_root"] != answer_key["fixture_root"] or plan["answer_key_root"] != answer_key["answer_key_root"]:
        failures.append("plan_material_mismatch")
    if session["model"] != plan["model"]["id"] or session["model_config_root"] != plan["model"]["config_root"]:
        failures.append("model_mismatch")
    contract = plan["arms"][session["arm"]]
    if session["tool_contract_root"] != contract["tool_contract_root"]:
        failures.append("tool_contract_mismatch")
    if session["termination"] != "completed":
        failures.append("termination_not_completed")
    if session["process"] != {"exit_code": 0, "signal": None}:
        failures.append("nonzero_exit")
    if session["semantic_interventions"]:
        failures.append("semantic_intervention")
    if session["violations"]:
        failures.append("session_violation")
    for name, pair in session["state"].items():
        if pair["before"] != pair["after"]:
            failures.append(f"{name}_drift")
    if any(item["state"] != "completed" for item in session["tool_calls"]):
        failures.append("incomplete_tool_call")
    output_bytes = sum(item["reported_output_bytes"] for item in session["tool_calls"])
    usage = session["usage"]
    observed_tokens = None
    uncached_token_proxy = None
    if usage["complete"]:
        observed_tokens = usage["input_tokens"] + usage["output_tokens"]
        uncached_token_proxy = usage["input_tokens"] - usage["cached_input_tokens"] + usage["output_tokens"]
    else:
        failures.append("usage_incomplete")
    budget = plan["budgets"]
    trace = session["trace"]
    trajectory_bytes = trace["native_trajectory_bytes"] + trace["atif_bytes"]
    verifier_output_bytes = trace["verifier_stdout_bytes"] + trace["verifier_stderr_bytes"]
    checks = ((session["elapsed_ms"] > budget["elapsed_ms"], "elapsed_limit"),
              (len(session["tool_calls"]) > budget["tool_calls"], "tool_call_limit"),
              (observed_tokens is not None and observed_tokens > budget["observed_tokens"], "post_run_token_limit"),
              (output_bytes > budget["total_tool_reported_output_bytes"], "total_tool_reported_output_limit"),
              (any(item["reported_output_bytes"] > budget["per_tool_reported_output_bytes"] for item in session["tool_calls"]), "per_tool_reported_output_limit"),
              (trajectory_bytes > budget["trajectory_bytes"], "trajectory_limit"),
              (verifier_output_bytes > budget["verifier_output_bytes"], "verifier_output_limit"))
    failures.extend(code for failed, code in checks if failed)
    if session["answer"] is None:
        failures.append("answer_missing")
    else:
        if len(canonical_bytes(session["answer"])) > budget["answer_bytes"]:
            failures.append("answer_limit")
        if session["answer"] != answer_key["expected"]:
            failures.append("answer_mismatch")
    result = {"schema": "vela.product-compression-score.v2", "score_root": "",
              "plan_root": plan["plan_root"], "session_root": session["session_root"],
              "session_id": session["session_id"], "arm": session["arm"],
              "passed": not failures, "failure_codes": sorted(set(failures)),
              "metrics": {"elapsed_ms": session["elapsed_ms"], "tool_call_count": len(session["tool_calls"]),
                          "observed_tokens": observed_tokens,
                          "uncached_token_proxy": uncached_token_proxy,
                          "semantic_intervention_count": len(session["semantic_interventions"]),
                          "tool_reported_output_bytes": output_bytes}}
    seal(result, "score_root")
    validate_score(result)
    return result


def median(values: Sequence[int]) -> Fraction:
    ordered = sorted(values)
    middle = len(ordered) // 2
    return Fraction(ordered[middle], 1) if len(ordered) % 2 else Fraction(ordered[middle - 1] + ordered[middle], 2)


def validate_report(value: Any) -> None:
    contract = {"schema": str, "report_root": str, "plan_root": str, "score_roots": [str],
                "pairs": [{"pair": str, "guided_faster": bool}],
                "elapsed_improvement_basis_points": (int, type(None)),
                "gates": {"all_sessions_pass": bool, "guided_faster_in_both_pairs": bool,
                          "median_elapsed_improvement_at_least_20_percent": bool,
                          "median_tool_calls_no_regression": bool,
                          "median_observed_tokens_no_regression": bool},
                "passed": bool, "failure_codes": [str]}
    shape(value, contract)
    if value["schema"] != "vela.product-compression-result.v2":
        raise ContractError("$.schema: wrong result schema")
    roots(value, ("plan_root",), "$")
    for index, item in enumerate(value["score_roots"]):
        matches(item, ROOT, f"$.score_roots[{index}]")
    if value["passed"] != (not value["failure_codes"]):
        raise ContractError("$.passed: must equal absence of failure codes")
    rooted(value, "report_root")


def build_report(plan: Any, answer_key: Any, sessions: Iterable[Any]) -> dict[str, Any]:
    validate_plan(plan)
    validate_answer_key(answer_key)
    scores = [score_session(plan, answer_key, session) for session in sessions]
    failures, by_id = [], {}
    expected = session_arms(plan)
    for score in scores:
        if score["session_id"] in by_id:
            failures.append("duplicate_session")
        by_id[score["session_id"]] = score
        if score["plan_root"] != plan["plan_root"] or score["arm"] != expected.get(score["session_id"]):
            failures.append("score_plan_mismatch")
        if not score["passed"] or score["failure_codes"]:
            failures.append("session_failure")
    if set(by_id) != set(expected) or len(scores) != len(expected):
        failures.append("incomplete_assignment")
    pairs = []
    for assignment in plan["assignments"]:
        retained = [by_id.get(item) for item in assignment["order"]]
        if any(item is None for item in retained):
            continue
        git = next(item for item in retained if item["arm"] == "git-files")
        guided = next(item for item in retained if item["arm"] == "vela-guided")
        faster = guided["metrics"]["elapsed_ms"] < git["metrics"]["elapsed_ms"]
        if not faster:
            failures.append(f"pair_{assignment['pair']}_not_faster")
        pairs.append({"pair": assignment["pair"], "guided_faster": faster})
    elapsed = commands = tokens = False
    improvement_bps = None
    if set(by_id) == set(expected):
        arm_scores = {arm: [item for item in by_id.values() if item["arm"] == arm] for arm in ARMS}
        med = {metric: tuple(median([item["metrics"][metric] for item in arm_scores[arm]]) for arm in ARMS)
               for metric in ("elapsed_ms", "tool_call_count")}
        if med["elapsed_ms"][0] > 0:
            improvement = (med["elapsed_ms"][0] - med["elapsed_ms"][1]) * 10_000 / med["elapsed_ms"][0]
            improvement_bps = improvement.numerator // improvement.denominator
            elapsed = improvement >= 2_000
        else:
            failures.append("zero_baseline_elapsed")
        commands = med["tool_call_count"][1] <= med["tool_call_count"][0]
        observed_by_arm = {
            arm: [item["metrics"]["observed_tokens"] for item in arm_scores[arm]]
            for arm in ARMS
        }
        if all(all(item is not None for item in values) for values in observed_by_arm.values()):
            token_medians = tuple(median(observed_by_arm[arm]) for arm in ARMS)
            tokens = token_medians[1] <= token_medians[0]
        else:
            failures.append("usage_incomplete")
        if not elapsed:
            failures.append("elapsed_improvement_below_20_percent")
        if not commands:
            failures.append("command_regression")
        if not tokens:
            failures.append("token_regression")
    result = {"schema": "vela.product-compression-result.v2", "report_root": "",
              "plan_root": plan["plan_root"], "score_roots": sorted(item["score_root"] for item in scores),
              "pairs": pairs, "elapsed_improvement_basis_points": improvement_bps,
              "gates": {"all_sessions_pass": set(by_id) == set(expected) and all(item["passed"] and not item["failure_codes"] for item in scores),
                        "guided_faster_in_both_pairs": len(pairs) == 2 and all(item["guided_faster"] for item in pairs),
                        "median_elapsed_improvement_at_least_20_percent": elapsed,
                        "median_tool_calls_no_regression": commands,
                        "median_observed_tokens_no_regression": tokens},
              "passed": not failures, "failure_codes": sorted(set(failures))}
    seal(result, "report_root")
    validate_report(result)
    return result


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    validate = commands.add_parser("validate")
    validate.add_argument("--kind", choices=("answer", "plan", "answer-key", "session", "score", "report"), required=True)
    validate.add_argument("--input", type=Path, required=True)
    score = commands.add_parser("score")
    for name in ("plan", "answer-key", "session", "output"):
        score.add_argument(f"--{name}", type=Path, required=True)
    report = commands.add_parser("report")
    report.add_argument("--plan", type=Path, required=True)
    report.add_argument("--answer-key", type=Path, required=True)
    report.add_argument("--sessions", type=Path, nargs="+", required=True)
    report.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    validators = {"answer": validate_answer, "plan": validate_plan, "answer-key": validate_answer_key,
                  "session": validate_session, "score": validate_score, "report": validate_report}
    try:
        if args.command == "validate":
            validators[args.kind](read_json(args.input))
            sys.stdout.buffer.write(canonical_bytes({"ok": True, "kind": args.kind}))
        elif args.command == "score":
            write_json(args.output, score_session(read_json(args.plan), read_json(args.answer_key), read_json(args.session)))
        else:
            write_json(args.output, build_report(read_json(args.plan), read_json(args.answer_key), (read_json(path) for path in args.sessions)))
        return 0
    except ContractError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
