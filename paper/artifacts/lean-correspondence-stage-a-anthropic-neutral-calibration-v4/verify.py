#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import json
import re
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

PACKAGE = Path(__file__).resolve().parent
REPO = PACKAGE.parents[2]
RUNTIME = PACKAGE.parent / "lean-correspondence-stage-a-runtime-qualification"
STAGE_A = PACKAGE.parent / "lean-correspondence-stage-a-open-pilot"
STOPPED_V2 = (
    PACKAGE.parent / "lean-correspondence-stage-a-anthropic-neutral-calibration"
)
STOPPED_V3 = (
    PACKAGE.parent / "lean-correspondence-stage-a-anthropic-neutral-calibration-v3"
)
ARTIFACT_ROOT = PACKAGE / "artifact-root.json"
PRODUCER = "0aee2129f2f7824f328d9576f72e42f240e08932"
PRODUCER_TREE = "e8929efa3559e6abd552a66bbd5e023d9461fad6"
RUNTIME_ARTIFACT_ROOT = (
    "sha256:57d49f290bcecb665b004ec54399361142b83590ed40d7291b8aabe00c8c0a2e"
)
OFFLINE_ROOT = "sha256:7a89479a46e004317cc69b78ffa1ea0c5fe7130a65c257de1dd43c9e31d6578e"
REGISTRATION_ROOT = (
    "sha256:2ddcd97a0dfff125ac88a6c102e58a0f380c929c6bc243a8e8298eb742dc6ef3"
)
RUN_ID = "neutral-calibration-anthropic-json-v4-lossless"
PERMIT_ROOT = "sha256:dfc9f20e029b7ea51eb28c6b3d81f70eace063c681d56d2c9ce7356b3dbe8b63"
REQUEST_ROOT = "sha256:cf67944d1872244c9d89ed3f7ad9cc27c3a37a4deba665f47a939985e2c62e8c"
FRAME_ROOT = "sha256:6ba0a6cd840dc28d8dbba2f9b019d0e37cee109924593a328c173f38000cf074"
SCHEMA_ROOT = "sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7"
PACKET_ROOT = "sha256:a38b18fb6284288f352e234aa32cffb79af880a03d8faf7c1e3492e6d8eba267"
RUN_ROOT = "sha256:ab8b5541536e3c5c88df7783150973cee1d3ba7dd75ebd79efa389973e2813bd"
PROVIDER_RESPONSE_ROOT = (
    "sha256:4b90af7d3453c95f8b59a49d6bf7761593cc2ef9ccbfee96fbff7abc8934c50e"
)
PARSED_RESPONSE_ROOT = (
    "sha256:95bf0c205c10167f57d769a0f77daef57ede2db3a6061464957c496e50eddc46"
)
STOPPED_V2_COMMIT = "30210517f3b1bee420bc61e9a4484ecff8b68ae7"
STOPPED_V2_ROOT = (
    "sha256:b72c5d8c5bdf66e528524719773dfc37dda98b7b219c841349a9c6e4874abb1b"
)
STOPPED_V3_COMMIT = "37a5a92c314b4f0345eb2d8aadf1890b4e59682d"
STOPPED_V3_ROOT = (
    "sha256:63cbbdf6ae6c7e906268b31f33198d06b8db0757e6db48b6187286cacd08dcb9"
)
SOURCE_ROOTS = {
    "controller.py": "sha256:0cd70e76736061ad2d1f8cf609be7a8b8eedf3090eafbe2493ce8c7be580265e",
    "orchestrator.go": "sha256:5daf77799bddc0103212c8727128f8b84801bf8d974d08d86c857c8e3e9d8c9f",
    "runner_relay.go": "sha256:8a3b9b23a8f036735162dae7cdd0ac009ab87352cff1bc2fae1deed2babf9834",
}
BINARY_IDENTITIES = {
    "anthropic_host_bridge": {
        "bytes": 5_918_210,
        "sha256": "sha256:c6a8cb2f256eea8654a82333fd59c9b8aa0c086c0fec3629c56bd4bef2794eb4",
    },
    "orchestrator": {
        "bytes": 2_097_266,
        "sha256": "sha256:5edb0cf20823e902c2ab96f9b2232d6755df81a78a02ee9089fbcf155d4b10cc",
    },
    "runner_relay": {
        "bytes": 1_704_062,
        "sha256": "sha256:2386d25cb23367b011684e863bbda680eee19f589b6b88cc4395910178a0ce4f",
    },
}
SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")

