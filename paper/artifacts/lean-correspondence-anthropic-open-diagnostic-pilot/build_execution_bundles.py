"""Materialize the six reviewed held bundles without provider contact."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parent
VELA = ROOT.parents[2]
RUNTIME = ROOT.parent / "lean-correspondence-stage-a-runtime-qualification"
EVIDENCE_SOURCES = ROOT / "evidence-sources"
DEFAULT_BASE = Path("/private/tmp/vela-stage-a-runtime-qualification-bundle-v1")
DEFAULT_OUTPUT = Path("/private/tmp/vela-anthropic-open-diagnostic-held-v2")
QUALIFIER = Path(
    "/private/tmp/vela-stage-a-runtime-qualification-maintained-v2/"
    "tools/evidence_qualification/qualification.py"
)
QUALIFIER_SOURCE = VELA / "tools/evidence_qualification/qualification.py"
QUALIFIER_PYTHON = Path(
    "/private/tmp/vela-stage-a-runtime-qualification-python-v1/.venv/bin/python"
)
ISSUED_AT = "2026-08-22T00:00:00Z"
EMPTY_SHA256 = "sha256:" + hashlib.sha256(b"").hexdigest()


def load(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )


def canonical(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode()


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical_root(value: Any) -> str:
    return digest(canonical(value) + b"\n")


def load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ValueError(f"module unavailable: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def _tool_boundary(
    target: Path, qualifier: Any, evidence: dict[str, Any]
) -> tuple[dict[str, Any], dict[str, Any]]:
    input_schema = qualifier.TOOL_INPUT_SCHEMAS[("read_file", "2")]
    boundary = {
        "api_version": "messages/v1",
        "lifecycle": [
            "thread.started",
            "turn.started",
            "tool.call",
            "tool.result",
            "item.completed",
            "turn.completed",
        ],
        "max_output_bytes": 65536,
        "max_tool_calls": 64,
        "mode": "read_only_offline_files",
        "mounts": [
            {
                "content_root": evidence["mount_content_root"],
                "read_only": True,
                "source": str((target / "workspace").resolve()),
                "target": "/workspace",
            }
        ],
        "network": False,
        "per_call_timeout_seconds": 30,
        "provider_adapter": "anthropic-messages-v1",
        "provider_organization": "anthropic",
        "schema": "vela.tooling.read-only-offline-tool-boundary.v2",
        "shell_interpolation": False,
        "tool_protocol_version": "offline-files/2",
        "tools": [
            {
                "allowed_argv": [],
                "file_roots": ["/workspace"],
                "input_schema": input_schema,
                "name": "read_file",
                "operations": ["read", "list", "stat", "search"],
                "version": "2",
            }
        ],
        "writes": False,
    }
    summary = qualifier.validate_tool_boundary(boundary)
    if summary["workspace_content_root"] != evidence["workspace_content_root"]:
        raise ValueError("workspace root derivation drift")
    return boundary, summary


def _build_preflight_bridge() -> tuple[tempfile.TemporaryDirectory[str], Path]:
    temporary = tempfile.TemporaryDirectory(
        prefix="vela-anthropic-diagnostic-bridge-preflight."
    )
    root = Path(temporary.name)
    outputs = []
    for ordinal in ("a", "b"):
        cache = root / f"cache-{ordinal}"
        output = root / f"bridge-{ordinal}"
        subprocess.run(
            [
                "go",
                "build",
                "-mod=readonly",
                "-trimpath",
                "-buildvcs=false",
                "-ldflags",
                "-s -w -buildid= -X main.providerAdapter=anthropic-messages-v1",
                "-o",
                str(output),
                "./cmd/bridge",
            ],
            cwd=RUNTIME / "runtime-runner",
            env={
                **os.environ,
                "CGO_ENABLED": "0",
                "GOCACHE": str(cache / "go-build"),
                "GOMODCACHE": str(cache / "go-mod"),
            },
            check=True,
        )
        outputs.append(output)
    if outputs[0].read_bytes() != outputs[1].read_bytes():
        temporary.cleanup()
        raise ValueError("independent bridge preflight builds differ")
    subprocess.run([str(outputs[0]), "--self-test"], check=True)
    return temporary, outputs[0]


def _rewrite_neutral_fixture(
    target: Path,
    qualifier: Any,
    config: dict[str, Any],
    boundary: dict[str, Any],
    evidence_manifest_root: str,
    workspace_preflight_root: str,
) -> None:
    fixture = target / "fixture"
    evidence_dir = fixture / "evidence"
    identity = config["neutral_fixture"]["identity"]
    identity.update(
        {
            "evidence_manifest_root": evidence_manifest_root,
            "tool_boundary_root": boundary["tool_boundary_root"],
            "tool_policy_root": boundary["tool_policy_root"],
            "workspace_content_root": boundary["workspace_content_root"],
            "workspace_preflight_root": workspace_preflight_root,
        }
    )
    configuration = config["configuration"]
    configuration_root = canonical_root(
        {
            "configuration": configuration,
            "tool_policy_root": boundary["tool_policy_root"],
        }
    )
    runtime = config["runtime"]
    image_receipt = load(target / runtime["oci_receipts"][0])
    runtime_source_root = image_receipt["source_root"]
    image_digest = image_receipt["image_digest"]
    registered_bytes = digest((target / config["schemas"]["registered"]).read_bytes())
    provider_bytes = digest((target / config["schemas"]["provider"]).read_bytes())
    common_permit = {
        **identity,
        "attempt": 1,
        "configuration_root": configuration_root,
        "image_digest": image_digest,
        "provider_schema_bytes": provider_bytes,
        "registered_schema_bytes": registered_bytes,
        "runner_version": configuration["runner_version"],
        "runtime_source_root": runtime_source_root,
        "timeout_seconds": configuration["timeout_seconds"],
    }
    template_path = target / config["neutral_fixture"]["permit_template"]
    consumed_path = target / config["neutral_fixture"]["consumed_permit"]
    template = load(template_path)
    consumed = load(consumed_path)
    template = {**template, **common_permit, "status": "held", "consumed_at": None}
    consumed = {
        **consumed,
        **common_permit,
        "status": "consumed",
        "consumed_at": consumed["consumed_at"],
    }
    write(template_path, template)
    write(consumed_path, consumed)
    permit_bytes = digest(consumed_path.read_bytes())

    stdout_path = evidence_dir / "tool.stdout"
    stderr_path = evidence_dir / "tool.stderr"
    stdout_path.write_bytes(
        (target / "workspace/assignment-manifest.json").read_bytes()
    )
    stderr_path.write_bytes(b"")
    arguments = {
        "operation": "read",
        "path": "/workspace/assignment-manifest.json",
        "query": "",
    }
    receipt = {
        "arguments": arguments,
        "arguments_root": canonical_root(arguments),
        "call_id": "call-01",
        "completed_at": "2026-08-21T00:00:00.500000Z",
        "exit_code": 0,
        "network_disabled": True,
        "schema": "vela.tooling.read-only-tool-receipt.v1",
        "started_at": "2026-08-21T00:00:00.300000Z",
        "stderr": "fixture/evidence/tool.stderr",
        "stderr_bytes": 0,
        "stderr_sha256": EMPTY_SHA256,
        "stdout": "fixture/evidence/tool.stdout",
        "stdout_bytes": len(stdout_path.read_bytes()),
        "stdout_sha256": digest(stdout_path.read_bytes()),
        "timeout_seconds": boundary["per_call_timeout_seconds"],
        "tool_name": "read_file",
        "writes_disabled": True,
    }
    receipt_root = canonical_root(receipt)
    write(evidence_dir / "tool-receipts.json", [receipt])

    old_events = [
        json.loads(line)
        for line in (evidence_dir / "provider-events.jsonl").read_text().splitlines()
    ]
    call = old_events[2]
    call["item"] = {
        "arguments": arguments,
        "call_id": "call-01",
        "tool_name": "read_file",
        "type": "tool_call",
    }
    result = old_events[3]
    result["item"] = {
        "call_id": "call-01",
        "exit_code": 0,
        "receipt_root": receipt_root,
        "stderr_bytes": 0,
        "stderr_sha256": EMPTY_SHA256,
        "stdout_bytes": receipt["stdout_bytes"],
        "stdout_sha256": receipt["stdout_sha256"],
        "tool_name": "read_file",
        "type": "tool_result",
    }
    event_lines = [canonical(event) + b"\n" for event in old_events]
    events_raw = b"".join(event_lines)
    (evidence_dir / "provider-events.jsonl").write_bytes(events_raw)
    raw_types = [
        "message_start",
        "message_delta.start",
        "content_block_stop.tool_use",
        "runner.tool_result",
        "content_block_stop.text",
        "message_stop",
    ]
    raw_events = [
        {
            "normalized_event_bytes": digest(line),
            "provider_adapter": "anthropic-messages-v1",
            "provider_event_type": raw_types[index],
            "provider_payload": {"opaque_fixture_sequence": index},
            "schema": "vela.tooling.raw-provider-event.v1",
            "sequence": index,
        }
        for index, line in enumerate(event_lines)
    ]
    raw_provider_bytes = b"".join(canonical(event) + b"\n" for event in raw_events)
    (evidence_dir / "provider-events.raw.jsonl").write_bytes(raw_provider_bytes)
    normalized_semantics = canonical_root(
        [
            {
                "arguments": arguments,
                "exit_code": 0,
                "stderr_bytes": 0,
                "stderr_sha256": EMPTY_SHA256,
                "stdout_bytes": receipt["stdout_bytes"],
                "stdout_sha256": receipt["stdout_sha256"],
                "tool_name": "read_file",
            }
        ]
    )
    equivalence_path = target / config["provider_equivalence"]
    equivalence = load(equivalence_path)
    for provider in equivalence["providers"]:
        provider["normalized_tool_semantics_root"] = normalized_semantics
        provider["participant_visible_atoms_root"] = identity["packet_root"]
        provider["registered_schema_bytes"] = registered_bytes
        provider["tool_semantics_root"] = boundary["tool_policy_root"]
        if provider["provider_adapter"] == "anthropic-messages-v1":
            provider["normalized_events_bytes"] = digest(events_raw)
            provider["raw_provider_events_bytes"] = digest(raw_provider_bytes)
            provider["tool_boundary_root"] = boundary["tool_boundary_root"]
            provider["tool_receipts_root"] = canonical_root([receipt])
    write(equivalence_path, equivalence)

    launch_path = target / config["neutral_fixture"]["launch"]
    launch = load(launch_path)
    launch.update(
        {
            "configuration_root": configuration_root,
            "evidence_manifest_root": evidence_manifest_root,
            "image_digest": image_digest,
            "permit_bytes": permit_bytes,
            "provider_adapter": "anthropic-messages-v1",
            "runtime_source_root": runtime_source_root,
            "tool_boundary_root": boundary["tool_boundary_root"],
            "tool_policy_root": boundary["tool_policy_root"],
            "workspace_content_root": boundary["workspace_content_root"],
            "workspace_preflight_root": workspace_preflight_root,
        }
    )
    write(launch_path, launch)
    launch_bytes = digest(launch_path.read_bytes())
    teardown_path = target / config["neutral_fixture"]["teardown_receipt"]
    teardown = load(teardown_path)
    teardown.update(
        {
            "evidence_manifest_root": evidence_manifest_root,
            "launch_bytes": launch_bytes,
            "permit_bytes": permit_bytes,
            "provider_adapter": "anthropic-messages-v1",
            "tool_boundary_root": boundary["tool_boundary_root"],
            "tool_policy_root": boundary["tool_policy_root"],
            "workspace_content_root": boundary["workspace_content_root"],
            "workspace_preflight_root": workspace_preflight_root,
        }
    )
    write(teardown_path, teardown)
    terminal_path = target / config["neutral_fixture"]["terminal_receipt"]
    terminal = load(terminal_path)
    terminal.update(
        {
            "configuration_root": configuration_root,
            "evidence_manifest_root": evidence_manifest_root,
            "image_digest": image_digest,
            "launch_bytes": launch_bytes,
            "permit_bytes": permit_bytes,
            "provider_adapter": "anthropic-messages-v1",
            "provider_events_bytes": digest(events_raw),
            "raw_provider_events_bytes": digest(raw_provider_bytes),
            "raw_provider_events_root": canonical_root(raw_events),
            "runtime_source_root": runtime_source_root,
            "teardown_receipt_bytes": digest(teardown_path.read_bytes()),
            "tool_boundary_root": boundary["tool_boundary_root"],
            "tool_policy_root": boundary["tool_policy_root"],
            "tool_receipts_bytes": digest(
                (evidence_dir / "tool-receipts.json").read_bytes()
            ),
            "tool_receipts_root": canonical_root([receipt]),
            "workspace_content_root": boundary["workspace_content_root"],
            "workspace_preflight_root": workspace_preflight_root,
        }
    )
    write(terminal_path, terminal)
    capture_paths = [
        consumed_path,
        launch_path,
        evidence_dir / "provider-events.jsonl",
        evidence_dir / "provider-events.raw.jsonl",
        evidence_dir / "provider-stderr.txt",
        evidence_dir / "response.raw.json",
        teardown_path,
        terminal_path,
        evidence_dir / "tool-receipts.json",
        stdout_path,
        stderr_path,
    ]
    entries = [
        {
            "bytes": len(path.read_bytes()),
            "path": path.relative_to(fixture).as_posix(),
            "sha256": digest(path.read_bytes()),
        }
        for path in sorted(capture_paths)
    ]
    manifest = {
        "entries": entries,
        "schema": "vela.tooling.neutral-capture-manifest.v1",
    }
    manifest["capture_root"] = canonical_root(manifest)
    write(target / config["neutral_fixture"]["capture_manifest"], manifest)


def stage_cell(
    base: Path,
    target: Path,
    row: dict[str, Any],
    qualifier: Any,
    preflight_bridge: Path,
) -> dict[str, Any]:
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(base, target)
    config_path = target / "qualification.json"
    config = load(config_path)
    run_id = row["cell_id"]
    evidence_module = load_module("diagnostic_evidence_tree", ROOT / "evidence_tree.py")
    evidence = evidence_module.materialize(
        cell_id=run_id,
        packet_path=ROOT / row["execution_packet_path"],
        source_root=EVIDENCE_SOURCES,
        destination=target / "workspace",
    )
    boundary_value, boundary = _tool_boundary(target, qualifier, evidence)
    write(target / "config/tool-boundary.json", boundary_value)
    bridge_result = subprocess.run(
        [str(preflight_bridge), "--validate-workspace", str(target / "workspace")],
        check=True,
        capture_output=True,
    )
    bridge_receipt = json.loads(bridge_result.stdout)
    bound_preflight_path = (
        target / "execution/offline-evidence/workspace-bridge-preflight.json"
    )
    write(
        bound_preflight_path,
        {
            "bridge_receipt": bridge_receipt,
            "evidence_manifest_root": evidence["evidence_manifest_root"],
            "schema": "vela.anthropic-offline-workspace-bound-preflight.v1",
            "status": "pass",
            "tool_boundary_root": boundary["tool_boundary_root"],
            "tool_policy_root": boundary["tool_policy_root"],
            "workspace_content_root": boundary["workspace_content_root"],
        },
    )
    workspace_preflight_root = digest(bound_preflight_path.read_bytes())
    config["configuration"].update(
        {
            "strict_arguments": [
                "approval_policy=never",
                "web_search=disabled",
                "tools=read_only_offline_files",
            ],
            "tools": "read_only_offline_files",
        }
    )
    configuration_root = canonical_root(
        {
            "configuration": config["configuration"],
            "tool_policy_root": boundary["tool_policy_root"],
        }
    )
    identity = {
        "registration_id": "anthropic-open-diagnostic-registration-v2",
        "assignment_id": row["cell_id"],
        "participant_id": row["participant_id"],
        "run_id": run_id,
        "condition": row["arm"],
        "prompt_root": row["prompt_root"],
        "packet_root": row["execution_packet_root"],
        "tool_boundary_root": boundary["tool_boundary_root"],
        "tool_policy_root": boundary["tool_policy_root"],
        "workspace_content_root": boundary["workspace_content_root"],
        "evidence_manifest_root": evidence["evidence_manifest_root"],
        "workspace_preflight_root": workspace_preflight_root,
    }
    permit_relative = f"permit/{run_id}.permit.json"
    old_permit = target / config["participant_permit"]["permit"]
    permit = load(old_permit)
    permit.update(identity)
    permit.update(
        {
            "attempt": 1,
            "configuration_root": configuration_root,
            "issued_at": ISSUED_AT,
            "status": "held",
            "consumed_at": None,
            "timeout_seconds": 1200,
        }
    )
    new_permit = target / permit_relative
    write(new_permit, permit)
    if old_permit != new_permit:
        old_permit.unlink()
    hold_path = target / config["participant_permit"]["hold"]
    hold = load(hold_path)
    hold.update(
        {
            "registration_id": identity["registration_id"],
            "assignment_id": identity["assignment_id"],
        }
    )
    write(hold_path, hold)
    config["participant_permit"] = {
        "hold": config["participant_permit"]["hold"],
        "permit": permit_relative,
        "consumed_permit": f"permit/{run_id}.permit.consumed.json",
        "identity": identity,
        "workspace_preflight": "execution/offline-evidence/workspace-bridge-preflight.json",
    }
    config["runtime"]["mounts"] = [
        {
            "source": str((target / "schemas").resolve()),
            "target": "/input",
            "read_only": True,
        },
        {
            "source": str((target / "workspace").resolve()),
            "target": "/workspace",
            "read_only": True,
        },
        {
            "source": str(
                (target / "runtime/source/vendor/ca-certificates.crt").resolve()
            ),
            "target": "/etc/ssl/certs/ca-certificates.crt",
            "read_only": True,
        },
    ]
    config["self_verification"] = {
        "command": [
            str(QUALIFIER_PYTHON),
            str(QUALIFIER),
            "--bundle",
            str(target.resolve()),
        ],
        "qualifier_sha256": config["self_verification"]["qualifier_sha256"],
        "environment_prefix": str(QUALIFIER_PYTHON.parent.parent),
        "jsonschema_module": config["self_verification"]["jsonschema_module"],
    }
    config["self_verification"]["qualifier_sha256"] = digest(QUALIFIER.read_bytes())
    write(config_path, config)

    compatibility_path = target / config["configuration"]["compatibility_receipt"]
    compatibility = load(compatibility_path)
    compatibility.update(
        {
            "accepted_arguments": config["configuration"]["strict_arguments"],
            "configuration_root": configuration_root,
            "provider_adapter": "anthropic-messages-v1",
            "tool_boundary_root": boundary["tool_boundary_root"],
            "tool_policy_root": boundary["tool_policy_root"],
            "workspace_content_root": boundary["workspace_content_root"],
        }
    )
    write(compatibility_path, compatibility)
    _rewrite_neutral_fixture(
        target,
        qualifier,
        config,
        boundary,
        evidence["evidence_manifest_root"],
        workspace_preflight_root,
    )
    write(config_path, config)

    input_dir = target / "execution/input"
    evidence_dir = target / "execution/offline-evidence"
    input_dir.mkdir(parents=True, exist_ok=True)
    evidence_dir.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(
        target / "schemas/provider.json", input_dir / "provider-schema.json"
    )
    shutil.copyfile(ROOT / row["prompt_path"], input_dir / "prompt.txt")
    shutil.copyfile(ROOT / row["execution_packet_path"], input_dir / "packet.json")
    materializer = RUNTIME / "run_input_materialize.py"
    subprocess.run(
        [
            sys.executable,
            str(materializer),
            "--schema",
            str(input_dir / "provider-schema.json"),
            "--run",
            str(input_dir / "run.json"),
            "--receipt",
            str(input_dir / "materialization-receipt.json"),
            "--run-id",
            run_id,
            "--model",
            "claude-opus-5",
            "--prompt-file",
            str(input_dir / "prompt.txt"),
            "--packet-file",
            str(input_dir / "packet.json"),
        ],
        check=True,
    )
    return identity


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-bundle", type=Path, default=DEFAULT_BASE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--skip-preflight", action="store_true")
    args = parser.parse_args()
    base = args.base_bundle.resolve()
    output = args.output.resolve()
    QUALIFIER.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(QUALIFIER_SOURCE, QUALIFIER)
    if not base.is_dir() or not QUALIFIER.is_file() or not QUALIFIER_PYTHON.is_file():
        raise ValueError(
            "maintained base bundle or fixed qualifier environment missing"
        )
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    schedule = load(ROOT / "assignment-schedule.json")
    offline = load_module("diagnostic_offline_qualify", RUNTIME / "offline_qualify.py")
    sys.path.insert(0, str(QUALIFIER.parents[3]))
    qualifier = load_module("diagnostic_maintained_qualifier", QUALIFIER)
    receipts = []
    bridge_temporary, preflight_bridge = _build_preflight_bridge()
    try:
        for row in schedule["rows"]:
            target = output / row["cell_id"]
            identity = stage_cell(base, target, row, qualifier, preflight_bridge)
            if not args.skip_preflight:
                launchability = offline.verify_launchable_image(
                    (target / "runtime/a.oci.tar").read_bytes(),
                    "anthropic-messages-v1",
                    qualifier,
                    target / "execution/input",
                    target / "execution/offline-evidence",
                )
                write(target / "execution/launchability.json", launchability)
            result = subprocess.run(
                [str(QUALIFIER_PYTHON), str(QUALIFIER), "--bundle", str(target)],
                check=True,
                capture_output=True,
            )
            receipt = json.loads(result.stdout)
            write(target / "execution/qualification-receipt.json", receipt)
            receipts.append(
                {
                    "cell_id": row["cell_id"],
                    "identity": identity,
                    "participant_permit_root": receipt["participant_permit_root"],
                    "qualification_root": receipt["qualification_root"],
                    "status": receipt["status"],
                }
            )
    finally:
        bridge_temporary.cleanup()
    write(
        output / "bundle-index.json",
        {
            "cells": receipts,
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-held-bundle-index.v2",
            "status": "held_offline_qualified",
        },
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
