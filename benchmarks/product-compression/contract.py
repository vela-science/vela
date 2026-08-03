#!/usr/bin/env python3
"""Small, Vela-specific contract helpers for the Harbor benchmark."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
from typing import Any


ROOT = re.compile(r"^sha256:[0-9a-f]{64}$")
ANSWER_SCHEMA = "vela.product-compression-answer.v10"
ANSWER_KEY_SCHEMA = "vela.product-compression-answer-key.v10"
FIXTURE_SCHEMA = "vela.product-compression-fixture.v8"
PLAN_SCHEMA = "vela.product-compression-plan.v12"
RESULT_SCHEMA = "vela.product-compression-native-harbor-result.v7"
POST_DECISION_SCENARIO = "erdos-post-decision-continuation"
TARGET_ABSENCE_SCENARIO = "explicit-target-absence"
ACTION_COMPLETE_TASK_CLASSES = (
    "target_continuation",
    "standing_discrimination",
    "cross_frontier_inheritance",
    "controlled_correction_impact",
    "explicit_target_absence",
)


class ContractError(ValueError):
    """A retained benchmark document violates its narrow contract."""


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha256_root(payload: bytes) -> str:
    return f"sha256:{hashlib.sha256(payload).hexdigest()}"


def record_root(value: dict[str, Any], field: str) -> str:
    return sha256_root(canonical_bytes({key: item for key, item in value.items() if key != field}))


def seal(value: dict[str, Any], field: str) -> dict[str, Any]:
    value[field] = record_root(value, field)
    return value


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ContractError(f"cannot read JSON {path}: {exc}") from exc


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(canonical_bytes(value))


def require_root(value: Any, location: str) -> str:
    if not isinstance(value, str) or not ROOT.fullmatch(value):
        raise ContractError(f"{location}: expected sha256 root")
    return value


def validate_answer(value: Any) -> None:
    """Check only Vela's scientific and authority invariants.

    The published JSON Schema owns the general document shape. Harbor's verifier
    compares a participant answer with the exact answer key.
    """
    try:
        if value["schema"] != ANSWER_SCHEMA:
            raise ContractError("$.schema: wrong answer schema")
        scenario = value["scenario"]
        frontier = value["frontier"]
    except (KeyError, TypeError) as exc:
        raise ContractError(f"answer is missing a required field: {exc}") from exc

    if scenario not in {
        "formal-foreign-reference-continuation",
        "quantum-certificate-supersession",
        POST_DECISION_SCENARIO,
        TARGET_ABSENCE_SCENARIO,
    }:
        raise ContractError("$.scenario: unsupported scenario")
    require_root(frontier.get("repository_root"), "$.frontier.repository_root")
    if scenario == TARGET_ABSENCE_SCENARIO:
        validate_target_absence(frontier, value.get("absence"))
        if "decision" in value or "continuation" in value:
            raise ContractError("$.absence: target-absence scenario cannot contain scientific state changes")
        return
    if scenario == POST_DECISION_SCENARIO:
        validate_terminal_continuation(frontier, value.get("continuation"))
        if "decision" in value:
            raise ContractError("$.decision: post-Decision scenario cannot contain a pending Decision")
        return
    if frontier.get("configured_targets") != 0:
        raise ContractError("$.frontier.configured_targets: Frontier must have no invented Target")
    if "continuation" in value:
        raise ContractError("$.continuation: pending-Decision scenario cannot contain a terminal continuation")
    try:
        decision = value["decision"]
        delta = decision["standing_delta"]
        scope = delta["scope"]
    except (KeyError, TypeError) as exc:
        raise ContractError(f"answer is missing a required Decision field: {exc}") from exc
    if not isinstance(decision.get("assertion"), str) or not decision["assertion"]:
        raise ContractError("$.decision.assertion: must identify the proposed scientific statement")
    if not isinstance(decision.get("conditions"), list):
        raise ContractError("$.decision.conditions: must be an array")
    if not isinstance(decision.get("limits"), list) or not decision["limits"]:
        raise ContractError("$.decision.limits: must retain at least one scope limit")
    for field in (
        "proposal_root", "source_submission_root", "proposed_claim_root",
        "verification_set_root", "inbox_entry_root",
    ):
        require_root(decision.get(field), f"$.decision.{field}")

    if (decision.get("human_decision_required"), decision.get("verification_is_acceptance")) != (True, False):
        raise ContractError("$.decision: authority boundary is misstated")
    if decision.get("protocol_gate") not in {"satisfied", "blocked"}:
        raise ContractError("$.decision.protocol_gate: invalid state")
    if not isinstance(decision.get("blockers"), list):
        raise ContractError("$.decision.blockers: must be an array")
    if decision.get("staleness") != "current":
        raise ContractError("$.decision.staleness: benchmark requires current state")
    next_obligation = decision.get("next_obligation")
    if not isinstance(next_obligation, dict) or any(
        not isinstance(next_obligation.get(field), str) or not next_obligation[field]
        for field in ("now", "if_accept", "if_reject")
    ):
        raise ContractError("$.decision.next_obligation: exact current branches are required")

    verifications = decision.get("verifications")
    if not isinstance(verifications, list) or not verifications:
        raise ContractError("$.decision.verifications: at least one scoped check is required")
    seen_verifications: set[str] = set()
    for index, verification in enumerate(verifications):
        location = f"$.decision.verifications[{index}]"
        if not isinstance(verification, dict):
            raise ContractError(f"{location}: expected object")
        record_id = verification.get("verification_record_id")
        if not isinstance(record_id, str) or record_id in seen_verifications:
            raise ContractError(f"{location}.verification_record_id: invalid or duplicate")
        seen_verifications.add(record_id)
        require_root(verification.get("verification_record_root"), f"{location}.verification_record_root")
        if verification.get("outcome") not in {"pass", "fail", "inconclusive", "error"}:
            raise ContractError(f"{location}.outcome: invalid")
        if verification.get("protocol_evidence_role") not in {
            "requirement_satisfying", "complementary", "blocking",
        }:
            raise ContractError(f"{location}.protocol_evidence_role: invalid")
        for field in ("property", "verifier"):
            if not isinstance(verification.get(field), str) or not verification[field]:
                raise ContractError(f"{location}.{field}: required")
        for field in ("satisfies_requirements", "does_not_establish"):
            if not isinstance(verification.get(field), list):
                raise ContractError(f"{location}.{field}: must be an array")

    claim_id = decision.get("proposed_claim_id")
    before = delta.get("before", {}).get("accepted")
    accepted = delta.get("if_accept", {}).get("accepted")
    rejected = delta.get("if_reject", {}).get("accepted")
    if not all(isinstance(items, list) for items in (before, accepted, rejected)):
        raise ContractError("$.decision.standing_delta: accepted sets must be arrays")
    if rejected != before:
        raise ContractError("$.decision.standing_delta.if_reject: rejection must preserve scoped Standing")
    if delta.get("before", {}).get("repository_root") != frontier.get("repository_root"):
        raise ContractError("$.decision.standing_delta.before: does not bind the inspected repository")

    counts = delta.get("counts")
    if not isinstance(counts, dict) or not isinstance(counts.get("unchanged_accepted_claims"), int):
        raise ContractError("$.decision.standing_delta.counts: exact counts are required")
    global_counts = counts.get("global_accepted_claims")
    if not isinstance(global_counts, dict):
        raise ContractError("$.decision.standing_delta.counts: global counts are required")
    for field, state in (("before", before), ("if_accept", accepted), ("if_reject", rejected)):
        if global_counts.get(field) != counts["unchanged_accepted_claims"] + len(state):
            raise ContractError("$.decision.standing_delta.counts: scoped and global counts disagree")

    requested_change = decision.get("requested_change")
    if not isinstance(requested_change, dict):
        raise ContractError("$.decision.requested_change: required")
    if scenario == "formal-foreign-reference-continuation":
        if requested_change != {"kind": "add_claim"}:
            raise ContractError("$.decision.requested_change: foreign continuation must add one Claim")
        if delta.get("transition") != "add accepted Claim" or scope != {
            "kind": "proposal_affected_claims",
            "target_claim_id": claim_id,
            "affected_claim_ids": [claim_id],
        }:
            raise ContractError("$.decision.standing_delta: invalid add-Claim scope")
        additions = [item for item in accepted if item not in before]
        if len(accepted) != len(before) + 1 or len(additions) != 1 or additions[0].get("claim_id") != claim_id:
            raise ContractError("$.decision.standing_delta.if_accept: must add exactly the proposed Claim")
    else:
        target_id = requested_change.get("target_claim_id")
        target_root = requested_change.get("target_claim_root")
        require_root(target_root, "$.decision.requested_change.target_claim_root")
        if requested_change.get("kind") != "supersede_claim" or not isinstance(target_id, str):
            raise ContractError("$.decision.requested_change: quantum scenario must supersede one Claim")
        if delta.get("transition") != "supersede accepted Claim with corrected Claim" or scope != {
            "kind": "proposal_affected_claims",
            "target_claim_id": target_id,
            "affected_claim_ids": [claim_id, target_id],
        }:
            raise ContractError("$.decision.standing_delta: invalid supersession scope")
        predecessor = [{"claim_id": target_id, "claim_root": target_root}]
        replacement = [{"claim_id": claim_id, "claim_root": decision.get("proposed_claim_root")}]
        if before != predecessor or rejected != predecessor or accepted != replacement:
            raise ContractError("$.decision.standing_delta: supersession must replace exactly the accepted predecessor")


def validate_target_absence(frontier: dict[str, Any], absence: Any) -> None:
    if frontier.get("configured_targets") != 0:
        raise ContractError("$.frontier.configured_targets: target-absence scenario requires zero Targets")
    if not isinstance(absence, dict) or absence != {
        "target_index_path": "targets.json",
        "target_index_configured": False,
        "availability": {"configured": 0, "stale": 0, "fresh": 0, "returned": 0},
        "returned_target_ids": [],
        "blocker_code": "target_index_not_configured",
        "next_valid_action": "inspect_frontier",
        "may_invent_target": False,
        "standing_changed": False,
        "authority_action_required": False,
    }:
        raise ContractError("$.absence: exact no-work result is required")


def validate_terminal_continuation(frontier: dict[str, Any], continuation: Any) -> None:
    if frontier.get("configured_targets") != 1:
        raise ContractError("$.frontier.configured_targets: post-Decision scenario requires one Target")
    require_root(frontier.get("target_index_root"), "$.frontier.target_index_root")
    if not isinstance(continuation, dict):
        raise ContractError("$.continuation: expected object")
    for field in (
        "accepted_claim_root", "origin_root", "proposal_root", "submission_root",
        "verification_root", "decision_event_root", "packet_root",
    ):
        require_root(continuation.get(field), f"$.continuation.{field}")
    if (
        continuation.get("standing_basis") != "compacted_origin"
        or continuation.get("archive_bytes_re_read") is not False
        or continuation.get("decision_actor") != "human"
        or continuation.get("decision_changes_standing") is not True
        or continuation.get("verification_is_acceptance") is not False
        or continuation.get("next_target_changes_standing") is not False
    ):
        raise ContractError("$.continuation: scientific Standing or authority is misstated")
    for first, last in (("accepted_first", "accepted_through"), ("next_first", "next_last")):
        if not isinstance(continuation.get(first), int) or not isinstance(continuation.get(last), int) or continuation[last] < continuation[first]:
            raise ContractError(f"$.continuation.{first}: invalid range")
    if continuation["next_first"] != continuation["accepted_through"] + 1:
        raise ContractError("$.continuation.next_target: not the first post-Decision Target")


def validate_answer_key(value: Any) -> None:
    if not isinstance(value, dict) or value.get("schema") != ANSWER_KEY_SCHEMA:
        raise ContractError("answer key has the wrong schema")
    require_root(value.get("fixture_root"), "$.fixture_root")
    require_root(value.get("answer_key_root"), "$.answer_key_root")
    validate_answer(value.get("expected"))
    if value.get("scenario") != value["expected"].get("scenario"):
        raise ContractError("answer key scenario does not match expected answer")
    if value["answer_key_root"] != record_root(value, "answer_key_root"):
        raise ContractError("$.answer_key_root: root mismatch")


def validate_action_complete_baseline(value: Any) -> None:
    """Validate the rooted, non-normative baseline for the next campaign."""
    if not isinstance(value, dict) or value.get("schema") != "vela.action-complete-campaign-baseline.v1":
        raise ContractError("campaign baseline has the wrong schema")
    require_root(value.get("baseline_root"), "$.baseline_root")
    if value["baseline_root"] != record_root(value, "baseline_root"):
        raise ContractError("$.baseline_root: root mismatch")
    if not isinstance(value.get("observed_at"), str) or not value["observed_at"]:
        raise ContractError("$.observed_at: required")

    source = value.get("source_state")
    if not isinstance(source, dict):
        raise ContractError("$.source_state: required")
    vela = source.get("vela")
    if not isinstance(vela, dict) or not isinstance(vela.get("version"), str):
        raise ContractError("$.source_state.vela: exact version is required")
    for field in ("binary_sha256", "source_identity_root"):
        require_root(vela.get(field), f"$.source_state.vela.{field}")
    for field in ("git_commit", "git_tree", "remote"):
        if not isinstance(vela.get(field), str) or not vela[field]:
            raise ContractError(f"$.source_state.vela.{field}: required")

    harbor = source.get("harbor")
    if not isinstance(harbor, dict) or not isinstance(harbor.get("version"), str) or not harbor["version"]:
        raise ContractError("$.source_state.harbor.version: required")

    frontiers = source.get("frontiers")
    if not isinstance(frontiers, list) or len(frontiers) != 4:
        raise ContractError("$.source_state.frontiers: exactly four Frontiers are required")
    expected_slugs = {"erdos", "formal-conjectures", "quantum-codes", "sidon-sets"}
    by_slug = {
        item.get("slug"): item
        for item in frontiers
        if isinstance(item, dict) and isinstance(item.get("slug"), str)
    }
    if set(by_slug) != expected_slugs or len(by_slug) != len(frontiers):
        raise ContractError("$.source_state.frontiers: canonical slugs are required exactly once")
    for slug, frontier in by_slug.items():
        for field in ("repository_root", "status_root", "offer_root"):
            require_root(frontier.get(field), f"$.source_state.frontiers[{slug}].{field}")
        for field in ("frontier_id", "git_commit", "git_tree", "remote"):
            if not isinstance(frontier.get(field), str) or not frontier[field]:
                raise ContractError(f"$.source_state.frontiers[{slug}].{field}: required")
        if frontier.get("integrity") != {"replay": "verified", "strict": "pass", "blocker_count": 0}:
            raise ContractError(f"$.source_state.frontiers[{slug}].integrity: strict replay must pass")
        availability = frontier.get("availability")
        if not isinstance(availability, dict):
            raise ContractError(f"$.source_state.frontiers[{slug}].availability: required")
        if slug == "erdos":
            if availability != {"configured": 1, "stale": 0, "fresh": 1, "returned": 1}:
                raise ContractError("$.source_state.frontiers[erdos].availability: one exact Target is required")
            target = frontier.get("target")
            if not isinstance(target, dict) or target.get("target_id") != "erdos:1056":
                raise ContractError("$.source_state.frontiers[erdos].target: exact Target is required")
            require_root(target.get("packet_root"), "$.source_state.frontiers[erdos].target.packet_root")
            if target.get("next_range") != {"first": 10430801, "last": 10431000, "inclusive": True}:
                raise ContractError("$.source_state.frontiers[erdos].target.next_range: unexpected range")
        else:
            if availability != {"configured": 0, "stale": 0, "fresh": 0, "returned": 0}:
                raise ContractError(f"$.source_state.frontiers[{slug}].availability: must expose exact absence")
            if not isinstance(frontier.get("next_action"), str) or not frontier["next_action"]:
                raise ContractError(f"$.source_state.frontiers[{slug}].next_action: blocker is required")

    observatory = source.get("observatory")
    if not isinstance(observatory, dict):
        raise ContractError("$.source_state.observatory: required")
    for field in ("manifest_sha256", "projection_root"):
        require_root(observatory.get(field), f"$.source_state.observatory.{field}")
    if observatory.get("authority") != "read_only_projection":
        raise ContractError("$.source_state.observatory.authority: must remain read-only")
    projected = observatory.get("frontiers")
    if not isinstance(projected, list) or len(projected) != 4:
        raise ContractError("$.source_state.observatory.frontiers: exact projection heads required")
    projected_by_slug = {item.get("slug"): item for item in projected if isinstance(item, dict)}
    for slug, frontier in by_slug.items():
        projection = projected_by_slug.get(slug)
        if not isinstance(projection, dict) or any(
            projection.get(field) != frontier[field]
            for field in ("git_commit", "git_tree", "repository_root")
        ):
            raise ContractError(f"$.source_state.observatory.frontiers[{slug}]: source drift")

    benchmark = value.get("benchmark")
    if not isinstance(benchmark, dict) or benchmark.get("implementation") != "native_harbor":
        raise ContractError("$.benchmark.implementation: Harbor must own execution")
    if benchmark.get("arms") != ["git-files", "vela-guided"]:
        raise ContractError("$.benchmark.arms: matched arms are required")
    if benchmark.get("task_classes") != list(ACTION_COMPLETE_TASK_CLASSES):
        raise ContractError("$.benchmark.task_classes: exact heterogeneous task set required")
    custody = benchmark.get("custody")
    if custody != {
        "answer_key_available_to_agent": False,
        "authority_credentials_available_to_agent": False,
        "canonical_checkout_mutable": False,
        "automatic_decision": False,
    }:
        raise ContractError("$.benchmark.custody: scientific and authority isolation is required")
    pilot = benchmark.get("instrumentation_pilot")
    if pilot != {"repetitions_per_arm": 2, "claim_credit": False}:
        raise ContractError("$.benchmark.instrumentation_pilot: pilot cannot earn claim credit")
    confirmatory = benchmark.get("confirmatory_design")
    if not isinstance(confirmatory, dict) or (
        confirmatory.get("power"),
        confirmatory.get("two_sided_alpha"),
        confirmatory.get("minimum_useful_effect"),
        confirmatory.get("sample_size_rule"),
    ) != (0.8, 0.05, 0.2, "computed_from_blinded_pilot_variance"):
        raise ContractError("$.benchmark.confirmatory_design: power-derived design is required")
    if benchmark.get("primary_metrics") != ["ETY", "VPAC", "FIE", "CPI", "correction_resilience"]:
        raise ContractError("$.benchmark.primary_metrics: compounding metrics are required")
    roots = benchmark.get("contract_roots")
    if not isinstance(roots, dict) or not roots:
        raise ContractError("$.benchmark.contract_roots: benchmark implementation must be bound")
    for name, root in roots.items():
        require_root(root, f"$.benchmark.contract_roots.{name}")