FILES = frozenset(
    {
        "README.md",
        "artifact-root.json",
        "execution-build.json",
        "seal.py",
        "terminal-outcome.json",
        "test_verify.py",
        "verify.py",
        "execution-sources/controller.py",
        "execution-sources/orchestrator.go",
        "execution-sources/runner_relay.go",
        "inputs/expected-request.json",
        "inputs/materialization-receipt.json",
        "inputs/offline-validation-receipt.json",
        "inputs/packet.json",
        "inputs/provider-schema.json",
        "inputs/request-transport-custody.json",
        "inputs/run.json",
        "permit/neutral-calibration-anthropic-json-v4-lossless.permit.consumed.json",
        "raw/actual-network-body-0001.raw.json",
        "raw/attempt-terminal.json",
        "raw/bridge-to-runner.raw.jsonl",
        "raw/bridge.stderr",
        "raw/bridge.stdout",
        "raw/container.stderr",
        "raw/credential-nonretention.json",
        "raw/endpoint-contact-receipt.json",
        "raw/lossless-network-request-custody.json",
        "raw/orchestrator.stderr",
        "raw/orchestrator.stdout",
        "raw/packet-custody.json",
        "raw/permit-release.json",
        "raw/process-teardown.json",
        "raw/provider-events.raw.jsonl",
        "raw/provider-request-frame.raw.jsonl",
        "raw/provider-response-0001.raw.json",
        "raw/provider-usage-0001.json",
        "raw/request-transport-custody.json",
        "raw/request.raw.json",
        "raw/response.raw.json",
        "raw/runner-to-bridge.raw.jsonl",
        "raw/terminal.json",
    }
)
DIRECTORIES = frozenset({"execution-sources", "inputs", "permit", "raw"})


class VerificationError(RuntimeError):
    pass


def require(condition: bool, label: str) -> None:
    if not condition:
        raise VerificationError(label)


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def duplicate_rejecting_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise VerificationError("duplicate_json_key")
        value[key] = item
    return value


def strict_json_bytes(raw: bytes) -> Any:
    try:
        return json.loads(raw, object_pairs_hook=duplicate_rejecting_pairs)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise VerificationError("invalid_json") from error


def load(relative: str) -> Any:
    return strict_json_bytes((PACKAGE / relative).read_bytes())


def exact_keys(value: Any, keys: set[str], label: str) -> None:
    require(type(value) is dict and set(value) == keys, label)


def exact_int(value: Any, expected: int, label: str) -> None:
    require(type(value) is int and value == expected, label)


def exact_sha(value: Any, expected: str, label: str) -> None:
    require(
        type(value) is str
        and SHA256.fullmatch(value) is not None
        and value == expected,
        label,
    )


def git(*arguments: str) -> str:
    return subprocess.run(
        ["git", *arguments], cwd=REPO, check=True, capture_output=True, text=True
    ).stdout.strip()


def expected_request_custody() -> dict[str, Any]:
    return {
        "schema": "vela.lossless-provider-request-custody.v1",
        "content_type": "application/json",
        "bytes": 4278,
        "sha256": REQUEST_ROOT,
        "payload_encoding": "base64-rfc4648-canonical",
        "decode_count": 1,
        "provider_schema_bytes": 2384,
        "provider_schema_sha256": SCHEMA_ROOT,
        "provider_schema_occurrences": 1,
        "endpoint_write_prepared": True,
    }


def validate_file_set() -> None:
    observed_files: set[str] = set()
    observed_directories: set[str] = set()
    inodes: set[tuple[int, int]] = set()
    for path in PACKAGE.rglob("*"):
        relative = path.relative_to(PACKAGE).as_posix()
        item = path.lstat()
        require(not stat.S_ISLNK(item.st_mode), "symlink_in_artifact")
        if stat.S_ISDIR(item.st_mode):
            observed_directories.add(relative)
            continue
        require(stat.S_ISREG(item.st_mode) and item.st_nlink == 1, "artifact_file_type")
        inode = (item.st_dev, item.st_ino)
        require(inode not in inodes, "artifact_hardlink_alias")
        inodes.add(inode)
        observed_files.add(relative)
    require(observed_files == FILES, "artifact_file_set")
    require(observed_directories == DIRECTORIES, "artifact_directory_set")


def seal_manifest(root: Path) -> dict[str, Any]:
    files = []
    for relative in sorted(FILES - {"artifact-root.json"}):
        raw = (root / relative).read_bytes()
        files.append({"path": relative, "bytes": len(raw), "sha256": digest(raw)})
    body = {"schema": "vela.stage-a-anthropic-neutral-v4-artifact.v1", "files": files}
    return {**body, "artifact_root": digest(canonical(body))}


