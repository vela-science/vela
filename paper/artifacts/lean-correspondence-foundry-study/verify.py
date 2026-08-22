#!/usr/bin/env python3
"""Deterministically verify the frozen protocol artifact; perform no study work."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from decimal import ROUND_HALF_EVEN, Decimal, localcontext
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parent
HEX40 = set("0123456789abcdef")


class VerificationError(ValueError):
    pass


def load_json(path: Path) -> Any:
    def reject_duplicate(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise VerificationError(f"duplicate_json_key:{path.name}:{key}")
            result[key] = value
        return result

    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate,
            parse_float=lambda value: (_ for _ in ()).throw(
                VerificationError(f"binary_float_forbidden:{path.name}:{value}")
            ),
        )
    except (OSError, json.JSONDecodeError) as error:
        raise VerificationError(f"json_invalid:{path.name}") from error


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def quantize(value: Decimal, places: int) -> str:
    quantum = Decimal(1).scaleb(-places)
    with localcontext() as context:
        context.prec = 50
        return format(value.quantize(quantum, rounding=ROUND_HALF_EVEN), f".{places}f")


def restricted_mean(values: list[str | None], cap: str) -> tuple[list[str], str]:
    cap_decimal = Decimal(cap)
    restricted = [
        cap_decimal if item is None else min(Decimal(item), cap_decimal)
        for item in values
    ]
    mean = sum(restricted, Decimal(0)) / Decimal(len(restricted))
    return [quantize(value, 9) for value in restricted], quantize(mean, 9)


def strict_lift(assisted: int, control: int, minimum: int) -> bool:
    return assisted > control and assisted - control >= minimum


def require(condition: bool, code: str) -> None:
    if not condition:
        raise VerificationError(code)


def verify_manifest() -> str:
    manifest = load_json(ROOT / "artifact-manifest.json")
    require(
        manifest.get("schema")
        == "vela.lean-correspondence-foundry-artifact-manifest.v1",
        "manifest_schema",
    )
    entries = manifest.get("entries")
    require(isinstance(entries, list) and entries, "manifest_entries")
    paths: list[str] = []
    for entry in entries:
        require(set(entry) == {"path", "bytes", "sha256"}, "manifest_entry_fields")
        path = entry["path"]
        require(
            isinstance(path, str)
            and "/" not in path
            and path != "artifact-manifest.json",
            "manifest_path",
        )
        target = ROOT / path
        require(
            target.is_file() and not target.is_symlink(), f"manifest_missing:{path}"
        )
        require(target.stat().st_size == entry["bytes"], f"manifest_bytes:{path}")
        require(sha256(target) == entry["sha256"], f"manifest_sha256:{path}")
        paths.append(path)
    require(paths == sorted(paths), "manifest_order")
    actual_material = sorted(
        path.name
        for path in ROOT.iterdir()
        if path.is_file()
        and path.name not in {"artifact-manifest.json"}
        and not path.name.startswith(".")
        and not path.name.endswith(".pyc")
    )
    require(paths == actual_material, "manifest_completeness")
    root = (
        "sha256:"
        + hashlib.sha256(
            canonical_bytes({"entries": entries, "schema": manifest["schema"]})
        ).hexdigest()
    )
    require(root == manifest.get("artifact_root"), "artifact_root")
    return root


def verify_contract(contract: dict[str, Any]) -> None:
    require(
        contract.get("status") == "method_frozen_execution_forbidden", "contract_status"
    )
    require(contract.get("authority_effect") == "none", "authority_effect")
    require(contract.get("scientific_result_obtained") is False, "scientific_result")
    require(
        contract.get("selected_confirmatory_families") == [],
        "selected_families_present",
    )
    require(
        contract.get("protected_adjudication_created") is False,
        "protected_adjudication_present",
    )
    require(contract.get("participant_permits_created") is False, "permits_present")
    require(
        contract.get("provider_calls_authorized") is False, "provider_calls_authorized"
    )
    require(contract.get("scoring_authorized") is False, "scoring_authorized")

    stage_a = contract["stage_a"]
    expected_a = (
        stage_a["participant_configurations"]
        * len(stage_a["cases"])
        * len(stage_a["arms"])
        * stage_a["fresh_sessions_per_configuration_case_arm"]
    )
    require(expected_a == stage_a["fixed_denominator"] == 12, "stage_a_denominator")
    require(
        len(stage_a["cases"]) == 3
        and "deliberately-invalid-byte-identity" in stage_a["cases"],
        "stage_a_cases",
    )
    require(
        stage_a["zero_retries"] is True and stage_a["zero_substitutions"] is True,
        "stage_a_retry",
    )
    require(stage_a["raw_composite_exact_maximum"] < 6, "stage_a_ceiling")

    stage_b = contract["stage_b"]
    expected_b = (
        stage_b["families"]
        * len(stage_b["primary_arms"])
        * stage_b["participant_configurations"]
        * stage_b["fresh_sessions_per_family_arm_configuration"]
    )
    require(
        expected_b == stage_b["primary_fixed_denominator"] == 72, "stage_b_denominator"
    )
    require(stage_b["families"] == 6, "family_count")
    require(
        stage_b["minimum_independently_versioned_repositories"] >= 3, "repository_count"
    )
    require(
        stage_b["maximum_families_per_repository"]
        * stage_b["minimum_independently_versioned_repositories"]
        >= 6,
        "family_balance",
    )
    optional = stage_b["optional_control"]
    require(optional["control_included"] is None, "control_decided_early")
    require(
        stage_b["primary_fixed_denominator"] + optional["additional_cells"]
        == optional["expanded_fixed_denominator"]
        == 108,
        "control_denominator",
    )
    require(
        stage_b["zero_retries"] is True and stage_b["zero_substitutions"] is True,
        "stage_b_retry",
    )
    require(stage_b["one_scoring_attempt"] is True, "scoring_attempt")

    expected_estimands = {
        "relation_validation_accuracy",
        "semantic_change_vs_environment_drift_accuracy",
        "downstream_impact_completeness",
        "false_authority_or_scientific_inference",
        "restricted_review_time",
    }
    require(set(contract["estimands"]) == expected_estimands, "estimands")
    aggregate = contract["aggregate_gates"]
    require(aggregate["equality_counts_as_positive_lift"] is False, "equality_gate")
    require(aggregate["speed_alone_can_pass"] is False, "speed_alone")
    require(aggregate["assisted_false_inference_maximum"] == 0, "aggregate_safety")
    require(
        Decimal(aggregate["restricted_time_ratio_maximum"]) == Decimal("0.8"),
        "time_ratio",
    )
    family = contract["per_family_gate"]
    require(family["assisted_composite_exact_minimum"] == 5, "family_correctness")
    require(
        family["assisted_composite_strict_increment_minimum"] > 0, "family_strict_lift"
    )
    require(family["assisted_false_inference_maximum"] == 0, "family_safety")
    require(
        contract["custody"]["copied_harness_code_forbidden"] is True, "copied_harness"
    )
    require(
        contract["custody"]["prelaunch_qualification_receipt_required"] is True,
        "qualifier_receipt",
    )
    stage_c = contract["stage_c"]
    require(
        stage_c
        == {
            "implemented": False,
            "derived_read_only": True,
            "protocol_object": False,
            "database_writer": False,
            "global_identifier": False,
            "standing_transport": False,
            "reconstruction_command_specified": True,
            "browser_qa_required": True,
        },
        "stage_c_boundary",
    )


def verify_bindings(bindings: dict[str, Any], repo: Path) -> None:
    main = bindings["vela_current_main"]
    require(
        len(main["commit"]) == 40 and set(main["commit"]) <= HEX40, "main_commit_shape"
    )
    require(len(main["tree"]) == 40 and set(main["tree"]) <= HEX40, "main_tree_shape")
    bound_commit = subprocess.check_output(
        ["git", "-C", str(repo), "rev-parse", f"{main['commit']}^{{commit}}"],
        text=True,
    ).strip()
    bound_tree = subprocess.check_output(
        ["git", "-C", str(repo), "rev-parse", f"{main['commit']}^{{tree}}"],
        text=True,
    ).strip()
    require(bound_commit == main["commit"], "vela_commit_drift")
    require(bound_tree == main["tree"], "vela_tree_drift")
    require(
        subprocess.run(
            [
                "git",
                "-C",
                str(repo),
                "merge-base",
                "--is-ancestor",
                main["commit"],
                "HEAD",
            ],
            check=False,
        ).returncode
        == 0,
        "vela_main_not_ancestor",
    )
    qualifier = bindings["maintained_evidence_qualifier"]
    qualifier_bytes = subprocess.check_output(
        ["git", "-C", str(repo), "show", f"{main['commit']}:{qualifier['path']}"]
    )
    require(
        hashlib.sha256(qualifier_bytes).hexdigest() == qualifier["sha256"],
        "qualifier_sha256",
    )
    qualifier_blob = subprocess.check_output(
        [
            "git",
            "-C",
            str(repo),
            "rev-parse",
            f"{main['commit']}:{qualifier['path']}",
        ],
        text=True,
    ).strip()
    require(qualifier_blob == qualifier["blob"], "qualifier_blob")
    require(
        qualifier["required_future_receipt_root"] is None,
        "unexpected_qualification_receipt",
    )
    prior = bindings["prior_36_cell_negative_result"]
    require(prior["fixed_denominator"] == 36, "prior_denominator")
    require(
        (
            prior["git_documents_exact"],
            prior["state_wrapper_exact"],
            prior["vela_exact"],
        )
        == ("12/12", "12/12", "11/12"),
        "prior_result_drift",
    )
    require(
        prior["vela_authority_errors"] == 1
        and prior["positive_gate"] == "not_supported",
        "prior_gate_drift",
    )
    require(prior["reuse"] == "forbidden", "prior_reuse")
    calibration = bindings["visible_calibration_packets"]
    require(
        len(calibration["cases"]) == 2
        and calibration["verdict"] == "PASS_GO_FOR_IMPORT",
        "calibration_binding",
    )
    require(
        bindings["lean_correspondence_kernel"]["verdict"] == "PASS", "kernel_verdict"
    )
    require(
        bindings["vela_web_plural_authority_frontiers"]["authority_effect"] == "none",
        "web_authority",
    )


def verify_schemas() -> None:
    response = load_json(ROOT / "response.schema.json")
    foundry = load_json(ROOT / "foundry-packet.schema.json")
    Draft202012Validator.check_schema(response)
    Draft202012Validator.check_schema(foundry)
    require(response.get("additionalProperties") is False, "response_open")
    require(foundry.get("additionalProperties") is False, "foundry_open")
    require(
        foundry["properties"]["authority_effect"].get("const") == "none",
        "foundry_authority",
    )
    require(
        foundry["properties"]["repository_authority_contexts"]["items"].get(
            "additionalProperties"
        )
        is False,
        "authority_context_open",
    )
    require(
        "global_standing" not in canonical_bytes(foundry).decode(),
        "global_standing_surface",
    )


def verify_scoring_fixtures() -> None:
    fixture = load_json(ROOT / "scoring-fixtures.json")
    require(fixture["no_scientific_data"] is True, "scoring_data")
    restricted, mean = restricted_mean(
        fixture["elapsed_seconds"], fixture["time_cap_seconds"]
    )
    require(restricted == fixture["expected_restricted_seconds"], "restricted_values")
    require(mean == fixture["expected_restricted_mean"], "restricted_mean")
    rate = fixture["rate_fixture"]
    require(
        quantize(Decimal(rate["numerator"]) / Decimal(rate["denominator"]), 12)
        == rate["expected"],
        "decimal_rate",
    )
    for item in fixture["lift_fixtures"]:
        require(
            strict_lift(
                item["assisted"], item["control"], item["minimum_strict_increment"]
            )
            is item["expected_positive"],
            "strict_lift_fixture",
        )


def run(repo: Path) -> dict[str, Any]:
    artifact_root = verify_manifest()
    contract = load_json(ROOT / "study-contract.json")
    bindings = load_json(ROOT / "evidence-bindings.json")
    verify_contract(contract)
    verify_bindings(bindings, repo)
    verify_schemas()
    verify_scoring_fixtures()
    return {
        "schema": "vela.lean-correspondence-foundry-protocol-verification.v1",
        "status": "PASS",
        "artifact_root": artifact_root,
        "authority_effect": "none",
        "scientific_result": False,
        "execution_authorized": False,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vela", type=Path, default=ROOT.parents[2])
    args = parser.parse_args()
    try:
        result = run(args.vela.resolve())
    except (
        VerificationError,
        KeyError,
        TypeError,
        subprocess.CalledProcessError,
    ) as error:
        print(json.dumps({"status": "FAIL", "error": str(error)}, sort_keys=True))
        return 1
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    sys.exit(main())
