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
from secure_reader import read_regular

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parent
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


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_pairs)


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
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
) -> Any:
    if type(relative) is not str:
        raise ValueError(f"{label} path must be a string")
    return read_regular(root, Path(relative), label, validator=validator)


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
    package_root = package_root.resolve()
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
    registration = load_json(package_root / "registration.json")
    if document["registration_root"] != registration["registration_root"]:
        raise ValueError("registration root mismatch")
    schedule = load_json(package_root / "assignment-schedule.json")
    expected = {row["cell_id"]: row for row in schedule["rows"]}
    permits = {
        cell_id: load_json(package_root / "permits" / f"{cell_id}.permit.json")
        for cell_id in expected
    }
    adjudication_value = load_json(package_root / "open-adjudication.json")
    adjudication = {item["case_id"]: item for item in adjudication_value["cases"]}
    cases = load_json(package_root / "case-selection.json")["cases"]
    allowed_evidence = {
        item["case_id"]: {atom["sha256"] for atom in item["base_atoms"]}
        for item in cases
    }
    manifest_paths = document["capture_manifests"]
    if type(manifest_paths) is not list or len(manifest_paths) != 6:
        raise ValueError("all six capture manifests are required")
    observed: dict[str, dict[str, Any]] = {}
    capture_bindings = []
    for manifest_path in manifest_paths:
        manifest_raw = read_bound(package_root, manifest_path, "capture_manifest")
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
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    score_to_file(load_json(args.input), args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
