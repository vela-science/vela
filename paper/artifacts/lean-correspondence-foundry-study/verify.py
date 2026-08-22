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


COUNT_METRICS = (
    "relation_correct",
    "change_classification_correct",
    "impact_complete",
    "composite_exact",
    "false_inference",
)
CORRECTNESS_METRICS = (
    "relation_correct",
    "change_classification_correct",
    "impact_complete",
)
SCORE_ROW_FIELDS = ("denominator", *COUNT_METRICS)


def expand_score_fixture_case(item: dict[str, Any]) -> dict[str, Any]:
    rows = item["summary_rows"]
    require(
        set(rows)
        == {
            "partition_counts",
            "family_assisted",
            "family_raw",
            "configuration_assisted",
            "configuration_raw",
            "aggregate_assisted",
            "aggregate_raw",
            "restricted_time_ratio",
        },
        f"score_fixture_fields:{item['name']}",
    )

    def arm(row: list[Any]) -> dict[str, Any]:
        require(len(row) == len(SCORE_ROW_FIELDS), f"score_fixture_row:{item['name']}")
        return dict(zip(SCORE_ROW_FIELDS, row, strict=True))

    family_assisted = rows["family_assisted"]
    family_raw = rows["family_raw"]
    configuration_assisted = rows["configuration_assisted"]
    configuration_raw = rows["configuration_raw"]
    require(
        len(family_assisted) == len(family_raw),
        f"score_fixture_family_rows:{item['name']}",
    )
    require(
        len(configuration_assisted) == len(configuration_raw),
        f"score_fixture_configuration_rows:{item['name']}",
    )
    return {
        "partition_counts": rows["partition_counts"],
        "families": [
            {"assisted": arm(assisted), "raw": arm(raw)}
            for assisted, raw in zip(family_assisted, family_raw, strict=True)
        ],
        "configurations": [
            {"assisted": arm(assisted), "raw": arm(raw)}
            for assisted, raw in zip(
                configuration_assisted, configuration_raw, strict=True
            )
        ],
        "aggregate": {
            "assisted": arm(rows["aggregate_assisted"]),
            "raw": arm(rows["aggregate_raw"]),
            "restricted_time_ratio": rows["restricted_time_ratio"],
        },
    }


def verify_score_summary_feasibility(
    summary: dict[str, Any], contract: dict[str, Any]
) -> None:
    invariants = contract["score_summary_invariants"]
    family_count = invariants["family_rows"]
    configuration_count = invariants["configuration_rows"]
    family_denominator = invariants["family_cells_per_arm"]
    configuration_denominator = invariants["configuration_cells_per_arm"]
    aggregate_denominator = invariants["aggregate_cells_per_arm"]
    require(
        set(summary) == {"partition_counts", "families", "configurations", "aggregate"},
        "score_summary_fields",
    )
    require(
        summary["partition_counts"]
        == {
            "assisted": aggregate_denominator,
            "raw": aggregate_denominator,
            "total": invariants["primary_total_cells"],
        },
        "score_partition_counts",
    )
    require(len(summary["families"]) == family_count, "score_family_count")
    require(
        len(summary["configurations"]) == configuration_count,
        "score_configuration_count",
    )

    expected_arm_fields = {"denominator", *COUNT_METRICS}

    def verify_arm(arm: dict[str, Any], denominator: int, scope: str) -> None:
        require(set(arm) == expected_arm_fields, f"score_arm_fields:{scope}")
        require(arm["denominator"] == denominator, f"score_denominator:{scope}")
        for metric in COUNT_METRICS:
            count = arm[metric]
            require(
                isinstance(count, int)
                and not isinstance(count, bool)
                and 0 <= count <= denominator,
                f"score_count:{scope}:{metric}",
            )
        require(
            all(
                arm["composite_exact"] <= arm[metric] for metric in CORRECTNESS_METRICS
            ),
            f"score_composite_component_feasibility:{scope}",
        )

    for index, family in enumerate(summary["families"]):
        require(set(family) == {"assisted", "raw"}, f"score_family_fields:{index}")
        for arm_name in ("assisted", "raw"):
            verify_arm(
                family[arm_name], family_denominator, f"family:{index}:{arm_name}"
            )
    for index, configuration in enumerate(summary["configurations"]):
        require(
            set(configuration) == {"assisted", "raw"},
            f"score_configuration_fields:{index}",
        )
        for arm_name in ("assisted", "raw"):
            verify_arm(
                configuration[arm_name],
                configuration_denominator,
                f"configuration:{index}:{arm_name}",
            )
    aggregate = summary["aggregate"]
    require(
        set(aggregate) == {"assisted", "raw", "restricted_time_ratio"},
        "score_aggregate_fields",
    )
    require(
        Decimal(aggregate["restricted_time_ratio"]) >= Decimal(0),
        "score_time_ratio",
    )
    for arm_name in ("assisted", "raw"):
        verify_arm(aggregate[arm_name], aggregate_denominator, f"aggregate:{arm_name}")
        for metric in ("denominator", *COUNT_METRICS):
            aggregate_value = aggregate[arm_name][metric]
            family_sum = sum(item[arm_name][metric] for item in summary["families"])
            configuration_sum = sum(
                item[arm_name][metric] for item in summary["configurations"]
            )
            require(
                family_sum == aggregate_value,
                f"score_family_aggregate_sum:{arm_name}:{metric}",
            )
            require(
                configuration_sum == aggregate_value,
                f"score_configuration_aggregate_sum:{arm_name}:{metric}",
            )


