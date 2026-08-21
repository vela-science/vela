#!/usr/bin/env python3
"""Freeze the stopped pilot custody and the neutral one-shot canary."""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
IMAGE = "sha256:0ce56e0a4d72dc6ab26cdfcfc1d0280ac0c419dd687e26dda9312d4a09257285"


def data(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def digest(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def root(value: Any) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


def write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data(value))


def tree_manifest(directory: Path) -> list[dict[str, Any]]:
    result = []
    for path in sorted(item for item in directory.rglob("*") if item.is_file()):
        content = path.read_bytes()
        result.append(
            {
                "path": path.relative_to(directory).as_posix(),
                "bytes": len(content),
                "sha256": digest(content),
            }
        )
    return result


def freeze_pilot() -> str:
    pilot = ROOT / "pilot-capture"
    files = tree_manifest(pilot)
    if len(files) != 138:
        raise SystemExit(f"expected 138 retained pilot files, got {len(files)}")
    for entry in files:
        content = (pilot / entry["path"]).read_bytes()
        for forbidden in (b"access_token", b"refresh_token", b"id_token", b"OPENAI_API_KEY", b"Bearer "):
            if forbidden in content:
                raise SystemExit(f"credential-shaped bytes in {entry['path']}")
    manifest = {
        "schema": "vela.inherited-correction-pilot-capture-manifest.v1",
        "status": "calibration_only_stopped",
        "confirmatory_credit": False,
        "scoring_authorized": False,
        "started_runs": 6,
        "terminal_runs": 6,
        "next_run_started": False,
        "files": files,
    }
    manifest_root = root(manifest)
    write(ROOT / "pilot-capture-manifest.json", manifest)
    stop = {
        "schema": "vela.inherited-correction-runtime-stop.v1",
        "stopped_at": "2026-08-21T16:21:25.419666Z",
        "status": "stopped_before_run_07",
        "registration_root": "sha256:7391c3c6adb74633886fd9fb2d35a257e7501bd37153acfb3e19ac850d0e9157",
        "authorization_root": "sha256:8fd487199665a4bc19e83b9d86037002e817b7439f5e99adfd0ea91fa64f34da",
        "participant_configuration_root": "sha256:ba006bae3cb7eee61a37298512690a1e9aab9d3813d0d89f959e9f632981c9f2",
        "started_run_ids": [f"run-{index:02d}" for index in range(1, 7)],
        "unstarted_first_run_id": "run-07",
        "reason": "model_context_window and auto-compaction are live per-request controls, while Codex provider input_tokens is cumulative across repeated tool turns; the frozen cumulative-input invalidation rule did not measure the intended construct",
        "disposition": "all six runs retained unchanged as calibration-only non-results; no retry, scoring, denominator credit, or empirical-lift claim",
        "pilot_capture_manifest_root": manifest_root,
        "standing_or_authority_effect": "none",
    }
    write(ROOT / "pilot-stop-record.json", stop)
    return manifest_root


def freeze_canary(pilot_root: str) -> None:
    canary = ROOT / "neutral-canary"
    packet = canary / "packet"
    prompt = canary / "input" / "prompt.txt"
    prompt.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            "python3",
            str(ROOT / "container-runtime" / "prepare-prompt.py"),
            "--packet",
            str(packet),
            "--output",
            str(prompt),
            "--condition",
            "neutral-runtime-calibration",
        ],
        check=True,
    )
    packet_manifest = tree_manifest(packet)
    packet_root = root(packet_manifest)
    prompt_root = digest(prompt.read_bytes())
    schema_bytes = digest((packet / "response-schema.json").read_bytes())
    registration = {
        "schema": "vela.inherited-correction-neutral-canary-registration.v1",
        "status": "calibration_only_authorized",
        "study_registration_root": "sha256:7391c3c6adb74633886fd9fb2d35a257e7501bd37153acfb3e19ac850d0e9157",
        "pilot_capture_manifest_root": pilot_root,
        "purpose": "qualify the exact end-to-end runtime without study packet facts or scoring",
        "confirmatory_denominator_credit": False,
        "scientific_claim_credit": False,
        "provider": "openai-chatgpt-oauth-codex",
        "model": "gpt-5.6-sol",
        "reasoning_effort": "high",
        "service_tier": "default",
        "image_digest": IMAGE,
        "codex_cli_version": "0.149.0",
        "packet_root": packet_root,
        "prompt_root": prompt_root,
        "response_schema_bytes": schema_bytes,
        "attempt": 1,
        "timeout_seconds": 600,
        "output_token_ceiling": 8192,
        "tools": "none",
        "sessions": 1,
        "retries": 0,
    }
    registration_root = root(registration)
    write(canary / "registration.json", registration)
    configuration = {
        "schema": "vela.inherited-correction-oci-participant-configuration.v1",
        "registration_root": registration_root,
        "image_digest": IMAGE,
        "base_image_digest": "sha256:cadbfafeb6baf87eaaffa40b3640209c4b7fd38cebde65059d15bc39cd636b85",
        "codex_cli_version": "0.149.0",
        "authentication": "read-only ChatGPT OAuth auth.json mount into disposable CODEX_HOME",
        "model": "gpt-5.6-sol",
        "reasoning_effort": "high",
        "service_tier": "default",
        "prompt_root": prompt_root,
        "response_schema_bytes": schema_bytes,
        "one_prompt": True,
        "one_model_turn": True,
        "tools": "none",
        "store": "ephemeral",
        "timeout_seconds": 600,
        "output_token_ceiling": 8192,
        "provider_usage_disposition": "cost telemetry only; only genuine provider context/output-limit failure invalidates",
        "attempt": 1,
        "retries": 0,
    }
    configuration_root = root(configuration)
    write(canary / "input" / "participant-configuration.json", configuration)
    (canary / "input" / "response-schema.json").write_bytes((packet / "response-schema.json").read_bytes())
    assignment = {
        "schema": "vela.inherited-correction-neutral-canary-assignment.v1",
        "registration_root": registration_root,
        "image_digest": IMAGE,
        "assignments": [
            {
                "run_id": "neutral-canary-01",
                "condition": "neutral-runtime-calibration",
                "participant_instance_id": "neutral-oci-01",
                "packet_root": packet_root,
            }
        ],
    }
    assignment_root = root(assignment)
    write(canary / "input" / "assignment.json", assignment)
    authorization = {
        "schema": "vela.inherited-correction-neutral-canary-authorization.v1",
        "status": "authorized_calibration_only",
        "authorization_source": "user-authorized exact study runtime repair relayed by coordinator task 01a024dc-f015-7950-be0f-181931282ebc",
        "registration_root": registration_root,
        "participant_configuration_root": configuration_root,
        "assignment_root": assignment_root,
        "max_sessions": 1,
        "confirmatory_sessions_authorized": 0,
    }
    write(canary / "authorization.json", authorization)
    permit = {
        "schema": "vela.inherited-correction-launch-permit.v1",
        "status": "authorized",
        "expires_at": "2026-08-22T23:59:59Z",
        "registration_root": registration_root,
        "image_digest": IMAGE,
        "participant_configuration_root": configuration_root,
        "assignment_root": assignment_root,
        "run_id": "neutral-canary-01",
        "condition": "neutral-runtime-calibration",
        "participant_instance_id": "neutral-oci-01",
        "prompt_root": prompt_root,
        "packet_root": packet_root,
        "attempt": 1,
    }
    write(canary / "permit-template" / "neutral-canary-01.permit.json", permit)
    write(
        canary / "permit-template" / "hold-state.default.json",
        {"schema": "vela.inherited-correction-hold.v1", "status": "hold", "reason": "default; no launch without an exact frozen release", "updated_at": "2026-08-21T16:21:25.419666Z"},
    )
    write(
        canary / "permit-template" / "hold-state.json",
        {"schema": "vela.inherited-correction-hold.v1", "status": "release", "reason": "one neutral calibration canary only; confirmatory study remains held", "updated_at": "2026-08-21T16:21:25.419666Z"},
    )
    manifest = {
        "schema": "vela.inherited-correction-neutral-canary-freeze.v1",
        "status": "frozen_prelaunch_0_of_1",
        "registration_root": registration_root,
        "participant_configuration_root": configuration_root,
        "assignment_root": assignment_root,
        "authorization_root": root(authorization),
        "permit_root": root(permit),
        "image_digest": IMAGE,
        "packet_root": packet_root,
        "prompt_root": prompt_root,
        "files": [
            entry
            for entry in tree_manifest(canary)
            if entry["path"] != "prelaunch-freeze.json"
        ],
        "confirmatory_status": "stopped_0_of_16_replacement_not_registered",
    }
    write(canary / "prelaunch-freeze.json", manifest)
    print(json.dumps({key: manifest[key] for key in ("registration_root", "participant_configuration_root", "assignment_root", "authorization_root", "permit_root", "image_digest", "packet_root", "prompt_root")}, indent=2, sort_keys=True))


if __name__ == "__main__":
    freeze_canary(freeze_pilot())
