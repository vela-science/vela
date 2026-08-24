#!/usr/bin/env python3
"""Fail-closed one-shot custody bridge for the held-out benchmark."""

from __future__ import annotations

import argparse
import importlib.util
import json
import math
import re
import shutil
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
FORBIDDEN_EVENT = re.compile(
    r"tool|command|patch|file_change|web_search|computer|compact|resume|continu",
    re.IGNORECASE,
)


class CustodyError(RuntimeError):
    pass


def load_benchmark() -> Any:
    path = ROOT / "benchmark.py"
    spec = importlib.util.spec_from_file_location("held_out_benchmark", path)
    if spec is None or spec.loader is None:
        raise CustodyError("benchmark_import_failed")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_bytes())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CustodyError(f"json_invalid:{path.name}") from error


def parse_time(value: Any) -> datetime:
    if not isinstance(value, str):
        raise CustodyError("timestamp_invalid")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise CustodyError("timestamp_invalid") from error
    if parsed.tzinfo is None:
        raise CustodyError("timestamp_timezone_missing")
    return parsed


def exact_keys(value: Any, keys: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != keys:
        raise CustodyError(f"{label}_fields_invalid")


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
    messages = [
        item for item in items if item.get("type") in {"agent_message", "message"}
    ]
    counts = (
        types.count("thread.started"),
        types.count("turn.started"),
        types.count("turn.completed"),
        len(messages),
    )
    if any(count > 1 for count in counts):
        raise CustodyError("provider_event_sequence_duplicate")
    if completed and counts != (1, 1, 1, 1):
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
        "response_count": len(messages),
        "tool_calls": 0,
        "turn_count": counts[1],
        "compactions": 0,
        "messages": messages,
    }


def static_state() -> dict[str, Any]:
    benchmark = load_benchmark()
    preregistration = load_json(ROOT / "preregistration.json")
    schedule = load_json(ROOT / "assignment-schedule.json")
    configuration = load_json(ROOT / "participant-configuration.json")
    freeze = load_json(ROOT / "prelaunch-freeze.json")
    mapping = load_json(ROOT / "configuration-mapping.json")
    bindings = preregistration["bindings"]
    if (
        benchmark.canonical_root(benchmark.assignment_plan())
        != bindings["assignment_plan_root"]
        or benchmark.assignment_plan()["seed_commitment"]
        != bindings["assignment_seed_commitment"]
    ):
        raise CustodyError("assignment_plan_root_drift")
    if benchmark.canonical_root(schedule) != freeze["assignment_root"]:
        raise CustodyError("assignment_root_drift")
    if (
        benchmark.canonical_root(configuration)
        != bindings["participant_configuration_root"]
    ):
        raise CustodyError("participant_configuration_root_drift")
    if (
        benchmark.canonical_root(mapping) != freeze["configuration_mapping_root"]
        or mapping.get("registration_root") != preregistration["registration_root"]
        or mapping.get("shared_study_configuration_root")
        != bindings["participant_configuration_root"]
        or mapping.get("family_condition_runtime_configuration_roots")
        != freeze["runtime_configuration_roots"]
    ):
        raise CustodyError("configuration_mapping_drift")
    rows = {item["run_id"]: item for item in schedule["assignments"]}
    if len(rows) != 36:
        raise CustodyError("assignment_count_invalid")
    if len({item["participant_instance_id"] for item in rows.values()}) != 36:
        raise CustodyError("participant_instance_duplicate")
    counts = {
        (family, condition): sum(
            item["family_id"] == family and item["condition"] == condition
            for item in rows.values()
        )
        for family in preregistration["families"]
        for condition in benchmark.CONDITIONS
    }
    if set(counts.values()) != {4}:
        raise CustodyError("assignment_balance_invalid")
    for family_id, family in benchmark.family_map().items():
        for condition in benchmark.CONDITIONS:
            files = benchmark.packet_files(family, condition)
            if (
                benchmark.packet_root(files)
                != bindings["packet_roots"][family_id][condition]
                or benchmark.byte_digest(benchmark.prompt_bytes(files))
                != bindings["prompt_roots"][family_id][condition]
            ):
                raise CustodyError("packet_or_prompt_root_drift")
            input_dir = ROOT / "conditions" / family_id / condition / "input"
            runtime_configuration = load_json(
                input_dir / "participant-configuration.json"
            )
            expected_root = freeze["runtime_configuration_roots"][family_id][condition]
            if benchmark.canonical_root(runtime_configuration) != expected_root:
                raise CustodyError("runtime_configuration_root_drift")
            if (input_dir / "assignment.json").read_bytes() != (
                ROOT / "assignment-schedule.json"
            ).read_bytes():
                raise CustodyError("runtime_assignment_copy_drift")
            if (
                benchmark.byte_digest((input_dir / "prompt.txt").read_bytes())
                != (bindings["prompt_roots"][family_id][condition])
            ):
                raise CustodyError("runtime_prompt_copy_drift")
    return {
        "benchmark": benchmark,
        "preregistration": preregistration,
        "schedule": schedule,
        "configuration": configuration,
        "freeze": freeze,
        "mapping": mapping,
        "mapping_root": freeze["configuration_mapping_root"],
        "rows": rows,
    }