def validate_producer() -> None:
    build = load("execution-build.json")
    exact_keys(
        build,
        {
            "schema",
            "producer",
            "runtime",
            "sources",
            "binaries",
            "build_parameters",
            "prior_denominator",
            "stopped_state",
        },
        "execution_build_shape",
    )
    require(
        build["schema"] == "vela.stage-a-anthropic-neutral-v4-execution-build.v1",
        "execution_build_schema",
    )
    producer = build["producer"]
    require(
        producer
        == {
            "artifact_root": RUNTIME_ARTIFACT_ROOT,
            "branch": "codex/stage-a-two-provider-runtime-qualification",
            "commit": PRODUCER,
            "offline_qualification_root": OFFLINE_ROOT,
            "registration_root": REGISTRATION_ROOT,
            "tree": PRODUCER_TREE,
        },
        "producer_binding",
    )
    require(git("rev-parse", f"{PRODUCER}^{{tree}}") == PRODUCER_TREE, "producer_tree")
    require(
        subprocess.run(
            ["git", "diff", "--quiet", PRODUCER, "--", str(RUNTIME.relative_to(REPO))],
            cwd=REPO,
            check=False,
        ).returncode
        == 0,
        "runtime_bytes_changed_after_review",
    )
    require(
        load_json_path(RUNTIME / "artifact-root.json")["artifact_root"]
        == RUNTIME_ARTIFACT_ROOT,
        "runtime_artifact_root",
    )
    require(
        load_json_path(RUNTIME / "offline-qualification.json")["record_root"]
        == OFFLINE_ROOT,
        "offline_root",
    )
    registration = load_json_path(RUNTIME / "registration.json")
    require(digest(canonical(registration)) == REGISTRATION_ROOT, "registration_root")
    runtime = build["runtime"]
    require(
        runtime
        == {
            "image_digest": "sha256:a95b75cfc449afc2ecb87a5808542339b1776ced9b15d91a27e795993acdbba0",
            "oci_archive_sha256": "sha256:646128f39c03260bdc72c191d91d3f96cc403a5f63ef3b3399c7dc3a6089d279",
            "runtime_source_root": "sha256:345b150207668e98a2a061c328e3552697b5370c50fb50dfbd96f598aaa65e30",
            "transport_custody_root": "sha256:d1813ba1ad996442e38efe92ec8556210ddc11ffc8037a23ebb0c5b007157fb6",
            "qualifier_commit": "cc3b88d8bfcfd7b4f720a023f049d5c365be9423",
            "qualifier_tree": "341e0d22fa570b1b5e8dd9f70b219c11308ba45f",
            "qualifier_sha256": "sha256:61591eec3304e299a9344888bc2a6f08cd32785b647ef5b0107da490dbf18013",
        },
        "runtime_binding",
    )
    for name, expected in build["sources"].items():
        require(
            name in {"controller.py", "orchestrator.go", "runner_relay.go"},
            "source_name",
        )
        exact_sha(
            expected,
            digest((PACKAGE / "execution-sources" / name).read_bytes()),
            "source_digest",
        )
    require(build["sources"] == SOURCE_ROOTS, "source_identity")
    for record in build["binaries"].values():
        exact_keys(record, {"bytes", "sha256"}, "binary_record_shape")
        require(type(record["bytes"]) is int and record["bytes"] > 0, "binary_size")
        require(
            type(record["sha256"]) is str
            and SHA256.fullmatch(record["sha256"]) is not None,
            "binary_digest",
        )
    require(build["binaries"] == BINARY_IDENTITIES, "binary_identity")
    require(
        build["build_parameters"]
        == {
            "anthropic_host_bridge": {
                "build_id": "",
                "cgo_enabled": "0",
                "goarch": "arm64",
                "goos": "darwin",
                "provider_adapter": "anthropic-messages-v1",
                "strip_debug": True,
                "trimpath": True,
            },
            "orchestrator": {
                "build_id": "",
                "cgo_enabled": "0",
                "goarch": "arm64",
                "goos": "darwin",
                "strip_debug": True,
                "trimpath": True,
            },
            "runner_relay": {
                "build_id": "",
                "cgo_enabled": "0",
                "goarch": "arm64",
                "goos": "linux",
                "strip_debug": True,
                "trimpath": True,
            },
        },
        "build_parameters",
    )
    require(
        build["prior_denominator"]
        == {
            "v2": {
                "artifact_root": STOPPED_V2_ROOT,
                "disposition": "permanent_consumed_non_call",
                "provider_calls": 0,
            },
            "v3": {
                "artifact_root": STOPPED_V3_ROOT,
                "disposition": "permanent_consumed_failed_exact_request",
                "provider_calls": 1,
            },
        },
        "prior_denominator",
    )
    stopped = build["stopped_state"]
    require(
        stopped
        == {
            "authority_effect": "none",
            "openai_neutral_permit_released": False,
            "participant_calls": 0,
            "participant_permits_released": 0,
            "scoring_attempts": 0,
            "stage_b_families_selected": 0,
        },
        "stopped_state",
    )
    for commit, directory, expected_root in (
        (STOPPED_V2_COMMIT, STOPPED_V2, STOPPED_V2_ROOT),
        (STOPPED_V3_COMMIT, STOPPED_V3, STOPPED_V3_ROOT),
    ):
        require(
            load_json_path(directory / "artifact-root.json")["artifact_root"]
            == expected_root,
            "prior_artifact_root",
        )
        require(
            subprocess.run(
                [
                    "git",
                    "diff",
                    "--quiet",
                    commit,
                    "--",
                    str(directory.relative_to(REPO)),
                ],
                cwd=REPO,
                check=False,
            ).returncode
            == 0,
            "prior_artifact_bytes_drift",
        )
    openai = load_json_path(
        RUNTIME / "offline-qualification-assets/openai-held_permit.json"
    )
    require(
        openai["status"] == "held" and openai["consumed_at"] is None,
        "openai_permit_state",
    )
    prelaunch = load_json_path(STAGE_A / "prelaunch-state.json")
    for key in (
        "provider_calls",
        "released_permits",
        "participant_responses",
        "scoring_attempts",
        "stage_b_families_selected",
    ):
        exact_int(prelaunch[key], 0, f"stage_a:{key}")
    exact_int(prelaunch["fixed_denominator"], 12, "stage_a_denominator")


