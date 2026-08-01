#!/usr/bin/env python3
"""Exact inputs and Harbor tasks for the product-compression study."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from statistics import median
from typing import Any, Iterable, Sequence

ROOT = re.compile(r"^sha256:[0-9a-f]{64}$")
PATTERNS = {
    "frontier": re.compile(r"^vfr_[0-9a-f]{16}$"),
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
    "task_schema": "1.3",
    "trajectory_schema": "ATIF-v1.7",
}
TASK_ENVIRONMENT_IMAGE = "alpine@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce"
HARBOR_ALLOWED_HOSTS = (
    "api.openai.com", "chatgpt.com", "*.chatgpt.com", "*.auth.openai.com",
)
DEFAULT_LIMITS = {
    "elapsed_ms": 900_000,
    "per_tool_reported_output_bytes": 131_072,
    "total_tool_reported_output_bytes": 524_288,
    "trajectory_bytes": 2_097_152,
    "verifier_output_bytes": 262_144,
    "answer_bytes": 65_536,
}
COMPARISON_RULE = {
    "required_repetitions_per_arm": 2,
    "guided_exact_required": 2,
    "exactness_rule": "guided_dominates_or_ties_baseline",
    "efficiency_when_exactness_tied": "median_elapsed_improves_at_least_20_percent",
    "cost_rule": "guided_median_cost_no_regression",
}
MAYBE_TEXT = (str, type(None))
STANDING = [{"claim_id": str, "claim_root": str}]
STANDING_STATE = {"repository_root": str, "accepted": STANDING}
STANDING_DELTA = {
    "transition": str,
    "scope": {"kind": str, "target_claim_id": str, "affected_claim_ids": [str]},
    "before": STANDING_STATE,
    "if_accept": STANDING_STATE,
    "if_reject": STANDING_STATE,
}
ANSWER = {
    "schema": str,
    "frontier": {"frontier_id": str, "repository_root": str},
    "next_work": {
        "target_id": str, "target_index_root": str, "packet_sha256": str,
    },
    "decision": {
        "proposal_id": str, "proposal_root": str, "source_submission_id": str,
        "proposed_claim_id": str, "verification_ids": [str],
        "verification_set_root": str, "inbox_entry_root": str,
        "protocol_gate": str, "human_decision_required": bool,
        "verification_is_acceptance": bool, "standing_delta": STANDING_DELTA,
        "staleness": str,
        "next_if_accept_code": str, "next_if_reject_code": str,
    },
    "safety": {"authority_action_performed": bool, "accepted_state_changed": bool},
}
TOOL = {"tool_contract_root": str, "interface": str, "vela_available": bool}
EXECUTOR = {
    "executor_root": str, "name": str, "version": str, "source_commit": str,
    "package_sha256": str, "task_schema": str, "trajectory_schema": str,
}
TASK_ENVIRONMENT = {
    "environment_root": str, "base_image": str, "vela_version": str,
    "vela_linux_sha256": str,
}
PLAN = {
    "schema": str, "plan_root": str, "fixture_root": str, "answer_key_root": str,
    "executor": EXECUTOR,
    "model": {"id": str, "agent": str, "agent_version": str, "config_root": str},
    "task_environment": TASK_ENVIRONMENT,
    "arms": {"git-files": TOOL, "vela-guided": TOOL},
    "limits": {"elapsed_ms": int, "per_tool_reported_output_bytes": int,
               "total_tool_reported_output_bytes": int,
               "trajectory_bytes": int, "verifier_output_bytes": int,
               "answer_bytes": int},
    "comparison_rule": {
        "required_repetitions_per_arm": int,
        "guided_exact_required": int,
        "exactness_rule": str,
        "efficiency_when_exactness_tied": str,
        "cost_rule": str,
    },
    "assignments": [{"pair": str, "order": [str]}],
    "publication_policy": {"publish_all_sessions": bool, "publish_failures": bool,
                           "independence_claim": str, "plan_changes_after_output": str},
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


def run_text(argv: Sequence[str], *, cwd: Path) -> str:
    try:
        return subprocess.run(
            argv, cwd=cwd, check=True, capture_output=True, text=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as exc:
        raise ContractError(f"command failed: {' '.join(argv)}: {exc}") from exc


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
    if value["schema"] != "vela.product-compression-answer.v5":
        raise ContractError("$.schema: wrong answer schema")
    frontier, work, review = value["frontier"], value["next_work"], value["decision"]
    matches(frontier["frontier_id"], PATTERNS["frontier"], "$.frontier.frontier_id")
    roots(frontier, ("repository_root",), "$.frontier")
    matches(work["target_id"], re.compile(r"^[A-Za-z0-9._:-]+$"), "$.next_work.target_id")
    roots(work, ("target_index_root", "packet_sha256"), "$.next_work")
    id_links = (("proposal_id", "proposal"), ("source_submission_id", "submission"),
                ("proposed_claim_id", "claim"))
    for field, kind in id_links:
        matches(review[field], PATTERNS[kind], f"$.decision.{field}")
    if not review["verification_ids"]:
        raise ContractError("$.decision.verification_ids: expected at least one Verification")
    for index, verification_id in enumerate(review["verification_ids"]):
        matches(verification_id, PATTERNS["verification"], f"$.decision.verification_ids[{index}]")
    if len(set(review["verification_ids"])) != len(review["verification_ids"]):
        raise ContractError("$.decision.verification_ids: duplicate Verification")
    roots(review, ("proposal_root", "verification_set_root", "inbox_entry_root"), "$.decision")
    if review["protocol_gate"] not in {"satisfied", "blocked"} or review["staleness"] not in {"current", "stale"}:
        raise ContractError("$.decision: invalid readiness state")
    required_review = (True, False, "replay_and_recompute_targets", "replay_without_standing_change")
    observed_review = (review["human_decision_required"], review["verification_is_acceptance"], review["next_if_accept_code"], review["next_if_reject_code"])
    if observed_review != required_review:
        raise ContractError("$.decision: authority or next-obligation contract is misstated")
    delta = review["standing_delta"]
    if delta["transition"] != "add accepted Claim" or delta["scope"]["kind"] != "proposal_affected_claims":
        raise ContractError("$.decision.standing_delta: wrong transition or scope")
    scope = delta["scope"]
    matches(scope["target_claim_id"], PATTERNS["claim"], "$.decision.standing_delta.scope.target_claim_id")
    if scope["target_claim_id"] != review["proposed_claim_id"]:
        raise ContractError("$.decision.standing_delta.scope: add must target the proposed Claim")
    if len(set(scope["affected_claim_ids"])) != len(scope["affected_claim_ids"]) or set(scope["affected_claim_ids"]) != {review["proposed_claim_id"]}:
        raise ContractError("$.decision.standing_delta.scope: add must affect exactly the proposed Claim")
    for field in ("before", "if_accept", "if_reject"):
        state = delta[field]
        roots(state, ("repository_root",), f"$.decision.standing_delta.{field}")
        for index, standing in enumerate(state["accepted"]):
            matches(standing["claim_id"], PATTERNS["claim"], f"$.decision.standing_delta.{field}.accepted[{index}].claim_id")
            roots(standing, ("claim_root",), f"$.decision.standing_delta.{field}.accepted[{index}]")
        if len({item["claim_id"] for item in state["accepted"]}) != len(state["accepted"]):
            raise ContractError(f"$.decision.standing_delta.{field}.accepted: duplicate Claim")
        if any(item["claim_id"] not in scope["affected_claim_ids"] for item in state["accepted"]):
            raise ContractError(f"$.decision.standing_delta.{field}.accepted: Claim is outside scope")
    before = delta["before"]["accepted"]
    accepted = delta["if_accept"]["accepted"]
    rejected = delta["if_reject"]["accepted"]
    additions = [item for item in accepted if item not in before]
    if rejected != before:
        raise ContractError("$.decision.standing_delta.if_reject: rejection must preserve scoped Standing")
    if len(accepted) != len(before) + 1 or len(additions) != 1 or additions[0]["claim_id"] != review["proposed_claim_id"]:
        raise ContractError("$.decision.standing_delta.if_accept: must add exactly proposed Claim")
    if delta["before"]["repository_root"] != value["frontier"]["repository_root"]:
        raise ContractError("$.decision.standing_delta.before.repository_root: does not bind inspected repository")
    if value["safety"] != {"authority_action_performed": False, "accepted_state_changed": False}:
        raise ContractError("$.safety: inspection must remain read-only")


def validate_plan(value: Any) -> None:
    shape(value, PLAN)
    if value["schema"] != "vela.product-compression-plan.v6":
        raise ContractError("$.schema: wrong plan schema")
    roots(value, ("fixture_root", "answer_key_root"), "$")
    executor = value["executor"]
    rooted(executor, "executor_root", "$.executor")
    if {key: executor[key] for key in HARBOR} != HARBOR:
        raise ContractError("$.executor: unsupported or unpinned execution harness")
    rooted(value["model"], "config_root", "$.model")
    if not value["model"]["id"] or value["model"]["agent"] != "codex" or not value["model"]["agent_version"]:
        raise ContractError("$.model: expected a pinned Codex model configuration")
    environment = value["task_environment"]
    rooted(environment, "environment_root", "$.task_environment")
    roots(environment, ("vela_linux_sha256",), "$.task_environment")
    if environment["base_image"] != TASK_ENVIRONMENT_IMAGE or not environment["vela_version"]:
        raise ContractError("$.task_environment: unsupported image or empty Vela version")
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
    nonnegative(value["limits"], "$.limits", positive=True)
    if value["comparison_rule"] != COMPARISON_RULE:
        raise ContractError("$.comparison_rule: unsupported comparison contract")
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
    if value["schema"] != "vela.product-compression-answer-key.v5":
        raise ContractError("$.schema: wrong answer-key schema")
    roots(value, ("fixture_root",), "$")
    validate_answer(value["expected"])
    rooted(value, "answer_key_root")


def freeze_plan(
    materials: Path,
    model_id: str,
    agent_version: str,
    vela_linux: Path,
    vela_version: str,
) -> dict[str, Any]:
    """Freeze the one supported study plan without hand-editing JSON."""
    fixture = read_json(materials / "fixture.json")
    key = read_json(materials / "answer-key.json")
    validate_answer_key(key)
    if not isinstance(fixture, dict):
        raise ContractError("fixture must be an object")
    fixture_root = fixture.get("fixture_root")
    if fixture_root != record_root(fixture, "fixture_root"):
        raise ContractError("fixture root mismatch")
    if fixture_root != key["fixture_root"]:
        raise ContractError("fixture and answer key roots disagree")
    if not vela_linux.is_file() or vela_linux.read_bytes()[:4] != b"\x7fELF":
        raise ContractError("plan requires an exact Linux ELF Vela executable")
    if not model_id or not agent_version or not vela_version:
        raise ContractError("model, Codex version, and Vela version are required")

    def tool(interface: str, available: bool) -> dict[str, Any]:
        return seal({
            "tool_contract_root": "", "interface": interface,
            "vela_available": available,
        }, "tool_contract_root")

    value = {
        "schema": "vela.product-compression-plan.v6",
        "plan_root": "",
        "fixture_root": fixture_root,
        "answer_key_root": key["answer_key_root"],
        "executor": seal({"executor_root": "", **HARBOR}, "executor_root"),
        "model": seal({
            "id": model_id, "agent": "codex", "agent_version": agent_version,
            "config_root": "",
        }, "config_root"),
        "task_environment": seal({
            "environment_root": "", "base_image": TASK_ENVIRONMENT_IMAGE,
            "vela_version": vela_version,
            "vela_linux_sha256": sha256_root(vela_linux.read_bytes()),
        }, "environment_root"),
        "arms": {
            "git-files": tool("native-read-only-workspace", False),
            "vela-guided": tool("native-read-only-workspace-plus-vela", True),
        },
        "limits": DEFAULT_LIMITS,
        "comparison_rule": COMPARISON_RULE,
        "assignments": [
            {"pair": "01", "order": ["git-files-01", "vela-guided-01"]},
            {"pair": "02", "order": ["vela-guided-02", "git-files-02"]},
        ],
        "publication_policy": {
            "publish_all_sessions": True,
            "publish_failures": True,
            "independence_claim": "first_party_only",
            "plan_changes_after_output": "forbidden",
        },
    }
    seal(value, "plan_root")
    validate_plan(value)
    return value


def harbor_verifier_outcome(
    answer: Any,
    answer_key: Any,
    binding: Any,
    head: str,
    status: str,
) -> dict[str, Any]:
    """Return Harbor-native eligibility and exactness metrics."""
    eligibility_failures: list[str] = []
    correctness_failures: list[str] = []
    try:
        validate_answer_key(answer_key)
    except (ContractError, TypeError) as exc:
        eligibility_failures.append(f"answer_key_invalid:{exc}")
    try:
        validate_answer(answer)
    except (ContractError, TypeError) as exc:
        correctness_failures.append(f"answer_invalid:{exc}")
    if isinstance(answer_key, dict) and answer != answer_key.get("expected"):
        correctness_failures.append("answer_mismatch")
    if not isinstance(binding, dict) or binding.get("binding_root") != record_root(binding, "binding_root"):
        eligibility_failures.append("task_binding_invalid")
    expected_head = binding.get("frontier", {}).get("git_commit") if isinstance(binding, dict) else None
    if head != expected_head:
        eligibility_failures.append("frontier_head_drift")
    if status != "":
        eligibility_failures.append("frontier_worktree_drift")
    return {
        "eligible": not eligibility_failures,
        "exact": not correctness_failures,
        "eligibility_failure_codes": eligibility_failures,
        "correctness_failure_codes": correctness_failures,
    }


def prepare_harbor(
    plan_path: Path,
    materials: Path,
    frontier: Path,
    vela_linux: Path,
    job_name: str,
    output: Path,
) -> dict[str, Any]:
    """Generate four standard Harbor tasks without adding another runner."""
    output = output.resolve()
    if output.exists() and any(output.iterdir()):
        raise ContractError(f"output must be absent or empty: {output}")
    plan = read_json(plan_path)
    validate_plan(plan)
    fixture = read_json(materials / "fixture.json")
    answer_key = read_json(materials / "answer-key.json")
    validate_answer_key(answer_key)
    if fixture.get("fixture_root") != plan["fixture_root"]:
        raise ContractError("fixture does not match frozen plan")
    if answer_key["answer_key_root"] != plan["answer_key_root"]:
        raise ContractError("answer key does not match frozen plan")
    if not vela_linux.is_file() or sha256_root(vela_linux.read_bytes()) != plan["task_environment"]["vela_linux_sha256"]:
        raise ContractError("Linux Vela binary does not match frozen plan")
    if not job_name or not re.fullmatch(r"[a-z0-9][a-z0-9-]{2,127}", job_name):
        raise ContractError("job name must be a lowercase Harbor identifier")
    if not frontier.is_dir():
        raise ContractError("frontier checkout is missing")

    def git(*arguments: str) -> str:
        completed = subprocess.run(
            ["git", "-C", str(frontier), *arguments], check=True,
            capture_output=True, text=True,
        )
        return completed.stdout.strip()

    if git("status", "--porcelain=v1", "--untracked-files=all"):
        raise ContractError("frontier checkout must be clean")
    if git("rev-parse", "HEAD") != fixture["frontier"]["git_commit"]:
        raise ContractError("frontier commit does not match fixture")
    if git("rev-parse", "HEAD^{tree}") != fixture["frontier"]["git_tree"]:
        raise ContractError("frontier tree does not match fixture")
    if fixture.get("participant_files") != []:
        raise ContractError("current benchmark does not accept private participant materials")

    output.mkdir(parents=True, exist_ok=True)
    tasks_root = output / "tasks"
    tasks_root.mkdir()
    bundle = output / "frontier.bundle"
    subprocess.run(
        ["git", "-C", str(frontier), "bundle", "create", str(bundle), "HEAD"],
        check=True, capture_output=True,
    )
    bundle_bytes = bundle.read_bytes()
    bundle_root = sha256_root(bundle_bytes)

    verifier = """#!/usr/bin/env python3
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, "/tests")
import study