def verify_prelaunch() -> dict[str, Any]:
    state = static_state()
    benchmark = state["benchmark"]
    benchmark.verify()
    preregistration = state["preregistration"]
    if preregistration["status"] != "held_pending_adjudication_binding_review":
        raise CustodyError("prelaunch_status_not_held")
    commitment = preregistration["bindings"]["adjudication_commitment"]
    if commitment["status"] != "frozen":
        raise CustodyError("adjudication_state_invalid")
    amendment = preregistration["bindings"]["launch_authorization_amendment"]
    if (
        amendment.get("status") != "authorized_held_pending_binding_review"
        or amendment.get("evaluator_commitment", {}).get("adjudication_root")
        != commitment.get("adjudication_root")
        or amendment.get("execution_state")
        != {
            "sessions_completed": 0,
            "fixed_denominator": 36,
            "permits_held": 36,
            "permits_consumed": 0,
            "provider_calls": 0,
            "protected_key_accesses": 0,
        }
    ):
        raise CustodyError("launch_amendment_invalid")
    hold = load_json(ROOT / "permit-template/hold-state.json")
    exact_keys(hold, {"schema", "status", "reason", "updated_at"}, "hold")
    if (
        hold["schema"] != "vela.inherited-correction-hold.v1"
        or hold["status"] != "hold"
    ):
        raise CustodyError("hold_not_active")
    runtime = load_json(ROOT / "runtime-binding.json")
    freeze = load_json(ROOT / "prelaunch-freeze.json")
    if (
        freeze.get("runtime_root") != benchmark.canonical_root(runtime)
        or freeze.get("image_digest") != runtime["container_image_digest"]
        or freeze.get("trust_bundle_bytes") != runtime["trust_bundle_bytes"]
    ):
        raise CustodyError("runtime_binding_drift")
    permits = sorted((ROOT / "permit-template").glob("heldout-run-*.permit.json"))
    if len(permits) != 36:
        raise CustodyError("permit_count_invalid")
    if list((ROOT / "permit-template").glob("*.consumed.json")):
        raise CustodyError("prelaunch_consumed_permit_present")
    for path in permits:
        permit = load_json(path)
        if permit["status"] != "held":
            raise CustodyError("permit_not_held")
        row = state["rows"].get(permit["run_id"])
        if row is None or any(
            permit[key] != row[key]
            for key in (
                "run_id",
                "participant_instance_id",
                "condition",
            )
        ):
            raise CustodyError("permit_assignment_mismatch")
        family_id = row["family_id"]
        condition = row["condition"]
        expected = {
            "schema": "vela.inherited-correction-launch-permit.v1",
            "status": "held",
            "expires_at": "not_authorized",
            "registration_root": preregistration["registration_root"],
            "image_digest": runtime["container_image_digest"],
            "participant_configuration_root": freeze["runtime_configuration_roots"][
                family_id
            ][condition],
            "assignment_root": freeze["assignment_root"],
            "run_id": row["run_id"],
            "condition": condition,
            "participant_instance_id": row["participant_instance_id"],
            "prompt_root": preregistration["bindings"]["prompt_roots"][family_id][
                condition
            ],
            "packet_root": preregistration["bindings"]["packet_roots"][family_id][
                condition
            ],
            "trust_bundle_bytes": runtime["trust_bundle_bytes"],
            "attempt": 1,
        }
        if permit != expected:
            raise CustodyError("permit_binding_drift")
    return {
        "schema": "vela.inherited-correction-held-out-prelaunch-receipt.v1",
        "status": "verified_hold",
        "registration_root": preregistration["registration_root"],
        "assignment_root": freeze["assignment_root"],
        "permits": 36,
        "consumed": 0,
    }