def load_json_path(path: Path) -> Any:
    return strict_json_bytes(path.read_bytes())


def validate_inputs_and_permit() -> None:
    bindings = {
        "inputs/packet.json": RUNTIME / "neutral-calibration/packet.json",
        "inputs/provider-schema.json": RUNTIME
        / "offline-qualification-assets/anthropic-provider_schema.json",
        "inputs/run.json": RUNTIME
        / "offline-qualification-assets/anthropic-run_input.json",
        "inputs/materialization-receipt.json": RUNTIME
        / "offline-qualification-assets/anthropic-materialization_receipt.json",
        "inputs/offline-validation-receipt.json": RUNTIME
        / "offline-qualification-assets/anthropic-offline_validation_receipt.json",
        "inputs/request-transport-custody.json": RUNTIME
        / "offline-qualification-assets/anthropic-request_transport_custody.json",
        "inputs/expected-request.json": RUNTIME
        / "offline-qualification-assets/anthropic-request_bytes.json",
    }
    for relative, source in bindings.items():
        require(
            (PACKAGE / relative).read_bytes() == source.read_bytes(), "input_binding"
        )
    exact_sha(
        digest((PACKAGE / "inputs/packet.json").read_bytes()),
        PACKET_ROOT,
        "packet_root",
    )
    exact_sha(
        digest((PACKAGE / "inputs/provider-schema.json").read_bytes()),
        SCHEMA_ROOT,
        "schema_root",
    )
    exact_sha(digest((PACKAGE / "inputs/run.json").read_bytes()), RUN_ROOT, "run_root")
    exact_sha(
        digest((PACKAGE / "inputs/expected-request.json").read_bytes()),
        REQUEST_ROOT,
        "request_root",
    )
    require(
        load("inputs/request-transport-custody.json") == expected_request_custody(),
        "input_transport_custody",
    )
    permit_relative = (
        "permit/neutral-calibration-anthropic-json-v4-lossless.permit.consumed.json"
    )
    source = RUNTIME / "offline-qualification-assets/anthropic-held_permit.json"
    require(
        (PACKAGE / permit_relative).read_bytes() == source.read_bytes(),
        "consumed_permit_bytes",
    )
    permit = load(permit_relative)
    require(
        permit["run_id"] == RUN_ID
        and permit["status"] == "held"
        and permit["consumed_at"] is None
        and digest(canonical(permit)) == PERMIT_ROOT,
        "consumed_permit_binding",
    )
    release = load("raw/permit-release.json")
    exact_keys(
        release,
        {
            "schema",
            "run_id",
            "permit_root",
            "source_state",
            "consumed_path",
            "attempt",
            "zero_retries",
            "released_at",
        },
        "permit_release_shape",
    )
    require(
        release["schema"] == "vela.stage-a-anthropic-neutral-permit-release.v2"
        and release["run_id"] == RUN_ID
        and release["permit_root"] == PERMIT_ROOT
        and release["source_state"] == "held"
        and release["consumed_path"] == Path(permit_relative).name
        and release["zero_retries"] is True
        and type(release["released_at"]) is str,
        "permit_release_binding",
    )
    exact_int(release["attempt"], 1, "permit_release_attempt")