def aggregate_flagship_gates_pass(
    summary: dict[str, Any], contract: dict[str, Any]
) -> bool:
    gates = contract["aggregate_gates"]
    pairs = (
        (
            "relation_correct",
            "assisted_relation_correct_minimum",
            "assisted_relation_strict_increment_minimum",
        ),
        (
            "change_classification_correct",
            "assisted_change_classification_correct_minimum",
            "assisted_change_classification_strict_increment_minimum",
        ),
        (
            "impact_complete",
            "assisted_impact_complete_minimum",
            "assisted_impact_strict_increment_minimum",
        ),
        (
            "composite_exact",
            "assisted_composite_exact_minimum",
            "assisted_composite_strict_increment_minimum",
        ),
    )
    for metric, minimum, increment in pairs:
        assisted = summary["assisted"][metric]
        raw = summary["raw"][metric]
        if assisted < gates[minimum] or not strict_lift(
            assisted, raw, gates[increment]
        ):
            return False
    return summary["assisted"]["false_inference"] <= gates[
        "assisted_false_inference_maximum"
    ] and Decimal(summary["restricted_time_ratio"]) <= Decimal(
        gates["restricted_time_ratio_maximum"]
    )


def family_flagship_gates_pass(
    families: list[dict[str, Any]], contract: dict[str, Any]
) -> bool:
    gate = contract["per_family_gate"]
    if len(families) != contract["stage_b"]["families"]:
        return False
    for family in families:
        assisted = family["assisted"]
        raw = family["raw"]
        if assisted["composite_exact"] < gate["assisted_composite_exact_minimum"]:
            return False
        if not strict_lift(
            assisted["composite_exact"],
            raw["composite_exact"],
            gate["assisted_composite_strict_increment_minimum"],
        ):
            return False
        if any(
            assisted[metric] < raw[metric]
            for metric in (
                "relation_correct",
                "change_classification_correct",
                "impact_complete",
            )
        ):
            return False
        if assisted["false_inference"] > gate["assisted_false_inference_maximum"]:
            return False
    return True