def expected_permit(run_id: str) -> dict[str, Any]:
    permit = load_json(ROOT / f"permit-template/{run_id}.permit.json")
    return permit


def permit_identity(permit: dict[str, Any]) -> dict[str, Any]:
    """Return the frozen root-bound fields shared by held and issued permits."""
    return {
        key: value
        for key, value in permit.items()
        if key not in {"status", "expires_at"}
    }


def validate_capture(capture_dir: Path, run_id: str) -> dict[str, Any]:
    state = static_state()
    benchmark = state["benchmark"]
    row = state["rows"].get(run_id)
    if row is None:
        raise CustodyError("run_unassigned")
    evidence = capture_dir / "evidence"
    permit_dir = capture_dir / "permit"
    unconsumed = permit_dir / f"{run_id}.permit.json"
    required = {
        "terminal": evidence / "terminal-receipt.json",
        "events": evidence / "provider-events.jsonl",
        "stderr": evidence / "provider-stderr.txt",
        "launch": evidence / "launch.json",
        "permit": permit_dir / f"{run_id}.permit.consumed.json",
    }
    if unconsumed.exists():
        raise CustodyError("permit_not_atomically_consumed")
    for name, path in required.items():
        if not path.is_file() or path.is_symlink():
            raise CustodyError(f"capture_{name}_missing")
    receipt = load_json(required["terminal"])
    launch = load_json(required["launch"])
    consumed = load_json(required["permit"])
    template = expected_permit(run_id)
    if permit_identity(consumed) != permit_identity(template):
        raise CustodyError("consumed_permit_drift")
    if consumed.get("status") != "authorized":
        raise CustodyError("permit_not_authorized")
    preregistration = state["preregistration"]
    runtime = load_json(ROOT / "runtime-binding.json")
    family_id = row["family_id"]
    condition = row["condition"]
    packet_root = preregistration["bindings"]["packet_roots"][family_id][condition]
    prompt_root = preregistration["bindings"]["prompt_roots"][family_id][condition]
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
        "launch",
    )
    expected_launch = {
        "schema": "vela.inherited-correction-launch.v1",
        "run_id": run_id,
        "participant_instance_id": row["participant_instance_id"],
        "condition": condition,
        "permit_bytes": benchmark.byte_digest(required["permit"].read_bytes()),
        "consumed_at": launch["consumed_at"],
    }
    if launch != expected_launch:
        raise CustodyError("launch_binding_invalid")
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
    exact_keys(receipt, receipt_keys, "terminal_receipt")
    expected_receipt = {
        "schema": "vela.inherited-correction-terminal-receipt.v1",
        "run_id": run_id,
        "participant_instance_id": row["participant_instance_id"],
        "condition": condition,
        "registration_root": preregistration["registration_root"],
        "image_digest": runtime["container_image_digest"],
        "assignment_root": state["freeze"]["assignment_root"],
        "participant_configuration_root": state["freeze"][
            "runtime_configuration_roots"
        ][family_id][condition],
        "trust_bundle_bytes": runtime["trust_bundle_bytes"],
        "prompt_root": prompt_root,
        "packet_root": packet_root,
        "attempt": 1,
        "timeout_seconds": 600,
        "cumulative_provider_usage_is_telemetry_only": True,
        "credential_retained": False,
    }
    for key, value in expected_receipt.items():
        if receipt.get(key) != value:
            raise CustodyError(f"receipt_drift:{key}")
    completed = receipt.get("status") == "completed"
    if receipt.get("status") not in {"completed", "non_result"}:
        raise CustodyError("terminal_status_invalid")
    if completed and (
        receipt.get("validation_error") is not None
        or receipt.get("process_exit_code") != 0
        or receipt.get("process_timed_out") is not False
    ):
        raise CustodyError("completed_status_inconsistent")
    if not completed and (
        not isinstance(receipt.get("validation_error"), str)
        or not receipt["validation_error"]
    ):
        raise CustodyError("non_result_reason_missing")
    duration = receipt.get("duration_seconds")
    if (
        isinstance(duration, bool)
        or not isinstance(duration, (int, float))
        or not math.isfinite(duration)
        or duration < 0
    ):
        raise CustodyError("duration_invalid")
    started = parse_time(receipt["provider_started_at"])
    completed_at = parse_time(receipt["provider_completed_at"])
    measured = (completed_at - started).total_seconds()
    if measured < 0 or abs(measured - duration) > 5:
        raise CustodyError("duration_timestamp_mismatch")
    if parse_time(launch["consumed_at"]) > started:
        raise CustodyError("provider_precedes_permit_consumption")
    if started > parse_time(consumed["expires_at"]):
        raise CustodyError("provider_after_permit_expiry")
    if completed and duration > 600:
        raise CustodyError("completed_after_timeout")
    if (
        receipt.get("process_timed_out")
        and "timeout" not in receipt["validation_error"].lower()
    ):
        raise CustodyError("timeout_status_mismatch")
    events_raw = required["events"].read_bytes()
    stderr_raw = required["stderr"].read_bytes()
    if receipt.get("provider_events_bytes") != benchmark.byte_digest(events_raw):
        raise CustodyError("provider_events_drift")
    if receipt.get("provider_stderr_bytes") != benchmark.byte_digest(stderr_raw):
        raise CustodyError("provider_stderr_drift")
    event_summary = validate_events(events_raw, completed)
    receipt_summary = {
        key: value for key, value in event_summary.items() if key != "messages"
    }
    if completed and receipt.get("event_receipt") != receipt_summary:
        raise CustodyError("event_receipt_mismatch")
    if (
        not completed
        and receipt.get("event_receipt") is not None
        and receipt["event_receipt"] != receipt_summary
    ):
        raise CustodyError("event_receipt_mismatch")
    response_path = evidence / "participant-response.raw.json"
    if response_path.is_symlink():
        raise CustodyError("response_symlink_forbidden")
    response_raw = response_path.read_bytes() if response_path.is_file() else None
    response = None
    if completed:
        if response_raw is None:
            raise CustodyError("completed_response_missing")
        if receipt.get("response_bytes") != benchmark.byte_digest(response_raw):
            raise CustodyError("response_bytes_drift")
        family = benchmark.family_map()[row["family_id"]]
        manifest = json.loads(
            benchmark.packet_files(family, condition)["PACKET-MANIFEST.json"]
        )
        response = json.loads(response_raw)
        benchmark.validate_response(response, family, manifest)
        try:
            event_response = json.loads(event_summary["messages"][0].get("text", ""))
        except json.JSONDecodeError as error:
            raise CustodyError("event_response_json_invalid") from error
        if event_response != response:
            raise CustodyError("event_response_mismatch")
    elif response_raw is not None:
        if receipt.get("response_bytes") != benchmark.byte_digest(response_raw):
            raise CustodyError("terminal_response_bytes_drift")
    elif receipt.get("response_bytes") is not None:
        raise CustodyError("unbound_response_digest")
    return {
        "state": state,
        "row": row,
        "receipt": receipt,
        "consumed": consumed,
        "event_summary": receipt_summary,
        "response": response,
        "paths": required,
        "response_path": response_path if response_raw is not None else None,
        "response_raw": response_raw,
    }


