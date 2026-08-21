#!/usr/bin/env python3
"""Fail-closed runtime-to-benchmark custody for the confirmatory study."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import shutil
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
STUDY = ROOT / "confirmatory-study"
BENCHMARK = ROOT.parent / "inherited-correction-benchmark"
RUNTIME = ROOT / "container-runtime"
CONDITIONS = ("git-documents", "vela")
SHA256 = re.compile(r"^sha256:[0-9a-f]{64}$")
FORBIDDEN_EVENT = re.compile(
    r"tool|command|patch|file_change|web_search|computer|compact|resume|continu",
    re.IGNORECASE,
)


class CustodyError(ValueError):
    """Stable confirmatory custody error."""


def encoded(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def digest(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def canonical_root(value: Any) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


def load(path: Path) -> Any:
    if not path.is_file() or path.is_symlink():
        raise CustodyError(f"custody_file_missing_or_unsafe:{path.name}")
    try:
        return json.loads(path.read_text())
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CustodyError(f"custody_json_invalid:{path.name}") from error


def parse_time(value: Any) -> datetime:
    if not isinstance(value, str):
        raise CustodyError("custody_timestamp_invalid")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise CustodyError("custody_timestamp_invalid") from error
    if parsed.tzinfo is None:
        raise CustodyError("custody_timestamp_timezone_missing")
    return parsed


def tree_manifest(
    directory: Path, excluded: set[str] | None = None
) -> list[dict[str, Any]]:
    excluded = excluded or set()
    files = []
    for path in sorted(directory.rglob("*")):
        if path.is_symlink():
            raise CustodyError("custody_symlink_forbidden")
        if not path.is_file():
            continue
        relative = path.relative_to(directory).as_posix()
        if relative in excluded:
            continue
        content = path.read_bytes()
        files.append(
            {"path": relative, "bytes": len(content), "sha256": digest(content)}
        )
    return files


def packet_root(directory: Path) -> str:
    return canonical_root(tree_manifest(directory))


def exact_keys(value: Any, keys: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != keys:
        raise CustodyError(f"{label}_fields_invalid")


def static_state() -> dict[str, Any]:
    freeze = load(STUDY / "prelaunch-freeze.json")
    for item in freeze.get("files", []):
        path = STUDY / item.get("path", "")
        if not path.is_file() or path.is_symlink():
            raise CustodyError("prelaunch_manifest_file_missing")
        content = path.read_bytes()
        if item.get("bytes") != len(content) or item.get("sha256") != digest(content):
            raise CustodyError("prelaunch_manifest_drift")
    if freeze.get("status") != "frozen_prelaunch_0_of_16_independent_review_required":
        raise CustodyError("prelaunch_status_invalid")
    if (
        freeze.get("confirmatory_provider_calls") != 0
        or freeze.get("permits_consumed") != []
    ):
        raise CustodyError("prelaunch_execution_state_invalid")
    if freeze.get("hold_status") != "hold" or freeze.get("scheduler") != "none":
        raise CustodyError("prelaunch_hold_invalid")
    registration = load(STUDY / "registration.json")
    if freeze.get("registration_root") != canonical_root(registration):
        raise CustodyError("prelaunch_registration_root_mismatch")
    scoring_bindings = registration.get("scoring_bindings")
    if not isinstance(scoring_bindings, dict):
        raise CustodyError("prelaunch_scoring_bindings_missing")
    expected_scoring_bytes = {
        "benchmark_implementation_bytes": digest(
            (BENCHMARK / "benchmark.py").read_bytes()
        ),
        "benchmark_tests_bytes": digest((BENCHMARK / "test_benchmark.py").read_bytes()),
        "runtime_custody_bridge_bytes": digest(Path(__file__).read_bytes()),
        "runtime_custody_tests_bytes": digest(
            (ROOT / "test_confirmatory_custody.py").read_bytes()
        ),
    }
    if any(
        scoring_bindings.get(key) != value
        for key, value in expected_scoring_bytes.items()
    ):
        raise CustodyError("prelaunch_scoring_implementation_drift")
    if freeze.get("scoring_bindings_root") != canonical_root(scoring_bindings):
        raise CustodyError("prelaunch_scoring_bindings_root_mismatch")
    runtime_manifest = [
        item
        for item in tree_manifest(RUNTIME)
        if "/node_modules/" not in f"/{item['path']}/"
    ]
    runtime_source_root = canonical_root(runtime_manifest)
    if (
        freeze.get("runtime_source_root") != runtime_source_root
        or registration.get("runtime_source_root") != runtime_source_root
    ):
        raise CustodyError("prelaunch_runtime_source_drift")
    assignment = load(STUDY / "assignment-schedule.json")
    if freeze.get("assignment_root") != canonical_root(assignment):
        raise CustodyError("prelaunch_assignment_root_mismatch")
    authorization = load(STUDY / "authorization.json")
    if freeze.get("authorization_root") != canonical_root(authorization):
        raise CustodyError("prelaunch_authorization_root_mismatch")
    study_configuration = load(STUDY / "participant-configuration.json")
    study_root = canonical_root(study_configuration)
    if freeze.get("participant_configuration_root") != study_root:
        raise CustodyError("prelaunch_study_configuration_root_mismatch")
    if authorization.get("participant_configuration_root") != study_root:
        raise CustodyError("authorization_study_configuration_mismatch")
    if authorization.get("registration_root") != freeze.get(
        "benchmark_registration_root"
    ):
        raise CustodyError("authorization_benchmark_registration_mismatch")
    mapping = load(STUDY / "configuration-mapping.json")
    expected_mapping = {
        "schema": "vela.inherited-correction-authorized-configuration-mapping.v1",
        "status": "authorized",
        "authorization_root": freeze.get("authorization_root"),
        "shared_study_configuration_root": study_root,
        "condition_runtime_configuration_roots": freeze.get(
            "condition_runtime_configuration_roots"
        ),
    }
    if mapping != expected_mapping:
        raise CustodyError("authorized_configuration_mapping_invalid")
    mapping_root = canonical_root(mapping)
    if freeze.get("authorized_configuration_mapping_root") != mapping_root:
        raise CustodyError("authorized_configuration_mapping_root_mismatch")
    rows = assignment.get("assignments")
    if not isinstance(rows, list) or len(rows) != 16:
        raise CustodyError("assignment_denominator_invalid")
    if len({row.get("run_id") for row in rows if isinstance(row, dict)}) != 16:
        raise CustodyError("assignment_run_duplicate")
    if (
        len(
            {
                row.get("participant_instance_id")
                for row in rows
                if isinstance(row, dict)
            }
        )
        != 16
    ):
        raise CustodyError("assignment_participant_duplicate")
    if any(
        sum(row.get("condition") == condition for row in rows) != 8
        for condition in CONDITIONS
    ):
        raise CustodyError("assignment_balance_invalid")
    condition_configurations = {}
    for condition in CONDITIONS:
        input_dir = STUDY / "conditions" / condition / "input"
        config = load(input_dir / "participant-configuration.json")
        root = canonical_root(config)
        if mapping["condition_runtime_configuration_roots"].get(condition) != root:
            raise CustodyError("condition_configuration_mapping_drift")
        expected_values = {
            "registration_root": freeze["registration_root"],
            "image_digest": freeze["image_digest"],
            "trust_bundle_bytes": freeze["trust_bundle_bytes"],
            "prompt_root": freeze["prompt_roots"][condition],
            "timeout_seconds": 600,
            "output_token_ceiling": 8192,
            "attempt": 1,
            "retries": 0,
            "model": "gpt-5.6-sol",
            "reasoning_effort": "high",
            "service_tier": "default",
            "tools": "none",
            "one_prompt": True,
            "one_model_turn": True,
        }
        if any(config.get(key) != value for key, value in expected_values.items()):
            raise CustodyError("condition_runtime_configuration_invalid")
        if digest((input_dir / "prompt.txt").read_bytes()) != config["prompt_root"]:
            raise CustodyError("condition_prompt_bytes_drift")
        if digest((input_dir / "response-schema.json").read_bytes()) != config.get(
            "response_schema_bytes"
        ):
            raise CustodyError("condition_response_schema_drift")
        if (input_dir / "assignment.json").read_bytes() != (
            STUDY / "assignment-schedule.json"
        ).read_bytes():
            raise CustodyError("condition_assignment_copy_drift")
        condition_configurations[condition] = config
    benchmark_assignments = [
        {key: row[key] for key in ("run_id", "participant_instance_id", "condition")}
        for row in rows
    ]
    if authorization.get("assignments") != benchmark_assignments:
        raise CustodyError("authorization_assignment_drift")
    for row in rows:
        if row.get("condition") not in CONDITIONS:
            raise CustodyError("assignment_condition_invalid")
        if row.get("packet_root") != freeze["packet_roots"][row["condition"]]:
            raise CustodyError("assignment_packet_root_invalid")
        permit_path = STUDY / "permit-template" / f"{row['run_id']}.permit.json"
        permit = load(permit_path)
        if freeze["permit_roots"].get(row["run_id"]) != canonical_root(permit):
            raise CustodyError("prelaunch_permit_root_drift")
    if list((STUDY / "permit-template").glob("*.consumed.json")):
        raise CustodyError("prelaunch_permit_already_consumed")
    for name in ("hold-state.default.json", "hold-state.json"):
        if load(STUDY / "permit-template" / name).get("status") != "hold":
            raise CustodyError("prelaunch_hold_released")
    return {
        "freeze": freeze,
        "registration": registration,
        "assignment": assignment,
        "authorization": authorization,
        "study_configuration": study_configuration,
        "study_configuration_root": study_root,
        "mapping_root": mapping_root,
        "condition_configurations": condition_configurations,
        "rows": {row["run_id"]: row for row in rows},
    }


def validate_response_shape(response: Any) -> dict[str, Any]:
    expected_keys = {
        "schema",
        "fixture_id",
        "predecessor_claim_id",
        "successor_claim_id",
        "consequences",
        "standing_effect",
        "source_or_evidence_binding",
    }
    if not isinstance(response, dict) or set(response) != expected_keys:
        raise CustodyError("runtime_response_fields_invalid")
    if response.get("schema") != "vela.inherited-correction-response.v1":
        raise CustodyError("runtime_response_schema_invalid")
    if response.get("fixture_id") != "bounded-calibration-correction-v1":
        raise CustodyError("runtime_response_fixture_invalid")
    consequences = response.get("consequences")
    if not isinstance(consequences, list) or len(consequences) != 4:
        raise CustodyError("runtime_response_consequences_invalid")
    ids = [item.get("claim_id") for item in consequences if isinstance(item, dict)]
    if ids != ["aggregate-e", "installation-d", "stability-c", "yield-b"]:
        raise CustodyError("runtime_response_claim_order_invalid")
    consequence_keys = {"claim_id", "classification", "action_code"}
    labels = {"affected", "unaffected", "must_reassess", "presently_unprovable"}
    actions = {
        "retrieve_exact_site_q_source",
        "no_correction_reassessment",
        "rerun_stability_method",
        "recalculate_with_successor_factor",
    }
    for item in consequences:
        if not isinstance(item, dict) or set(item) != consequence_keys:
            raise CustodyError("runtime_response_consequence_fields_invalid")
        if (
            item.get("classification") not in labels
            or item.get("action_code") not in actions
        ):
            raise CustodyError("runtime_response_closed_value_invalid")
    for key in expected_keys - {"schema", "fixture_id", "consequences"}:
        if not isinstance(response.get(key), str) or not response[key].strip():
            raise CustodyError(f"runtime_response_value_invalid:{key}")
    return response


def validate_events(raw: bytes, completed: bool) -> dict[str, Any]:
    events = []
    for index, line in enumerate(raw.splitlines(), 1):
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise CustodyError(f"provider_event_json_invalid:{index}") from error
        if not isinstance(event, dict):
            raise CustodyError("provider_event_not_object")
        event_type = str(event.get("type", ""))
        item = event.get("item")
        item_type = str(item.get("type", "")) if isinstance(item, dict) else ""
        if FORBIDDEN_EVENT.search(f"{event_type}:{item_type}"):
            raise CustodyError("provider_event_forbidden")
        events.append(event)
    types = [str(event.get("type", "")) for event in events]
    items = [event["item"] for event in events if isinstance(event.get("item"), dict)]
    agent_messages = [
        item for item in items if item.get("type") in {"agent_message", "message"}
    ]
    thread_count = types.count("thread.started")
    turn_started = types.count("turn.started")
    turn_completed = types.count("turn.completed")
    if any(
        count > 1
        for count in (thread_count, turn_started, turn_completed, len(agent_messages))
    ):
        raise CustodyError("provider_event_sequence_duplicate")
    if completed and (
        thread_count,
        turn_started,
        turn_completed,
        len(agent_messages),
    ) != (1, 1, 1, 1):
        raise CustodyError("provider_event_sequence_incomplete")
    usage_events = [
        event["usage"] for event in events if isinstance(event.get("usage"), dict)
    ]
    usage = usage_events[-1] if usage_events else None
    if completed and usage is None:
        raise CustodyError("provider_usage_missing")
    if usage is not None:
        for key in ("input_tokens", "cached_input_tokens", "output_tokens"):
            value = usage.get(key)
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise CustodyError(f"provider_usage_invalid:{key}")
        for key, value in usage.items():
            if "token" in key and (
                isinstance(value, bool) or not isinstance(value, int) or value < 0
            ):
                raise CustodyError(f"provider_usage_invalid:{key}")
        if usage["output_tokens"] > 8192:
            raise CustodyError("provider_output_token_ceiling")
    return {
        "usage": usage,
        "event_count": len(events),
        "response_count": len(agent_messages),
        "tool_calls": 0,
        "turn_count": turn_started,
        "compactions": 0,
        "agent_messages": agent_messages,
    }


def validate_capture(capture_dir: Path, run_id: str) -> dict[str, Any]:
    state = static_state()
    row = state["rows"].get(run_id)
    if row is None:
        raise CustodyError("runtime_run_not_assigned")
    condition = row["condition"]
    permit_dir = capture_dir / "permit"
    evidence_dir = capture_dir / "evidence"
    unconsumed = permit_dir / f"{run_id}.permit.json"
    consumed = permit_dir / f"{run_id}.permit.consumed.json"
    if unconsumed.exists() or not consumed.is_file() or consumed.is_symlink():
        raise CustodyError("runtime_permit_not_atomically_consumed")
    permit = load(consumed)
    frozen_permit_path = STUDY / "permit-template" / f"{run_id}.permit.json"
    frozen_permit = load(frozen_permit_path)
    if (
        consumed.read_bytes() != frozen_permit_path.read_bytes()
        or permit != frozen_permit
    ):
        raise CustodyError("runtime_consumed_permit_drift")
    freeze = state["freeze"]
    config_root = freeze["condition_runtime_configuration_roots"][condition]
    expected_permit = {
        "schema": "vela.inherited-correction-launch-permit.v1",
        "status": "authorized",
        "expires_at": permit.get("expires_at"),
        "registration_root": freeze["registration_root"],
        "image_digest": freeze["image_digest"],
        "participant_configuration_root": config_root,
        "assignment_root": freeze["assignment_root"],
        "run_id": run_id,
        "condition": condition,
        "participant_instance_id": row["participant_instance_id"],
        "prompt_root": freeze["prompt_roots"][condition],
        "packet_root": freeze["packet_roots"][condition],
        "trust_bundle_bytes": freeze["trust_bundle_bytes"],
        "attempt": 1,
    }
    if permit != expected_permit or freeze["permit_roots"].get(
        run_id
    ) != canonical_root(permit):
        raise CustodyError("runtime_permit_binding_invalid")
    launch_path = evidence_dir / "launch.json"
    receipt_path = evidence_dir / "terminal-receipt.json"
    events_path = evidence_dir / "provider-events.jsonl"
    stderr_path = evidence_dir / "provider-stderr.txt"
    response_path = evidence_dir / "participant-response.raw.json"
    for path in (launch_path, receipt_path, events_path, stderr_path):
        if not path.is_file() or path.is_symlink():
            raise CustodyError(f"runtime_evidence_missing:{path.name}")
    launch = load(launch_path)
    exact_keys(
        launch,
        {
            "schema",
            "run_id",
            "participant_instance_id",
            "condition",
            "permit_bytes",
            "consumed_at",
        },
        "runtime_launch",
    )
    if launch != {
        "schema": "vela.inherited-correction-launch.v1",
        "run_id": run_id,
        "participant_instance_id": row["participant_instance_id"],
        "condition": condition,
        "permit_bytes": digest(consumed.read_bytes()),
        "consumed_at": launch["consumed_at"],
    }:
        raise CustodyError("runtime_launch_binding_invalid")
    receipt = load(receipt_path)
    receipt_keys = {
        "schema",
        "run_id",
        "condition",
        "participant_instance_id",
        "attempt",
        "status",
        "validation_error",
        "provider_started_at",
        "provider_completed_at",
        "duration_seconds",
        "timeout_seconds",
        "process_exit_code",
        "process_timed_out",
        "registration_root",
        "image_digest",
        "participant_configuration_root",
        "assignment_root",
        "trust_bundle_bytes",
        "prompt_root",
        "packet_root",
        "provider_events_bytes",
        "provider_stderr_bytes",
        "response_bytes",
        "event_receipt",
        "cumulative_provider_usage_is_telemetry_only",
        "credential_retained",
    }
    exact_keys(receipt, receipt_keys, "runtime_receipt")
    expected_bindings = {
        "schema": "vela.inherited-correction-terminal-receipt.v1",
        "run_id": run_id,
        "condition": condition,
        "participant_instance_id": row["participant_instance_id"],
        "attempt": 1,
        "timeout_seconds": 600,
        "registration_root": freeze["registration_root"],
        "image_digest": freeze["image_digest"],
        "participant_configuration_root": config_root,
        "assignment_root": freeze["assignment_root"],
        "trust_bundle_bytes": freeze["trust_bundle_bytes"],
        "prompt_root": freeze["prompt_roots"][condition],
        "packet_root": freeze["packet_roots"][condition],
        "cumulative_provider_usage_is_telemetry_only": True,
        "credential_retained": False,
    }
    if any(receipt.get(key) != value for key, value in expected_bindings.items()):
        raise CustodyError("runtime_receipt_binding_invalid")
    if receipt["provider_events_bytes"] != digest(events_path.read_bytes()):
        raise CustodyError("runtime_provider_events_drift")
    if receipt["provider_stderr_bytes"] != digest(stderr_path.read_bytes()):
        raise CustodyError("runtime_provider_stderr_drift")
    completed = receipt.get("status") == "completed"
    if receipt.get("status") not in {"completed", "non_result"}:
        raise CustodyError("runtime_terminal_status_invalid")
    if completed:
        if (
            receipt.get("validation_error") is not None
            or receipt.get("process_exit_code") != 0
            or receipt.get("process_timed_out") is not False
        ):
            raise CustodyError("runtime_completed_status_inconsistent")
    elif (
        not isinstance(receipt.get("validation_error"), str)
        or not receipt["validation_error"]
    ):
        raise CustodyError("runtime_non_result_reason_missing")
    duration = receipt.get("duration_seconds")
    if (
        isinstance(duration, bool)
        or not isinstance(duration, (int, float))
        or not math.isfinite(duration)
        or duration < 0
    ):
        raise CustodyError("runtime_duration_invalid")
    started = parse_time(receipt["provider_started_at"])
    completed_at = parse_time(receipt["provider_completed_at"])
    measured = (completed_at - started).total_seconds()
    if measured < 0 or abs(measured - duration) > 5:
        raise CustodyError("runtime_duration_timestamp_mismatch")
    if parse_time(launch["consumed_at"]) > started:
        raise CustodyError("runtime_provider_precedes_permit_consumption")
    if started > parse_time(permit["expires_at"]):
        raise CustodyError("runtime_provider_after_permit_expiry")
    if completed and duration > 600:
        raise CustodyError("runtime_completed_after_timeout")
    if (
        receipt.get("process_timed_out")
        and "timeout" not in receipt["validation_error"].lower()
    ):
        raise CustodyError("runtime_timeout_status_mismatch")
    event_summary = validate_events(events_path.read_bytes(), completed)
    comparable_event_summary = {
        key: value for key, value in event_summary.items() if key != "agent_messages"
    }
    if (
        receipt.get("event_receipt") is not None
        and receipt["event_receipt"] != comparable_event_summary
    ):
        raise CustodyError("runtime_event_receipt_mismatch")
    if completed and receipt.get("event_receipt") is None:
        raise CustodyError("runtime_event_receipt_missing")
    response = None
    if receipt.get("response_bytes") is None:
        if response_path.exists():
            raise CustodyError("runtime_unbound_response_present")
        if completed:
            raise CustodyError("runtime_completed_response_missing")
    else:
        if not response_path.is_file() or response_path.is_symlink():
            raise CustodyError("runtime_response_missing")
        response_bytes = response_path.read_bytes()
        if receipt["response_bytes"] != digest(response_bytes):
            raise CustodyError("runtime_response_drift")
        try:
            response = json.loads(response_bytes)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            if completed:
                raise CustodyError("runtime_response_json_invalid") from error
        if completed:
            validate_response_shape(response)
            messages = event_summary["agent_messages"]
            try:
                event_response = json.loads(messages[0].get("text", ""))
            except json.JSONDecodeError as error:
                raise CustodyError("runtime_event_response_json_invalid") from error
            if event_response != response:
                raise CustodyError("runtime_event_response_mismatch")
    return {
        "state": state,
        "row": row,
        "permit": permit,
        "launch": launch,
        "receipt": receipt,
        "event_summary": comparable_event_summary,
        "response": response,
        "paths": {
            "permit": consumed,
            "launch": launch_path,
            "receipt": receipt_path,
            "events": events_path,
            "stderr": stderr_path,
            "response": response_path if response_path.is_file() else None,
        },
    }


def evidence_manifest(validated: dict[str, Any]) -> dict[str, Any]:
    state = validated["state"]
    receipt = validated["receipt"]
    paths = validated["paths"]
    condition = validated["row"]["condition"]
    return {
        "schema": "vela.inherited-correction-runtime-custody.v1",
        "run_id": validated["row"]["run_id"],
        "condition": condition,
        "participant_instance_id": validated["row"]["participant_instance_id"],
        "benchmark_registration_root": state["freeze"]["benchmark_registration_root"],
        "runtime_registration_root": state["freeze"]["registration_root"],
        "authorization_root": state["freeze"]["authorization_root"],
        "assignment_root": state["freeze"]["assignment_root"],
        "shared_study_configuration_root": state["study_configuration_root"],
        "authorized_configuration_mapping_root": state["mapping_root"],
        "condition_runtime_configuration_root": state["freeze"][
            "condition_runtime_configuration_roots"
        ][condition],
        "prompt_root": state["freeze"]["prompt_roots"][condition],
        "packet_root": state["freeze"]["packet_roots"][condition],
        "image_digest": state["freeze"]["image_digest"],
        "trust_bundle_bytes": state["freeze"]["trust_bundle_bytes"],
        "runtime_source_root": state["freeze"]["runtime_source_root"],
        "attempt": 1,
        "timeout_seconds": 600,
        "terminal_status": receipt["status"],
        "provider_started_at": receipt["provider_started_at"],
        "provider_completed_at": receipt["provider_completed_at"],
        "duration_seconds": receipt["duration_seconds"],
        "usage": validated["event_summary"]["usage"],
        "consumed_permit_bytes": digest(paths["permit"].read_bytes()),
        "launch_bytes": digest(paths["launch"].read_bytes()),
        "terminal_receipt_bytes": digest(paths["receipt"].read_bytes()),
        "provider_events_bytes": digest(paths["events"].read_bytes()),
        "provider_stderr_bytes": digest(paths["stderr"].read_bytes()),
        "runtime_response_bytes": digest(paths["response"].read_bytes())
        if paths["response"]
        else None,
    }


def ingest(capture_dir: Path, runs_dir: Path, run_id: str) -> Path:
    validated = validate_capture(capture_dir.resolve(), run_id)
    run_dir = runs_dir.resolve() / run_id
    if run_dir.exists():
        raise CustodyError("runtime_run_already_ingested")
    run_dir.mkdir(parents=True)
    try:
        shutil.copytree(
            BENCHMARK / "conditions" / validated["row"]["condition"], run_dir / "packet"
        )
        (run_dir / "authorization.json").write_bytes(
            encoded(validated["state"]["authorization"])
        )
        runtime_dir = run_dir / "runtime"
        runtime_dir.mkdir()
        names = {
            "permit": "consumed-permit.json",
            "launch": "launch.json",
            "receipt": "terminal-receipt.json",
            "events": "provider-events.jsonl",
            "stderr": "provider-stderr.txt",
            "response": "participant-response.raw.json",
        }
        for identity, destination in names.items():
            source = validated["paths"].get(identity)
            if source is not None:
                shutil.copyfile(source, runtime_dir / destination)
        manifest = evidence_manifest(validated)
        (run_dir / "runtime-evidence.json").write_bytes(encoded(manifest))
        receipt = validated["receipt"]
        status = (
            "completed"
            if receipt["status"] == "completed"
            else ("timed_out" if receipt["process_timed_out"] else "failed")
        )
        record = {
            "schema": "vela.inherited-correction-run.v2",
            "run_id": run_id,
            "participant_instance_id": validated["row"]["participant_instance_id"],
            "participant_configuration_root": validated["state"][
                "study_configuration_root"
            ],
            "condition_runtime_configuration_root": manifest[
                "condition_runtime_configuration_root"
            ],
            "authorized_configuration_mapping_root": validated["state"]["mapping_root"],
            "condition": validated["row"]["condition"],
            "packet_root": manifest["packet_root"],
            "registration_root": manifest["benchmark_registration_root"],
            "runtime_registration_root": manifest["runtime_registration_root"],
            "authorization_root": manifest["authorization_root"],
            "runtime_custody_root": canonical_root(manifest),
            "status": status,
            "started_at": receipt["provider_started_at"],
            "completed_at": receipt["provider_completed_at"],
            "duration_seconds": receipt["duration_seconds"],
            "tool_calls": validated["event_summary"]["tool_calls"],
            "timeout_seconds": 600,
            "attempt": 1,
        }
        (run_dir / "run.json").write_bytes(encoded(record))
        if status == "completed":
            (run_dir / "response.json").write_bytes(encoded(validated["response"]))
    except Exception:
        shutil.rmtree(run_dir)
        raise
    return run_dir


def validate_ingested_run(run_dir: Path) -> dict[str, Any]:
    record = load(run_dir / "run.json")
    if record.get("schema") != "vela.inherited-correction-run.v2":
        raise CustodyError("runtime_ingested_run_schema_invalid")
    capture = run_dir / "runtime"
    if not capture.is_dir() or capture.is_symlink():
        raise CustodyError("runtime_ingested_evidence_directory_invalid")
    required_runtime = {
        "consumed-permit.json",
        "launch.json",
        "terminal-receipt.json",
        "provider-events.jsonl",
        "provider-stderr.txt",
    }
    observed_runtime = {path.name for path in capture.iterdir()}
    if not required_runtime.issubset(observed_runtime) or observed_runtime - (
        required_runtime | {"participant-response.raw.json"}
    ):
        raise CustodyError("runtime_ingested_evidence_set_invalid")
    if any(path.is_symlink() or not path.is_file() for path in capture.iterdir()):
        raise CustodyError("runtime_ingested_evidence_unsafe")
    expected_top = {
        "run.json",
        "authorization.json",
        "runtime-evidence.json",
        "packet",
        "runtime",
    }
    if record.get("status") == "completed":
        expected_top.add("response.json")
    if {path.name for path in run_dir.iterdir()} != expected_top:
        raise CustodyError("runtime_ingested_run_file_set_invalid")
    reconstructed = run_dir.parent / f".{run_dir.name}.custody-reconstruction"
    if reconstructed.exists():
        raise CustodyError("runtime_reconstruction_collision")
    reconstructed.mkdir()
    try:
        permit_dir = reconstructed / "permit"
        evidence_dir = reconstructed / "evidence"
        permit_dir.mkdir()
        evidence_dir.mkdir()
        shutil.copyfile(
            capture / "consumed-permit.json",
            permit_dir / f"{run_dir.name}.permit.consumed.json",
        )
        for name in (
            "launch.json",
            "terminal-receipt.json",
            "provider-events.jsonl",
            "provider-stderr.txt",
        ):
            shutil.copyfile(capture / name, evidence_dir / name)
        raw_response = capture / "participant-response.raw.json"
        if raw_response.is_file():
            shutil.copyfile(raw_response, evidence_dir / raw_response.name)
        validated = validate_capture(reconstructed, run_dir.name)
        manifest = evidence_manifest(validated)
    finally:
        shutil.rmtree(reconstructed)
    if load(run_dir / "runtime-evidence.json") != manifest:
        raise CustodyError("runtime_evidence_manifest_drift")
    expected_root = canonical_root(manifest)
    if record.get("runtime_custody_root") != expected_root:
        raise CustodyError("runtime_custody_root_mismatch")
    expected_record = {
        "schema": "vela.inherited-correction-run.v2",
        "run_id": run_dir.name,
        "participant_instance_id": validated["row"]["participant_instance_id"],
        "participant_configuration_root": validated["state"][
            "study_configuration_root"
        ],
        "condition_runtime_configuration_root": manifest[
            "condition_runtime_configuration_root"
        ],
        "authorized_configuration_mapping_root": validated["state"]["mapping_root"],
        "condition": validated["row"]["condition"],
        "packet_root": manifest["packet_root"],
        "registration_root": manifest["benchmark_registration_root"],
        "runtime_registration_root": manifest["runtime_registration_root"],
        "authorization_root": manifest["authorization_root"],
        "runtime_custody_root": expected_root,
        "status": "completed"
        if validated["receipt"]["status"] == "completed"
        else ("timed_out" if validated["receipt"]["process_timed_out"] else "failed"),
        "started_at": validated["receipt"]["provider_started_at"],
        "completed_at": validated["receipt"]["provider_completed_at"],
        "duration_seconds": validated["receipt"]["duration_seconds"],
        "tool_calls": validated["event_summary"]["tool_calls"],
        "timeout_seconds": 600,
        "attempt": 1,
    }
    if record != expected_record:
        raise CustodyError("runtime_benchmark_record_not_bridge_generated")
    response_path = run_dir / "response.json"
    if record["status"] == "completed":
        if not response_path.is_file() or response_path.is_symlink():
            raise CustodyError("runtime_benchmark_response_missing")
        if load(response_path) != validated["response"]:
            raise CustodyError("runtime_benchmark_response_drift")
    elif response_path.exists():
        raise CustodyError("runtime_non_result_benchmark_response_present")
    return {
        "record": record,
        "manifest": manifest,
        "response_path": response_path if response_path.is_file() else None,
    }


def complete_custody(runs_dir: Path) -> dict[str, Any]:
    state = static_state()
    expected = set(state["rows"])
    observed = {
        path.name
        for path in runs_dir.iterdir()
        if path.is_dir() and not path.name.startswith(".")
    }
    if observed != expected:
        raise CustodyError(f"runtime_fixed_denominator_incomplete:{len(observed)}/16")
    entries = []
    permit_bytes = set()
    for run_id in sorted(expected):
        validated = validate_ingested_run(runs_dir / run_id)
        manifest = validated["manifest"]
        if manifest["consumed_permit_bytes"] in permit_bytes:
            raise CustodyError("runtime_consumed_permit_duplicate")
        permit_bytes.add(manifest["consumed_permit_bytes"])
        entries.append(
            {
                "run_id": run_id,
                "condition": manifest["condition"],
                "runtime_custody_root": canonical_root(manifest),
                "terminal_receipt_bytes": manifest["terminal_receipt_bytes"],
                "consumed_permit_bytes": manifest["consumed_permit_bytes"],
                "provider_events_bytes": manifest["provider_events_bytes"],
                "runtime_response_bytes": manifest["runtime_response_bytes"],
            }
        )
    value = {
        "schema": "vela.inherited-correction-complete-runtime-custody.v1",
        "benchmark_registration_root": state["freeze"]["benchmark_registration_root"],
        "runtime_registration_root": state["freeze"]["registration_root"],
        "authorization_root": state["freeze"]["authorization_root"],
        "assignment_root": state["freeze"]["assignment_root"],
        "shared_study_configuration_root": state["study_configuration_root"],
        "authorized_configuration_mapping_root": state["mapping_root"],
        "runs": entries,
    }
    value["complete_runtime_custody_root"] = canonical_root(value)
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("verify-prelaunch")
    ingest_parser = sub.add_parser("ingest")
    ingest_parser.add_argument("--capture-dir", type=Path, required=True)
    ingest_parser.add_argument("--runs-dir", type=Path, required=True)
    ingest_parser.add_argument("--run-id", required=True)
    args = parser.parse_args()
    if args.command == "verify-prelaunch":
        static_state()
        print("confirmatory custody prelaunch: verified")
    elif args.command == "ingest":
        print(ingest(args.capture_dir, args.runs_dir, args.run_id))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CustodyError as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(2)
