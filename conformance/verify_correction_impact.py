#!/usr/bin/env python3
"""Clean-room correction-impact reader for the language-neutral fixture."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from collections import defaultdict
from copy import deepcopy
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "conformance" / "fixtures" / "correction"
INPUT_PATH = FIXTURES / "diamond-input.json"
EXPECTED_PATH = FIXTURES / "diamond-expected.json"
ADVERSARIAL_PATH = FIXTURES / "diamond-adversarial.json"
SHA256 = re.compile(r"^sha256:[0-9a-f]{64}$")
CLAIM_ID = re.compile(r"^vcl_[0-9a-f]{64}$")
EXPECTED_RULES = {
    "depends_on": "hard_dependency",
    "discovery": "discovery_only",
    "supports": "support_route",
}
sys.path.insert(0, str(ROOT / "conformance" / "readers" / "python"))
from canonical import canonical_bytes


class ContractError(ValueError):
    """A stable fail-closed contract error."""


def canonical_root(value: object) -> str:
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def fail(code: str) -> None:
    raise ContractError(code)


def validate_claim_ref(value: object) -> None:
    if not isinstance(value, dict):
        fail("correction_transition_claim_missing")
    if not CLAIM_ID.fullmatch(value.get("claim_id", "")):
        fail("correction_claim_id_invalid")
    if not SHA256.fullmatch(value.get("claim_root", "")):
        fail("correction_sha256_invalid")


def validate_input(value: object) -> dict:
    if not isinstance(value, dict) or value.get("schema") != "vela.correction-impact-input.v1":
        fail("correction_impact_schema_invalid")
    if not isinstance(value.get("fixture_id"), str) or not value["fixture_id"].strip():
        fail("correction_text_invalid")
    transition = value.get("transition")
    if not isinstance(transition, dict) or transition.get("kind") not in {
        "correct_claim",
        "supersede_claim",
        "retract_claim",
    }:
        fail("correction_transition_kind_invalid")
    validate_claim_ref(transition.get("predecessor"))
    validate_claim_ref(transition.get("successor"))
    if (
        transition["predecessor"]["claim_id"] == transition["successor"]["claim_id"]
        or transition["predecessor"]["claim_root"]
        == transition["successor"]["claim_root"]
    ):
        fail("correction_transition_not_distinct")

    claims = value.get("claims")
    relations = value.get("relations")
    rules = value.get("relation_rules")
    bounds = value.get("bounds")
    if not all(
        [
            isinstance(claims, list),
            isinstance(relations, list),
            isinstance(rules, list),
            isinstance(bounds, dict),
        ]
    ):
        fail("correction_impact_schema_invalid")
    if len(claims) > bounds.get("max_claims", -1):
        fail("correction_claim_bound_exceeded")
    if len(relations) > bounds.get("max_relations", -1):
        fail("correction_relation_bound_exceeded")

    claim_by_id: dict[str, dict] = {}
    roots: set[str] = set()
    for claim in claims:
        validate_claim_ref(claim)
        claim_id = claim["claim_id"]
        if claim_id in claim_by_id:
            fail("correction_claim_id_duplicate")
        if claim["claim_root"] in roots:
            fail("correction_claim_root_duplicate")
        claim_by_id[claim_id] = claim
        roots.add(claim["claim_root"])
    for required in (transition["predecessor"], transition["successor"]):
        candidate = claim_by_id.get(required["claim_id"])
        if candidate is None or candidate.get("claim_root") != required["claim_root"]:
            fail("correction_transition_claim_missing")

    rule_map: dict[str, str] = {}
    for rule in rules:
        if not isinstance(rule, dict) or rule.get("kind") in rule_map:
            fail("correction_relation_rule_duplicate")
        kind = rule.get("kind")
        effect = rule.get("effect")
        if EXPECTED_RULES.get(kind) != effect:
            fail("correction_relation_rule_conflict")
        rule_map[kind] = effect

    relation_ids: set[str] = set()
    relation_roots: set[str] = set()
    for relation in relations:
        if not isinstance(relation, dict) or not relation.get("relation_id"):
            fail("correction_text_invalid")
        if not SHA256.fullmatch(relation.get("relation_root", "")):
            fail("correction_sha256_invalid")
        if relation["relation_id"] in relation_ids:
            fail("correction_relation_id_duplicate")
        if relation["relation_root"] in relation_roots:
            fail("correction_relation_root_duplicate")
        relation_ids.add(relation["relation_id"])
        relation_roots.add(relation["relation_root"])
        if (
            relation.get("source_claim_id") not in claim_by_id
            or relation.get("target_claim_id") not in claim_by_id
        ):
            fail("correction_relation_endpoint_missing")
        if relation["source_claim_id"] == relation["target_claim_id"]:
            fail("correction_relation_self_loop")
        if relation.get("kind") not in rule_map:
            fail("correction_relation_unknown")
    return value


def derive(value: object) -> dict:
    data = validate_input(value)
    transition = data["transition"]
    bounds = data["bounds"]
    base = {
        "schema": "vela.correction-impact-projection.v1",
        "fixture_id": data["fixture_id"],
        "transition": transition,
        "retained_predecessor": transition["predecessor"],
        "bounds": bounds,
    }
    diagnostics = []
    if not bounds["complete_claim_set"]:
        diagnostics.append("claim_set_incomplete")
    if not bounds["complete_relation_set"]:
        diagnostics.append("relation_set_incomplete")
    if diagnostics:
        return base | {
            "status": "incomplete",
            "affected_claims": [],
            "unaffected_claims": [],
            "lost_support_routes": [],
            "surviving_support_routes": [],
            "repair_obligations": [],
            "diagnostics": diagnostics,
        }

    claims = {claim["claim_id"]: claim for claim in data["claims"]}
    rules = {rule["kind"]: rule["effect"] for rule in data["relation_rules"]}
    unavailable = transition["predecessor"]["claim_id"]
    repair: set[str] = set()
    changed: set[str] = set()
    causes: dict[str, set[str]] = defaultdict(set)

    while True:
        before = (
            set(repair),
            set(changed),
            {claim_id: set(relation_ids) for claim_id, relation_ids in causes.items()},
        )
        for relation in data["relations"]:
            if rules[relation["kind"]] != "hard_dependency":
                continue
            target = relation["target_claim_id"]
            source = relation["source_claim_id"]
            if target == unavailable or target in repair:
                causes[source].add(relation["relation_id"])
                causes[source].update(causes[target])
                repair.add(source)
                changed.discard(source)

        support_by_source: dict[str, list[dict]] = defaultdict(list)
        for relation in data["relations"]:
            if rules[relation["kind"]] == "support_route":
                support_by_source[relation["source_claim_id"]].append(relation)
        for source, relations in support_by_source.items():
            lost = [
                relation
                for relation in relations
                if relation["target_claim_id"] == unavailable
                or relation["target_claim_id"] in repair
            ]
            surviving = [relation for relation in relations if relation not in lost]
            if not lost:
                continue
            for relation in lost:
                causes[source].add(relation["relation_id"])
                causes[source].update(causes[relation["target_claim_id"]])
            if not surviving:
                repair.add(source)
                changed.discard(source)
            elif source not in repair:
                changed.add(source)
        after = (
            set(repair),
            set(changed),
            {claim_id: set(relation_ids) for claim_id, relation_ids in causes.items()},
        )
        if before == after:
            break

    affected = []
    for claim_id in sorted(repair | changed):
        claim = claims[claim_id]
        affected.append(
            {
                "claim_id": claim_id,
                "claim_root": claim["claim_root"],
                "classification": (
                    "repair_required" if claim_id in repair else "route_changed"
                ),
                "causal_relation_ids": sorted(causes[claim_id]),
            }
        )
    unaffected = [
        {"claim_id": claim_id, "claim_root": claim["claim_root"]}
        for claim_id, claim in sorted(claims.items())
        if claim_id
        not in {
            transition["predecessor"]["claim_id"],
            transition["successor"]["claim_id"],
        }
        and claim_id not in repair
        and claim_id not in changed
    ]

    support_sources_with_loss = {
        relation["source_claim_id"]
        for relation in data["relations"]
        if rules[relation["kind"]] == "support_route"
        and (
            relation["target_claim_id"] == unavailable
            or relation["target_claim_id"] in repair
        )
    }
    lost_routes = []
    surviving_routes = []
    for relation in data["relations"]:
        if (
            rules[relation["kind"]] != "support_route"
            or relation["source_claim_id"] not in support_sources_with_loss
        ):
            continue
        route = {
            "relation_id": relation["relation_id"],
            "relation_root": relation["relation_root"],
            "source_claim_id": relation["source_claim_id"],
            "target_claim_id": relation["target_claim_id"],
            "target_claim_root": claims[relation["target_claim_id"]]["claim_root"],
        }
        if relation["target_claim_id"] == unavailable or relation["target_claim_id"] in repair:
            lost_routes.append(route)
        else:
            surviving_routes.append(route)
    lost_routes.sort(key=lambda route: route["relation_id"])
    surviving_routes.sort(key=lambda route: route["relation_id"])

    obligations = []
    for claim_id in sorted(repair):
        claim = claims[claim_id]
        discharge = claim.get("repair_condition")
        if not discharge:
            fail(f"repair_condition_missing_for_affected_claim:{claim_id}")
        relation_ids = sorted(causes[claim_id])
        preimage = {
            "schema": "vela.correction-repair-obligation.v1",
            "claim_id": claim_id,
            "claim_root": claim["claim_root"],
            "causal_relation_ids": relation_ids,
            "discharge_condition": discharge,
        }
        obligations.append(
            {
                "obligation_root": canonical_root(preimage),
                "claim_id": claim_id,
                "claim_root": claim["claim_root"],
                "causal_relation_ids": relation_ids,
                "discharge_condition": discharge,
            }
        )

    return base | {
        "status": "complete",
        "affected_claims": affected,
        "unaffected_claims": unaffected,
        "lost_support_routes": lost_routes,
        "surviving_support_routes": surviving_routes,
        "repair_obligations": obligations,
        "diagnostics": [],
    }


def apply_mutation(source: dict, mutation: dict) -> dict:
    value = deepcopy(source)
    operation = mutation["op"]
    if operation == "set_relation_kind":
        relation = next(
            relation
            for relation in value["relations"]
            if relation["relation_id"] == mutation["relation_id"]
        )
        relation["kind"] = mutation["value"]
    elif operation == "set_rule_effect":
        rule = next(
            rule
            for rule in value["relation_rules"]
            if rule["kind"] == mutation["kind"]
        )
        rule["effect"] = mutation["value"]
    elif operation == "set_relation_target":
        relation = next(
            relation
            for relation in value["relations"]
            if relation["relation_id"] == mutation["relation_id"]
        )
        relation["target_claim_id"] = mutation["value"]
    elif operation == "set_predecessor_root":
        value["transition"]["predecessor"]["claim_root"] = mutation["value"]
    elif operation == "set_complete_relation_set":
        value["bounds"]["complete_relation_set"] = mutation["value"]
    elif operation == "remove_relation":
        value["relations"] = [
            relation
            for relation in value["relations"]
            if relation["relation_id"] != mutation["relation_id"]
        ]
    elif operation == "add_connected_cycle":
        value["relations"].extend(
            [
                {
                    "relation_id": "relation-b-depends-on-c",
                    "relation_root": "sha256:" + "05" * 32,
                    "kind": "depends_on",
                    "source_claim_id": "vcl_" + "b" * 64,
                    "target_claim_id": "vcl_" + "c" * 64,
                },
                {
                    "relation_id": "relation-c-depends-on-b",
                    "relation_root": "sha256:" + "06" * 32,
                    "kind": "depends_on",
                    "source_claim_id": "vcl_" + "c" * 64,
                    "target_claim_id": "vcl_" + "b" * 64,
                },
            ]
        )
    elif operation == "set_max_relations":
        value["bounds"]["max_relations"] = mutation["value"]
    else:
        fail(f"unknown_adversarial_mutation:{operation}")
    return value


def verify_adversarial(source: dict) -> None:
    vectors = json.loads(ADVERSARIAL_PATH.read_text(encoding="utf-8"))
    if vectors.get("schema") != "vela.correction-impact-adversarial.v1":
        fail("correction_adversarial_schema_invalid")
    if vectors.get("base_input_root") != canonical_root(source):
        fail("correction_adversarial_base_root_mismatch")
    for case in vectors.get("cases", []):
        mutated = apply_mutation(source, case["mutation"])
        expected_error = case.get("expected_error")
        if expected_error:
            try:
                derive(mutated)
            except ContractError as error:
                if str(error) != expected_error:
                    fail(f"{case['id']}:expected_{expected_error}:observed_{error}")
            else:
                fail(f"{case['id']}:expected_error")
            continue
        projection = derive(mutated)
        expected = case["expected_projection"]
        if projection["status"] != expected["status"]:
            fail(f"{case['id']}:status_mismatch")
        if projection["diagnostics"] != expected["diagnostics"]:
            fail(f"{case['id']}:diagnostic_mismatch")
        affected = [claim["claim_id"] for claim in projection["affected_claims"]]
        if affected != expected["affected_claim_ids"]:
            fail(f"{case['id']}:affected_set_mismatch")
        if "repair_required_claim_ids" in expected:
            repair = [
                claim["claim_id"]
                for claim in projection["affected_claims"]
                if claim["classification"] == "repair_required"
            ]
            if repair != expected["repair_required_claim_ids"]:
                fail(f"{case['id']}:repair_set_mismatch")
        if "surviving_support_routes" in expected and len(
            projection["surviving_support_routes"]
        ) != expected["surviving_support_routes"]:
            fail(f"{case['id']}:surviving_route_mismatch")


def main() -> int:
    source = json.loads(INPUT_PATH.read_text(encoding="utf-8"))
    projection = derive(source)
    if "--emit" in sys.argv:
        print(
            json.dumps(
                {
                    "projection": projection,
                    "projection_root": canonical_root(projection),
                },
                indent=2,
                ensure_ascii=False,
            )
        )
        return 0
    expected = json.loads(EXPECTED_PATH.read_text(encoding="utf-8"))
    if canonical_bytes(projection) != canonical_bytes(expected.get("projection")):
        print("correction-impact: clean-room projection mismatch", file=sys.stderr)
        return 1
    if canonical_root(projection) != expected.get("projection_root"):
        print("correction-impact: clean-room root mismatch", file=sys.stderr)
        return 1
    verify_adversarial(source)
    print(f"correction-impact: ok ({expected['projection_root']})")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ContractError, OSError, json.JSONDecodeError) as error:
        print(f"correction-impact: {error}", file=sys.stderr)
        raise SystemExit(1) from error