def evidence_manifest(validated: dict[str, Any]) -> dict[str, Any]:
    benchmark = validated["state"]["benchmark"]
    preregistration = validated["state"]["preregistration"]
    receipt = validated["receipt"]
    row = validated["row"]
    paths = validated["paths"]
    runtime = load_json(ROOT / "runtime-binding.json")
    state = validated["state"]
    runtime_configuration_root = state["freeze"]["runtime_configuration_roots"][
        row["family_id"]
    ][row["condition"]]
    return {
        "schema": "vela.inherited-correction-held-out-runtime-custody.v1",
        "run_id": row["run_id"],
        "participant_instance_id": row["participant_instance_id"],
        "family_id": row["family_id"],
        "condition": row["condition"],
        "registration_root": preregistration["registration_root"],
        "assignment_root": state["freeze"]["assignment_root"],
        "shared_study_configuration_root": preregistration["bindings"][
            "participant_configuration_root"
        ],
        "runtime_configuration_root": runtime_configuration_root,
        "configuration_mapping_root": state["mapping_root"],
        "packet_root": receipt["packet_root"],
        "prompt_root": receipt["prompt_root"],
        "image_digest": runtime["container_image_digest"],
        "trust_bundle_bytes": runtime["trust_bundle_bytes"],
        "runtime_root": benchmark.canonical_root(runtime),
        "attempt": 1,
        "timeout_seconds": 600,
        "terminal_status": receipt["status"],
        "provider_started_at": receipt["provider_started_at"],
        "provider_completed_at": receipt["provider_completed_at"],
        "duration_seconds": receipt["duration_seconds"],
        "usage": validated["event_summary"]["usage"],
        "consumed_permit_bytes": benchmark.byte_digest(paths["permit"].read_bytes()),
        "launch_bytes": benchmark.byte_digest(paths["launch"].read_bytes()),
        "terminal_receipt_bytes": benchmark.byte_digest(paths["terminal"].read_bytes()),
        "provider_events_bytes": benchmark.byte_digest(paths["events"].read_bytes()),
        "provider_stderr_bytes": benchmark.byte_digest(paths["stderr"].read_bytes()),
        "runtime_response_bytes": (
            benchmark.byte_digest(validated["response_path"].read_bytes())
            if validated["response_path"] is not None
            else None
        ),
    }