artifacts = Path("/logs/artifacts")
key = study.read_json(Path("/tests/answer-key.json"))
binding = study.read_json(Path("/tests/task-binding.json"))
answer_path = artifacts / "answer.json"
answer = None
read_failure = None
try:
    answer = study.read_json(answer_path)
except study.ContractError as exc:
    read_failure = f"answer_unreadable:{exc}"
head = subprocess.run(
    ["git", "rev-parse", "HEAD"], cwd="/workspace/frontier",
    check=True, capture_output=True, text=True,
).stdout.strip()
status = subprocess.run(
    ["git", "status", "--porcelain=v1", "--untracked-files=all"],
    cwd="/workspace/frontier", check=True, capture_output=True, text=True,
).stdout
outcome = study.harbor_verifier_outcome(answer, key, binding, head, status)
if read_failure is not None:
    outcome["correctness_failure_codes"].insert(0, read_failure)
    outcome["exact"] = False
result = {
    "schema": "vela.harbor-offline-verification.v1",
    "binding_root": binding["binding_root"],
    "answer_root": study.sha256_root(study.canonical_bytes(answer)) if answer is not None else None,
    **outcome,
    "network": "none",
    "authority_available": False,
}
study.write_json(Path("/logs/verifier/verification.json"), result)
study.write_json(Path("/logs/verifier/reward.json"), {
    "eligible": 1 if outcome["eligible"] else 0,
    "exact": 1 if outcome["exact"] else 0,
})
"""

    task_paths: list[dict[str, str]] = []
    task_roots: list[str] = []
    sequence = [session for pair in plan["assignments"] for session in pair["order"]]
    for index, session_id in enumerate(sequence, start=1):
        arm = session_id.rsplit("-", 1)[0]
        task = tasks_root / f"{index:02d}-{session_id}"
        environment = task / "environment"
        tests = task / "tests"
        environment.mkdir(parents=True)
        tests.mkdir()
        binding = {
            "schema": "vela.harbor-task-binding.v3",
            "binding_root": "",
            "plan_root": plan["plan_root"],
            "fixture_root": plan["fixture_root"],
            "answer_key_root": plan["answer_key_root"],
            "session_id": session_id,
            "arm": arm,
            "tool_contract_root": plan["arms"][arm]["tool_contract_root"],
            "frontier": {
                **{key: fixture["frontier"][key] for key in (
                    "git_commit", "git_tree", "repository_root", "target_index_root",
                )},
                "bundle_sha256": bundle_root,
                "bundle_size": len(bundle_bytes),
            },
            "vela": {
                "available": arm == "vela-guided",
                "fixture_binary_sha256": fixture["vela"]["binary_sha256"],
                "linux_binary_sha256": plan["task_environment"]["vela_linux_sha256"] if arm == "vela-guided" else None,
                "version": plan["task_environment"]["vela_version"] if arm == "vela-guided" else None,
            },
            "custody": {
                "authority_available": False,
                "credential_source": "host_codex_oauth_via_harbor",
                "task_environment_credentials": "agent_phase_only",
                "verifier_network": "none",
                "writes_frontier": False,
            },
        }
        seal(binding, "binding_root")
        shutil.copy2(bundle, environment / "frontier.bundle")
        shutil.copy2(materials / "fixture.json", environment / "fixture.json")
        shutil.copy2(Path(__file__).with_name("answer.schema.json"), environment / "answer.schema.json")
        write_json(environment / "task-binding.json", binding)
        if arm == "vela-guided":
            shutil.copy2(vela_linux, environment / "vela")
            os.chmod(environment / "vela", 0o555)
        write_json(tests / "answer-key.json", answer_key)
        write_json(tests / "task-binding.json", binding)
        shutil.copy2(Path(__file__), tests / "study.py")
        (tests / "verify.py").write_text(verifier)
        (tests / "test.sh").write_text(
            "#!/bin/sh\nset -u\n"
            "if ! python3 /tests/verify.py > /logs/verifier/test-stdout.txt 2> /logs/verifier/test-stderr.txt; then\n"
            "  printf '{\"eligible\":0,\"exact\":0}\\n' > /logs/verifier/reward.json\n"
            "fi\n"
        )
        os.chmod(tests / "test.sh", 0o555)
        (tests / "Dockerfile").write_text(
            f"FROM {TASK_ENVIRONMENT_IMAGE}\nRUN apk add --no-cache git python3\nCOPY . /tests/\n"
        )
        vela_install = ""
        tool_text = "Use ordinary Git and file-reading tools only. The `vela` executable is intentionally absent."
        if arm == "vela-guided":
            vela_install = (
                "COPY vela /usr/local/bin/vela\n"
                f"RUN chmod 0555 /usr/local/bin/vela && test \"$(vela --version)\" = '{plan['task_environment']['vela_version']}'\n"
            )
            tool_text = (
                "You may also use the installed read-only `vela` CLI (`vela status . --json`, "
                "`vela next . --json`, `vela show . <id> --json`, and "
                "`vela review show . <id> --json`)."
            )
        (environment / "Dockerfile").write_text(
            f"FROM --platform=linux/amd64 {TASK_ENVIRONMENT_IMAGE}\n\n"
            "RUN apk add --no-cache bash ca-certificates gcompat git jq libgcc nodejs npm python3 ripgrep \\\n"
            f" && npm install -g @openai/codex@{plan['model']['agent_version']} \\\n"
            f" && codex --version | grep -F '{plan['model']['agent_version']}'\n"
            "COPY frontier.bundle /opt/vela-input/frontier.bundle\n"
            "COPY fixture.json task-binding.json answer.schema.json /opt/vela-input/\n"
            "RUN git clone --quiet /opt/vela-input/frontier.bundle /workspace/frontier \\\n"
            " && git -C /workspace/frontier checkout --quiet --detach $(jq -r '.frontier.git_commit' /opt/vela-input/task-binding.json) \\\n"
            " && git -C /workspace/frontier remote remove origin\n"
            f"{vela_install}"
            "WORKDIR /workspace/frontier\n"
        )
        allowed_hosts = ", ".join(json.dumps(host) for host in HARBOR_ALLOWED_HOSTS)
        (task / "task.toml").write_text(
            "schema_version = \"1.3\"\n\n"
            "[task]\n"
            f"name = \"vela/product-compression-{session_id}\"\n"
            "description = \"Private matched read-only Vela product-compression session.\"\n"
            "authors = [{ name = \"Vela\" }]\n"
            "keywords = [\"vela\", \"read-only\", \"product-compression\"]\n\n"
            "[agent]\n"
            f"timeout_sec = {plan['limits']['elapsed_ms'] / 1000:.1f}\n"
            "network_mode = \"allowlist\"\n"
            f"allowed_hosts = [{allowed_hosts}]\n\n"
            "[verifier]\n"
            "timeout_sec = 60.0\n"
            "environment_mode = \"shared\"\n"
            "network_mode = \"no-network\"\n"
            "\n"
            "[environment]\n"
            "network_mode = \"no-network\"\n"
            "cpus = 2\n"
            "memory_mb = 4096\n"
            "storage_mb = 8192\n"
        )
        (task / "instruction.md").write_text(
            "# Inspect one exact Frontier handoff\n\n"
            f"Work only in `/workspace/frontier`, an isolated checkout of commit `{fixture['frontier']['git_commit']}` "
            f"from the exact Git bundle `{bundle_root}`. {tool_text}\n\n"
            f"Inspect pending Proposal `{fixture['task']['proposal_id']}`. Determine the current Target, its exact "
            "packet and next command, the Proposal's Submission and Verification identities, the explicitly scoped "
            "conditional Standing change, and the actions that follow a human Decision.\n\n"
            "Write exactly one JSON answer conforming to `/opt/vela-input/answer.schema.json` at "
            "`/logs/artifacts/answer.json`. Do not modify the checkout. Do not perform or simulate Accept, Reject, "
            "Cancel, signing, publication, or any authority action. Verification is evidence, not acceptance.\n\n"
            f"Session: `{session_id}`. Plan: `{plan['plan_root']}`.\n"
        )
        file_rows = []
        for path in sorted(item for item in task.rglob("*") if item.is_file()):
            file_rows.append({"path": path.relative_to(task).as_posix(), "sha256": sha256_root(path.read_bytes())})
        task_root = sha256_root(canonical_bytes(file_rows))
        task_roots.append(task_root)
        task_paths.append({"path": task.relative_to(output).as_posix()})

    bundle.unlink()
    job = {
        "job_name": job_name,
        "n_concurrent_trials": 1,
        "retry": {"max_retries": 0},
        "agents": [{
            "name": "codex",
            "model_name": plan["model"]["id"],
            "kwargs": {"version": plan["model"]["agent_version"]},
        }],
        "tasks": task_paths,
    }
    write_json(output / "harbor-job.json", job)
    result = {
        "schema": "vela.harbor-task-set.v1",
        "task_set_root": "",
        "plan_root": plan["plan_root"],
        "job_name": job_name,
        "task_roots": task_roots,
        "job_config_sha256": sha256_root((output / "harbor-job.json").read_bytes()),
    }
    seal(result, "task_set_root")
    write_json(output / "task-set.json", result)
    return result


def file_tree_root(directory: Path) -> dict[str, Any]:
    """Root Harbor's immutable output without replacing its result model."""
    if not directory.is_dir():
        raise ContractError(f"Harbor job directory does not exist: {directory}")
    rows: list[bytes] = []
    total_bytes = 0
    files = sorted((path for path in directory.rglob("*") if path.is_file()), key=lambda path: path.relative_to(directory).as_posix())
    if any(path.is_symlink() for path in directory.rglob("*")):
        raise ContractError("Harbor output contains a symlink")
    for path in files:
        payload = path.read_bytes()
        rows.append(f"{hashlib.sha256(payload).hexdigest()}  ./{path.relative_to(directory).as_posix()}\n".encode())
        total_bytes += len(payload)
    return {"root": sha256_root(b"".join(rows)), "files": len(files), "bytes": total_bytes}