def validate_request_transport() -> dict[str, Any]:
    expected_request = (PACKAGE / "inputs/expected-request.json").read_bytes()
    schema = (PACKAGE / "inputs/provider-schema.json").read_bytes()
    for relative in ("raw/request.raw.json", "raw/actual-network-body-0001.raw.json"):
        require(
            (PACKAGE / relative).read_bytes() == expected_request,
            "request_network_byte_drift",
        )
    require(
        (PACKAGE / "raw/request-transport-custody.json").read_bytes()
        == (PACKAGE / "inputs/request-transport-custody.json").read_bytes(),
        "runner_transport_custody",
    )
    transcript = (PACKAGE / "raw/runner-to-bridge.raw.jsonl").read_bytes()
    frame_raw = (PACKAGE / "raw/provider-request-frame.raw.jsonl").read_bytes()
    require(
        transcript == frame_raw
        and frame_raw.endswith(b"\n")
        and frame_raw.count(b"\n") == 1,
        "request_frame_copy",
    )
    frame = strict_json_bytes(frame_raw)
    exact_keys(frame, {"type", "adapter", "endpoint", "payload"}, "request_frame_shape")
    require(
        frame["type"] == "provider_request"
        and frame["adapter"] == "anthropic-messages-v1"
        and frame["endpoint"] == "https://api.anthropic.com/v1/messages",
        "request_frame_identity",
    )
    payload = frame["payload"]
    exact_keys(
        payload,
        {
            "schema",
            "encoding",
            "content_type",
            "bytes",
            "sha256",
            "base64",
            "provider_schema_bytes",
            "provider_schema_sha256",
            "provider_schema_base64",
            "provider_schema_occurrences",
        },
        "request_payload_shape",
    )
    require(
        payload["schema"] == "vela.lossless-provider-request-payload.v1"
        and payload["encoding"] == "base64-rfc4648-canonical"
        and payload["content_type"] == "application/json",
        "request_payload_semantics",
    )
    for key, expected in (
        ("bytes", 4278),
        ("provider_schema_bytes", 2384),
        ("provider_schema_occurrences", 1),
    ):
        exact_int(payload[key], expected, f"payload:{key}")
    exact_sha(payload["sha256"], REQUEST_ROOT, "payload_root")
    exact_sha(payload["provider_schema_sha256"], SCHEMA_ROOT, "payload_schema_root")
    try:
        body = base64.b64decode(payload["base64"], validate=True)
        decoded_schema = base64.b64decode(
            payload["provider_schema_base64"], validate=True
        )
    except ValueError as error:
        raise VerificationError("payload_base64") from error
    require(
        base64.b64encode(body).decode() == payload["base64"]
        and base64.b64encode(decoded_schema).decode()
        == payload["provider_schema_base64"],
        "payload_base64_canonical",
    )
    require(
        body == expected_request
        and decoded_schema == schema
        and body.count(schema) == 1,
        "payload_decode_binding",
    )
    exact_sha(digest(frame_raw), FRAME_ROOT, "frame_root")
    custody = load("raw/lossless-network-request-custody.json")
    exact_keys(
        custody,
        {
            "schema",
            "request_ordinal",
            "outbound_frame_count",
            "frame_sha256",
            "frame_payload_encoding",
            "frame_decode_count",
            "pre_frame_request_bytes",
            "pre_frame_request_sha256",
            "decoded_network_body_bytes",
            "decoded_network_body_sha256",
            "endpoint_write_request_bytes",
            "endpoint_write_request_sha256",
            "provider_schema_bytes",
            "provider_schema_sha256",
            "provider_schema_occurrences",
            "byte_identical",
            "json_reserialization",
        },
        "network_custody_shape",
    )
    for key, expected in (
        ("request_ordinal", 1),
        ("outbound_frame_count", 1),
        ("frame_decode_count", 1),
        ("pre_frame_request_bytes", 4278),
        ("decoded_network_body_bytes", 4278),
        ("endpoint_write_request_bytes", 4278),
        ("provider_schema_bytes", 2384),
        ("provider_schema_occurrences", 1),
    ):
        exact_int(custody[key], expected, f"network_custody:{key}")
    require(
        custody["schema"] == "vela.stage-a-lossless-network-request-custody.v1"
        and custody["frame_sha256"] == FRAME_ROOT
        and custody["frame_payload_encoding"] == "base64-rfc4648-canonical"
        and custody["pre_frame_request_sha256"] == REQUEST_ROOT
        and custody["decoded_network_body_sha256"] == REQUEST_ROOT
        and custody["endpoint_write_request_sha256"] == REQUEST_ROOT
        and custody["provider_schema_sha256"] == SCHEMA_ROOT
        and custody["byte_identical"] is True
        and custody["json_reserialization"] is False,
        "network_custody_binding",
    )
    return custody


