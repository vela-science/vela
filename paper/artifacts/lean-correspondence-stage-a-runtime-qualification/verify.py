#!/usr/bin/env python3
"""Fail-closed verification of the offline-qualified, still-held candidate."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
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
    "sha256:f4a5e56bc6b17cee3a9efd6d0ff91eaf743e2200302a5fda37c337f66838f6d0"
)
EXPECTED_OFFLINE_RECORD_ROOT = (
    "sha256:74d2e5d47fc5f5165444cf1908bc2408829e2642ea6a01948aea793b763e559a"
)
EXPECTED_QUALIFIER = {
    "git_commit": "586c305915f9f192822a720df7fd5abf416d9439",
    "git_tree": "59c1847e8b4a8f57ba515febc487b0ce0e68c37f",
    "path": "tools/evidence_qualification/qualification.py",
    "sha256": "sha256:6db638f5cec4df9eac53fe8edc2376fcc4db89afe3f08b977d47873669c41ddc",
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
        "configuration_root": "sha256:62cb5930870d03623d5428fd207fa215ffa6805d985f2e829f500301c87567c7",
        "qualification_root": "sha256:9425cb0b8d01745a736d218d00149ac1debd7569040bfb97dc60ab5e95069a0c",
        "image_digest": "sha256:cefa48877aedfd755e5b53aa402b52f0b6176c14931e8a6cdf30562381101ef3",
        "tool_boundary_root": "sha256:fe029f490165b034571b516ffed1a6af63f1873a1fc890203060b7a81daf4b74",
    },
    "anthropic-messages-v1": {
        "organization": "Anthropic",
        "model": "claude-opus-5",
        "run_id": "neutral-calibration-anthropic",
        "configuration_root": "sha256:437f303c5df25118f70869cebe5179e61459f7518cf0229442a144ffbc2f7e23",
        "qualification_root": "sha256:2048cfabbb4b4d8a2c865c38055b61ea3c9af7dcc395d02ad13c8cf82755e2bd",
        "image_digest": "sha256:fc5dc5837c1c6e43e174d206371291454cf6fe0431b7b99b99df8cfa3e63dd5f",
        "tool_boundary_root": "sha256:5b10101846b4408b59cd5eb6482c23469d363bf73ebf08363915339585986af2",
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
        == "vela.lean-correspondence-stage-a-runtime-qualification-candidate.v2",
        "registration_schema",
    )
    require(
        value.get("status")
        == "held_offline_qualified_schema_registry_and_credentials_blocked",
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
        == [
            "stage_a_participant_schema_not_registered_by_qualifier",
            "provider_runtime_execution_images_not_materialized",
            "platform_api_credentials_absent",
        ],
        "blockers_drift",
    )
    schema = value.get("provider_schema_boundary", {})
    require(
        schema.get("participant_provider_derivatives") == []
        and schema.get("status") == "held_exact_registry_mismatch",
        "participant_schema_not_held",
    )
    require(
        digest((STAGE_A / "response.schema.json").read_bytes())
        == schema.get("authoritative_schema_sha256"),
        "stage_a_schema_bytes",
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
            == "candidate_configuration_offline_fixture_qualified_held",
            "configuration_status",
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
        "schema": "vela.lean-correspondence-stage-a-runtime-qualification-verification.v2",
        "status": "pass_exact_held_offline_qualification_with_schema_and_credentials_blockers",
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
