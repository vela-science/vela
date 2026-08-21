#!/usr/bin/env python3
"""Run one preregistered cold-successor session exactly once."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


PROMPT = (
    "You are one context-isolated cold successor. Read TASK.md first, then use "
    "only files inside the current packet directory. Do not access parent "
    "directories, environment variables, a network, or external context. "
    "Return only the JSON response required by TASK.md and "
    "response-template.json. Do not reveal reasoning or commentary."
)

NON_RESULT_RESPONSE = {
    "schema": "vela.inherited-correction-response.v1",
    "fixture_id": "bounded-calibration-correction-v1",
    "predecessor_claim_id": "participant-non-result",
    "successor_claim_id": "participant-non-result",
    "consequences": [
        {
            "claim_id": "aggregate-e",
            "classification": "unaffected",
            "action_code": "no_correction_reassessment",
        },
        {
            "claim_id": "installation-d",
            "classification": "unaffected",
            "action_code": "no_correction_reassessment",
        },
        {
            "claim_id": "stability-c",
            "classification": "unaffected",
            "action_code": "no_correction_reassessment",
        },
        {
            "claim_id": "yield-b",
            "classification": "unaffected",
            "action_code": "no_correction_reassessment",
        },
    ],
    "standing_effect": "participant-non-result",
    "source_or_evidence_binding": "participant-non-result",
}

LABELS = {
    "affected",
    "unaffected",
    "must_reassess",
    "presently_unprovable",
}
ACTION_CODES = {
    "retrieve_exact_site_q_source",
    "no_correction_reassessment",
    "rerun_stability_method",
    "recalculate_with_successor_factor",
}


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="microseconds").replace(
        "+00:00", "Z"
    )


def json_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def canonical_root(value: Any) -> str:
    data = json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return digest(data)


def load(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def tool_count(events: bytes) -> int:
    tool_ids: set[str] = set()
    for line_number, raw in enumerate(events.splitlines(), start=1):
        try:
            event = json.loads(raw)
        except json.JSONDecodeError:
            continue
        item = event.get("item")
        if not isinstance(item, dict):
            continue
        item_type = str(item.get("type", ""))
        if item_type in {
            "command_execution",
            "mcp_tool_call",
            "web_search",
            "computer_use",
            "file_change",
        } or item_type.endswith("_tool_call"):
            tool_ids.add(str(item.get("id", f"line-{line_number}")))
    return len(tool_ids)


def usage_from_events(events: bytes) -> dict[str, Any] | None:
    found = None
    for raw in events.splitlines():
        try:
            event = json.loads(raw)
        except json.JSONDecodeError:
            continue
        usage = event.get("usage")
        if isinstance(usage, dict):
            found = usage
    return found


def usage_value(usage: dict[str, Any] | None, key: str) -> int | None:
    if not isinstance(usage, dict):
        return None
    value = usage.get(key)
    if isinstance(value, bool) or not isinstance(value, int):
        return None
    return value


def closed_response_valid(candidate: Any) -> bool:
    if not isinstance(candidate, dict):
        return False
    if set(candidate) != {
        "schema",
        "fixture_id",
        "predecessor_claim_id",
        "successor_claim_id",
        "consequences",
        "standing_effect",
        "source_or_evidence_binding",
    }:
        return False
    if candidate.get("schema") != "vela.inherited-correction-response.v1":
        return False
    if candidate.get("fixture_id") != "bounded-calibration-correction-v1":
        return False
    for field in (
        "predecessor_claim_id",
        "successor_claim_id",
        "standing_effect",
        "source_or_evidence_binding",
    ):
        if not isinstance(candidate.get(field), str) or not candidate[field].strip():
            return False
    consequences = candidate.get("consequences")
    if not isinstance(consequences, list) or len(consequences) != 4:
        return False
    expected_ids = ["aggregate-e", "installation-d", "stability-c", "yield-b"]
    for item, claim_id in zip(consequences, expected_ids, strict=True):
        if not isinstance(item, dict) or set(item) != {
            "claim_id",
            "classification",
            "action_code",
        }:
            return False
        if item.get("claim_id") != claim_id:
            return False
        if item.get("classification") not in LABELS:
            return False
        if item.get("action_code") not in ACTION_CODES:
            return False
    return True


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact-root", type=Path, required=True)
    parser.add_argument("--runs-dir", type=Path, required=True)
    parser.add_argument("--authorization", type=Path, required=True)
    parser.add_argument("--participant-configuration", type=Path, required=True)
    parser.add_argument("--run-id", required=True)
    args = parser.parse_args()

    artifact_root = args.artifact_root.resolve()
    runs_dir = args.runs_dir.resolve()
    authorization_path = args.authorization.resolve()
    configuration_path = args.participant_configuration.resolve()
    authorization = load(authorization_path)
    configuration = load(configuration_path)
    configuration_root = canonical_root(configuration)
    if authorization["participant_configuration_root"] != configuration_root:
        raise SystemExit("participant_configuration_root_mismatch")
    assignment = next(
        (item for item in authorization["assignments"] if item["run_id"] == args.run_id),
        None,
    )
    if assignment is None:
        raise SystemExit("run_not_assigned")

    benchmark = artifact_root / "benchmark.py"
    started_at = utc_now()
    start_command = [
        sys.executable,
        str(benchmark),
        "start",
        "--authorization",
        str(authorization_path),
        "--runs-dir",
        str(runs_dir),
        "--run-id",
        assignment["run_id"],
        "--participant-instance-id",
        assignment["participant_instance_id"],
        "--participant-configuration-root",
        configuration_root,
        "--condition",
        assignment["condition"],
        "--started-at",
        started_at,
    ]
    subprocess.run(start_command, check=True)

    run_dir = runs_dir / assignment["run_id"]
    packet_dir = run_dir / "packet"
    raw_events_path = run_dir / "provider-events.jsonl"
    raw_stderr_path = run_dir / "provider-stderr.txt"
    candidate_path = run_dir / "participant-response.raw.json"
    response_path = run_dir / "response-to-freeze.json"
    response_schema = configuration_path.parent / "response-schema.json"
    command = [
        "codex",
        "exec",
        "--ignore-user-config",
        "--ignore-rules",
        "--ephemeral",
        "--skip-git-repo-check",
        "--model",
        configuration["model"],
        "-c",
        f'model_reasoning_effort="{configuration["reasoning_effort"]}"',
        "-c",
        f'service_tier="{configuration["service_tier"]}"',
        "-c",
        'approval_policy="never"',
        "-c",
        f'model_context_window={configuration["combined_context_token_ceiling"]}',
        "-c",
        f'model_auto_compact_token_limit={configuration["input_token_ceiling"]}',
        "-c",
        'model_auto_compact_token_limit_scope="total"',
        "-c",
        'web_search="disabled"',
        "-c",
        'agents.enabled=false',
        "-c",
        'memories.use_memories=false',
        "-c",
        'history.persistence="none"',
        "-c",
        'tools.view_image=false',
        "-c",
        'shell_environment_policy.inherit="none"',
        "-c",
        'tool_output_token_limit=4096',
        "--sandbox",
        "read-only",
        "--cd",
        str(packet_dir),
        "--output-schema",
        str(response_schema),
        "--output-last-message",
        str(candidate_path),
        "--json",
        PROMPT,
    ]

    monotonic_start = time.monotonic()
    provider_started_at = utc_now()
    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    timed_out = False
    try:
        stdout, stderr = process.communicate(timeout=configuration["timeout_seconds"])
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
    provider_completed_at = utc_now()
    elapsed = time.monotonic() - monotonic_start
    raw_events_path.write_bytes(stdout)
    raw_stderr_path.write_bytes(stderr)

    response_source = "participant"
    normalization_reason = None
    usage = usage_from_events(stdout)
    try:
        candidate = load(candidate_path)
        if not closed_response_valid(candidate):
            raise ValueError("participant response violates the closed response contract")
        if timed_out:
            raise ValueError("participant process timed out")
        if process.returncode != 0:
            raise ValueError("participant process failed")
        input_tokens = usage_value(usage, "input_tokens")
        output_tokens = usage_value(usage, "output_tokens")
        if input_tokens is not None and input_tokens > configuration["input_token_ceiling"]:
            raise ValueError("participant input token ceiling exceeded")
        if output_tokens is not None and output_tokens > configuration["output_token_ceiling"]:
            raise ValueError("participant output token ceiling exceeded")
        response = candidate
    except (FileNotFoundError, json.JSONDecodeError, ValueError) as error:
        response_source = "prespecified_non_result"
        normalization_reason = type(error).__name__
        response = NON_RESULT_RESPONSE
    response_path.write_bytes(json_bytes(response))

    completed_at = utc_now()
    observed_tools = tool_count(stdout)
    finish_command = [
        sys.executable,
        str(benchmark),
        "finish",
        "--run-dir",
        str(run_dir),
        "--response",
        str(response_path),
        "--tool-calls",
        str(observed_tools),
        "--completed-at",
        completed_at,
    ]
    subprocess.run(finish_command, check=True)

    telemetry = {
        "schema": "vela.inherited-correction-provider-telemetry.v1",
        "run_id": assignment["run_id"],
        "participant_instance_id": assignment["participant_instance_id"],
        "condition": assignment["condition"],
        "provider": configuration["provider"],
        "interface": configuration["interface"],
        "model": configuration["model"],
        "reasoning_effort": configuration["reasoning_effort"],
        "service_tier": configuration["service_tier"],
        "provider_started_at": provider_started_at,
        "provider_completed_at": provider_completed_at,
        "monotonic_elapsed_seconds": elapsed,
        "process_exit_code": process.returncode,
        "process_timed_out": timed_out,
        "tool_calls": observed_tools,
        "usage": usage,
        "cost": {
            "amount": None,
            "currency": "USD",
            "basis": "not_exposed_by_chatgpt_authenticated_codex_cli",
        },
        "response_source": response_source,
        "normalization_reason": normalization_reason,
        "provider_events_bytes": digest(stdout),
        "provider_stderr_bytes": digest(stderr),
        "participant_response_bytes": (
            digest(candidate_path.read_bytes()) if candidate_path.is_file() else None
        ),
        "frozen_response_bytes": digest(response_path.read_bytes()),
        "command_contract": configuration["command_contract"],
    }
    (run_dir / "telemetry.json").write_bytes(json_bytes(telemetry))
    print(json.dumps(telemetry, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