def validate_provider_and_terminal(network: dict[str, Any]) -> None:
    transcript = (PACKAGE / "raw/bridge-to-runner.raw.jsonl").read_bytes()
    require(
        transcript == (PACKAGE / "raw/provider-events.raw.jsonl").read_bytes(),
        "provider_event_copy",
    )
    lines = transcript.splitlines()
    require(len(lines) == 3, "provider_event_count")
    endpoint_frame, provider_frame, terminal_frame = [
        strict_json_bytes(line) for line in lines
    ]
    exact_keys(
        endpoint_frame,
        {"type", "provider_calls", "request_custody"},
        "endpoint_frame_shape",
    )
    require(
        endpoint_frame["type"] == "endpoint_attempt"
        and endpoint_frame["request_custody"] == expected_request_custody(),
        "endpoint_frame_binding",
    )
    exact_int(endpoint_frame["provider_calls"], 1, "endpoint_frame_calls")
    exact_keys(provider_frame, {"type", "raw"}, "provider_frame_shape")
    provider_raw = (PACKAGE / "raw/provider-response-0001.raw.json").read_bytes()
    require(
        provider_frame["type"] == "provider_event"
        and provider_frame["raw"].encode() == provider_raw,
        "provider_raw_binding",
    )
    exact_sha(digest(provider_raw), PROVIDER_RESPONSE_ROOT, "provider_response_root")
    provider = strict_json_bytes(provider_raw)
    exact_keys(
        provider,
        {
            "id",
            "type",
            "role",
            "model",
            "content",
            "stop_reason",
            "stop_sequence",
            "stop_details",
            "usage",
        },
        "provider_response_shape",
    )
    require(
        type(provider["id"]) is str
        and provider["type"] == "message"
        and provider["role"] == "assistant"
        and provider["model"] == "claude-opus-5"
        and provider["stop_reason"] == "end_turn"
        and provider["stop_sequence"] is None
        and provider["stop_details"] is None,
        "provider_response_semantics",
    )
    require(
        type(provider["content"]) is list and len(provider["content"]) == 2,
        "provider_content",
    )
    exact_keys(
        provider["content"][0],
        {"type", "thinking", "signature"},
        "provider_thinking_shape",
    )
    exact_keys(provider["content"][1], {"type", "text"}, "provider_text_shape")
    require(
        provider["content"][0]["type"] == "thinking"
        and type(provider["content"][0]["thinking"]) is str
        and type(provider["content"][0]["signature"]) is str
        and provider["content"][1]["type"] == "text"
        and type(provider["content"][1]["text"]) is str,
        "provider_content_types",
    )
    usage = provider["usage"]
    exact_keys(
        usage,
        {
            "input_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
            "cache_creation",
            "output_tokens",
            "output_tokens_details",
            "service_tier",
            "inference_geo",
        },
        "usage_shape",
    )
    for key, expected in (
        ("input_tokens", 1891),
        ("cache_creation_input_tokens", 0),
        ("cache_read_input_tokens", 0),
        ("output_tokens", 459),
    ):
        exact_int(usage[key], expected, f"usage:{key}")
    require(
        usage["cache_creation"]
        == {"ephemeral_1h_input_tokens": 0, "ephemeral_5m_input_tokens": 0}
        and usage["output_tokens_details"] == {"thinking_tokens": 132}
        and usage["service_tier"] == "standard"
        and usage["inference_geo"] == "global",
        "usage_semantics",
    )
    usage_receipt = load("raw/provider-usage-0001.json")
    require(
        usage_receipt
        == {
            "schema": "vela.stage-a-anthropic-usage-custody.v1",
            "response_ordinal": 1,
            "provider_response_sha256": PROVIDER_RESPONSE_ROOT,
            "usage": usage,
        },
        "usage_receipt",
    )
    response_raw = (PACKAGE / "raw/response.raw.json").read_bytes()
    exact_sha(digest(response_raw), PARSED_RESPONSE_ROOT, "parsed_response_root")
    response = strict_json_bytes(response_raw)
    exact_keys(
        response,
        {
            "schema",
            "assignment_id",
            "relation_validation",
            "change_classification",
            "impact_closure",
            "authority_scientific_inference",
            "uncertainty",
        },
        "parsed_response_shape",
    )
    require(
        response["schema"] == "lean-correspondence.review-response.v1"
        and response["assignment_id"] == "lc-neutral-calibration"
        and response["relation_validation"] == "valid"
        and response["change_classification"] == "neither"
        and response["impact_closure"] == []
        and response["authority_scientific_inference"]
        == {
            "repository_authority_effect": "none",
            "scientific_status": "not_established",
        }
        and type(response["uncertainty"]) is list
        and response["uncertainty"]
        and all(type(item) is str for item in response["uncertainty"]),
        "parsed_response_semantics",
    )
    require(
        provider["content"][1]["text"].encode() == response_raw, "provider_text_binding"
    )
    exact_keys(
        terminal_frame, {"type", "body", "provider_calls"}, "terminal_frame_shape"
    )
    require(
        terminal_frame["type"] == "terminal" and terminal_frame["body"] == response,
        "terminal_frame_body",
    )
    exact_int(terminal_frame["provider_calls"], 1, "terminal_frame_calls")
    runner = load("raw/terminal.json")
    require(
        runner
        == {
            "schema": "vela.stage-a-runner-terminal.v1",
            "status": "completed",
            "run_id": RUN_ID,
            "adapter": "anthropic-messages-v1",
            "provider_calls": 1,
            "packet_sha256": PACKET_ROOT,
            "request_sha256": REQUEST_ROOT,
            "response_sha256": PARSED_RESPONSE_ROOT,
            "credential_retained": False,
        },
        "runner_terminal",
    )
    endpoint = load("raw/endpoint-contact-receipt.json")
    exact_keys(
        endpoint,
        {
            "schema",
            "run_id",
            "provider",
            "endpoint",
            "endpoint_attempt_receipts",
            "provider_calls",
            "source",
            "call_count_derivation",
            "initial_request_custody",
            "pre_frame_request_sha256",
            "actual_network_body_sha256",
            "byte_identical",
        },
        "endpoint_receipt_shape",
    )
    require(
        endpoint["schema"] == "vela.stage-a-endpoint-contact-receipt.v4"
        and endpoint["run_id"] == RUN_ID
        and endpoint["provider"] == "Anthropic"
        and endpoint["endpoint"] == "https://api.anthropic.com/v1/messages"
        and endpoint["endpoint_attempt_receipts"] == [endpoint_frame]
        and endpoint["source"] == "host-tee-of-bridge-to-runner-frame-stream"
        and endpoint["call_count_derivation"]
        == "closed_sequential_endpoint_attempt_receipts_only"
        and endpoint["initial_request_custody"] == expected_request_custody()
        and endpoint["pre_frame_request_sha256"] == REQUEST_ROOT
        and endpoint["actual_network_body_sha256"] == REQUEST_ROOT
        and endpoint["byte_identical"] is True,
        "endpoint_receipt_binding",
    )
    exact_int(endpoint["provider_calls"], 1, "endpoint_receipt_calls")
    attempt = load("raw/attempt-terminal.json")
    exact_keys(
        attempt,
        {
            "schema",
            "run_id",
            "permit_root",
            "attempt",
            "retries",
            "endpoint_attempt_receipts",
            "provider_calls",
            "bridge_provider_calls",
            "runner_provider_calls",
            "terminal_provider_calls",
            "custody_provider_calls",
            "runner_terminal_present",
            "orchestrator_exit_code",
            "status",
            "provider_response_terminal_success",
            "calibration_outcome",
            "positive_qualification",
            "lossless_initial_request_custody",
            "lossless_network_request_custody",
            "exact_request_custody_complete",
            "credential_retained",
            "credential_fd_closed",
            "participant_permits_released",
            "openai_neutral_permit_released",
            "scoring_attempts",
            "stage_b_families_selected",
            "authority_effect",
        },
        "attempt_terminal_shape",
    )
    exact_int(attempt["attempt"], 1, "attempt_number")
    exact_int(attempt["retries"], 0, "retry_count")
    for key in (
        "provider_calls",
        "bridge_provider_calls",
        "runner_provider_calls",
        "terminal_provider_calls",
        "custody_provider_calls",
    ):
        exact_int(attempt[key], 1, f"attempt:{key}")
    for key in (
        "participant_permits_released",
        "scoring_attempts",
        "stage_b_families_selected",
    ):
        exact_int(attempt[key], 0, f"attempt:{key}")
    require(
        attempt["schema"] == "vela.stage-a-anthropic-neutral-attempt.v4"
        and attempt["run_id"] == RUN_ID
        and attempt["permit_root"] == PERMIT_ROOT
        and attempt["endpoint_attempt_receipts"] == [endpoint_frame]
        and attempt["runner_terminal_present"] is True
        and attempt["orchestrator_exit_code"] == 0
        and attempt["status"] == "completed"
        and attempt["provider_response_terminal_success"] is True
        and attempt["calibration_outcome"] == "result_pending_independent_review"
        and attempt["positive_qualification"] is False
        and attempt["lossless_initial_request_custody"] == expected_request_custody()
        and attempt["lossless_network_request_custody"] == network
        and attempt["exact_request_custody_complete"] is True
        and attempt["credential_retained"] is False
        and attempt["credential_fd_closed"] is True
        and attempt["openai_neutral_permit_released"] is False
        and attempt["authority_effect"] == "none",
        "attempt_terminal",
    )


