#!/usr/bin/env python3
"""Freeze the distinct prospective runtime-repair canary-02."""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
RUNTIME = ROOT / "container-runtime"
SOURCE_CANARY = ROOT / "neutral-canary"
OUTPUT = ROOT / "neutral-canary-02"
IMAGE = "sha256:13b753749787d68d628cea899f6b9875c0fc51c43877599b9aabf2009fe83388"
BASE_IMAGE = "sha256:cadbfafeb6baf87eaaffa40b3640209c4b7fd38cebde65059d15bc39cd636b85"


def encoded(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def digest(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def canonical_root(value: Any) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


def write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded(value))


def tree_manifest(directory: Path, excluded: set[str] | None = None) -> list[dict[str, Any]]:
    excluded = excluded or set()
    files = []
    for path in sorted(item for item in directory.rglob("*") if item.is_file()):
        relative = path.relative_to(directory).as_posix()
        if relative in excluded or "/node_modules/" in f"/{relative}/":
            continue
        content = path.read_bytes()
        files.append({"path": relative, "bytes": len(content), "sha256": digest(content)})
    return files


def command_bytes(arguments: list[str]) -> bytes:
    result = subprocess.run(arguments, check=True, capture_output=True)
    if result.stderr:
        raise SystemExit(f"unexpected stderr from {arguments!r}")
    return result.stdout


def main() -> int:
    if OUTPUT.exists():
        raise SystemExit(f"refusing to overwrite existing {OUTPUT}")
    shutil.copytree(SOURCE_CANARY / "packet", OUTPUT / "packet")
    (OUTPUT / "input").mkdir(parents=True)
    subprocess.run(
        [
            "python3",
            str(RUNTIME / "prepare-prompt.py"),
            "--packet",
            str(OUTPUT / "packet"),
            "--output",
            str(OUTPUT / "input/prompt.txt"),
            "--condition",
            "neutral-runtime-calibration",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
    )

    with tempfile.TemporaryDirectory() as temporary:
        temporary_path = Path(temporary)
        for mode in ("corrected", "legacy"):
            source = temporary_path / mode
            source.mkdir()
            subprocess.run(
                [str(RUNTIME / "preflight-config.sh"), mode, IMAGE, str(source)],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            shutil.copytree(source, OUTPUT / "offline-preflight" / mode)

    cli_evidence = OUTPUT / "cli-evidence"
    cli_evidence.mkdir()
    (cli_evidence / "version.txt").write_bytes(
        command_bytes(["docker", "run", "--rm", "--tmpfs", "/codex-home:rw,nosuid,size=16m,uid=10001,gid=10001", "--entrypoint", "codex", IMAGE, "--version"])
    )
    (cli_evidence / "features-list.txt").write_bytes(
        command_bytes(["docker", "run", "--rm", "--tmpfs", "/codex-home:rw,nosuid,size=16m,uid=10001,gid=10001", "--entrypoint", "codex", IMAGE, "features", "list"])
    )
    help_raw = command_bytes(
        ["docker", "run", "--rm", "--tmpfs", "/codex-home:rw,nosuid,size=16m,uid=10001,gid=10001", "--entrypoint", "codex", IMAGE, "--help"]
    )
    help_normalized = b"\n".join(line.rstrip() for line in help_raw.splitlines()) + b"\n"
    (cli_evidence / "help.txt").write_bytes(help_normalized)
    (cli_evidence / "help-raw.sha256").write_text(digest(help_raw) + "\n")
    strict_overrides = json.loads(
        command_bytes(
            [
                "node",
                "--input-type=module",
                "-e",
                f"import {{STRICT_OVERRIDES}} from {json.dumps((RUNTIME / 'strict-config.mjs').as_uri())}; process.stdout.write(JSON.stringify(STRICT_OVERRIDES))",
            ]
        )
    )
    strict_overrides_root = canonical_root(strict_overrides)
    packet_manifest = tree_manifest(OUTPUT / "packet")
    packet_root = canonical_root(packet_manifest)
    prompt_root = digest((OUTPUT / "input/prompt.txt").read_bytes())
    schema_bytes = digest((OUTPUT / "packet/response-schema.json").read_bytes())
    expected_response = {
        "schema": "neutral.runtime-canary-response.v1",
        "identifiers": ["amber", "birch", "cobalt", "dune"],
        "sum": 42,
        "binding": "sha256:4f951558bcd35270879e2551c2ae91b715b8793bbb0498a266db7381ca88cc4f",
    }
    expected_response_root = canonical_root(expected_response)
    preflight_root = canonical_root(tree_manifest(OUTPUT / "offline-preflight"))
    cli_evidence_root = canonical_root(tree_manifest(cli_evidence))
    runtime_source_root = canonical_root(tree_manifest(RUNTIME))
    amendment = {
        "schema": "vela.inherited-correction-neutral-canary-amendment.v1",
        "status": "prospective_before_canary_02",
        "parent_canary_commit": "2c7aed112f9701b58b8373c156c244b68e616886",
        "parent_canary_result_root": "sha256:d1a792b3085c232affb0038924a54fc109a34154cbf0f2363258d0f3b86635ee",
        "parent_canary_disposition": "terminal calibration evidence; unchanged; no retry",
        "defect": "Codex CLI 0.149.0 strict parser rejects tools.view_image",
        "repair": "replace only with supported features.view_image=false and bind all supported disables to one tested override list",
        "residual_tool_boundary": "0.149.0 exposes no supported configuration key that guarantees removal of every residual class such as apply_patch; the empty read-only workdir and streaming harness terminate on the first tool, continuation, or compaction event",
        "official_config_reference": "https://developers.openai.com/codex/config-reference",
        "local_cli_evidence_root": cli_evidence_root,
        "offline_preflight_root": preflight_root,
        "runtime_source_root": runtime_source_root,
        "confirmatory_effect": "none; replacement registration remains unfrozen and 0/16",
    }
    amendment_root = canonical_root(amendment)
    write(OUTPUT / "amendment.json", amendment)
    registration = {
        "schema": "vela.inherited-correction-neutral-canary-registration.v2",
        "status": "calibration_only_authorized",
        "amendment_root": amendment_root,
        "study_registration_root": "sha256:7391c3c6adb74633886fd9fb2d35a257e7501bd37153acfb3e19ac850d0e9157",
        "purpose": "qualify corrected strict parsing and exact one-response runtime without study facts or scoring",
        "confirmatory_denominator_credit": False,
        "scientific_claim_credit": False,
        "provider": "openai-chatgpt-oauth-codex",
        "model": "gpt-5.6-sol",
        "reasoning_effort": "high",
        "service_tier": "default",
        "image_digest": IMAGE,
        "base_image_digest": BASE_IMAGE,
        "codex_cli_version": "0.149.0",
        "packet_root": packet_root,
        "prompt_root": prompt_root,
        "response_schema_bytes": schema_bytes,
        "expected_response_root": expected_response_root,
        "strict_overrides_root": strict_overrides_root,
        "offline_preflight_root": preflight_root,
        "attempt": 1,
        "timeout_seconds": 600,
        "output_token_ceiling": 8192,
        "tools": "none tolerated",
        "sessions": 1,
        "retries": 0,
    }
    registration_root = canonical_root(registration)
    write(OUTPUT / "registration.json", registration)
    configuration = {
        "schema": "vela.inherited-correction-oci-participant-configuration.v2",
        "registration_root": registration_root,
        "image_digest": IMAGE,
        "base_image_digest": BASE_IMAGE,
        "codex_cli_version": "0.149.0",
        "authentication": "read-only ChatGPT OAuth auth.json mount into disposable CODEX_HOME",
        "model": "gpt-5.6-sol",
        "reasoning_effort": "high",
        "service_tier": "default",
        "prompt_root": prompt_root,
        "response_schema_bytes": schema_bytes,
        "expected_response_root": expected_response_root,
        "strict_overrides_root": strict_overrides_root,
        "strict_overrides": strict_overrides,
        "one_prompt": True,
        "one_model_turn": True,
        "tools": "none",
        "tool_boundary": "supported disables plus immediate streaming abort and terminal failure on any tool event",
        "workdir": "empty read-only participant workdir",
        "store": "ephemeral",
        "timeout_seconds": 600,
        "output_token_ceiling": 8192,
        "provider_usage_disposition": "cost telemetry only; only genuine provider context/output-limit failure invalidates",
        "attempt": 1,
        "retries": 0,
    }
    configuration_root = canonical_root(configuration)
    write(OUTPUT / "input/participant-configuration.json", configuration)
    (OUTPUT / "input/response-schema.json").write_bytes(
        (OUTPUT / "packet/response-schema.json").read_bytes()
    )
    assignment = {
        "schema": "vela.inherited-correction-neutral-canary-assignment.v2",
        "registration_root": registration_root,
        "image_digest": IMAGE,
        "assignments": [
            {
                "run_id": "neutral-canary-02",
                "condition": "neutral-runtime-calibration",
                "participant_instance_id": "neutral-oci-02",
                "packet_root": packet_root,
            }
        ],
    }
    assignment_root = canonical_root(assignment)
    write(OUTPUT / "input/assignment.json", assignment)
    authorization = {
        "schema": "vela.inherited-correction-neutral-canary-authorization.v2",
        "status": "authorized_calibration_only",
        "authorization_source": "coordinator task 01a024dc-f015-7950-be0f-181931282ebc prospective canary-02 authorization",
        "registration_root": registration_root,
        "participant_configuration_root": configuration_root,
        "assignment_root": assignment_root,
        "max_sessions": 1,
        "confirmatory_sessions_authorized": 0,
    }
    authorization_root = canonical_root(authorization)
    write(OUTPUT / "authorization.json", authorization)
    permit = {
        "schema": "vela.inherited-correction-launch-permit.v1",
        "status": "authorized",
        "expires_at": "2026-08-22T23:59:59Z",
        "registration_root": registration_root,
        "image_digest": IMAGE,
        "participant_configuration_root": configuration_root,
        "assignment_root": assignment_root,
        "run_id": "neutral-canary-02",
        "condition": "neutral-runtime-calibration",
        "participant_instance_id": "neutral-oci-02",
        "prompt_root": prompt_root,
        "packet_root": packet_root,
        "attempt": 1,
    }
    permit_root = canonical_root(permit)
    write(OUTPUT / "permit-template/neutral-canary-02.permit.json", permit)
    write(
        OUTPUT / "permit-template/hold-state.default.json",
        {"schema": "vela.inherited-correction-hold.v1", "status": "hold", "reason": "default; no launch without exact frozen release", "updated_at": "2026-08-21T16:46:26Z"},
    )
    write(
        OUTPUT / "permit-template/hold-state.json",
        {"schema": "vela.inherited-correction-hold.v1", "status": "release", "reason": "one distinct neutral calibration canary-02 only; confirmatory study remains held", "updated_at": "2026-08-21T16:46:26Z"},
    )
    freeze = {
        "schema": "vela.inherited-correction-neutral-canary-freeze.v2",
        "status": "frozen_prelaunch_0_of_1",
        "amendment_root": amendment_root,
        "registration_root": registration_root,
        "participant_configuration_root": configuration_root,
        "assignment_root": assignment_root,
        "authorization_root": authorization_root,
        "permit_root": permit_root,
        "image_digest": IMAGE,
        "packet_root": packet_root,
        "prompt_root": prompt_root,
        "expected_response_root": expected_response_root,
        "strict_overrides_root": strict_overrides_root,
        "offline_preflight_root": preflight_root,
        "files": tree_manifest(OUTPUT, {"prelaunch-freeze.json"}),
        "canary_01_status": "terminal_failed_closed_unchanged",
        "confirmatory_status": "stopped_0_of_16_replacement_not_registered",
    }
    write(OUTPUT / "prelaunch-freeze.json", freeze)
    print(json.dumps({key: freeze[key] for key in freeze if key.endswith("_root") or key == "image_digest"}, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
