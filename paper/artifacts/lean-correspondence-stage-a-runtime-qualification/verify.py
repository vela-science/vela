#!/usr/bin/env python3
"""Verify the held Stage A runtime qualification candidate without provider use."""

from __future__ import annotations

import argparse
import copy
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
TOOL_POLICY = PACKAGE / "tool-policy.json"
ARTIFACT_ROOT = PACKAGE / "artifact-root.json"
SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")


class CandidateError(ValueError):
    """The held candidate drifted or weakened a mandatory boundary."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise CandidateError(message)


def load_json(path: Path) -> Any:
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in items:
            require(key not in value, f"duplicate_json_key:{key}")
            value[key] = item
        return value

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


def git_value(*arguments: str) -> str:
    result = subprocess.run(
        ["git", *arguments],
        cwd=REPOSITORY,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def information_boundary_root(registration: dict[str, Any], tool_root: str) -> str:
    return canonical_root(
        {
            "stage_a_directory_tree": registration["stage_a_binding"][
                "pilot_directory_tree"
            ],
            "response_schema_sha256": registration["provider_schema_derivation"][
                "authoritative_schema_sha256"
            ],
            "tool_policy_sha256": tool_root,
        }
    )


def validate_candidate(
    registration: dict[str, Any], tool_policy: dict[str, Any], *, check_git: bool
) -> dict[str, Any]:
    require(
        registration.get("schema")
        == "vela.lean-correspondence-stage-a-runtime-qualification-candidate.v1",
        "registration_schema_invalid",
    )
    require(
        registration.get("status") == "held_blocked_exact_qualifier_and_credentials",
        "registration_not_held",
    )
    require(registration.get("authority_effect") == "none", "authority_effect")

    calls = registration.get("calls", {})
    require(
        calls
        == {
            "authentication_requests": 0,
            "model_requests": 0,
            "provider_calls": 0,
            "schema_compilation_requests": 0,
        },
        "provider_call_ledger_invalid",
    )

    permits = registration.get("permits", {})
    require(permits.get("neutral_calibration") == [], "early_permit_creation")
    require(
        permits.get("neutral_calibration_status") == "withheld_not_created",
        "early_permit_release",
    )
    require(permits.get("pilot_permits_released") == 0, "early_pilot_release")
    require(permits.get("pilot_permits_consumed") == 0, "early_pilot_consume")
    require(permits.get("pilot_permits_touched") is False, "pilot_permits_touched")

    runtime = registration.get("runtime_contract", {})
    for key in (
        "fresh_context",
        "fresh_process",
        "no_hidden_answer_access",
        "no_persistent_memory",
        "no_repo_mutation",
        "no_scheduler",
    ):
        require(runtime.get(key) is True, f"runtime_boundary_mutable:{key}")
    require(runtime.get("retry_paths") == "absent", "scheduler_or_retry_path")
    require(runtime.get("runner_source_root") is None, "unqualified_runner_root")
    require(
        runtime.get("provider_specific_adapter_source_roots") == [],
        "unqualified_adapter_root",
    )

    require(
        tool_policy.get("schema")
        == "vela.lean-correspondence-stage-a-read-only-tool-policy.v1",
        "tool_policy_schema_invalid",
    )
    require(
        tool_policy.get("filesystem", {}).get("assignment_mount") == "read_only"
        and tool_policy["filesystem"].get("root_filesystem") == "read_only"
        and tool_policy["filesystem"].get("repository_mutation") == "forbidden",
        "mutable_tool_boundary",
    )
    require(
        tool_policy.get("network", {}).get("participant_tool_network") == "none"
        and tool_policy["network"].get("server_hosted_tools") == "forbidden",
        "participant_network_or_extra_tools",
    )
    tool_names = [tool.get("name") for tool in tool_policy.get("tools", [])]
    require(
        tool_names == ["list_files", "read_file", "search_text", "run_read_only_shell"],
        "tool_set_substitution",
    )
    require(
        tool_policy.get("read_only_shell", {}).get("executables_byte_bound_in_image")
        is True,
        "mutable_shell_executable_boundary",
    )
    tool_root = digest(TOOL_POLICY.read_bytes()) if TOOL_POLICY.exists() else ""
    boundary_root = information_boundary_root(registration, tool_root)

    configurations = registration.get("participant_configurations", [])
    require(len(configurations) == 2, "participant_configuration_count")
    exact = {
        "configuration-a": ("OpenAI", "gpt-5.6-sol"),
        "configuration-b": ("Anthropic", "claude-opus-5"),
    }
    require(
        {item.get("slot_id") for item in configurations} == set(exact),
        "configuration_slot_substitution",
    )
    organizations = set()
    for configuration in configurations:
        slot = configuration["slot_id"]
        provider, model = exact[slot]
        require(
            (configuration.get("provider_organization"), configuration.get("model"))
            == (provider, model),
            "provider_or_model_substitution",
        )
        organizations.add(provider)
        require(
            configuration.get("status") == "selected_held_unqualified",
            "configuration_early_qualification",
        )
        require(
            configuration.get("tool_policy_sha256") == tool_root
            and configuration.get("information_boundary_root") == boundary_root,
            "cross_provider_atom_or_tool_mismatch",
        )
        parameters = configuration.get("parameters", {})
        require(
            parameters.get("timeout_seconds") == 1200
            and parameters.get("retries") == 0
            and parameters.get("temperature") == "omitted",
            "runtime_parameter_drift",
        )
        body = copy.deepcopy(configuration)
        observed_root = body.pop("configuration_root", None)
        require(observed_root == canonical_root(body), "configuration_root_drift")
    require(len(organizations) == 2, "same_provider_organization_substitution")
    require(
        configurations[0]["information_boundary_root"]
        == configurations[1]["information_boundary_root"],
        "cross_provider_information_boundary_mismatch",
    )

    expected_endpoints = {
        "OpenAI": "https://api.openai.com/v1/responses",
        "Anthropic": "https://api.anthropic.com/v1/messages",
    }
    for configuration in configurations:
        api = configuration.get("api", {})
        require(
            api.get("endpoint")
            == expected_endpoints[configuration["provider_organization"]],
            "provider_endpoint_substitution",
        )
        require(
            api.get("provider_cli") == "none" and api.get("sdk") == "none_raw_https",
            "unbound_cli_or_sdk",
        )

    schema = registration.get("provider_schema_derivation", {})
    require(schema.get("provider_derivatives") == [], "unsupported_schema_claimed")
    require(
        schema.get("unproved_keywords_present") == ["minItems", "minLength", "pattern"],
        "unsupported_response_schema_not_held",
    )
    require(
        schema.get("exact_qualifier_allowed_deletions") == ["uniqueItems=true"],
        "qualifier_schema_allowlist_drift",
    )

    qualifier = registration.get("maintained_qualifier", {})
    require(qualifier.get("copied") is False, "qualifier_copy_forbidden")
    require(
        qualifier.get("reuse_mode") == "invoke_repository_path_at_exact_blob",
        "qualifier_reuse_mode",
    )
    require(
        qualifier.get("qualification_receipt_root") is None,
        "stale_or_early_qualifier_receipt",
    )
    require(
        qualifier.get("blob") == "be1982fc09c8d859b7da131c242de243e6f989b8"
        and qualifier.get("sha256")
        == "sha256:628ac203a48ef19c649dd64dedc010d104d728eb0edbb66392e93955fab872b9",
        "qualifier_root_drift",
    )

    credentials = registration.get("credentials", [])
    require(len(credentials) == 2, "credential_prerequisite_count")
    required_names = {"OPENAI_API_KEY", "ANTHROPIC_API_KEY"}
    require(
        {item.get("environment_name") for item in credentials} == required_names,
        "credential_class_substitution",
    )
    for item in credentials:
        require(
            item.get("presence") == "absent"
            and item.get("retained") is False
            and item.get("value_observed") is False,
            "credential_retention_or_status_drift",
        )
        require(
            item.get("injection")
            == "anonymous_inherited_descriptor_read_once_then_zeroed",
            "credential_injection_boundary",
        )

    image = registration.get("image_boundary", {})
    require(
        image.get("status") == "withheld_not_materialized"
        and image.get("images") == []
        and image.get("absolute_read_only_mounts") == []
        and image.get("trust_bundle_root") is None,
        "missing_or_drifted_trust_runtime_image_roots_not_held",
    )

    gates = registration.get("offline_gates", {})
    require(
        gates.get("maintained_qualifier_tool_surface") == "fail"
        and gates.get("provider_schema_derivation") == "fail",
        "offline_blocker_erased",
    )
    require(
        registration.get("authorization", {}).get(
            "neutral_calibration_separately_authorizable"
        )
        is False
        and registration["authorization"].get("pilot_execution_authorized") is False,
        "early_authorization",
    )

    raw_artifact = b"".join(
        (PACKAGE / name).read_bytes()
        for name in ("README.md", "registration.json", "tool-policy.json")
    )
    require(b"sk-" not in raw_artifact, "possible_credential_value_retained")

    if check_git:
        stage = registration["stage_a_binding"]
        method = registration["method_binding"]
        require(
            git_value("rev-parse", f"{stage['pilot_commit']}^{{tree}}")
            == stage["pilot_tree"],
            "stage_a_tree_drift",
        )
        require(
            git_value(
                "rev-parse",
                f"{stage['pilot_commit']}:paper/artifacts/lean-correspondence-stage-a-open-pilot",
            )
            == stage["pilot_directory_tree"],
            "stage_a_participant_bytes_drift",
        )
        require(
            git_value(
                "rev-parse",
                f"{method['producer_commit']}:paper/artifacts/lean-correspondence-foundry-study",
            )
            == method["reviewed_method_directory_tree"],
            "reviewed_method_tree_drift",
        )
        qualifier_path = REPOSITORY / qualifier["path"]
        require(
            git_value("hash-object", str(qualifier_path)) == qualifier["blob"]
            and digest(qualifier_path.read_bytes()) == qualifier["sha256"],
            "maintained_qualifier_blob_drift",
        )
        response_path = REPOSITORY / schema["authoritative_registered_schema"]
        require(
            digest(response_path.read_bytes()) == schema["authoritative_schema_sha256"],
            "registered_response_schema_drift",
        )

    return {
        "schema": "vela.lean-correspondence-stage-a-runtime-qualification-verification.v1",
        "status": "pass_exact_held_blocker",
        "authority_effect": "none",
        "provider_calls": 0,
        "neutral_calibration_permits": 0,
        "registration_root": digest(REGISTRATION.read_bytes()),
        "tool_policy_root": tool_root,
        "information_boundary_root": boundary_root,
        "configuration_roots": [
            configuration["configuration_root"] for configuration in configurations
        ],
        "qualification_receipt_root": None,
        "neutral_calibration_separately_authorizable": False,
    }


def validate_artifact_root(value: dict[str, Any]) -> str:
    require(
        value.get("schema")
        == "vela.lean-correspondence-stage-a-runtime-qualification-artifact-root.v1",
        "artifact_root_schema_invalid",
    )
    entries = value.get("entries")
    require(isinstance(entries, list) and entries, "artifact_root_entries_invalid")
    derived = []
    for path in sorted(PACKAGE.iterdir()):
        if path.is_file() and path.name != ARTIFACT_ROOT.name:
            raw = path.read_bytes()
            derived.append(
                {"path": path.name, "bytes": len(raw), "sha256": digest(raw)}
            )
    require(entries == derived, "artifact_entry_drift")
    body = {"schema": value["schema"], "entries": entries}
    observed = value.get("artifact_root")
    require(observed == canonical_root(body), "artifact_root_drift")
    return observed


def verify(*, check_credentials: bool = True, check_git: bool = True) -> dict[str, Any]:
    registration = load_json(REGISTRATION)
    tool_policy = load_json(TOOL_POLICY)
    receipt = validate_candidate(registration, tool_policy, check_git=check_git)
    receipt["artifact_root"] = validate_artifact_root(load_json(ARTIFACT_ROOT))
    if check_credentials:
        unexpected = sorted(
            item["environment_name"]
            for item in registration["credentials"]
            if item["environment_name"] in os.environ
        )
        require(not unexpected, "credential_presence_drift")
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skip-credential-presence", action="store_true")
    args = parser.parse_args()
    try:
        receipt = verify(check_credentials=not args.skip_credential_presence)
    except (CandidateError, subprocess.CalledProcessError) as error:
        print(json.dumps({"status": "blocked", "error": str(error)}, sort_keys=True))
        return 2
    print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