def configuration_flagship_gates_pass(
    configurations: list[dict[str, Any]], contract: dict[str, Any]
) -> bool:
    gates = contract["aggregate_gates"]
    if len(configurations) != contract["stage_b"]["participant_configurations"]:
        return False
    for configuration in configurations:
        assisted = configuration["assisted"]
        raw = configuration["raw"]
        if gates["strict_composite_increment_required_per_configuration"] and not (
            assisted["composite_exact"] > raw["composite_exact"]
        ):
            return False
        predicates = (
            (
                "relation_correct",
                "relation_noninferiority_required_per_configuration",
                "relation_reversals_allowed_per_configuration",
            ),
            (
                "change_classification_correct",
                "change_classification_noninferiority_required_per_configuration",
                "change_classification_reversals_allowed_per_configuration",
            ),
            (
                "impact_complete",
                "impact_noninferiority_required_per_configuration",
                "impact_reversals_allowed_per_configuration",
            ),
        )
        for metric, noninferiority_key, reversals_key in predicates:
            reversal = assisted[metric] < raw[metric]
            if gates[noninferiority_key] and reversal:
                return False
            if not gates[reversals_key] and reversal:
                return False
        if assisted["false_inference"] > gates["assisted_false_inference_maximum"]:
            return False
        if assisted["false_inference"] > raw["false_inference"]:
            return False
    return True


