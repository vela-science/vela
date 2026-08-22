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
SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")
EXPECTED_REGISTRATION_ROOT = (
    "sha256:17712ef57556cb3d0c3039d2250f5e3147b5741591e676e38c8410eec97c08f7"
)
EXPECTED_OFFLINE_RECORD_ROOT = (
    "sha256:a822929b2266b1a0dfd0ca212de64773c88d94ee779980f4c04a34cc58dbfe4e"
)
EXPECTED_QUALIFIER = {
    "git_commit": "cc3b88d8bfcfd7b4f720a023f049d5c365be9423",
    "git_tree": "341e0d22fa570b1b5e8dd9f70b219c11308ba45f",
    "path": "tools/evidence_qualification/qualification.py",
    "sha256": "sha256:61591eec3304e299a9344888bc2a6f08cd32785b647ef5b0107da490dbf18013",
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
        "run_id": "neutral-calibration-openai",
        "configuration_root": "sha256:96555c45c33ed2a106cfb261025b752a4eeb1514aa180985ecd5ea0551a6616d",
        "qualification_root": "sha256:4f1bead055215f9b3c27247d8a3a07a7caae25ed343b9f3cfda3c329effbcccb",
        "image_digest": "sha256:a51a4742979549f6d77511c722e86d8425f73c45b5b6681c6756d1d949b199d6",
        "tool_boundary_root": "sha256:0b2e1fb701f70b02f9cc7ad79201f84374dfeb904299b59a6667d36eb4e59c69",
        "runtime_source_root": "sha256:27daaf77de66f872fc9fbbbfc69bb6dd0bd03e982da2db4de1db85600b843f13",
        "participant_permit_root": "sha256:96a9c8af3d079ab8c73dd8eaaca05d62eebde2c70efe97a192b462edf2f7ff03",
        "provider_schema_bytes": "sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7",
        "launchability_sha256": "sha256:564712aa1486ec8c603b4f9120fbed2fd353273e9f77eaa1c0754452336f7a5b",
    },
    "anthropic-messages-v1": {
        "organization": "Anthropic",
        "model": "claude-opus-5",
        "run_id": "neutral-calibration-anthropic",
        "configuration_root": "sha256:10a9a0569f63a523e7dd6dab768c9dc255aa244c026337f217142cd2a1483163",
        "qualification_root": "sha256:a653ae75b0c600366ee78db2092ab904bc9540d94dc1ad14b110adb10e047002",
        "image_digest": "sha256:cd460011d0527da49eeeaf343bf76d6fdeb745b15ea1051e9ac3454c72dd480d",
        "tool_boundary_root": "sha256:01dfbda69c1c7760423fdba41eaac18687a73d9fe683a8a5f207fdc8abe2a7d9",
        "runtime_source_root": "sha256:3104933b541c577a1521445981928e5acfb1a55019622f53c59874dd52c96fb5",
        "participant_permit_root": "sha256:4bed98283ffb3af24ed0c99d7d4e135276770fef8288c11fbe87e9c8b0d37b9f",
        "provider_schema_bytes": "sha256:f34dc8c6ded17e94d2f3a9389112eb1bdfa59e3b9977f7a5f994e473bef70ad7",
        "launchability_sha256": "sha256:bc1842c8747bfacddf2c8cb2f15ac4e875ba2e7a96b50bef304b1284de03b3ef",
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
        "provider_contract",
        "provider_schema",
        "tool_boundary",
        "held_permit",
        "hold_state",
    }
    require(type(retained) is dict and set(retained) == required, "retained_set")
    image_raw = (PACKAGE / retained["image"]["path"]).read_bytes()
    runner_raw = (PACKAGE / retained["runner"]["path"]).read_bytes()
    bridge_raw = (PACKAGE / retained["bridge"]["path"]).read_bytes()
    contract = load_json(PACKAGE / retained["provider_contract"]["path"])
    launchability = load_json(PACKAGE / retained["launchability"]["path"])
    source_manifest = load_json(PACKAGE / retained["source_manifest"]["path"])
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
        },
        "provider_contract_boundary",
    )
    require(
        contract.get("tools")
        == [
            {
                "name": "shell",
                "allowed_argv": [
                    "git",
                    "--no-optional-locks",
                    "status",
                    "--short",
                ],
                "cwd": "/workspace",
                "read_only": True,
                "shell_interpolation": False,
            },
            {
                "name": "read_file",
                "workspace": "/workspace",
                "operations": ["read", "list", "stat"],
                "regular_files_only": True,
                "symlinks": False,
                "path_escape": False,
                "write": False,
            },
        ]
        and contract.get("events") == expected_events,
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
            "opt/vela/provider-contract.json",
            "etc/ssl/certs/ca-certificates.crt",
        }
        and files["opt/vela/runner"] == runner_raw
        and files["opt/vela/bridge"] == bridge_raw
        and members["opt/vela/runner"].mode == 0o755
        and members["opt/vela/bridge"].mode == 0o755
        and digest(layer_raw) == layer_digest == launchability["layer_digest"],
        "oci_launchable_rootfs",
    )


def git_value(*arguments: str) -> str:
    return subprocess.run(
        ["git", *arguments], cwd=REPOSITORY, check=True, capture_output=True, text=True
    ).stdout.strip()


def validate_offline(value: dict[str, Any]) -> dict[str, dict[str, Any]]:
    require(
        value.get("schema")
        == "vela.lean-correspondence-stage-a-offline-runtime-qualification.v1",
        "offline_schema",
    )
    require(value.get("status") == "offline_qualified_hold", "offline_status")
    for key in ("provider_calls", "neutral_calibrations_run", "participant_calls"):
        exact_int(value.get(key), 0, f"offline_counter:{key}")
    require(value.get("authority_effect") == "none", "offline_authority")
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
        require(
            record.get("consumed_neutral_permit_exists") is False,
            "offline_early_permit_consume",
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
        == "vela.lean-correspondence-stage-a-runtime-qualification-candidate.v3",
        "registration_schema",
    )
    require(
        value.get("status")
        == "held_offline_qualified_launchable_runtime_credentials_only_blocked",
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
    require(value.get("stage_a_binding") == EXPECTED_STAGE_A, "stage_a_binding")
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
        == ["platform_api_credentials_absent"],
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
            permit.get("status") == "held" and permit.get("consumed") is False,
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
            == "candidate_configuration_stage_a_schema_launchable_runtime_qualified_held",
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
            "status": "qualified_hold_exact_stage_a_schema_and_launchable_runtimes",
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
        and runtime.get("tool_mode") == "read_only_offline_shell_files"
        and runtime.get("provider_equivalence_root")
        == "sha256:bc40341349f6f771be5eef2481fcef3bf72d278b2df65d5df05d01e62e271720",
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
            credential.get("presence") == "absent"
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
        "status": "pass_exact_held_offline_qualification_with_credentials_only_blocker",
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