def ingest(capture_dir: Path, runs_dir: Path, run_id: str) -> Path:
    validated = validate_capture(capture_dir, run_id)
    benchmark = validated["state"]["benchmark"]
    row = validated["row"]
    receipt = validated["receipt"]
    target = runs_dir / run_id
    if target.exists():
        raise CustodyError("run_already_ingested")
    runtime = target / "runtime"
    runtime.mkdir(parents=True)
    copies = {
        "terminal-receipt.json": validated["paths"]["terminal"],
        "provider-events.jsonl": validated["paths"]["events"],
        "provider-stderr.txt": validated["paths"]["stderr"],
        "launch.json": validated["paths"]["launch"],
        "consumed-permit.json": validated["paths"]["permit"],
    }
    if validated["response_path"] is not None:
        copies["participant-response.raw.json"] = validated["response_path"]
    for name, source in copies.items():
        shutil.copyfile(source, runtime / name)
    preregistration = validated["state"]["preregistration"]
    manifest = evidence_manifest(validated)
    manifest["runtime_custody_root"] = benchmark.canonical_root(manifest)
    (target / "runtime-evidence.json").write_bytes(benchmark.json_bytes(manifest))
    status = (
        "completed"
        if receipt["status"] == "completed"
        else ("timed_out" if receipt["process_timed_out"] else "failed")
    )
    run = {
        "schema": "vela.inherited-correction-held-out-run.v1",
        "run_id": run_id,
        "participant_instance_id": row["participant_instance_id"],
        "family_id": row["family_id"],
        "condition": row["condition"],
        "registration_root": preregistration["registration_root"],
        "assignment_root": manifest["assignment_root"],
        "shared_study_configuration_root": manifest["shared_study_configuration_root"],
        "runtime_configuration_root": manifest["runtime_configuration_root"],
        "configuration_mapping_root": manifest["configuration_mapping_root"],
        "packet_root": manifest["packet_root"],
        "prompt_root": manifest["prompt_root"],
        "image_digest": manifest["image_digest"],
        "trust_bundle_bytes": manifest["trust_bundle_bytes"],
        "runtime_root": manifest["runtime_root"],
        "runtime_custody_root": manifest["runtime_custody_root"],
        "status": status,
        "started_at": receipt["provider_started_at"],
        "completed_at": receipt["provider_completed_at"],
        "duration_seconds": receipt["duration_seconds"],
        "tool_calls": validated["event_summary"]["tool_calls"],
        "attempt": 1,
        "timeout_seconds": 600,
    }
    (target / "run.json").write_bytes(benchmark.json_bytes(run))
    if receipt["status"] == "completed" and validated["response_raw"] is not None:
        (target / "response.json").write_bytes(validated["response_raw"])
    return target


