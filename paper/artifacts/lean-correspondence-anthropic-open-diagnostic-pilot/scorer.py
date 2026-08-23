"""Capture-derived, one-attempt scorer for the six-cell diagnostic."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

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
CAPTURE_KEYS = {
    "attempt",
    "capture_root",
    "cell_id",
    "entries",
    "participant_id",
    "permit_root",
    "provider_calls",
    "run_id",
    "schema",
    "terminal_status",
}
ENTRY_KEYS = {"bytes", "path", "role", "sha256"}
CAPTURE_ROLES = {"custody", "raw_response", "terminal", "usage"}
CUSTODY_KEYS = {
    "attempt",
    "cell_id",
    "participant_id",
    "permit_root",
    "provider_calls",
    "raw_response_root",
    "restricted_seconds",
    "run_id",
    "schema",
    "terminal_root",
    "terminal_status",
    "tool_call_count",
    "usage_root",
}
TERMINAL_KEYS = {
    "attempt",
    "cell_id",
    "provider_calls",
    "restricted_seconds",
    "run_id",
    "schema",
    "status",
}
USAGE_KEYS = {"cell_id", "input_tokens", "output_tokens", "schema", "tool_call_count"}
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


def read_bound(root: Path, relative: Any, label: str) -> bytes:
    if type(relative) is not str:
        raise ValueError(f"{label} path must be a string")
    path = Path(relative)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise ValueError(f"{label} path is unsafe")
    candidate = root / path
    cursor = root
    for part in path.parts:
        cursor /= part
        if stat.S_ISLNK(os.lstat(cursor).st_mode):
            raise ValueError(f"{label} path contains symlink")
    metadata = os.lstat(candidate)
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise ValueError(f"{label} must be one regular single-link file")
    descriptor = os.open(candidate, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    try:
        opened = os.fstat(descriptor)
        raw = b""
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            raw += chunk
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (
        (opened.st_dev, opened.st_ino) != (after.st_dev, after.st_ino)
        or opened.st_nlink != 1
        or len(raw) != opened.st_size
    ):
        raise ValueError(f"{label} custody drift")
    return raw


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
        "no_false_authority_or_scientific_inference": response[
            "authority_scientific_inference"
        ]
        == adjudication["authority_scientific_inference"],
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
        if type(manifest) is not dict or set(manifest) != CAPTURE_KEYS:
            raise ValueError("capture manifest shape invalid")
        body = {key: value for key, value in manifest.items() if key != "capture_root"}
        if (
            manifest["schema"]
            != "vela.lean-correspondence-anthropic-open-diagnostic-capture.v2"
            or manifest["capture_root"] != canonical_root(body)
            or type(manifest["cell_id"]) is not str
            or manifest["cell_id"] not in expected
            or manifest["cell_id"] in observed
            or exact_int(manifest["attempt"], "capture attempt") != 1
            or exact_int(manifest["provider_calls"], "capture provider_calls") != 1
            or manifest["terminal_status"]
            not in {"response", "failure", "timeout", "malformed"}
        ):
            raise ValueError("capture identity or root invalid")
        cell_id = manifest["cell_id"]
        row = expected[cell_id]
        permit = permits[cell_id]
        if (
            manifest["run_id"] != permit["run_id"]
            or manifest["participant_id"] != row["participant_id"]
            or manifest["permit_root"] != maintained_root(permit)
        ):
            raise ValueError("capture permit/run/participant cross-binding")
        entries = manifest["entries"]
        if type(entries) is not list or len(entries) != 4:
            raise ValueError("capture requires four exact evidence files")
        by_role: dict[str, tuple[dict[str, Any], bytes]] = {}
        for entry in entries:
            if (
                type(entry) is not dict
                or set(entry) != ENTRY_KEYS
                or entry["role"] not in CAPTURE_ROLES
                or entry["role"] in by_role
                or exact_int(entry["bytes"], "capture bytes") < 0
                or type(entry["sha256"]) is not str
                or ROOT_RE.fullmatch(entry["sha256"]) is None
            ):
                raise ValueError("capture entry invalid")
            raw = read_bound(package_root, entry["path"], entry["role"])
            if len(raw) != entry["bytes"] or raw_root(raw) != entry["sha256"]:
                raise ValueError("capture entry root drift")
            by_role[entry["role"]] = (entry, raw)
        if set(by_role) != CAPTURE_ROLES:
            raise ValueError("capture role denominator drift")
        terminal = json.loads(by_role["terminal"][1], object_pairs_hook=_pairs)
        usage = json.loads(by_role["usage"][1], object_pairs_hook=_pairs)
        custody = json.loads(by_role["custody"][1], object_pairs_hook=_pairs)
        if type(terminal) is not dict or set(terminal) != TERMINAL_KEYS:
            raise ValueError("terminal receipt shape invalid")
        if type(usage) is not dict or set(usage) != USAGE_KEYS:
            raise ValueError("usage receipt shape invalid")
        if type(custody) is not dict or set(custody) != CUSTODY_KEYS:
            raise ValueError("custody receipt shape invalid")
        seconds = restricted_decimal(terminal["restricted_seconds"])
        tool_count = exact_int(usage["tool_call_count"], "tool_call_count")
        if tool_count < 0 or any(
            exact_int(usage[name], name) < 0
            for name in ("input_tokens", "output_tokens")
        ):
            raise ValueError("usage count must be nonnegative")
        consistent = (
            terminal["schema"]
            == "vela.lean-correspondence-anthropic-open-diagnostic-terminal.v2"
            and usage["schema"]
            == "vela.lean-correspondence-anthropic-open-diagnostic-usage.v2"
            and custody["schema"]
            == "vela.lean-correspondence-anthropic-open-diagnostic-custody-receipt.v2"
            and terminal["cell_id"] == usage["cell_id"] == custody["cell_id"] == cell_id
            and terminal["run_id"] == custody["run_id"] == permit["run_id"]
            and terminal["attempt"] == custody["attempt"] == 1
            and terminal["provider_calls"] == custody["provider_calls"] == 1
            and terminal["status"]
            == custody["terminal_status"]
            == manifest["terminal_status"]
            and terminal["restricted_seconds"] == custody["restricted_seconds"]
            and custody["participant_id"] == row["participant_id"]
            and custody["permit_root"] == manifest["permit_root"]
            and custody["raw_response_root"] == by_role["raw_response"][0]["sha256"]
            and custody["terminal_root"] == by_role["terminal"][0]["sha256"]
            and custody["usage_root"] == by_role["usage"][0]["sha256"]
            and custody["tool_call_count"] == tool_count
        )
        if not consistent:
            raise ValueError("terminal/usage/custody binding drift")
        if manifest["terminal_status"] == "response":
            response = validate_response(
                json.loads(by_role["raw_response"][1], object_pairs_hook=_pairs),
                row["source_assignment_id"],
            )
            components = derive_components(
                response, adjudication[row["case_id"]], allowed_evidence[row["case_id"]]
            )
        else:
            if by_role["raw_response"][1] or seconds != Decimal(1200):
                raise ValueError(
                    "non-response must retain empty raw bytes and 1200 seconds"
                )
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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=Path)
    args = parser.parse_args()
    result = score_document(load_json(args.input))
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