def flagship_pass(summary: dict[str, Any], contract: dict[str, Any]) -> bool:
    verify_score_summary_feasibility(summary, contract)
    return (
        family_flagship_gates_pass(summary["families"], contract)
        and aggregate_flagship_gates_pass(summary["aggregate"], contract)
        and configuration_flagship_gates_pass(summary["configurations"], contract)
    )


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
        aggregate["relation_noninferiority_required_per_configuration"] is True,
        "configuration_relation_noninferiority",
    )
    require(
        aggregate["relation_reversals_allowed_per_configuration"] is False,
        "configuration_relation_reversal",
    )
    require(
        aggregate["flagship_requires_every_configuration_relation_gate"] is True,
        "configuration_relation_flagship",
    )
    require(
        aggregate["change_classification_noninferiority_required_per_configuration"]
        is True,
        "configuration_change_noninferiority",
    )
    require(
        aggregate["change_classification_reversals_allowed_per_configuration"] is False,
        "configuration_change_reversal",
    )
    require(
        aggregate["impact_noninferiority_required_per_configuration"] is True,
        "configuration_impact_noninferiority",
    )
    require(
        aggregate["impact_reversals_allowed_per_configuration"] is False,
        "configuration_impact_reversal",
    )
    require(
        aggregate["flagship_requires_every_configuration_correctness_and_safety_gate"]
        is True,
        "configuration_correctness_safety_flagship",
    )
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
        contract["score_summary_invariants"]
        == {
            "family_rows": 6,
            "configuration_rows": 2,
            "family_cells_per_arm": 6,
            "configuration_cells_per_arm": 18,
            "aggregate_cells_per_arm": 36,
            "primary_total_cells": 72,
            "composite_bounded_by_each_correctness_component": True,
            "family_and_configuration_margins_equal_aggregate": True,
            "arm_partition_counts_exact": True,
        },
        "score_summary_invariants",
    )
    require(
        contract["custody"]["copied_harness_code_forbidden"] is True, "copied_harness"
    )
    require(
        contract["custody"]["prelaunch_qualification_receipt_required"] is True,
        "qualifier_receipt",
    )
    custody = contract["custody"]
    require(
        custody["selected_family_assignment_binding_independent_review_required"]
        is True,
        "selected_binding_review_required",
    )
    require(
        custody["selected_binding_review_timing"]
        == "after held-out family and assignment binding freeze; before runtime qualification or any Stage B participant permit",
        "selected_binding_review_timing",
    )
    require(
        custody["selected_binding_review_exact_pass_required"] is True,
        "selected_binding_review_exact_pass",
    )
    require(
        custody["selected_binding_review_root_equality_required"] is True,
        "selected_binding_review_root_equality",
    )
    require(
        custody["binding_change_invalidates_selected_binding_review"] is True,
        "selected_binding_review_invalidation",
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


def verify_prelaunch_state(state: dict[str, Any], machine: dict[str, Any]) -> None:
    expected_states = {
        "method_frozen",
        "stage_a_passed",
        "selection_frozen_pending_independent_review",
        "selected_binding_independent_review_passed",
        "runtime_qualified",
        "ready_for_stage_b_permit_creation",
        "stopped",
    }
    require(
        machine.get("schema")
        == "vela.lean-correspondence-stage-b-prelaunch-state-machine.v1",
        "prelaunch_machine_schema",
    )
    require(machine.get("authority_effect") == "none", "prelaunch_machine_authority")
    require(
        set(machine.get("states", [])) == expected_states, "prelaunch_machine_states"
    )
    expected_transitions = {
        (
            "method_frozen",
            "stage_a_passed",
            "stage_a_independent_exact_pass",
            "must_remain_null",
        ),
        (
            "stage_a_passed",
            "selection_frozen_pending_independent_review",
            "six_eligible_families_three_repositories_and_complete_assignment_prelaunch_binding_frozen",
            "must_remain_null",
        ),
        (
            "selection_frozen_pending_independent_review",
            "selected_binding_independent_review_passed",
            "independent_exact_pass_review_root_equals_family_assignment_prelaunch_binding_root",
            "must_remain_null",
        ),
        (
            "selected_binding_independent_review_passed",
            "runtime_qualified",
            "maintained_evidence_qualifier_exact_pass_on_same_binding",
            "create_from_maintained_qualifier_exact_pass",
        ),
        (
            "runtime_qualified",
            "ready_for_stage_b_permit_creation",
            "selected_binding_review_and_qualification_current_and_all_other_prelaunch_gates_pass",
            "must_remain_bound_and_current",
        ),
    }
    actual_transitions = {
        (
            item.get("from"),
            item.get("to"),
            item.get("guard"),
            item.get("qualification_receipt_effect"),
        )
        for item in machine.get("transitions", [])
    }
    require(
        actual_transitions == expected_transitions
        and len(machine.get("transitions", [])) == len(expected_transitions),
        "prelaunch_machine_transitions",
    )
    closed = machine.get("closed_gate", {})
    require(
        closed.get("stage_b_permits_releasable_only_in_state")
        == "ready_for_stage_b_permit_creation",
        "permit_release_state_gate",
    )
    require(
        closed.get("required_selected_binding_review_status") == "PASS",
        "permit_review_status_gate",
    )
    require(
        closed.get("reviewed_root_must_equal_binding_root") is True,
        "permit_review_root_gate",
    )
    require(
        closed.get("review_must_follow_binding_freeze") is True,
        "permit_review_order_gate",
    )
    require(
        closed.get("runtime_qualification_must_follow_review") is True,
        "permit_qualification_order_gate",
    )
    require(
        closed.get("qualification_receipt_root_null_before_runtime_transition") is True,
        "pre_runtime_qualification_receipt_gate",
    )
    require(
        any(
            item.get("event")
            == "family_assignment_or_prelaunch_binding_changes_after_review"
            and item.get("to") == "selection_frozen_pending_independent_review"
            and "selected_binding_review" in item.get("clears", [])
            and "qualification_receipt_root" in item.get("clears", [])
            for item in machine.get("invalidations", [])
        ),
        "selected_binding_review_invalidation_transition",
    )
    require(
        machine.get("invalidations")
        and all(
            "qualification_receipt_root" in item.get("clears", [])
            and "qualification_receipt" not in item.get("clears", [])
            for item in machine["invalidations"]
        ),
        "qualification_receipt_invalidation_transition",
    )

    current_state = state["state"]
    require(current_state in expected_states, "prelaunch_state")
    require(state["authority_effect"] == "none", "prelaunch_authority")
    binding_root = state["family_assignment_prelaunch_binding_root"]
    review = state["selected_binding_review"]
    pre_runtime = {
        "method_frozen",
        "stage_a_passed",
        "selection_frozen_pending_independent_review",
        "selected_binding_independent_review_passed",
    }
    if current_state in pre_runtime:
        require(
            state["qualification_receipt_root"] is None,
            "pre_runtime_qualification_receipt",
        )
    if current_state == "method_frozen":
        require(state["selected_family_count"] == 0, "frozen_selected_family_count")
        require(binding_root is None, "frozen_binding_root")
        require(
            review
            == {
                "status": "not_requested",
                "reviewed_binding_root": None,
                "review_commit": None,
                "independent": False,
            },
            "frozen_selected_binding_review",
        )
    post_selection = {
        "selection_frozen_pending_independent_review",
        "selected_binding_independent_review_passed",
        "runtime_qualified",
        "ready_for_stage_b_permit_creation",
    }
    if current_state in post_selection:
        require(state["selected_family_count"] == 6, "selected_family_count")
        require(binding_root is not None, "selected_binding_root")

    post_review = {
        "selected_binding_independent_review_passed",
        "runtime_qualified",
        "ready_for_stage_b_permit_creation",
    }
    if current_state in post_review:
        require(review["status"] == "PASS", "selected_binding_review_status")
        require(review["independent"] is True, "selected_binding_review_independence")
        require(review["review_commit"] is not None, "selected_binding_review_commit")
        require(
            review["reviewed_binding_root"] == binding_root,
            "selected_binding_review_root_mismatch",
        )

    if current_state in {"runtime_qualified", "ready_for_stage_b_permit_creation"}:
        require(
            state["qualification_receipt_root"] is not None, "qualification_receipt"
        )

    if state["stage_b_permits_created"] or state["stage_b_permits_releasable"]:
        require(
            current_state == "ready_for_stage_b_permit_creation",
            "permit_state_not_ready",
        )
        require(review["status"] == "PASS", "permit_without_review_pass")
        require(review["independent"] is True, "permit_without_independent_review")
        require(
            review["reviewed_binding_root"] == binding_root,
            "permit_review_root_mismatch",
        )
        require(
            state["qualification_receipt_root"] is not None,
            "permit_without_qualification",
        )


def verify_schemas() -> tuple[dict[str, Any], dict[str, Any]]:
    response = load_json(ROOT / "response.schema.json")
    foundry = load_json(ROOT / "foundry-packet.schema.json")
    prelaunch_schema = load_json(ROOT / "prelaunch-state.schema.json")
    prelaunch_state = load_json(ROOT / "prelaunch-state.json")
    prelaunch_machine = load_json(ROOT / "prelaunch-state-machine.json")
    Draft202012Validator.check_schema(response)
    Draft202012Validator.check_schema(foundry)
    Draft202012Validator.check_schema(prelaunch_schema)
    errors = sorted(
        Draft202012Validator(prelaunch_schema).iter_errors(prelaunch_state),
        key=lambda error: list(error.absolute_path),
    )
    require(not errors, f"prelaunch_state_schema:{errors[0].message if errors else ''}")
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
    verify_prelaunch_state(prelaunch_state, prelaunch_machine)
    return prelaunch_state, prelaunch_machine


def verify_scoring_fixtures() -> None:
    fixture = load_json(ROOT / "scoring-fixtures.json")
    contract = load_json(ROOT / "study-contract.json")
    require(fixture["no_scientific_data"] is True, "scoring_data")
    require(
        tuple(fixture["score_row_fields"]) == SCORE_ROW_FIELDS,
        "score_fixture_row_fields",
    )
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
    expected_cases = {
        "realizable_registered_positive",
        "realizable_configuration_null_with_aggregate_lift",
        "aggregate_passes_but_configuration_relation_reverses",
        "aggregate_passes_but_configuration_change_reverses",
        "aggregate_passes_but_configuration_impact_reverses",
        "realizable_overall_null",
    }
    require(
        {item["name"] for item in fixture["flagship_cases"]} == expected_cases
        and len(fixture["flagship_cases"]) == len(expected_cases),
        "flagship_fixture_cases",
    )
    for item in fixture["flagship_cases"]:
        summary = expand_score_fixture_case(item)
        require(
            flagship_pass(summary, contract) is item["expected_pass"],
            f"flagship_fixture:{item['name']}",
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