def validate_ingested_run(run_dir: Path) -> dict[str, Any]:
    benchmark = load_benchmark()
    run = load_json(run_dir / "run.json")
    evidence = load_json(run_dir / "runtime-evidence.json")
    if run.get("run_id") != run_dir.name or evidence.get("run_id") != run_dir.name:
        raise CustodyError("ingested_run_identity_drift")
    runtime_dir = run_dir / "runtime"
    if not runtime_dir.is_dir() or runtime_dir.is_symlink():
        raise CustodyError("ingested_runtime_directory_invalid")
    required_runtime = {
        "consumed-permit.json",
        "launch.json",
        "terminal-receipt.json",
        "provider-events.jsonl",
        "provider-stderr.txt",
    }
    observed_runtime = {path.name for path in runtime_dir.iterdir()}
    if not required_runtime.issubset(observed_runtime) or observed_runtime - (
        required_runtime | {"participant-response.raw.json"}
    ):
        raise CustodyError("ingested_runtime_evidence_set_invalid")
    if any(path.is_symlink() or not path.is_file() for path in runtime_dir.iterdir()):
        raise CustodyError("ingested_runtime_evidence_unsafe")
    response_path = run_dir / "response.json"
    expected_top = {"run.json", "runtime-evidence.json", "runtime"}
    if run.get("status") == "completed":
        expected_top.add("response.json")
    if {path.name for path in run_dir.iterdir()} != expected_top:
        raise CustodyError("ingested_run_file_set_invalid")
    reconstructed = run_dir.parent / f".{run_dir.name}.custody-reconstruction"
    if reconstructed.exists():
        raise CustodyError("ingested_reconstruction_collision")
    (reconstructed / "permit").mkdir(parents=True)
    (reconstructed / "evidence").mkdir()
    try:
        shutil.copyfile(
            runtime_dir / "consumed-permit.json",
            reconstructed / "permit" / f"{run_dir.name}.permit.consumed.json",
        )
        for name in (
            "launch.json",
            "terminal-receipt.json",
            "provider-events.jsonl",
            "provider-stderr.txt",
        ):
            shutil.copyfile(runtime_dir / name, reconstructed / "evidence" / name)
        raw_response = runtime_dir / "participant-response.raw.json"
        if raw_response.is_file():
            shutil.copyfile(
                raw_response,
                reconstructed / "evidence" / "participant-response.raw.json",
            )
        validated = validate_capture(reconstructed, run_dir.name)
        expected_manifest = evidence_manifest(validated)
    finally:
        shutil.rmtree(reconstructed)
    claimed = evidence.pop("runtime_custody_root", None)
    expected_root = benchmark.canonical_root(expected_manifest)
    if evidence != expected_manifest or claimed != expected_root:
        raise CustodyError("runtime_custody_root_drift")
    receipt = validated["receipt"]
    expected_status = (
        "completed"
        if receipt["status"] == "completed"
        else ("timed_out" if receipt["process_timed_out"] else "failed")
    )
    expected_run = {
        "schema": "vela.inherited-correction-held-out-run.v1",
        "run_id": run_dir.name,
        "participant_instance_id": validated["row"]["participant_instance_id"],
        "family_id": validated["row"]["family_id"],
        "condition": validated["row"]["condition"],
        "registration_root": expected_manifest["registration_root"],
        "assignment_root": expected_manifest["assignment_root"],
        "shared_study_configuration_root": expected_manifest[
            "shared_study_configuration_root"
        ],
        "runtime_configuration_root": expected_manifest["runtime_configuration_root"],
        "configuration_mapping_root": expected_manifest["configuration_mapping_root"],
        "packet_root": expected_manifest["packet_root"],
        "prompt_root": expected_manifest["prompt_root"],
        "image_digest": expected_manifest["image_digest"],
        "trust_bundle_bytes": expected_manifest["trust_bundle_bytes"],
        "runtime_root": expected_manifest["runtime_root"],
        "runtime_custody_root": expected_root,
        "status": expected_status,
        "started_at": receipt["provider_started_at"],
        "completed_at": receipt["provider_completed_at"],
        "duration_seconds": receipt["duration_seconds"],
        "tool_calls": validated["event_summary"]["tool_calls"],
        "attempt": 1,
        "timeout_seconds": 600,
    }
    if run != expected_run:
        raise CustodyError("ingested_run_not_bridge_generated")
    if expected_status == "completed":
        if not response_path.is_file() or response_path.is_symlink():
            raise CustodyError("ingested_response_missing")
        if response_path.read_bytes() != validated["response_raw"]:
            raise CustodyError("ingested_response_drift")
    elif response_path.exists():
        raise CustodyError("ingested_non_result_response_present")
    return {
        "run": run,
        "run_bytes": benchmark.byte_digest((run_dir / "run.json").read_bytes()),
        "response_bytes": (
            benchmark.byte_digest(response_path.read_bytes())
            if response_path.is_file()
            else None
        ),
        "runtime_custody_root": expected_root,
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
        raise CustodyError(f"fixed_denominator_incomplete:{len(observed)}/36")
    entries = []
    custody_roots = set()
    for run_id in sorted(expected):
        item = validate_ingested_run(runs_dir / run_id)
        if item["runtime_custody_root"] in custody_roots:
            raise CustodyError("runtime_custody_duplicate")
        custody_roots.add(item["runtime_custody_root"])
        entries.append(
            {
                "run_id": run_id,
                "family_id": item["run"]["family_id"],
                "condition": item["run"]["condition"],
                "run_bytes": item["run_bytes"],
                "response_bytes": item["response_bytes"],
                "runtime_custody_root": item["runtime_custody_root"],
            }
        )
    value = {
        "schema": "vela.inherited-correction-held-out-complete-custody.v1",
        "registration_root": state["preregistration"]["registration_root"],
        "assignment_root": state["freeze"]["assignment_root"],
        "runs": entries,
    }
    value["complete_custody_root"] = state["benchmark"].canonical_root(value)
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
        print(json.dumps(verify_prelaunch(), indent=2, sort_keys=True))
    else:
        print(ingest(args.capture_dir, args.runs_dir, args.run_id))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (CustodyError, OSError, KeyError, ValueError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(2) from error