def validate_teardown_and_outcome(network: dict[str, Any]) -> None:
    for name in (
        "bridge.stderr",
        "bridge.stdout",
        "container.stderr",
        "orchestrator.stderr",
        "orchestrator.stdout",
    ):
        require(
            (PACKAGE / "raw" / name).read_bytes() == b"", "unexpected_process_output"
        )
    teardown = load("raw/process-teardown.json")
    require(
        teardown
        == {
            "schema": "vela.stage-a-anthropic-neutral-process-teardown.v2",
            "status": "completed",
            "credential_fd_closed": True,
            "credential_retained": False,
            "bridge_fd_closed": True,
            "participant_network": "none",
            "children": [
                {"name": "participant_runner_container", "exit_code": 0},
                {"name": "anthropic_host_bridge", "exit_code": 0},
            ],
        },
        "teardown",
    )
    credential = load("raw/credential-nonretention.json")
    require(
        credential
        == {
            "schema": "vela.stage-a-credential-nonretention.v1",
            "credential_source": "authorized_exact_file",
            "injection": "inherited_descriptor_only",
            "environment_injection": False,
            "credential_fd_closed": True,
            "credential_buffer_scrubbed": True,
            "evidence_scan_no_credential_bytes": True,
            "source_metadata_stable": True,
            "credential_retained": False,
        },
        "credential_nonretention",
    )
    outcome = load("terminal-outcome.json")
    exact_keys(
        outcome,
        {
            "schema",
            "status",
            "calibration_outcome",
            "positive_qualification",
            "run_id",
            "permit_root",
            "consumed_permit_sha256",
            "permit_consumed",
            "attempt",
            "retries",
            "released_at",
            "provider_calls",
            "endpoint_attempt_receipts",
            "provider_response_terminal_success",
            "request",
            "provider_response_sha256",
            "parsed_response_sha256",
            "usage_receipt_sha256",
            "terminal_receipt_sha256",
            "attempt_terminal_sha256",
            "endpoint_contact_receipt_sha256",
            "credential_retained",
            "credential_fd_closed",
            "credential_buffer_scrubbed",
            "participant_permits_released",
            "openai_neutral_permit_released",
            "scoring_attempts",
            "stage_b_families_selected",
            "authority_effect",
            "independent_review",
        },
        "outcome_shape",
    )
    require(
        outcome["schema"] == "vela.stage-a-anthropic-neutral-v4-terminal-outcome.v1"
        and outcome["status"] == "terminal_success_pending_independent_review"
        and outcome["calibration_outcome"] == "result_pending_independent_review"
        and outcome["positive_qualification"] is False
        and outcome["run_id"] == RUN_ID
        and outcome["permit_root"] == PERMIT_ROOT
        and outcome["permit_consumed"] is True
        and outcome["provider_response_terminal_success"] is True
        and outcome["request"]
        == {
            "pre_frame_bytes": 4278,
            "pre_frame_sha256": REQUEST_ROOT,
            "frame_sha256": FRAME_ROOT,
            "payload_encoding": "base64-rfc4648-canonical",
            "decode_count": 1,
            "actual_network_body_bytes": 4278,
            "actual_network_body_sha256": REQUEST_ROOT,
            "endpoint_write_request_bytes": 4278,
            "endpoint_write_request_sha256": REQUEST_ROOT,
            "provider_schema_bytes": 2384,
            "provider_schema_sha256": SCHEMA_ROOT,
            "provider_schema_occurrences": 1,
            "byte_identical": True,
            "json_reserialization": False,
        }
        and outcome["provider_response_sha256"] == PROVIDER_RESPONSE_ROOT
        and outcome["parsed_response_sha256"] == PARSED_RESPONSE_ROOT
        and outcome["credential_retained"] is False
        and outcome["credential_fd_closed"] is True
        and outcome["credential_buffer_scrubbed"] is True
        and outcome["openai_neutral_permit_released"] is False
        and outcome["authority_effect"] == "none"
        and outcome["independent_review"]
        == "required_before_any_positive_qualification_or_follow_on_action",
        "outcome_semantics",
    )
    for key, expected in (
        ("attempt", 1),
        ("retries", 0),
        ("provider_calls", 1),
        ("endpoint_attempt_receipts", 1),
        ("participant_permits_released", 0),
        ("scoring_attempts", 0),
        ("stage_b_families_selected", 0),
    ):
        exact_int(outcome[key], expected, f"outcome:{key}")
    derived = {
        "consumed_permit_sha256": digest(
            (
                PACKAGE
                / "permit/neutral-calibration-anthropic-json-v4-lossless.permit.consumed.json"
            ).read_bytes()
        ),
        "usage_receipt_sha256": digest(
            (PACKAGE / "raw/provider-usage-0001.json").read_bytes()
        ),
        "terminal_receipt_sha256": digest((PACKAGE / "raw/terminal.json").read_bytes()),
        "attempt_terminal_sha256": digest(
            (PACKAGE / "raw/attempt-terminal.json").read_bytes()
        ),
        "endpoint_contact_receipt_sha256": digest(
            (PACKAGE / "raw/endpoint-contact-receipt.json").read_bytes()
        ),
    }
    require(
        all(outcome[key] == value for key, value in derived.items()),
        "outcome_digest_binding",
    )
    for relative in FILES:
        if relative == "artifact-root.json":
            continue
        raw = (PACKAGE / relative).read_bytes()
        require(
            (b"sk-" + b"ant-") not in raw
            and (b"ANTHROPIC_" + b"API_KEY" + b"=") not in raw,
            "credential_shaped_bytes",
        )


def verify() -> dict[str, Any]:
    validate_file_set()
    validate_producer()
    validate_inputs_and_permit()
    network = validate_request_transport()
    validate_provider_and_terminal(network)
    validate_teardown_and_outcome(network)
    observed_manifest = load("artifact-root.json")
    require(observed_manifest == seal_manifest(PACKAGE), "artifact_manifest")
    return {
        "schema": "vela.stage-a-anthropic-neutral-v4-verification.v1",
        "status": "PASS_TERMINAL_SUCCESS_PENDING_INDEPENDENT_REVIEW",
        "artifact_root": observed_manifest["artifact_root"],
        "provider_calls": 1,
        "retries": 0,
        "positive_qualification": False,
        "parsed_response_sha256": PARSED_RESPONSE_ROOT,
        "authority_effect": "none",
    }


def main() -> int:
    try:
        result = verify()
    except (VerificationError, subprocess.CalledProcessError) as error:
        print(json.dumps({"status": "FAIL", "error": str(error)}, sort_keys=True))
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