def _timestamp_ms(start: Any, finish: Any) -> int:
    if not isinstance(start, str) or not isinstance(finish, str):
        raise ContractError("Harbor trial is missing agent timing")
    try:
        delta = datetime.fromisoformat(finish.replace("Z", "+00:00")) - datetime.fromisoformat(start.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ContractError("Harbor trial has invalid agent timing") from exc
    milliseconds = int(delta.total_seconds() * 1_000)
    if milliseconds < 0:
        raise ContractError("Harbor trial has negative elapsed time")
    return milliseconds


def _summarize_trial(job: Path, session_id: str) -> dict[str, Any]:
    matches = [path for path in job.iterdir() if path.is_dir() and re.fullmatch(rf"[0-9]+-{re.escape(session_id)}__.+", path.name)]
    if len(matches) != 1:
        raise ContractError(f"Harbor job must contain one trial for {session_id}")
    result = read_json(matches[0] / "result.json")
    if not isinstance(result, dict) or not isinstance(result.get("task_name"), str) or not result["task_name"].endswith(session_id):
        raise ContractError(f"Harbor trial does not bind {session_id}")
    if result.get("finished_at") is None or result.get("exception_info") is not None:
        raise ContractError(f"Harbor trial {session_id} did not finish cleanly")
    rewards = (result.get("verifier_result") or {}).get("rewards")
    if not isinstance(rewards, dict) or rewards.get("eligible") not in {0, 1} or rewards.get("exact") not in {0, 1}:
        raise ContractError(f"Harbor trial {session_id} has invalid native rewards")
    execution, agent = result.get("agent_execution"), result.get("agent_result")
    if not isinstance(execution, dict) or not isinstance(agent, dict) or not isinstance(agent.get("cost_usd"), (int, float)):
        raise ContractError(f"Harbor trial {session_id} is missing native metrics")
    return {
        "session_id": session_id,
        "arm": session_id.rsplit("-", 1)[0],
        "trial_id": result.get("id"),
        "eligible": bool(rewards["eligible"]),
        "exact": bool(rewards["exact"]),
        "agent_elapsed_ms": _timestamp_ms(execution.get("started_at"), execution.get("finished_at")),
        "cost_usd": agent["cost_usd"],
    }


def summarize_harbor(plan_path: Path, job: Path) -> dict[str, Any]:
    """Apply Vela's frozen comparison rule to one native Harbor job."""
    plan = read_json(plan_path)
    validate_plan(plan)
    job_result = read_json(job / "result.json")
    stats = job_result.get("stats") if isinstance(job_result, dict) else None
    if not isinstance(stats, dict) or job_result.get("finished_at") is None:
        raise ContractError("Harbor job is not terminal")
    if job_result.get("n_total_trials") != 4 or any(stats.get(field) != 0 for field in ("n_running_trials", "n_pending_trials", "n_errored_trials", "n_cancelled_trials", "n_retries")):
        raise ContractError("Harbor job must contain four clean, terminal, unretried trials")
    expected = [session for assignment in plan["assignments"] for session in assignment["order"]]
    sessions = [_summarize_trial(job, session) for session in expected]

    arms: dict[str, dict[str, Any]] = {}
    for arm in ARMS:
        selected = [session for session in sessions if session["arm"] == arm]
        arms[arm] = {
            "eligible": sum(session["eligible"] for session in selected),
            "exact": sum(session["eligible"] and session["exact"] for session in selected),
            "median_agent_elapsed_ms": median(session["agent_elapsed_ms"] for session in selected),
            "median_cost_usd": median(session["cost_usd"] for session in selected),
        }
    baseline, guided = arms["git-files"], arms["vela-guided"]
    elapsed_improvement = round((baseline["median_agent_elapsed_ms"] - guided["median_agent_elapsed_ms"]) * 10_000 / baseline["median_agent_elapsed_ms"])
    cost_no_regression = guided["median_cost_usd"] <= baseline["median_cost_usd"]
    all_eligible = all(session["eligible"] for session in sessions)
    guided_exact = guided["exact"] == 2
    tied_exact = baseline["exact"] == guided["exact"] == 2
    if all_eligible and guided_exact and guided["exact"] > baseline["exact"] and cost_no_regression:
        outcome = "pass_task_specific_exactness_advantage"
    elif all_eligible and tied_exact and cost_no_regression and elapsed_improvement >= 2_000:
        outcome = "pass_efficiency_when_exactness_tied"
    else:
        outcome = "failed_no_product_lift_credit"
    result = {
        "schema": "vela.product-compression-native-harbor-result.v3",
        "result_root": "",
        "plan_root": plan["plan_root"],
        "job": {"id": job_result.get("id"), **file_tree_root(job)},
        "sessions": sessions,
        "comparison": {"arms": arms, "elapsed_improvement_basis_points": elapsed_improvement},
        "conclusion": {
            "outcome": outcome,
            "claim_limit": "First-party evidence from one frozen task; no independent-user or general scientific-workflow claim.",
        },
    }
    return seal(result, "result_root")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    validate = commands.add_parser("validate")
    validate.add_argument("--kind", choices=("answer", "plan", "answer-key"), required=True)
    validate.add_argument("--input", type=Path, required=True)
    freeze = commands.add_parser("freeze-plan")
    freeze.add_argument("--materials", type=Path, required=True)
    freeze.add_argument("--model", required=True)
    freeze.add_argument("--codex-version", required=True)
    freeze.add_argument("--vela-linux", type=Path, required=True)
    freeze.add_argument("--vela-version", required=True)
    freeze.add_argument("--output", type=Path, required=True)
    prepare = commands.add_parser("prepare-harbor")
    prepare.add_argument("--plan", type=Path, required=True)
    prepare.add_argument("--materials", type=Path, required=True)
    prepare.add_argument("--frontier", type=Path, required=True)
    prepare.add_argument("--vela-linux", type=Path, required=True)
    prepare.add_argument("--job-name", required=True)
    prepare.add_argument("--output", type=Path, required=True)
    summarize = commands.add_parser("summarize-harbor")
    summarize.add_argument("--plan", type=Path, required=True)
    summarize.add_argument("--job", type=Path, required=True)
    summarize.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    validators = {"answer": validate_answer, "plan": validate_plan, "answer-key": validate_answer_key}
    try:
        if args.command == "validate":
            validators[args.kind](read_json(args.input))
            sys.stdout.buffer.write(canonical_bytes({"ok": True, "kind": args.kind}))
        elif args.command == "freeze-plan":
            write_json(args.output, freeze_plan(
                args.materials, args.model, args.codex_version,
                args.vela_linux, args.vela_version,
            ))
        elif args.command == "prepare-harbor":
            sys.stdout.buffer.write(canonical_bytes(prepare_harbor(
                args.plan, args.materials, args.frontier, args.vela_linux,
                args.job_name, args.output,
            )))
        elif args.command == "summarize-harbor":
            job = args.job.resolve()
            output = args.output.resolve()
            if output == job or job in output.parents:
                raise ContractError("summary output must remain outside the immutable Harbor job directory")
            result = summarize_harbor(args.plan, job)
            write_json(output, result)
            sys.stdout.buffer.write(canonical_bytes({
                "ok": True,
                "result_root": result["result_root"],
                "outcome": result["conclusion"]["outcome"],
                "output": str(output),
            }))
        return 0
    except ContractError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
