#!/usr/bin/env python3
"""Fail-closed verification of the offline-qualified, still-held candidate."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import subprocess
import tarfile
from pathlib import Path
from typing import Any

PACKAGE = Path(__file__).resolve().parent
REPOSITORY = PACKAGE.parents[2]
REGISTRATION = PACKAGE / "registration.json"
OFFLINE = PACKAGE / "offline-qualification.json"
ARTIFACT_ROOT = PACKAGE / "artifact-root.json"
STAGE_A = REPOSITORY / "paper/artifacts/lean-correspondence-stage-a-open-pilot"
STOPPED = (
    REPOSITORY
    / "paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration"
)
STOPPED_V3 = (
    REPOSITORY
    / "paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v3"
)
SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")
EXPECTED_REGISTRATION_ROOT = (
    "sha256:f84bd9dcd6f9de6f8765c1ad25361f6579d7721cdc8d57937ad55c4205988ed4"
)
EXPECTED_OFFLINE_RECORD_ROOT = (
    "sha256:51df9fe89d649e5fbb6519d2f02eefaaf5dc672c350de1fcc58fab5047944e3f"
)
EXPECTED_QUALIFIER = {
    "git_commit": "cc3b88d8bfcfd7b4f720a023f049d5c365be9423",
    "git_tree": "341e0d22fa570b1b5e8dd9f70b219c11308ba45f",
    "path": "tools/evidence_qualification/qualification.py",
    "sha256": "sha256:61591eec3304e299a9344888bc2a6f08cd32785b647ef5b0107da490dbf18013",
}
EXPECTED_CORRECTIVE_ANCESTRY = {
    "invalid_permit_origin_commit": "9da1c79425c79af632197a719ca45ca07ab22a6c",
    "invalid_permit_origin_relationship": "ancestor_of_reviewed_predecessor_not_direct_parent",
    "reviewed_predecessor_commit": "b333186cae1274ebb48353ba72e1ab3be42adcc0",
    "reviewed_predecessor_parent_commit": "5be82cb3ab1ef11e7e870675337ae3704118fd46",
    "successor_direct_parent_commit": "b333186cae1274ebb48353ba72e1ab3be42adcc0",
    "stopped_evidence_commit": "30210517f3b1bee420bc61e9a4484ecff8b68ae7",
    "stopped_evidence_tree": "a2c878542e92442134f56b79501448ba14e16e28",
    "prospective_successor_direct_parent_commit": "37a5a92c314b4f0345eb2d8aadf1890b4e59682d",
}
EXPECTED_STAGE_A = {
    "artifact_root": "sha256:f89d335912adbbd0e3b3f1cb98ec3f4fa78a27f3742652ac7244eaa86ed6aca8",
    "participant_calls": 0,
    "participant_permits_consumed": 0,
    "participant_permits_released": 0,
    "pilot_commit": "c4a90b63d2f35c16876ff8b36f48742ae2c6ea7d",
    "pilot_directory_tree": "2a5dfdb5f82f3c3540efe89f6752a441fdfa9dd3",
    "pilot_tree": "af0cd1e65bf0efb1ca60897f47bab1cbc6e69232",
    "qualification_receipt_root": None,
    "status": "zero_of_twelve_all_participant_permits_held",
}
EXPECTED_PROVIDER = {
    "openai-responses-v1": {
        "organization": "OpenAI",
        "model": "gpt-5.6-sol",
        "run_id": "neutral-calibration-openai-json-v2",
        "configuration_root": "sha256:96555c45c33ed2a106cfb261025b752a4eeb1514aa180985ecd5ea0551a6616d",
        "qualification_root": "sha256:f5f4892ae2ddd6e871f8e3eeb8f8faeb34a7137fc6070df8b023815a6505e4f6",
        "image_digest": "sha256:f0ce5175fe6f72bb44f355f7f443e814bec91efb75c28326b3d97fee54aef4fb",
        "tool_boundary_root": "sha256:0b2e1fb701f70b02f9cc7ad79201f84374dfeb904299b59a6667d36eb4e59c69",
        "runtime_source_root": "sha256:5ec5d345bd9ecc8828ffbf5aa7da60b10164d5ac1a821fd8e7bf37e63c7fd8b6",
        "participant_permit_root": "sha256:b41826ce2a5897f854f4c9116fc40c5ef189e5f17bad1ea8979c90c946ad04ea",
        "provider_schema_bytes": "sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7",
        "launchability_sha256": "sha256:790bbf9505e21503118bab52fcd132a4e365c2c571f244fcb1d450bd6145d231",
    },
    "anthropic-messages-v1": {
        "organization": "Anthropic",
        "model": "claude-opus-5",
        "run_id": "neutral-calibration-anthropic-json-v4-lossless",
        "configuration_root": "sha256:10a9a0569f63a523e7dd6dab768c9dc255aa244c026337f217142cd2a1483163",
        "qualification_root": "sha256:74e07177b119edf0e9fcf18940cce9fa06757526092bc38f18595471debb623e",
        "image_digest": "sha256:315fd2ae42a140f3be8dd05d34031f83aca6fa29e421f86ca335a4dfafd6b2f6",
        "tool_boundary_root": "sha256:01dfbda69c1c7760423fdba41eaac18687a73d9fe683a8a5f207fdc8abe2a7d9",
        "runtime_source_root": "sha256:4a44868aaf4a5d00dd7c21aa9e95ced13c9674b659b3e24aca6bac90d15ad460",
        "participant_permit_root": "sha256:1db30a246b6727cf1e01f923b80f4247defab2318e63483c5d6efd58689c1e36",
        "provider_schema_bytes": "sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7",
        "launchability_sha256": "sha256:a180d19a49287c6271fc7aea61da8f041aaf3c0e3db4a9e309559f84dc4c2ff8",
    },
}
EXPECTED_FROZEN_CONFIGURATION = {
    "openai-responses-v1": {
        "api": {
            "endpoint": "https://api.openai.com/v1/responses",
            "library": "go1.26.2_standard_library_net_http",
            "provider_api_version": "responses-v1",
            "provider_cli": "none",
            "sdk": "none_raw_https",
        },
        "model_snapshot_semantics": "canonical_pinned_model_id_not_gpt-5.6_alias",
        "parameters": {
            "background": False,
            "max_output_tokens": 32768,
            "parallel_tool_calls": False,
            "reasoning_context": "current_turn",
            "reasoning_effort": "high",
            "retries": 0,
            "service_tier": "default",
            "store": False,
            "temperature": "omitted",
            "timeout_seconds": 1200,
        },
    },
    "anthropic-messages-v1": {
        "api": {
            "anthropic_version_header": "2023-06-01",
            "endpoint": "https://api.anthropic.com/v1/messages",
            "library": "go1.26.2_standard_library_net_http",
            "provider_api_version": "messages-v1_with_anthropic-version-2023-06-01",
            "provider_cli": "none",
            "sdk": "none_raw_https",
        },
        "model_snapshot_semantics": "canonical_pinned_model_id_not_pre-4.6_alias",
        "parameters": {
            "max_tokens": 32768,
            "output_config_effort": "high",
            "retries": 0,
            "service_tier": "standard_only",
            "temperature": "omitted",
            "thinking": "adaptive_default",
            "timeout_seconds": 1200,
        },
    },
}
EXPECTED_RETIRED_PERMITS = {
    "openai-responses-v1": {
        "run_id": "neutral-calibration-openai",
        "permit_root": "sha256:96a9c8af3d079ab8c73dd8eaaca05d62eebde2c70efe97a192b462edf2f7ff03",
    },
    "anthropic-messages-v1": {
        "run_id": "neutral-calibration-anthropic",
        "permit_root": "sha256:4bed98283ffb3af24ed0c99d7d4e135276770fef8288c11fbe87e9c8b0d37b9f",
    },
}
EXPECTED_NEUTRAL_CONTENT = {
    "content_equivalence_root": "sha256:1818fb5da3c0b3c57f24083eaa54acb448ce9cd14f59cbd0bd0d2aaef2dac8b8",
    "expected_response_schema_root": "sha256:b2d9bee1c76bc1f25f134fd50697f4e4a820a36bd61a84081edd5c542d749268",
    "information_equivalent": True,
    "packet_path": "neutral-calibration/packet.json",
    "packet_root": "sha256:a38b18fb6284288f352e234aa32cffb79af880a03d8faf7c1e3492e6d8eba267",
    "prompt_path": "neutral-calibration/prompt.txt",
    "prompt_root": "sha256:3443fa942b90f84718cc4e6918ebf6d121ebc40cf58f5b3c610f4e983c4d4ed9",
    "provider_adapters": ["openai-responses-v1", "anthropic-messages-v1"],
    "schema": "vela.stage-a-neutral-content-equivalence.v1",
    "semantic_atoms_root": "sha256:a19a614c3ef81d7f78fa9952af05535dccaa8d3d41cb0c2bafed6aafb9b1a9d8",
}
EXPECTED_PRIOR_CONSUMED_NON_CALL = {
    "schema": "vela.stage-a-consumed-neutral-non-call-lineage.v1",
    "producer_commit": "30210517f3b1bee420bc61e9a4484ecff8b68ae7",
    "producer_tree": "a2c878542e92442134f56b79501448ba14e16e28",
    "artifact_root": "sha256:b72c5d8c5bdf66e528524719773dfc37dda98b7b219c841349a9c6e4874abb1b",
    "provider_adapter": "anthropic-messages-v1",
    "run_id": "neutral-calibration-anthropic-json-v2",
    "permit_root": "sha256:b9ba39cf1c511043324ca8dfbc02b6c59d91f457a2e560a37d78d32a1b84cdbe",
    "consumed_permit_bytes": "sha256:69cf9f72ed814b4a39916189a3241ec4e01e4f965fa5ffa31e1beef3727c57fe",
    "endpoint_contact_receipt_bytes": "sha256:798a8733f655c0e5aa4e16ddec6dc8471d3fb2897b6c3eeb5940907e0f58ac4f",
    "permit_consumed": True,
    "provider_calls": 0,
    "endpoint_contacted": False,
    "retryable": False,
    "replacement_authorized": False,
    "denominator_disposition": "permanent_consumed_non_call",
    "authority_effect": "none",
}
EXPECTED_PRIOR_CONSUMED_FAILED_EXACT_REQUEST = {
    "schema": "vela.stage-a-consumed-neutral-failed-exact-request-lineage.v1",
    "producer_commit": "37a5a92c314b4f0345eb2d8aadf1890b4e59682d",
    "producer_tree": "e5c1449b626c62db5215ea260a5f6ede6942d9fa",
    "artifact_root": "sha256:63cbbdf6ae6c7e906268b31f33198d06b8db0757e6db48b6187286cacd08dcb9",
    "provider_adapter": "anthropic-messages-v1",
    "run_id": "neutral-calibration-anthropic-json-v3-replacement",
    "permit_root": "sha256:7ddf24c9dbeac2cdce1a4ca1972a0984287dbcf528881ae01cbfe297217e2f32",
    "consumed_permit_bytes": "sha256:92bb69095536f0d7be026baed085b530ced623d2408d8e0c63fc2175a4b1a6f3",
    "endpoint_contact_receipt_bytes": "sha256:e0615bd59e62a73694e9d48ae02b650b7d699d6d4bf6edd7058874fe4c5623a7",
    "permit_consumed": True,
    "provider_calls": 1,
    "endpoint_contacted": True,
    "provider_response": "terminal_success",
    "calibration_outcome": "non_result_failed_exact_request",
    "positive_qualification": False,
    "retryable": False,
    "replacement_authorized": False,
    "denominator_disposition": "permanent_consumed_failed_exact_request",
    "authority_effect": "none",
}
EXPECTED_CALL_DERIVATION = {
    "schema": "vela.stage-a-provider-call-derivation.v1",
    "source": "successful_endpoint_write_request_attempt_receipts_only",
    "controller_source_path": "neutral_controller.py",
    "controller_source_sha256": "sha256:a3f17f8f6006caf02ce0928051170b6b07f08d1ed37ad273f50de7ebe82d7b9a",
    "controller": 0,
    "bridge": 0,
    "runner": 0,
    "terminal": 0,
    "custody": 0,
    "endpoint_write_receipts": 0,
    "pre_request_failures_count_as_calls": False,
}


class CandidateError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise CandidateError(message)


def load_json(path: Path) -> Any:
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in items:
            require(key not in result, f"duplicate_json_key:{key}")
            result[key] = value
        return result

    try:
        return json.loads(path.read_bytes(), object_pairs_hook=pairs)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CandidateError(f"invalid_json:{path.name}") from error


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def digest(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical_root(value: Any) -> str:
    return digest(canonical_bytes(value))


def exact_int(value: Any, expected: int, label: str) -> None:
    require(type(value) is int and value == expected, label)


def expected_request_bytes(
    adapter: str, run_input: dict[str, Any], packet: bytes, schema: bytes
) -> bytes:
    tool_specs = [
        {
            "name": "read_file",
            "description": "Read, list, stat, or literal-search exact UTF-8 evidence below the read-only /workspace assignment tree.",
            "input_schema": {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": False,
                "required": ["operation", "path", "query"],
                "properties": {
                    "operation": {
                        "enum": ["read", "list", "stat", "search"],
                        "type": "string",
                    },
                    "path": {"minLength": 1, "pattern": "^/", "type": "string"},
                    "query": {"maxLength": 256, "type": "string"},
                },
            },
        },
    ]
    sentinel = "__VELA_EXACT_PROVIDER_SCHEMA_BYTES__"
    if adapter == "openai-responses-v1":
        tools = [
            {
                "type": "function",
                "name": tool["name"],
                "description": tool["description"],
                "parameters": tool["input_schema"],
                "strict": True,
            }
            for tool in tool_specs
        ]
        value = {
            "model": run_input["model"],
            "background": False,
            "store": False,
            "parallel_tool_calls": False,
            "max_output_tokens": 32768,
            "reasoning": {"effort": "high"},
            "service_tier": "default",
            "input": [
                {
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": run_input["prompt"]},
                        {"type": "input_text", "text": packet.decode()},
                    ],
                }
            ],
            "tools": tools,
            "text": {
                "format": {
                    "type": "json_schema",
                    "name": "stage_a_response",
                    "schema": sentinel,
                    "strict": True,
                }
            },
        }
    else:
        value = {
            "model": run_input["model"],
            "max_tokens": 32768,
            "service_tier": "standard_only",
            "thinking": {"type": "adaptive"},
            "output_config": {
                "effort": "high",
                "format": {"type": "json_schema", "schema": sentinel},
            },
            "messages": [
                {
                    "role": "user",
                    "content": run_input["prompt"] + "\n" + packet.decode(),
                }
            ],
            "tools": tool_specs,
        }
    template = canonical_bytes(value)
    needle = json.dumps(sentinel).encode()
    require(template.count(needle) == 1, "request_schema_splice")
    return template.replace(needle, schema, 1)


def expected_request_custody(request: bytes, provider_schema: bytes) -> dict[str, Any]:
    return {
        "schema": "vela.lossless-provider-request-custody.v1",
        "content_type": "application/json",
        "bytes": len(request),
        "sha256": digest(request),
        "payload_encoding": "base64-rfc4648-canonical",
        "decode_count": 1,
        "provider_schema_bytes": len(provider_schema),
        "provider_schema_sha256": digest(provider_schema),
        "provider_schema_occurrences": 1,
        "endpoint_write_prepared": True,
    }


def validate_launchable_runtime(
    record: dict[str, Any], expected: dict[str, Any], adapter: str
) -> None:
    retained = record.get("retained")
    required = {
        "image",
        "launchability",
        "source_manifest",
        "build_a",
        "build_b",
        "runner",
        "bridge",
        "preflight",
        "provider_contract",
        "provider_schema",
        "tool_boundary",
        "held_permit",
        "hold_state",
        "neutral_packet",
        "neutral_prompt",
        "retired_permit",
        "run_input",
        "materialization_receipt",
        "offline_validation_receipt",
        "request_bytes",
        "request_transport_custody",
    }
    require(type(retained) is dict and set(retained) == required, "retained_set")
    image_raw = (PACKAGE / retained["image"]["path"]).read_bytes()
    runner_raw = (PACKAGE / retained["runner"]["path"]).read_bytes()
    bridge_raw = (PACKAGE / retained["bridge"]["path"]).read_bytes()
    preflight_raw = (PACKAGE / retained["preflight"]["path"]).read_bytes()
    contract = load_json(PACKAGE / retained["provider_contract"]["path"])
    launchability = load_json(PACKAGE / retained["launchability"]["path"])
    exact_int(launchability.get("provider_calls"), 0, "launchability_provider_calls")
    exact_int(
        launchability.get("endpoint_write_receipts"),
        0,
        "launchability_endpoint_write_receipts",
    )
    source_manifest = load_json(PACKAGE / retained["source_manifest"]["path"])
    neutral_packet = (PACKAGE / retained["neutral_packet"]["path"]).read_bytes()
    neutral_prompt = (PACKAGE / retained["neutral_prompt"]["path"]).read_bytes()
    held_permit = load_json(PACKAGE / retained["held_permit"]["path"])
    retirement = load_json(PACKAGE / retained["retired_permit"]["path"])
    run_raw = (PACKAGE / retained["run_input"]["path"]).read_bytes()
    run_input = load_json(PACKAGE / retained["run_input"]["path"])
    materialization = load_json(PACKAGE / retained["materialization_receipt"]["path"])
    validation = load_json(PACKAGE / retained["offline_validation_receipt"]["path"])
    request_raw = (PACKAGE / retained["request_bytes"]["path"]).read_bytes()
    request_transport_custody = load_json(
        PACKAGE / retained["request_transport_custody"]["path"]
    )
    provider_schema_raw = (PACKAGE / retained["provider_schema"]["path"]).read_bytes()
    start = materialization.get("raw_inserted_start")
    end = materialization.get("raw_inserted_end")
    for key, expected_number in (
        ("source_bytes", len(provider_schema_raw)),
        ("raw_inserted_start", start),
        ("raw_inserted_end", end),
    ):
        require(type(materialization.get(key)) is int, "materialized_range_type")
        if key == "source_bytes":
            exact_int(materialization.get(key), expected_number, "materialized_size")
    exact_int(validation.get("endpoint_write_receipts"), 0, "offline_validation_count")
    exact_int(validation.get("provider_calls"), 0, "offline_validation_count")
    for key, expected_number in (
        ("request_bytes", len(request_raw)),
        ("bridge_decoded_request_bytes", len(request_raw)),
        ("bridge_decode_count", 1),
        ("provider_schema_occurrences", 1),
    ):
        exact_int(validation.get(key), expected_number, f"offline_validation:{key}")
    for key, expected_number in (
        ("bytes", len(request_raw)),
        ("decode_count", 1),
        ("provider_schema_bytes", len(provider_schema_raw)),
        ("provider_schema_occurrences", 1),
    ):
        exact_int(
            request_transport_custody.get(key),
            expected_number,
            f"request_transport_custody:{key}",
        )
    require(
        materialization
        == {
            "schema": "vela.stage-a-run-input-materialization.v1",
            "source_path": "/input/provider-schema.json",
            "source_regular": True,
            "source_single_link": True,
            "source_no_follow": True,
            "source_pre_post_same_inode": True,
            "source_bytes": len(provider_schema_raw),
            "source_sha256": digest(provider_schema_raw),
            "raw_inserted_start": start,
            "raw_inserted_end": end,
            "raw_inserted_sha256": digest(provider_schema_raw),
            "run_json_sha256": digest(run_raw),
            "mounted_schema_root": digest(provider_schema_raw),
            "request_schema_sha256": digest(provider_schema_raw),
            "parse_reserialization_used": False,
        }
        and end == start + len(provider_schema_raw)
        and run_raw[start:end] == provider_schema_raw,
        "materialization_custody",
    )
    require(
        run_input.get("run_id") == expected["run_id"]
        and run_input.get("provider_schema_path") == "/input/provider-schema.json"
        and type(run_input.get("provider_schema_bytes")) is int
        and run_input.get("provider_schema_bytes") == len(provider_schema_raw)
        and run_input.get("provider_schema_sha256") == digest(provider_schema_raw)
        and run_input.get("materialization_receipt_path")
        == "/input/materialization-receipt.json"
        and run_input.get("packet_path") == "/input/packet.json"
        and run_input.get("output_dir") == "/evidence",
        "run_input_binding",
    )
    require(
        validation
        == {
            "schema": "vela.stage-a-offline-pre-request-validation.v1",
            "status": "pass",
            "adapter": adapter,
            "run_id": expected["run_id"],
            "run_json_sha256": digest(run_raw),
            "mounted_schema_root": digest(provider_schema_raw),
            "request_schema_sha256": digest(provider_schema_raw),
            "request_sha256": digest(request_raw),
            "request_bytes": len(request_raw),
            "request_payload_sha256": digest(request_raw),
            "request_payload_encoding": "base64-rfc4648-canonical",
            "bridge_decoded_request_sha256": digest(request_raw),
            "bridge_decoded_request_bytes": len(request_raw),
            "bridge_decode_count": 1,
            "provider_schema_occurrences": 1,
            "endpoint_write_prepared": True,
            "participant_validation_path": "exact_runner_prepare_lossless_frame_bridge_decode_and_write_preparation",
            "dummy_credential_fd": True,
            "credential_secret": False,
            "endpoint_contact_forbidden": True,
            "endpoint_write_receipts": 0,
            "provider_calls": 0,
        }
        and request_raw.count(provider_schema_raw) == 1
        and request_transport_custody
        == expected_request_custody(request_raw, provider_schema_raw)
        and request_raw
        == expected_request_bytes(
            adapter, run_input, neutral_packet, provider_schema_raw
        ),
        "offline_same_input_validation",
    )
    retired = EXPECTED_RETIRED_PERMITS[adapter]
    require(
        digest(neutral_packet) == EXPECTED_NEUTRAL_CONTENT["packet_root"]
        and neutral_packet
        == (PACKAGE / EXPECTED_NEUTRAL_CONTENT["packet_path"]).read_bytes()
        and digest(neutral_prompt) == EXPECTED_NEUTRAL_CONTENT["prompt_root"]
        and neutral_prompt
        == (PACKAGE / EXPECTED_NEUTRAL_CONTENT["prompt_path"]).read_bytes(),
        "neutral_content_retained_binding",
    )
    require(
        held_permit.get("provider_schema_bytes") == expected["provider_schema_bytes"]
        and held_permit.get("configuration_root") == expected["configuration_root"]
        and held_permit.get("image_digest") == expected["image_digest"]
        and held_permit.get("runtime_source_root") == expected["runtime_source_root"]
        and held_permit.get("run_id") == expected["run_id"]
        and held_permit.get("packet_root") == EXPECTED_NEUTRAL_CONTENT["packet_root"]
        and held_permit.get("prompt_root") == EXPECTED_NEUTRAL_CONTENT["prompt_root"]
        and held_permit.get("status") == "held"
        and held_permit.get("consumed_at") is None,
        "neutral_permit_packet_binding",
    )
    require(
        retirement
        == {
            "schema": "vela.stage-a-neutral-permit-retirement.v1",
            "provider_adapter": adapter,
            "run_id": retired["run_id"],
            "original_permit_root": retired["permit_root"],
            "invalid_permit_origin_commit": EXPECTED_CORRECTIVE_ANCESTRY[
                "invalid_permit_origin_commit"
            ],
            "invalid_permit_origin_relationship": EXPECTED_CORRECTIVE_ANCESTRY[
                "invalid_permit_origin_relationship"
            ],
            "reviewed_predecessor_commit": EXPECTED_CORRECTIVE_ANCESTRY[
                "reviewed_predecessor_commit"
            ],
            "reviewed_predecessor_parent_commit": EXPECTED_CORRECTIVE_ANCESTRY[
                "reviewed_predecessor_parent_commit"
            ],
            "original_state": "held_unconsumed",
            "retirement_reason": "packet_root_preimage_is_plaintext_not_runner_loadable_canonical_json",
            "successor_permit_root": expected["participant_permit_root"],
            "status": "retired_non_releasable",
            "consumed": False,
            "releasable": False,
            "authority_effect": "none",
        },
        "retired_permit_binding",
    )
    expected_events = {
        "raw_bytes_retained_before_normalization": True,
        "terminal_and_teardown_receipts_required": True,
        "tool_arguments_retained": True,
        "tool_results_retained": True,
        "usage_retained_as_telemetry_only": True,
    }
    if adapter == "openai-responses-v1":
        expected_events.update(
            {
                "decoded_argument_bytes_retained": True,
                "function_call_arguments_decode_count": 1,
                "function_call_arguments_wire_type": "json_string",
                "raw_to_decoded_argument_binding_required": True,
            }
        )
    require(
        canonical_root(source_manifest) == expected["runtime_source_root"],
        "retained_source_root",
    )
    for label, builder in (("build_a", "independent-a"), ("build_b", "independent-b")):
        receipt = load_json(PACKAGE / retained[label]["path"])
        require(
            receipt.get("builder") == builder
            and receipt.get("empty_cache") is True
            and receipt.get("network_during_build") is False
            and receipt.get("source_root") == expected["runtime_source_root"]
            and receipt.get("image_digest") == expected["image_digest"]
            and receipt.get("oci_tar_bytes") == digest(image_raw),
            "retained_build_receipt",
        )
    require(
        digest((PACKAGE / retained["launchability"]["path"]).read_bytes())
        == expected["launchability_sha256"],
        "launchability_bytes",
    )
    require(
        contract.get("provider_adapter") == adapter
        and contract.get("endpoint")
        == EXPECTED_FROZEN_CONFIGURATION[adapter]["api"]["endpoint"]
        and contract.get("transport")
        == {
            "credential_fd": 4,
            "credential_retained": False,
            "host_bridge_fd": 3,
            "host_bridge_single_endpoint": EXPECTED_FROZEN_CONFIGURATION[adapter][
                "api"
            ]["endpoint"],
            "participant_network": False,
            "proxy_environment": "ignored",
            "redirects": "rejected",
            "unrestricted_clients": False,
            "provider_request_frame": "lossless_canonical_base64_exact_bytes",
            "provider_request_payload_schema": "vela.lossless-provider-request-payload.v1",
            "provider_request_custody_schema": "vela.lossless-provider-request-custody.v1",
            "payload_encoding": "base64-rfc4648-canonical",
            "payload_decode_count": 1,
            "endpoint_write": "decoded_payload_bytes_without_json_reserialization",
        },
        "provider_contract_boundary",
    )
    require(
        contract.get("tool_lifecycle")
        == {
            "max_tool_calls": 64,
            "parallel_tool_calls": False,
            "sequential_call_result_pairs": True,
            "max_output_bytes": 65536,
            "per_call_timeout_seconds": 30,
        }
        and contract.get("tools")
        == [
            {
                "name": "read_file",
                "workspace": "/workspace",
                "operations": ["read", "list", "stat", "search"],
                "regular_files_only": True,
                "symlinks": False,
                "hardlinks": False,
                "descriptor_relative_no_follow": True,
                "path_escape": False,
                "write": False,
                "network": False,
                "query_max_bytes": 256,
            },
        ]
        and contract.get("events") == expected_events
        and contract.get("packet_input")
        == {
            "mount_path": "/input/packet.json",
            "regular_file_only": True,
            "single_link_only": True,
            "no_follow": True,
            "canonical_json_object": True,
            "recursive_duplicate_keys_rejected": True,
            "recursive_objects_arrays_primitives_canonical": True,
            "number_lexemes_preserved": True,
            "inline_reconstruction": False,
            "permit_byte_root_required": True,
            "request_byte_root_receipt_required": True,
            "injection": (
                "input[0].content[1].text_exact_packet_bytes"
                if adapter == "openai-responses-v1"
                else "messages[0].content_exact_prompt_newline_packet_bytes"
            ),
        },
        "provider_contract_tool_custody",
    )
    require(
        launchability
        == {
            "schema": "vela.stage-a-runtime-launchability.v1",
            "provider_adapter": adapter,
            "oci_archive_sha256": digest(image_raw),
            "image_digest": expected["image_digest"],
            "layer_digest": launchability.get("layer_digest"),
            "platform": "linux/arm64",
            "entrypoint": ["/opt/vela/runner"],
            "self_test": {
                "network": "none",
                "root_filesystem": "read_only",
                "capabilities": "all_dropped",
                "no_new_privileges": True,
                "exit_code": 0,
                "stdout_sha256": digest(b""),
                "stderr_sha256": digest(b""),
                "runner_version": "neutral-runner/1",
                "host_bridge_self_test": True,
            },
            "provider_calls": 0,
            "endpoint_write_receipts": 0,
            "offline_pre_request_validation": {
                "status": "pass",
                "network": "none",
                "same_run_input": True,
                "dummy_non_secret_credential_fd": True,
                "receipt_sha256": digest(
                    (
                        PACKAGE / retained["offline_validation_receipt"]["path"]
                    ).read_bytes()
                ),
            },
            "credential_values_observed": False,
        },
        "launchability_receipt",
    )
    try:
        with tarfile.open(fileobj=io.BytesIO(image_raw), mode="r") as archive:
            outer = {
                item.name: archive.extractfile(item).read()
                for item in archive.getmembers()
                if item.isfile()
            }
        index = json.loads(outer["index.json"])
        manifest_digest = index["manifests"][0]["digest"]
        manifest = json.loads(
            outer["blobs/sha256/" + manifest_digest.removeprefix("sha256:")]
        )
        config_digest = manifest["config"]["digest"]
        config = json.loads(
            outer["blobs/sha256/" + config_digest.removeprefix("sha256:")]
        )
        layer_digest = manifest["layers"][0]["digest"]
        layer_raw = outer["blobs/sha256/" + layer_digest.removeprefix("sha256:")]
        with tarfile.open(fileobj=io.BytesIO(layer_raw), mode="r") as layer:
            members = {item.name: item for item in layer.getmembers()}
            files = {
                name: layer.extractfile(item).read() for name, item in members.items()
            }
    except (KeyError, IndexError, tarfile.TarError, json.JSONDecodeError) as error:
        raise CandidateError("launchable_oci_invalid") from error
    require(manifest_digest == expected["image_digest"], "image_manifest_binding")
    require(
        config.get("os") == "linux"
        and config.get("architecture") == "arm64"
        and config.get("config", {}).get("User") == "65532:65532"
        and config.get("config", {}).get("Entrypoint") == ["/opt/vela/runner"]
        and config.get("config", {}).get("Labels", {}).get("org.vela.runtime-mode")
        == "held-host-bridge-network-none",
        "oci_runtime_config",
    )
    require(
        set(files)
        == {
            "opt/vela/runner",
            "opt/vela/bridge",
            "opt/vela/preflight",
            "opt/vela/provider-contract.json",
            "etc/ssl/certs/ca-certificates.crt",
        }
        and files["opt/vela/runner"] == runner_raw
        and files["opt/vela/bridge"] == bridge_raw
        and files["opt/vela/preflight"] == preflight_raw
        and members["opt/vela/runner"].mode == 0o755
        and members["opt/vela/bridge"].mode == 0o755
        and members["opt/vela/preflight"].mode == 0o755
        and digest(layer_raw) == layer_digest == launchability["layer_digest"],
        "oci_launchable_rootfs",
    )


def git_value(*arguments: str) -> str:
    return subprocess.run(
        ["git", *arguments], cwd=REPOSITORY, check=True, capture_output=True, text=True
    ).stdout.strip()


def validate_offline(value: dict[str, Any]) -> dict[str, dict[str, Any]]:
    require(
        type(value) is dict
        and set(value)
        == {
            "schema",
            "status",
            "qualifier",
            "trust_bundle_sha256",
            "prior_consumed_non_call",
            "prior_consumed_failed_exact_request",
            "provider_records",
            "provider_call_derivation",
            "provider_calls",
            "neutral_calibrations_run",
            "participant_calls",
            "authority_effect",
            "record_root",
        },
        "offline_record_not_closed",
    )
    exact_int(
        value.get("prior_consumed_non_call", {}).get("provider_calls"),
        0,
        "prior_consumed_provider_calls",
    )
    require(
        value.get("schema")
        == "vela.lean-correspondence-stage-a-offline-runtime-qualification.v1",
        "offline_schema",
    )
    require(value.get("status") == "offline_qualified_hold", "offline_status")
    for key in ("provider_calls", "neutral_calibrations_run", "participant_calls"):
        exact_int(value.get(key), 0, f"offline_counter:{key}")
    require(value.get("authority_effect") == "none", "offline_authority")
    for key in (
        "controller",
        "bridge",
        "runner",
        "terminal",
        "custody",
        "endpoint_write_receipts",
    ):
        exact_int(
            value.get("provider_call_derivation", {}).get(key),
            0,
            f"provider_call_derivation:{key}",
        )
    require(
        value.get("prior_consumed_non_call") == EXPECTED_PRIOR_CONSUMED_NON_CALL,
        "prior_consumed_non_call",
    )
    exact_int(
        value.get("prior_consumed_failed_exact_request", {}).get("provider_calls"),
        1,
        "prior_consumed_failed_exact_request_provider_calls",
    )
    require(
        value.get("prior_consumed_failed_exact_request")
        == EXPECTED_PRIOR_CONSUMED_FAILED_EXACT_REQUEST,
        "prior_consumed_failed_exact_request",
    )
    require(
        value.get("provider_call_derivation") == EXPECTED_CALL_DERIVATION,
        "provider_call_derivation",
    )
    require(
        digest(
            (PACKAGE / EXPECTED_CALL_DERIVATION["controller_source_path"]).read_bytes()
        )
        == EXPECTED_CALL_DERIVATION["controller_source_sha256"],
        "provider_call_controller_source",
    )
    require(
        load_json(STOPPED / "artifact-root.json").get("artifact_root")
        == EXPECTED_PRIOR_CONSUMED_NON_CALL["artifact_root"]
        and digest((STOPPED / "endpoint-contact-receipt.json").read_bytes())
        == EXPECTED_PRIOR_CONSUMED_NON_CALL["endpoint_contact_receipt_bytes"]
        and digest(
            (
                STOPPED
                / "permit/neutral-calibration-anthropic-json-v2.permit.consumed.json"
            ).read_bytes()
        )
        == EXPECTED_PRIOR_CONSUMED_NON_CALL["consumed_permit_bytes"],
        "prior_consumed_non_call_bytes",
    )
    require(
        load_json(STOPPED_V3 / "artifact-root.json").get("artifact_root")
        == EXPECTED_PRIOR_CONSUMED_FAILED_EXACT_REQUEST["artifact_root"]
        and digest((STOPPED_V3 / "raw/endpoint-contact-receipt.json").read_bytes())
        == EXPECTED_PRIOR_CONSUMED_FAILED_EXACT_REQUEST[
            "endpoint_contact_receipt_bytes"
        ]
        and digest(
            (
                STOPPED_V3
                / "permit/neutral-calibration-anthropic-json-v3-replacement.permit.consumed.json"
            ).read_bytes()
        )
        == EXPECTED_PRIOR_CONSUMED_FAILED_EXACT_REQUEST["consumed_permit_bytes"],
        "prior_consumed_failed_exact_request_bytes",
    )
    body = dict(value)
    observed_root = body.pop("record_root", None)
    require(observed_root == canonical_root(body), "offline_inner_root")
    qualifier = value.get("qualifier")
    require(
        qualifier
        == {
            "commit": EXPECTED_QUALIFIER["git_commit"],
            "tree": EXPECTED_QUALIFIER["git_tree"],
            "sha256": EXPECTED_QUALIFIER["sha256"],
        },
        "offline_qualifier_binding",
    )
    records = value.get("provider_records")
    require(type(records) is list and len(records) == 2, "offline_provider_count")
    by_adapter: dict[str, dict[str, Any]] = {}
    for record in records:
        adapter = record.get("provider_adapter")
        require(
            adapter in EXPECTED_PROVIDER and adapter not in by_adapter,
            "offline_adapter",
        )
        expected = EXPECTED_PROVIDER[adapter]
        require(
            record.get("provider_organization") == expected["organization"],
            "offline_provider",
        )
        require(record.get("model") == expected["model"], "offline_model")
        require(
            record.get("held_neutral_run_id") == expected["run_id"], "offline_run_id"
        )
        exact_int(record.get("provider_calls"), 0, "offline_provider_calls")
        exact_int(
            record.get("endpoint_write_receipts"),
            0,
            "offline_endpoint_write_receipts",
        )
        require(
            record.get("consumed_neutral_permit_exists") is False,
            "offline_early_permit_consume",
        )
        require(
            record.get("permit_state")
            == "held_non_releasable_pending_independent_review"
            and record.get("offline_pre_request_validation") == "pass",
            "offline_permit_preflight_state",
        )
        receipt = record.get("qualification_receipt")
        require(
            type(receipt) is dict and receipt.get("status") == "qualified_hold",
            "qualifier_receipt_status",
        )
        for key in (
            "provider_calls",
            "scientific_sessions",
            "participant_permits_consumed",
        ):
            exact_int(receipt.get(key), 0, f"qualifier_counter:{key}")
        require(receipt.get("authority_effect") == "none", "qualifier_authority")
        for key in (
            "configuration_root",
            "qualification_root",
            "image_digest",
            "tool_boundary_root",
            "runtime_source_root",
            "participant_permit_root",
            "provider_schema_bytes",
        ):
            require(receipt.get(key) == expected[key], f"qualifier_binding:{key}")
        gates = receipt.get("gates")
        require(
            type(gates) is dict
            and gates
            and all(item is True for item in gates.values()),
            "qualifier_gate",
        )
        for retained in record.get("retained", {}).values():
            require(
                type(retained) is dict and set(retained) == {"path", "bytes", "sha256"},
                "retained_shape",
            )
            path = PACKAGE / retained["path"]
            require(
                path.resolve().is_relative_to(PACKAGE.resolve())
                and path.is_file()
                and not path.is_symlink(),
                "retained_path",
            )
            raw = path.read_bytes()
            require(
                type(retained["bytes"]) is int and retained["bytes"] == len(raw),
                "retained_size",
            )
            require(retained["sha256"] == digest(raw), "retained_digest")
        validate_launchable_runtime(record, expected, adapter)
        by_adapter[adapter] = record
    require(
        len({item["qualification_receipt"]["image_digest"] for item in records}) == 2,
        "provider_images_not_distinct",
    )
    require(
        len(
            {
                item["qualification_receipt"]["participant_permit_root"]
                for item in records
            }
        )
        == 2,
        "neutral_permits_not_distinct",
    )
    require(
        len(
            {
                item["qualification_receipt"]["provider_equivalence_root"]
                for item in records
            }
        )
        == 1,
        "provider_equivalence_drift",
    )
    require(observed_root == EXPECTED_OFFLINE_RECORD_ROOT, "offline_record_root")
    return by_adapter


def validate_registration(
    value: dict[str, Any], records: dict[str, dict[str, Any]], *, check_git: bool
) -> None:
    require(
        value.get("schema")
        == "vela.lean-correspondence-stage-a-runtime-qualification-candidate.v5",
        "registration_schema",
    )
    require(
        value.get("status") == "held_offline_validated_pending_independent_review",
        "registration_status",
    )
    require(value.get("authority_effect") == "none", "registration_authority")
    calls = value.get("calls", {})
    for key in (
        "authentication_requests",
        "model_requests",
        "participant_calls",
        "provider_calls",
        "schema_compilation_requests",
    ):
        exact_int(calls.get(key), 0, f"call_ledger:{key}")
    require(
        value.get("maintained_qualifier") == EXPECTED_QUALIFIER, "qualifier_binding"
    )
    require(
        value.get("corrective_ancestry") == EXPECTED_CORRECTIVE_ANCESTRY,
        "corrective_ancestry",
    )
    require(value.get("stage_a_binding") == EXPECTED_STAGE_A, "stage_a_binding")
    require(
        value.get("prior_consumed_non_call") == EXPECTED_PRIOR_CONSUMED_NON_CALL,
        "registration_prior_consumed_non_call",
    )
    require(
        value.get("prior_consumed_failed_exact_request")
        == EXPECTED_PRIOR_CONSUMED_FAILED_EXACT_REQUEST,
        "registration_prior_consumed_failed_exact_request",
    )
    require(
        value.get("provider_call_derivation") == EXPECTED_CALL_DERIVATION,
        "registration_provider_call_derivation",
    )
    authorization = value.get("authorization", {})
    require(
        authorization.get("neutral_calibration_execution_authorized") is False
        and authorization.get("participant_execution_authorized") is False,
        "early_authorization",
    )
    blockers = value.get("blockers")
    require(
        type(blockers) is list
        and [item.get("id") for item in blockers]
        == ["independent_exact_review_required"],
        "blockers_drift",
    )
    schema = value.get("provider_schema_boundary", {})
    expected_rules = [
        ["/properties/impact_closure/uniqueItems", "uniqueItems", True],
        [
            "/properties/impact_closure/items/properties/evidence_ids/minItems",
            "minItems",
            1,
        ],
        [
            "/properties/impact_closure/items/properties/evidence_ids/uniqueItems",
            "uniqueItems",
            True,
        ],
        ["/properties/uncertainty/uniqueItems", "uniqueItems", True],
    ]
    require(
        schema.get("maintained_registry_rules") == expected_rules
        and schema.get("status") == "qualified_exact_maintained_four_rule_registry",
        "participant_schema_registry",
    )
    require(
        digest((STAGE_A / "response.schema.json").read_bytes())
        == schema.get("authoritative_schema_sha256"),
        "stage_a_schema_bytes",
    )
    derivatives = schema.get("participant_provider_derivatives")
    require(
        type(derivatives) is list
        and derivatives
        == [
            {
                "provider_adapter": adapter,
                "provider_schema_sha256": EXPECTED_PROVIDER[adapter][
                    "provider_schema_bytes"
                ],
            }
            for adapter in ("openai-responses-v1", "anthropic-messages-v1")
        ],
        "participant_schema_derivatives",
    )
    permits = value.get("neutral_calibration_permits")
    require(type(permits) is list and len(permits) == 2, "neutral_permit_count")
    for permit in permits:
        require(
            permit.get("status") == "held_non_releasable_pending_independent_review"
            and permit.get("consumed") is False,
            "neutral_permit_released",
        )
        expected = EXPECTED_PROVIDER.get(permit.get("provider_adapter"))
        require(
            expected is not None
            and permit.get("provider_organization") == expected["organization"]
            and permit.get("run_id") == expected["run_id"],
            "neutral_permit_cross_binding",
        )
        require(
            permit.get("permit_root") == expected["participant_permit_root"],
            "neutral_permit_root",
        )
        require(
            permit.get("offline_pre_request_validation") == "pass",
            "neutral_permit_offline_validation",
        )
    retired_permits = value.get("retired_neutral_calibration_permits")
    require(
        type(retired_permits) is list and len(retired_permits) == 2,
        "retired_neutral_permit_count",
    )
    for retirement in retired_permits:
        adapter = retirement.get("provider_adapter")
        expected = EXPECTED_PROVIDER.get(adapter)
        retired = EXPECTED_RETIRED_PERMITS.get(adapter)
        require(expected is not None and retired is not None, "retired_permit_adapter")
        require(
            retirement.get("run_id") == retired["run_id"]
            and retirement.get("original_permit_root") == retired["permit_root"]
            and retirement.get("invalid_permit_origin_commit")
            == EXPECTED_CORRECTIVE_ANCESTRY["invalid_permit_origin_commit"]
            and retirement.get("invalid_permit_origin_relationship")
            == EXPECTED_CORRECTIVE_ANCESTRY["invalid_permit_origin_relationship"]
            and retirement.get("reviewed_predecessor_commit")
            == EXPECTED_CORRECTIVE_ANCESTRY["reviewed_predecessor_commit"]
            and retirement.get("reviewed_predecessor_parent_commit")
            == EXPECTED_CORRECTIVE_ANCESTRY["reviewed_predecessor_parent_commit"]
            and retirement.get("successor_permit_root")
            == expected["participant_permit_root"]
            and retirement.get("original_state") == "held_unconsumed"
            and retirement.get("status") == "retired_non_releasable"
            and retirement.get("consumed") is False
            and retirement.get("releasable") is False,
            "retired_permit_state",
        )
    content = value.get("neutral_calibration_content")
    require(
        content
        == {
            **EXPECTED_NEUTRAL_CONTENT,
            "packet_bytes": len(
                (PACKAGE / EXPECTED_NEUTRAL_CONTENT["packet_path"]).read_bytes()
            ),
            "prompt_bytes": len(
                (PACKAGE / EXPECTED_NEUTRAL_CONTENT["prompt_path"]).read_bytes()
            ),
            "runner_packet_mount_path": "/input/packet.json",
            "inline_packet_allowed": False,
            "request_binding": "exact request.raw.json SHA-256 bound in packet custody and terminal receipts",
        }
        and load_json(PACKAGE / "neutral-calibration/content-equivalence.json")
        == EXPECTED_NEUTRAL_CONTENT,
        "neutral_content_equivalence",
    )
    configurations = value.get("participant_configurations")
    require(
        type(configurations) is list and len(configurations) == 2, "configuration_count"
    )
    for configuration in configurations:
        expected = EXPECTED_PROVIDER.get(configuration.get("provider_adapter"))
        require(expected is not None, "configuration_adapter")
        receipt = records[configuration["provider_adapter"]]["qualification_receipt"]
        require(
            configuration.get("provider_organization") == expected["organization"]
            and configuration.get("model") == expected["model"],
            "configuration_provider_model",
        )
        frozen = EXPECTED_FROZEN_CONFIGURATION[configuration["provider_adapter"]]
        require(
            configuration.get("api") == frozen["api"]
            and configuration.get("model_snapshot_semantics")
            == frozen["model_snapshot_semantics"]
            and configuration.get("parameters") == frozen["parameters"],
            "frozen_configuration_drift",
        )
        for key in (
            "configuration_root",
            "image_digest",
            "qualification_root",
            "tool_boundary_root",
        ):
            require(
                configuration.get(key) == receipt.get(key) == expected[key],
                f"configuration_cross_binding:{key}",
            )
        require(
            configuration.get("status")
            == "candidate_configuration_exact_schema_pre_request_validated_held",
            "configuration_status",
        )
    offline = value.get("offline_qualification", {})
    require(
        offline
        == {
            "authority_effect": "none",
            "neutral_calibrations_run": 0,
            "path": "offline-qualification.json",
            "provider_calls": 0,
            "record_root": EXPECTED_OFFLINE_RECORD_ROOT,
            "status": "qualified_hold_exact_schema_launchable_runtimes_and_offline_same_input_preflight",
        },
        "registration_offline_binding",
    )
    runtime = value.get("runtime_boundary", {})
    require(
        runtime.get("image_role") == "launchable_provider_specific_held_runtime"
        and runtime.get("host_bridge")
        == "single_exact_provider_endpoint_owned_outside_networkless_participant_image"
        and runtime.get("mounts_read_only") is True
        and runtime.get("network_during_offline_qualification") is False
        and runtime.get("participant_network_until_authorized") is False
        and runtime.get("writes") is False
        and runtime.get("tool_mode") == "read_only_offline_files"
        and runtime.get("provider_equivalence_root")
        == "sha256:bc40341349f6f771be5eef2481fcef3bf72d278b2df65d5df05d01e62e271720"
        and runtime.get("run_input_materialization")
        == "exact_raw_schema_file_byte_splice_no_parse_reserialization"
        and runtime.get("offline_same_input_pre_request_validation") is True
        and runtime.get("provider_calls_derived_from_endpoint_write_receipts_only")
        is True
        and runtime.get("provider_request_transport")
        == "canonical_base64_lossless_single_decode_exact_endpoint_write"
        and runtime.get("provider_request_payload_schema")
        == "vela.lossless-provider-request-payload.v1"
        and runtime.get("provider_request_custody_schema")
        == "vela.lossless-provider-request-custody.v1",
        "runtime_boundary",
    )
    runtime_images = runtime.get("runtime_images")
    require(type(runtime_images) is list and len(runtime_images) == 2, "runtime_images")
    for image in runtime_images:
        adapter = image.get("provider_adapter")
        expected = EXPECTED_PROVIDER.get(adapter)
        require(expected is not None, "runtime_image_adapter")
        require(
            image
            == {
                "provider_adapter": adapter,
                "image_digest": expected["image_digest"],
                "runtime_source_root": expected["runtime_source_root"],
                "launchability_receipt_sha256": expected["launchability_sha256"],
                "run_input_sha256": records[adapter]["retained"]["run_input"]["sha256"],
                "materialization_receipt_sha256": records[adapter]["retained"][
                    "materialization_receipt"
                ]["sha256"],
                "offline_validation_receipt_sha256": records[adapter]["retained"][
                    "offline_validation_receipt"
                ]["sha256"],
                "request_bytes_sha256": records[adapter]["retained"]["request_bytes"][
                    "sha256"
                ],
                "request_transport_custody_sha256": records[adapter]["retained"][
                    "request_transport_custody"
                ]["sha256"],
            },
            "runtime_image_binding",
        )
    credentials = value.get("credentials")
    require(type(credentials) is list and len(credentials) == 2, "credential_count")
    require(
        {item.get("environment_name") for item in credentials}
        == {"OPENAI_API_KEY", "ANTHROPIC_API_KEY"},
        "credential_names",
    )
    for credential in credentials:
        require(
            credential.get("presence") == "not_checked_in_this_correction"
            and credential.get("retained") is False
            and credential.get("value_observed") is False,
            "credential_state",
        )
    raw = b"".join(
        (PACKAGE / name).read_bytes()
        for name in ("README.md", "registration.json", "offline-qualification.json")
    )
    require(b"sk-" not in raw and b"Bearer " not in raw, "credential_shaped_bytes")
    if check_git:
        require(
            git_value(
                "rev-parse",
                f"{EXPECTED_CORRECTIVE_ANCESTRY['reviewed_predecessor_commit']}^",
            )
            == EXPECTED_CORRECTIVE_ANCESTRY["reviewed_predecessor_parent_commit"],
            "corrective_parent_ancestry",
        )
        ancestry = subprocess.run(
            [
                "git",
                "merge-base",
                "--is-ancestor",
                EXPECTED_CORRECTIVE_ANCESTRY["invalid_permit_origin_commit"],
                EXPECTED_CORRECTIVE_ANCESTRY["reviewed_predecessor_commit"],
            ],
            cwd=REPOSITORY,
            check=False,
        )
        require(
            ancestry.returncode == 0
            and EXPECTED_CORRECTIVE_ANCESTRY["invalid_permit_origin_commit"]
            != EXPECTED_CORRECTIVE_ANCESTRY["reviewed_predecessor_parent_commit"],
            "corrective_origin_ancestry",
        )
        require(
            git_value("rev-parse", f"{EXPECTED_STAGE_A['pilot_commit']}^{{tree}}")
            == EXPECTED_STAGE_A["pilot_tree"],
            "stage_a_tree",
        )
        require(
            git_value(
                "rev-parse",
                f"{EXPECTED_STAGE_A['pilot_commit']}:paper/artifacts/lean-correspondence-stage-a-open-pilot",
            )
            == EXPECTED_STAGE_A["pilot_directory_tree"],
            "stage_a_directory_tree",
        )
        require(
            git_value("rev-parse", f"{EXPECTED_QUALIFIER['git_commit']}^{{tree}}")
            == EXPECTED_QUALIFIER["git_tree"],
            "qualifier_tree",
        )
        qualifier = subprocess.run(
            [
                "git",
                "show",
                f"{EXPECTED_QUALIFIER['git_commit']}:{EXPECTED_QUALIFIER['path']}",
            ],
            cwd=REPOSITORY,
            check=True,
            capture_output=True,
        ).stdout
        require(
            digest(qualifier) == EXPECTED_QUALIFIER["sha256"],
            "qualifier_bytes",
        )
        require(
            git_value(
                "rev-parse",
                f"{EXPECTED_PRIOR_CONSUMED_FAILED_EXACT_REQUEST['producer_commit']}^{{tree}}",
            )
            == EXPECTED_PRIOR_CONSUMED_FAILED_EXACT_REQUEST["producer_tree"],
            "prior_failed_exact_request_tree",
        )
        historical_parent = subprocess.run(
            [
                "git",
                "merge-base",
                "--is-ancestor",
                EXPECTED_CORRECTIVE_ANCESTRY[
                    "prospective_successor_direct_parent_commit"
                ],
                "HEAD",
            ],
            cwd=REPOSITORY,
            check=False,
        )
        require(historical_parent.returncode == 0, "prospective_successor_ancestry")
    require(canonical_root(value) == EXPECTED_REGISTRATION_ROOT, "registration_root")


def validate_artifact_root(value: dict[str, Any]) -> str:
    require(
        value.get("schema")
        == "vela.lean-correspondence-stage-a-runtime-qualification-artifact-root.v2",
        "artifact_schema",
    )
    entries = []
    for path in sorted(PACKAGE.rglob("*")):
        if (
            not path.is_file()
            or path == ARTIFACT_ROOT
            or "__pycache__" in path.parts
            or ".ruff_cache" in path.parts
            or path.suffix == ".pyc"
        ):
            continue
        raw = path.read_bytes()
        entries.append(
            {
                "path": path.relative_to(PACKAGE).as_posix(),
                "bytes": len(raw),
                "sha256": digest(raw),
            }
        )
    require(value.get("entries") == entries, "artifact_entries")
    body = {"schema": value["schema"], "entries": entries}
    require(value.get("artifact_root") == canonical_root(body), "artifact_root")
    return value["artifact_root"]


def verify(*, check_credentials: bool = True, check_git: bool = True) -> dict[str, Any]:
    offline = load_json(OFFLINE)
    records = validate_offline(offline)
    validate_registration(load_json(REGISTRATION), records, check_git=check_git)
    artifact_root = validate_artifact_root(load_json(ARTIFACT_ROOT))
    if check_credentials:
        unexpected = sorted(
            name
            for name in ("OPENAI_API_KEY", "ANTHROPIC_API_KEY")
            if name in os.environ
        )
        require(not unexpected, "credential_presence_drift")
    return {
        "schema": "vela.lean-correspondence-stage-a-runtime-qualification-verification.v3",
        "status": "pass_exact_held_offline_validation_pending_independent_review",
        "artifact_root": artifact_root,
        "offline_record_root": EXPECTED_OFFLINE_RECORD_ROOT,
        "provider_calls": 0,
        "neutral_calibrations_run": 0,
        "participant_calls": 0,
        "participant_permits_released": 0,
        "authority_effect": "none",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skip-credential-presence", action="store_true")
    args = parser.parse_args()
    try:
        result = verify(check_credentials=not args.skip_credential_presence)
    except (CandidateError, subprocess.CalledProcessError) as error:
        print(json.dumps({"status": "blocked", "error": str(error)}, sort_keys=True))
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
