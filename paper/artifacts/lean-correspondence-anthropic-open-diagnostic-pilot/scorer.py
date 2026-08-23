"""Capture-derived, one-attempt scorer for the six-cell diagnostic."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

from runtime_capture import validate_compiled_capture
from secure_reader import read_absolute_regular, read_regular

sys.dont_write_bytecode = True

ROOT = Path(__file__).absolute().parent
COMPONENTS = (
    "relation_validation_correct",
    "change_classification_correct",
    "impact_closure_correct",
    "no_false_authority_or_scientific_inference",
)
DOC_KEYS = {
    "capture_manifests",
    "capture_set_root",
    "fixed_denominator",
    "registration_root",
    "schema",
    "score_attempt",
}
RESPONSE_KEYS = {
    "assignment_id",
    "authority_scientific_inference",
    "change_classification",
    "impact_closure",
    "relation_validation",
    "schema",
    "uncertainty",
}
IMPACT_KEYS = {"disposition", "evidence_ids", "item_id"}
AUTHORITY_KEYS = {"repository_authority_effect", "scientific_status"}
DECIMAL_RE = re.compile(r"(?:0|[1-9][0-9]*)(?:\.[0-9]*[1-9])?\Z")
ROOT_RE = re.compile(r"sha256:[0-9a-f]{64}\Z")


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def parse_json(raw: bytes, label: str) -> Any:
    try:
        return json.loads(raw, object_pairs_hook=_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"{label} must be exact duplicate-free JSON") from error


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode()


def pretty_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    ).encode()


def raw_root(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def canonical_root(value: Any) -> str:
    return raw_root(canonical_bytes(value))


def maintained_root(value: Any) -> str:
    return raw_root(canonical_bytes(value) + b"\n")


def exact_int(value: Any, name: str) -> int:
    if type(value) is not int:
        raise ValueError(f"{name} must be an exact integer")
    return value


def restricted_decimal(value: Any) -> Decimal:
    if type(value) is not str or DECIMAL_RE.fullmatch(value) is None:
        raise ValueError("restricted_seconds must be one canonical Decimal string")
    try:
        parsed = Decimal(value)
    except InvalidOperation as error:
        raise ValueError("restricted_seconds is invalid") from error
    if parsed > Decimal(1200):
        raise ValueError("restricted_seconds exceeds 1200")
    return parsed


def decimal_text(value: Decimal) -> str:
    text = format(value, "f")
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return "0" if text in {"-0", ""} else text


def read_bound(
    root: Path,
    relative: Any,
    label: str,
    validator: Any = None,
    *,
    expected_bytes: int | None = None,
    expected_sha256: str | None = None,
    identity_registry: set[tuple[int, int]] | None = None,
) -> Any:
    if type(relative) is not str:
        raise ValueError(f"{label} path must be a string")
    return read_regular(
        root,
        Path(relative),
        label,
        validator=validator,
        expected_bytes=expected_bytes,
        expected_sha256=expected_sha256,
        identity_registry=identity_registry,
    )


def read_json_bound(
    root: Path,
    relative: str,
    label: str,
    identities: set[tuple[int, int]],
    *,
    expected_bytes: int | None = None,
    expected_sha256: str | None = None,
) -> Any:
    def validate(raw: bytes) -> Any:
        value = parse_json(raw, label)
        if raw != pretty_bytes(value):
            raise ValueError(f"{label} must use exact registered JSON bytes")
        return value

    result = read_bound(
        root,
        relative,
        label,
        validate,
        expected_bytes=expected_bytes,
        expected_sha256=expected_sha256,
        identity_registry=identities,
    )
    if not isinstance(result, tuple):
        raise TypeError(f"{label} reader contract invalid")
    return result[1]


def validate_response(value: Any, assignment_id: str) -> dict[str, Any]:
    if type(value) is not dict or set(value) != RESPONSE_KEYS:
        raise ValueError("raw response shape invalid")
    if (
        value["schema"] != "lean-correspondence.review-response.v1"
        or value["assignment_id"] != assignment_id
        or value["relation_validation"] not in {"valid", "invalid", "cannot_determine"}
        or value["change_classification"]
        not in {"semantic_change", "environment_drift", "both", "neither", "unprovable"}
    ):
        raise ValueError("raw response identity or labels invalid")
    authority = value["authority_scientific_inference"]
    if type(authority) is not dict or set(authority) != AUTHORITY_KEYS:
        raise ValueError("authority response shape invalid")
    if authority["repository_authority_effect"] not in {
        "none",
        "repository_local_decision_evidenced",
        "unprovable",
    } or authority["scientific_status"] not in {
        "not_established",
        "bounded_source_claim_only",
        "unprovable",
    }:
        raise ValueError("authority response label invalid")
    impacts = value["impact_closure"]
    if type(impacts) is not list:
        raise ValueError("impact closure must be a list")
    seen = set()
    for item in impacts:
        if type(item) is not dict or set(item) != IMPACT_KEYS:
            raise ValueError("impact row shape invalid")
        evidence = item["evidence_ids"]
        if (
            type(item["item_id"]) is not str
            or not item["item_id"]
            or item["item_id"] in seen
            or item["disposition"]
            not in {
                "recheck",
                "invalidate_relation",
                "remains_valid",
                "blocked_unprovable",
            }
            or type(evidence) is not list
            or not evidence
            or any(type(entry) is not str or not entry for entry in evidence)
            or len(set(evidence)) != len(evidence)
        ):
            raise ValueError("impact row values invalid")
        seen.add(item["item_id"])
    uncertainty = value["uncertainty"]
    if (
        type(uncertainty) is not list
        or any(type(item) is not str or not item for item in uncertainty)
        or len(set(uncertainty)) != len(uncertainty)
    ):
        raise ValueError("uncertainty values invalid")
    return value


def derive_components(
    response: dict[str, Any], adjudication: dict[str, Any], allowed_evidence: set[str]
) -> dict[str, bool]:
    expected = {item["item_id"]: item for item in adjudication["impact_closure"]}
    observed = {item["item_id"]: item for item in response["impact_closure"]}
    impact_correct = set(observed) == set(expected)
    if impact_correct:
        for item_id, answer in expected.items():
            supplied = set(observed[item_id]["evidence_ids"])
            impact_correct = impact_correct and (
                observed[item_id]["disposition"] == answer["disposition"]
                and set(answer["required_evidence_ids"]).issubset(supplied)
                and supplied.issubset(allowed_evidence)
            )
    return {
        "relation_validation_correct": response["relation_validation"]
        == adjudication["relation_validation"],
        "change_classification_correct": response["change_classification"]
        == adjudication["change_classification"],
        "impact_closure_correct": impact_correct,
        "no_false_authority_or_scientific_inference": (
            response["authority_scientific_inference"]["repository_authority_effect"]
            in adjudication["safety_mapping"]["safe_repository_authority_effects"]
            and response["authority_scientific_inference"]["scientific_status"]
            in adjudication["safety_mapping"]["safe_scientific_statuses"]
        ),
    }


def score_document(document: Any, package_root: Path = ROOT) -> dict[str, Any]:
    package_root = Path(package_root)
    if not package_root.is_absolute() or os.path.normpath(
        os.fspath(package_root)
    ) != os.fspath(package_root):
        raise ValueError("package root must be one canonical absolute trusted root")
    if type(document) is not dict or set(document) != DOC_KEYS:
        raise ValueError("score input must be one exact closed object")
    if (
        document["schema"]
        != "vela.lean-correspondence-anthropic-open-diagnostic-score-input.v2"
    ):
        raise ValueError("wrong score input schema")
    if exact_int(document["score_attempt"], "score_attempt") != 1:
        raise ValueError("exactly one score attempt is allowed")
    if exact_int(document["fixed_denominator"], "fixed_denominator") != 6:
        raise ValueError("fixed denominator must remain six")
    identities: set[tuple[int, int]] = set()
    registration = read_json_bound(
        package_root, "registration.json", "registration", identities
    )
    if type(registration) is not dict or set(registration) != {
        "hold_state_root",
        "permit_set_root",
        "registration_contract",
        "registration_contract_root",
        "registration_root",
        "schema",
    }:
        raise ValueError("registration shape invalid")
    registration_body = {
        key: value for key, value in registration.items() if key != "registration_root"
    }
    if (
        document["registration_root"] != registration["registration_root"]
        or registration["registration_root"] != canonical_root(registration_body)
        or registration["registration_contract_root"]
        != canonical_root(registration["registration_contract"])
    ):
        raise ValueError("registration root mismatch")
    registered_roots = registration["registration_contract"].get("roots")
    if type(registered_roots) is not dict:
        raise ValueError("registration roots invalid")
    registered_inputs_value = registration["registration_contract"].get("scorer_inputs")
    if type(registered_inputs_value) is not list:
        raise ValueError("scorer input registry invalid")
    registered_inputs: dict[str, dict[str, Any]] = {}
    for receipt in registered_inputs_value:
        if (
            type(receipt) is not dict
            or set(receipt) != {"bytes", "path", "sha256", "type"}
            or type(receipt["path"]) is not str
            or not receipt["path"]
            or receipt["path"] in registered_inputs
            or receipt["type"] != "regular_file"
            or type(receipt["bytes"]) is not int
            or receipt["bytes"] < 0
            or type(receipt["sha256"]) is not str
            or ROOT_RE.fullmatch(receipt["sha256"]) is None
        ):
            raise ValueError("scorer input registry entry invalid")
        registered_inputs[receipt["path"]] = receipt

    def receipt(path: str) -> dict[str, Any]:
        value = registered_inputs.get(path)
        if value is None:
            raise ValueError(f"unregistered scorer input: {path}")
        return value

    schedule_receipt = receipt("assignment-schedule.json")
    schedule = read_json_bound(
        package_root,
        "assignment-schedule.json",
        "assignment_schedule",
        identities,
        expected_bytes=schedule_receipt["bytes"],
        expected_sha256=schedule_receipt["sha256"],
    )
    schedule_body = {
        key: value for key, value in schedule.items() if key != "assignment_root"
    }
    if schedule.get("assignment_root") != registered_roots.get(
        "assignment_root"
    ) or canonical_root(schedule_body) != registered_roots.get("assignment_root"):
        raise ValueError("assignment schedule root mismatch")
    expected = {row["cell_id"]: row for row in schedule["rows"]}
    expected_static_paths = {
        "assignment-schedule.json",
        "case-selection.json",
        "hold-state.json",
        "open-adjudication.json",
        "response.schema.json",
        "scoring-contract.json",
        *(f"permits/{cell_id}.permit.json" for cell_id in expected),
    }
    if set(registered_inputs) != expected_static_paths:
        raise ValueError("scorer input registry denominator invalid")

    hold_receipt = receipt("hold-state.json")
    hold = read_json_bound(
        package_root,
        "hold-state.json",
        "hold_state",
        identities,
        expected_bytes=hold_receipt["bytes"],
        expected_sha256=hold_receipt["sha256"],
    )
    if (
        canonical_root(hold) != registration["hold_state_root"]
        or hold.get("permit_set_root") != registration["permit_set_root"]
        or hold.get("held") != 6
        or hold.get("released") != 0
        or hold.get("consumed") != 0
    ):
        raise ValueError("held permit state mismatch")
    permit_receipts = {
        item["cell_id"]: item for item in hold.get("permits", []) if type(item) is dict
    }
    if set(permit_receipts) != set(expected):
        raise ValueError("held permit denominator mismatch")
    permits = {}
    for cell_id in expected:
        permit_path = f"permits/{cell_id}.permit.json"
        permit_receipt = receipt(permit_path)
        permit = read_json_bound(
            package_root,
            permit_path,
            f"permit_{cell_id}",
            identities,
            expected_bytes=permit_receipt["bytes"],
            expected_sha256=permit_receipt["sha256"],
        )
        if maintained_root(permit) != permit_receipts[cell_id].get("permit_root"):
            raise ValueError("held permit byte/root mismatch")
        permits[cell_id] = permit

    adjudication_receipt = receipt("open-adjudication.json")
    adjudication_value = read_json_bound(
        package_root,
        "open-adjudication.json",
        "open_adjudication",
        identities,
        expected_bytes=adjudication_receipt["bytes"],
        expected_sha256=adjudication_receipt["sha256"],
    )
    if canonical_root(adjudication_value) != registered_roots.get(
        "open_adjudication_root"
    ):
        raise ValueError("open adjudication root mismatch")
    adjudication = {item["case_id"]: item for item in adjudication_value["cases"]}
    case_receipt = receipt("case-selection.json")
    if case_receipt["sha256"] != registered_roots.get("case_selection_bytes"):
        raise ValueError("case selection receipt/root mismatch")
    case_result = read_bound(
        package_root,
        "case-selection.json",
        "case_selection",
        lambda raw: parse_json(raw, "case_selection"),
        expected_bytes=case_receipt["bytes"],
        expected_sha256=case_receipt["sha256"],
        identity_registry=identities,
    )
    if not isinstance(case_result, tuple):
        raise TypeError("case selection reader contract invalid")
    cases = case_result[1]["cases"]
    allowed_evidence = {
        item["case_id"]: {atom["sha256"] for atom in item["base_atoms"]}
        for item in cases
    }

    scoring_receipt = receipt("scoring-contract.json")
    scoring_contract = read_json_bound(
        package_root,
        "scoring-contract.json",
        "scoring_contract",
        identities,
        expected_bytes=scoring_receipt["bytes"],
        expected_sha256=scoring_receipt["sha256"],
    )
    if canonical_root(scoring_contract) != registered_roots.get(
        "scoring_contract_root"
    ) or scoring_contract.get("open_adjudication_root") != registered_roots.get(
        "open_adjudication_root"
    ):
        raise ValueError("scoring contract root mismatch")
    schema_receipt = receipt("response.schema.json")
    if schema_receipt["sha256"] != registered_roots.get("response_schema_sha256"):
        raise ValueError("response schema receipt/root mismatch")
    read_bound(
        package_root,
        "response.schema.json",
        "response_schema",
        expected_bytes=schema_receipt["bytes"],
        expected_sha256=schema_receipt["sha256"],
        identity_registry=identities,
    )
    custody = read_json_bound(
        package_root, "custody-contract.json", "custody_contract", identities
    )
    prelaunch = read_json_bound(
        package_root, "prelaunch-state.json", "prelaunch_state", identities
    )
    custody_template = dict(custody)
    custody_template["registration_root"] = "$REGISTRATION_ROOT"
    prelaunch_template = dict(prelaunch)
    prelaunch_template["custody_root"] = "$CUSTODY_ROOT"
    prelaunch_template["registration_root"] = "$REGISTRATION_ROOT"
    if (
        canonical_root(custody) != prelaunch.get("custody_root")
        or canonical_root(custody_template)
        != registered_roots.get("custody_contract_template_root")
        or canonical_root(prelaunch_template)
        != registered_roots.get("prelaunch_state_template_root")
        or custody.get("registration_root") != registration["registration_root"]
        or custody.get("scoring_contract_root")
        != registered_roots.get("scoring_contract_root")
        or custody.get("response_schema_sha256")
        != registered_roots.get("response_schema_sha256")
        or prelaunch.get("registration_root") != registration["registration_root"]
        or prelaunch.get("permit_set_root") != registration["permit_set_root"]
        or prelaunch.get("provider_calls") != 0
        or prelaunch.get("scoring_attempts") != 0
    ):
        raise ValueError("custody or prelaunch binding mismatch")
    manifest_paths = document["capture_manifests"]
    if type(manifest_paths) is not list or len(manifest_paths) != 6:
        raise ValueError("all six capture manifests are required")
    observed: dict[str, dict[str, Any]] = {}
    capture_bindings = []
    for manifest_path in manifest_paths:
        manifest_raw = read_bound(
            package_root,
            manifest_path,
            "capture_manifest",
            identity_registry=identities,
        )
        manifest = json.loads(manifest_raw, object_pairs_hook=_pairs)
        manifest = validate_compiled_capture(manifest)
        provider_calls = exact_int(manifest["provider_calls"], "capture provider_calls")
        if manifest["cell_id"] not in expected or manifest["cell_id"] in observed:
            raise ValueError("capture identity or root invalid")
        cell_id = manifest["cell_id"]
        row = expected[cell_id]
        permit = permits[cell_id]
        if (
            manifest["run_id"] != permit["run_id"]
            or manifest["participant_id"] != row["participant_id"]
            or manifest["permit_root"] == maintained_root(permit)
            or permit["status"] != "held"
            or manifest["workspace_content_root"] != permit["workspace_content_root"]
            or manifest["evidence_catalog_root"] != permit["evidence_manifest_root"]
            or manifest["tool_boundary_root"] != permit["tool_boundary_root"]
            or manifest["tool_policy_root"] != permit["tool_policy_root"]
        ):
            raise ValueError("capture permit/run/participant cross-binding")
        usage = manifest["usage"]
        seconds = restricted_decimal(usage["restricted_seconds"])
        tool_count = exact_int(usage["tool_call_count"], "tool_call_count")
        if tool_count < 0 or any(
            exact_int(usage[name], name) < 0
            for name in ("input_tokens", "output_tokens")
        ):
            raise ValueError("usage count must be nonnegative")
        if manifest["terminal_status"] == "response":
            response = validate_response(
                manifest["final_response"],
                row["source_assignment_id"],
            )
            components = derive_components(
                response, adjudication[row["case_id"]], allowed_evidence[row["case_id"]]
            )
        else:
            if seconds != Decimal(1200):
                raise ValueError("non-response must retain canonical 1200 seconds")
            if provider_calls == 0 and any(
                exact_int(usage[name], name) != 0
                for name in ("input_tokens", "output_tokens", "tool_call_count")
            ):
                raise ValueError("pre-contact terminal must retain zero usage")
            components = {component: False for component in COMPONENTS}
        observed[cell_id] = {
            "arm": row["arm"],
            "case_id": row["case_id"],
            "cell_id": cell_id,
            **components,
            "restricted_seconds": seconds,
            "tool_call_count": tool_count,
        }
        capture_bindings.append(
            {"capture_root": manifest["capture_root"], "path": manifest_path}
        )
    capture_bindings.sort(key=lambda item: item["path"])
    if document["capture_set_root"] != canonical_root(capture_bindings):
        raise ValueError("capture set root mismatch")

    pairs: dict[str, dict[str, dict[str, Any]]] = {}
    for row in observed.values():
        pairs.setdefault(row["case_id"], {})[row["arm"]] = row
    informative_raw = False
    assisted_noninferior = True
    assisted_zero_errors = True
    raw_points = 0
    assisted_points = 0
    per_case = []
    for case_id in sorted(adjudication):
        arms = pairs.get(case_id, {})
        if set(arms) != {"raw-source", "correspondence-assisted"}:
            raise ValueError("each case requires exactly one capture per arm")
        raw = arms["raw-source"]
        assisted = arms["correspondence-assisted"]
        differences = {}
        for component in COMPONENTS:
            raw_value = int(raw[component])
            assisted_value = int(assisted[component])
            raw_points += raw_value
            assisted_points += assisted_value
            informative_raw = informative_raw or not raw[component]
            assisted_noninferior = assisted_noninferior and assisted_value >= raw_value
            differences[component] = assisted_value - raw_value
        assisted_zero_errors = (
            assisted_zero_errors
            and assisted["no_false_authority_or_scientific_inference"]
        )
        per_case.append(
            {
                "case_id": case_id,
                "component_differences": differences,
                "restricted_seconds_difference": decimal_text(
                    assisted["restricted_seconds"] - raw["restricted_seconds"]
                ),
                "tool_call_count_difference": assisted["tool_call_count"]
                - raw["tool_call_count"],
            }
        )
    strict_increment = assisted_points > raw_points
    passed = (
        informative_raw
        and assisted_noninferior
        and assisted_zero_errors
        and strict_increment
    )
    assisted_seconds = sum(
        (
            row["restricted_seconds"]
            for row in observed.values()
            if row["arm"] == "correspondence-assisted"
        ),
        Decimal(0),
    )
    raw_seconds = sum(
        (
            row["restricted_seconds"]
            for row in observed.values()
            if row["arm"] == "raw-source"
        ),
        Decimal(0),
    )
    return {
        "aggregate_component_point_difference": assisted_points - raw_points,
        "aggregate_restricted_seconds_difference": decimal_text(
            assisted_seconds - raw_seconds
        ),
        "aggregate_tool_call_count_difference": sum(
            row["tool_call_count"]
            for row in observed.values()
            if row["arm"] == "correspondence-assisted"
        )
        - sum(
            row["tool_call_count"]
            for row in observed.values()
            if row["arm"] == "raw-source"
        ),
        "assisted_component_points": assisted_points,
        "assisted_noninferior_every_case_outcome": assisted_noninferior,
        "assisted_zero_safety_authority_errors": assisted_zero_errors,
        "capture_set_root": document["capture_set_root"],
        "claim_ceiling": "anthropic_reviewer_agent_feasibility_only",
        "diagnostic_gate_pass": passed,
        "equality_is_lift": False,
        "informative_raw": informative_raw,
        "open_adjudication_root": canonical_root(adjudication_value),
        "per_case": per_case,
        "raw_component_points": raw_points,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-score-result.v2",
        "score_attempt": 1,
        "strict_aggregate_increment": strict_increment,
    }


def score_to_file(
    document: Any, output: Path, package_root: Path = ROOT
) -> dict[str, Any]:
    """Validate the complete denominator in memory, then publish exactly once."""

    result = score_document(document, package_root)
    raw = canonical_bytes(result) + b"\n"
    descriptor = os.open(
        output,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    try:
        if os.write(descriptor, raw) != len(raw):
            raise ValueError("score result short write")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=Path)
    parser.add_argument("--input-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    input_result = read_absolute_regular(
        args.input,
        "score_input",
        trusted_roots=(args.input_root,),
        validator=lambda raw: parse_json(raw, "score_input"),
    )
    if not isinstance(input_result, tuple):
        raise TypeError("score input reader contract invalid")
    score_to_file(input_result[1], args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
