#!/usr/bin/env python3
"""Pure ADR 0004 fact-manifest resolver and removable projections.

The manifest is a bounded, canonical fact packet produced after exact Git,
Vela replay, named-decision, Receipt, and verifier inspection.  This module
does not perform those checks, read a repository, contact a service, write
frontier state, or infer anything about a child result.  It only:

1. verifies that one supplied fact envelope is strictly shaped and content
   addressed;
2. verifies that every selected-revision fact repeats the same full-root
   DependencyObservation tuple; and
3. projects dependency standing, correction-aware CI, and an accepted-state
   context pack from those facts.

All functions below are deterministic transformations of in-memory values.
The command-line adapters live in separate files.
"""

from __future__ import annotations

import copy
import hashlib
import json
import re
from typing import Any

from dependency_observation import (
    ObservationError,
    canonical_bytes as observation_canonical_bytes,
    observation_root,
    validate_observation,
)


FACT_MANIFEST_SCHEMA = "vela.verifiable-composition.fact-manifest.v0"
FACT_ENVELOPE_SCHEMA = "vela.verifiable-composition.fact-envelope.v0"
RESOLUTION_SCHEMA = "vela.verifiable-composition.resolution.v0"
CI_PROJECTION_SCHEMA = "vela.verifiable-composition.correction-ci.v0"
CONTEXT_PACK_SCHEMA = "vela.verifiable-composition.context-pack.v0"
INSPECTION_RESULT_SCHEMA = "vela.verifiable-composition.delivery-inspection.v0"
INSPECTION_ENVELOPE_SCHEMA = (
    "vela.verifiable-composition.delivery-inspection-envelope.v0"
)

MAX_DOCUMENT_BYTES = 1024 * 1024
MAX_TEXT_BYTES = 64 * 1024
MAX_LIST_ITEMS = 256
MAX_SAFE_INTEGER = 2**53 - 1

SHA256 = re.compile(r"^sha256:[0-9a-f]{64}$")
GIT_OID = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
AUTHORITY = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:@/+~-]*$")
EVENT_ID = re.compile(r"^vev_[0-9a-f]{16}$")
SIGNATURE = re.compile(r"^(?:v1:)?[0-9a-f]{128}$")
EVENT_CONTENT_FIELDS = (
    "schema",
    "kind",
    "target",
    "actor",
    "timestamp",
    "reason",
    "before_hash",
    "after_hash",
    "payload",
    "caveats",
)

FINDING_STANDING = {"accepted", "corrected", "superseded", "withdrawn"}
DECISION_STANDING = {"valid", "revoked", "missing", "invalid"}
VERIFIER_STANDING = {"valid", "revoked", "missing", "invalid"}
EVIDENCE_STANDING = {"available", "missing", "invalid"}

ACTIVE_ROLES = {"hard", "data", "method"}
REVIEW_ONLY_ROLES = {"soft", "contextual"}

DEPENDENCY_STATUSES = {
    "satisfied",
    "warning",
    "review_required",
    "blocked",
    "stale",
    "forked",
    "unresolvable",
}


class ManifestError(ValueError):
    """One stable, fail-closed manifest validation result."""

    def __init__(self, code: str, detail: str = "") -> None:
        super().__init__(code)
        self.code = code
        self.detail = detail


def _reject_constant(value: str) -> None:
    raise ManifestError("invalid:nonfinite_number", value)


def _object_without_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ManifestError("duplicate:object_name", key)
        result[key] = value
    return result


def canonical_bytes(value: Any) -> bytes:
    """Return the one canonical JSON encoding used by the projection."""

    _validate_safe_json(value)
    try:
        return json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    except (TypeError, ValueError, RecursionError) as error:
        raise ManifestError("invalid:canonical_json", type(error).__name__) from error


def _validate_safe_json(value: Any, path: str = "$") -> None:
    if value is None or isinstance(value, (bool, str)):
        return
    if isinstance(value, int):
        if abs(value) > MAX_SAFE_INTEGER:
            raise ManifestError("invalid:unsafe_integer", path)
        return
    if isinstance(value, float):
        raise ManifestError("invalid:float", path)
    if isinstance(value, list):
        for index, item in enumerate(value):
            _validate_safe_json(item, f"{path}[{index}]")
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise ManifestError("invalid:object_key", path)
            _validate_safe_json(item, f"{path}.{key}")
        return
    raise ManifestError("invalid:json_type", path)


def sha256_root(raw: bytes) -> str:
    return f"sha256:{hashlib.sha256(raw).hexdigest()}"


def fact_manifest_root(manifest: dict[str, Any]) -> str:
    return sha256_root(canonical_bytes(manifest))


def finding_revision_root(finding: dict[str, Any]) -> str:
    """Mirror Vela's finding hash: mutable review links are excluded."""

    value = copy.deepcopy(finding)
    value["links"] = []
    return sha256_root(canonical_bytes(value))


def build_envelope(manifest: dict[str, Any]) -> dict[str, Any]:
    """Build, but do not authorize, one content-addressed fact envelope."""

    validate_fact_manifest(manifest)
    return {
        "schema": FACT_ENVELOPE_SCHEMA,
        "fact_manifest_root": fact_manifest_root(manifest),
        "fact_manifest": copy.deepcopy(manifest),
    }


def parse_envelope(raw: bytes) -> dict[str, Any]:
    """Parse and validate one bounded fact envelope without side effects."""

    if len(raw) > MAX_DOCUMENT_BYTES:
        raise ManifestError("oversized:document")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ManifestError("invalid:utf8") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_object_without_duplicates,
            parse_constant=_reject_constant,
        )
    except ManifestError:
        raise
    except (json.JSONDecodeError, RecursionError) as error:
        raise ManifestError("invalid:json") from error
    validate_envelope(value)
    return value


def validate_envelope(value: Any) -> None:
    _exact_object(
        value,
        {"schema", "fact_manifest_root", "fact_manifest"},
        "envelope",
    )
    _exact(value, "schema", FACT_ENVELOPE_SCHEMA)
    _root(value, "fact_manifest_root")
    manifest = value["fact_manifest"]
    validate_fact_manifest(manifest)
    derived = fact_manifest_root(manifest)
    if value["fact_manifest_root"] != derived:
        raise ManifestError(
            "mismatch:fact_manifest_root",
            f"asserted {value['fact_manifest_root']}; derived {derived}",
        )


def validate_fact_manifest(value: Any) -> None:
    _exact_object(
        value,
        {
            "schema",
            "dependency",
            "accepted_finding",
            "last_seen",
            "delivered",
            "delivery_inspection",
            "standing",
        },
        "fact_manifest",
    )
    _exact(value, "schema", FACT_MANIFEST_SCHEMA)
    _validate_safe_json(value)

    dependency = value["dependency"]
    try:
        validate_observation(dependency)
    except ObservationError as error:
        raise ManifestError("invalid:dependency_observation", str(error)) from error
    expected_decision_id = f"vev_{dependency['decision_event_content_root'][7:23]}"
    if dependency["decision_event_id"] != expected_decision_id:
        raise ManifestError(
            "mismatch:decision_event_id",
            "decision event handle does not match its full content root",
        )
    _require_sorted_roots(dependency["receipt_roots"], "dependency.receipt_roots")
    _require_sorted_attachments(
        dependency["verifier_attachments"],
        "dependency.verifier_attachments",
    )

    _validate_finding(value["accepted_finding"], dependency)
    _validate_state_root(value["last_seen"], "last_seen")
    _validate_state_root(value["delivered"], "delivered")
    parent_bindings = {
        "parent_git_commit": "git_commit",
        "parent_git_tree": "git_tree",
        "parent_event_log_root": "event_log_root",
        "parent_snapshot_root": "snapshot_root",
    }
    for dependency_field, state_field in parent_bindings.items():
        if dependency[dependency_field] != value["last_seen"][state_field]:
            raise ManifestError(
                f"mismatch:dependency_{dependency_field}",
                "the exact dependency parent must equal the last-seen state",
            )
    _validate_delivery_inspection(
        value["delivery_inspection"],
        value["last_seen"],
        value["delivered"],
        dependency,
    )
    _validate_standing(
        value["standing"],
        dependency,
        value["delivery_inspection"],
    )


def _validate_finding(value: Any, dependency: dict[str, Any]) -> None:
    if not isinstance(value, dict):
        raise ManifestError("invalid:accepted_finding")
    identifier = value.get("id")
    if identifier != dependency["finding_id"]:
        raise ManifestError(
            "mismatch:finding_id",
            "accepted finding handle does not match the exact dependency tuple",
        )
    links = value.get("links")
    if not isinstance(links, list) or len(links) > MAX_LIST_ITEMS:
        raise ManifestError("invalid:accepted_finding_links")
    assertion = value.get("assertion")
    if not isinstance(assertion, dict):
        raise ManifestError("invalid:accepted_finding_assertion")
    _bounded_text(assertion.get("text"), "accepted_finding.assertion.text")
    conditions = value.get("conditions")
    if not isinstance(conditions, dict):
        raise ManifestError("invalid:accepted_finding_conditions")
    condition_text = conditions.get("text")
    if condition_text is not None:
        _bounded_text(condition_text, "accepted_finding.conditions.text")
    flags = value.get("flags")
    if not isinstance(flags, dict) or not isinstance(flags.get("retracted"), bool):
        raise ManifestError("invalid:accepted_finding_flags")
    derived = finding_revision_root(value)
    if derived != dependency["finding_revision_root"]:
        raise ManifestError(
            "mismatch:finding_revision_root",
            f"dependency asserts {dependency['finding_revision_root']}; finding derives {derived}",
        )


def _validate_state_root(value: Any, label: str) -> None:
    _exact_object(
        value,
        {"git_commit", "git_tree", "event_log_root", "snapshot_root"},
        label,
    )
    _git_oid(value, "git_commit")
    _git_oid(value, "git_tree")
    if len(value["git_commit"]) != len(value["git_tree"]):
        raise ManifestError(f"invalid:{label}_git_object_format")
    _root(value, "event_log_root")
    _root(value, "snapshot_root")


def _validate_delivery_inspection(
    value: Any,
    last_seen: dict[str, Any],
    delivered: dict[str, Any],
    dependency: dict[str, Any],
) -> None:
    _exact_object(
        value,
        {"schema", "inspection_root", "result"},
        "delivery_inspection",
    )
    _exact(value, "schema", INSPECTION_ENVELOPE_SCHEMA)
    _root(value, "inspection_root")
    result = value["result"]
    fields = {
        "schema",
        "verification",
        "bundle_root",
        "state_path",
        "last_seen_git_commit",
        "last_seen_git_tree",
        "delivered_git_commit",
        "delivered_git_tree",
        "merge_base",
        "git_relation",
        "event_relation",
        "last_seen_snapshot",
        "delivered_snapshot",
        "last_seen_events",
        "delivered_events",
        "last_seen_state_document_root",
        "delivered_state_document_root",
    }
    _exact_object(result, fields, "delivery_inspection.result")
    _exact(result, "schema", INSPECTION_RESULT_SCHEMA)
    _exact(result, "verification", "verified")
    _root(result, "bundle_root")
    _bounded_relative_path(result.get("state_path"), "state_path")
    for field in (
        "last_seen_git_commit",
        "last_seen_git_tree",
        "delivered_git_commit",
        "delivered_git_tree",
        "merge_base",
    ):
        _git_oid(result, field)
    _enum(
        result,
        "git_relation",
        {"same", "descendant", "ancestor", "forked"},
    )
    _enum(
        result,
        "event_relation",
        {"same", "descendant", "ancestor", "forked"},
    )
    for field in (
        "last_seen_state_document_root",
        "delivered_state_document_root",
    ):
        _root(result, field)
    if value["inspection_root"] != sha256_root(canonical_bytes(result)):
        raise ManifestError("mismatch:delivery_inspection_root")

    last_events = _validate_events(result["last_seen_events"], "last_seen_events")
    delivered_events = _validate_events(
        result["delivered_events"],
        "delivered_events",
    )
    for field in ("last_seen_snapshot", "delivered_snapshot"):
        if not isinstance(result[field], dict):
            raise ManifestError(f"invalid:{field}")
    derived_last = {
        "git_commit": result["last_seen_git_commit"],
        "git_tree": result["last_seen_git_tree"],
        "event_log_root": _event_log_root(result["last_seen_events"]),
        "snapshot_root": sha256_root(canonical_bytes(result["last_seen_snapshot"])),
    }
    derived_delivered = {
        "git_commit": result["delivered_git_commit"],
        "git_tree": result["delivered_git_tree"],
        "event_log_root": _event_log_root(result["delivered_events"]),
        "snapshot_root": sha256_root(canonical_bytes(result["delivered_snapshot"])),
    }
    if derived_last != last_seen:
        raise ManifestError("mismatch:inspection_last_seen")
    if derived_delivered != delivered:
        raise ManifestError("mismatch:inspection_delivered")

    derived_git_relation = _git_relation(
        result["last_seen_git_commit"],
        result["delivered_git_commit"],
        result["merge_base"],
    )
    if result["git_relation"] != derived_git_relation:
        raise ManifestError("mismatch:inspection_git_relation")
    derived_event_relation = _sequence_relation(last_events, delivered_events)
    if result["event_relation"] != derived_event_relation:
        raise ManifestError("mismatch:inspection_event_relation")
    compatible = {
        "same": {"same"},
        "descendant": {"same", "descendant"},
        "ancestor": {"same", "ancestor"},
        "forked": {"same", "descendant", "ancestor", "forked"},
    }
    if derived_event_relation not in compatible[derived_git_relation]:
        raise ManifestError("mismatch:inspection_git_event_continuity")

    decision_matches = [
        event
        for event in result["last_seen_events"]
        if _event_content_root(event) == dependency["decision_event_content_root"]
    ]
    if len(decision_matches) != 1:
        raise ManifestError("mismatch:inspection_parent_decision")
    decision = decision_matches[0]
    actor = decision.get("actor")
    actor_id = actor.get("id") if isinstance(actor, dict) else None
    if (
        decision["id"] != dependency["decision_event_id"]
        or decision["signature"] != dependency["decision_signature"]
        or actor_id != dependency["authority_id"]
        or dependency["decision_event_content_root"] not in delivered_events
    ):
        raise ManifestError("mismatch:inspection_parent_decision")


def _validate_events(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or len(value) > 4096:
        raise ManifestError(f"invalid:{label}")
    roots = [_event_content_root(event) for event in value]
    if len(roots) != len(set(roots)):
        raise ManifestError(f"duplicate:{label}")
    return roots


def _event_content_root(event: Any) -> str:
    if not isinstance(event, dict):
        raise ManifestError("invalid:event")
    required = set(EVENT_CONTENT_FIELDS) | {"id", "signature"}
    if set(event) != required:
        raise ManifestError("invalid:event_fields")
    identifier = event["id"]
    signature = event["signature"]
    if not isinstance(identifier, str) or not EVENT_ID.fullmatch(identifier):
        raise ManifestError("invalid:event_id")
    if not isinstance(signature, str) or not SIGNATURE.fullmatch(signature):
        raise ManifestError("invalid:event_signature")
    preimage = {field: event[field] for field in EVENT_CONTENT_FIELDS}
    root = sha256_root(canonical_bytes(preimage))
    if identifier != f"vev_{root[7:23]}":
        raise ManifestError("mismatch:event_id")
    return root


def _event_log_root(events: list[dict[str, Any]]) -> str:
    stripped: list[dict[str, Any]] = []
    identifiers: set[str] = set()
    for event in events:
        _event_content_root(event)
        if event["id"] in identifiers:
            raise ManifestError("duplicate:event_id")
        identifiers.add(event["id"])
        item = copy.deepcopy(event)
        item.pop("signature")
        stripped.append(item)
    stripped.sort(key=lambda event: event["id"])
    return sha256_root(canonical_bytes(stripped))


def _git_relation(last_seen: str, delivered: str, merge_base: str) -> str:
    if last_seen == delivered:
        if merge_base != last_seen:
            raise ManifestError("mismatch:inspection_merge_base")
        return "same"
    if merge_base == last_seen:
        return "descendant"
    if merge_base == delivered:
        return "ancestor"
    return "forked"


def _sequence_relation(last_seen: list[str], delivered: list[str]) -> str:
    if last_seen == delivered:
        return "same"
    if delivered[: len(last_seen)] == last_seen:
        return "descendant"
    if last_seen[: len(delivered)] == delivered:
        return "ancestor"
    return "forked"


def _bounded_relative_path(value: Any, label: str) -> None:
    if (
        not isinstance(value, str)
        or not value
        or value.startswith("/")
        or "\\" in value
        or len(value.encode("utf-8")) > 1024
        or any(part in {"", ".", ".."} for part in value.split("/"))
    ):
        raise ManifestError(f"invalid:{label}")


def _validate_standing(
    value: Any,
    dependency: dict[str, Any],
    delivery_inspection: dict[str, Any],
) -> None:
    _exact_object(
        value,
        {
            "selected_finding_revision_root",
            "decision_event_content_root",
            "authority_id",
            "receipt_roots",
            "verifier_attachments",
            "premise_digest",
            "finding_status",
            "decision_status",
            "verifier_status",
            "evidence_status",
            "change_event",
        },
        "standing",
    )
    for field in (
        "selected_finding_revision_root",
        "decision_event_content_root",
        "premise_digest",
    ):
        _root(value, field)
    _authority(value, "authority_id")
    _root_list(value, "receipt_roots")
    _attachments(value, "verifier_attachments")
    _require_sorted_roots(value["receipt_roots"], "standing.receipt_roots")
    _require_sorted_attachments(
        value["verifier_attachments"],
        "standing.verifier_attachments",
    )
    _enum(value, "finding_status", FINDING_STANDING)
    _enum(value, "decision_status", DECISION_STANDING)
    _enum(value, "verifier_status", VERIFIER_STANDING)
    _enum(value, "evidence_status", EVIDENCE_STANDING)

    bindings = {
        "selected_finding_revision_root": "finding_revision_root",
        "decision_event_content_root": "decision_event_content_root",
        "authority_id": "authority_id",
        "receipt_roots": "receipt_roots",
        "verifier_attachments": "verifier_attachments",
        "premise_digest": "premise_digest",
    }
    for standing_field, dependency_field in bindings.items():
        if value[standing_field] != dependency[dependency_field]:
            raise ManifestError(
                f"mismatch:standing_{standing_field}",
                "later standing does not refer to the exact selected dependency tuple",
            )

    changed_dimensions = [
        name
        for name, changed in (
            ("finding", value["finding_status"] != "accepted"),
            ("decision", value["decision_status"] == "revoked"),
            ("verifier", value["verifier_status"] == "revoked"),
        )
        if changed
    ]
    if len(changed_dimensions) > 1:
        raise ManifestError(
            "invalid:standing_multiple_changes",
            "one fact manifest carries one exact later change event; combined standing changes require separately delivered manifests",
        )
    changed = bool(changed_dimensions)
    _validate_change_event(
        value["change_event"],
        changed,
        value,
        delivery_inspection,
    )


def _validate_change_event(
    value: Any,
    changed: bool,
    standing: dict[str, Any],
    delivery_inspection: dict[str, Any],
) -> None:
    if not changed:
        if value is not None:
            raise ManifestError("unexpected:change_event")
        return
    if value is None:
        raise ManifestError("missing:change_event")
    _exact_object(
        value,
        {
            "event_id",
            "event_content_root",
            "event_signature",
            "authority_id",
            "effect",
            "inspection_result_root",
        },
        "change_event",
    )
    event_id = value["event_id"]
    if not isinstance(event_id, str) or not EVENT_ID.fullmatch(event_id):
        raise ManifestError("invalid:change_event.event_id")
    _root(value, "event_content_root")
    signature = value["event_signature"]
    if not isinstance(signature, str) or not SIGNATURE.fullmatch(signature):
        raise ManifestError("invalid:change_event.event_signature")
    _authority(value, "authority_id")
    _enum(
        value,
        "effect",
        {
            "corrected",
            "superseded",
            "withdrawn",
            "decision_revoked",
            "verifier_revoked",
        },
    )
    _root(value, "inspection_result_root")
    expected_id = f"vev_{value['event_content_root'][7:23]}"
    if event_id != expected_id:
        raise ManifestError("mismatch:change_event.event_id")
    if value["inspection_result_root"] != delivery_inspection["inspection_root"]:
        raise ManifestError("mismatch:change_event.inspection_result_root")
    result = delivery_inspection["result"]
    last_roots = [_event_content_root(event) for event in result["last_seen_events"]]
    matching = [
        event
        for event in result["delivered_events"]
        if _event_content_root(event) == value["event_content_root"]
    ]
    if value["event_content_root"] in last_roots or len(matching) != 1:
        raise ManifestError("mismatch:change_event.delivered_history")
    event = matching[0]
    actor = event.get("actor")
    actor_id = actor.get("id") if isinstance(actor, dict) else None
    payload = event.get("payload")
    effect = payload.get("dependency_effect") if isinstance(payload, dict) else None
    if (
        event["id"] != value["event_id"]
        or event["signature"] != value["event_signature"]
        or actor_id != value["authority_id"]
        or effect != value["effect"]
    ):
        raise ManifestError("mismatch:change_event.delivered_history")
    expected_effect = _standing_effect(standing)
    if value["effect"] != expected_effect:
        raise ManifestError("mismatch:change_event.effect")


def _standing_effect(standing: dict[str, Any]) -> str:
    if standing["finding_status"] != "accepted":
        return standing["finding_status"]
    if standing["decision_status"] == "revoked":
        return "decision_revoked"
    if standing["verifier_status"] == "revoked":
        return "verifier_revoked"
    raise ManifestError("invalid:change_event.effect")


def resolve_envelope(envelope: dict[str, Any]) -> dict[str, Any]:
    """Classify one validated manifest without claiming child truth."""

    validate_envelope(envelope)
    manifest = envelope["fact_manifest"]
    dependency = manifest["dependency"]
    inspection = manifest["delivery_inspection"]
    inspection_result = inspection["result"]
    standing = manifest["standing"]
    relation = inspection_result["git_relation"]
    verification = inspection_result["verification"]

    status: str
    code: str
    reasons: list[str]

    if verification != "verified" or relation == "unresolvable":
        status = "unresolvable"
        code = "unresolvable:continuity_not_verified"
        reasons = [
            "the supplied fact packet does not contain a verified Git and event-history continuity result"
        ]
    elif relation == "ancestor":
        status = "stale"
        code = "stale:delivered_root_precedes_last_seen"
        reasons = [
            "the delivered root is a verified ancestor of the recorded last-seen root"
        ]
    elif relation == "forked":
        status = "forked"
        code = "forked:delivered_root_outside_selected_lineage"
        reasons = [
            "the delivered root is valid but not a descendant of the selected lineage"
        ]
    elif standing["evidence_status"] != "available":
        status = "unresolvable"
        code = f"unresolvable:evidence_{standing['evidence_status']}"
        reasons = [
            "one or more exact Receipt, verifier, decision, or finding bytes are unavailable or invalid"
        ]
    elif standing["decision_status"] in {"missing", "invalid"}:
        status = "unresolvable"
        code = f"unresolvable:decision_{standing['decision_status']}"
        reasons = ["the exact named authority decision cannot be verified"]
    elif standing["verifier_status"] in {"missing", "invalid"}:
        status = "unresolvable"
        code = f"unresolvable:verifier_{standing['verifier_status']}"
        reasons = ["the exact selected verifier material cannot be verified"]
    elif standing["finding_status"] in {"corrected", "superseded"}:
        if dependency["role"] in REVIEW_ONLY_ROLES:
            status = "warning"
            code = f"warning:finding_{standing['finding_status']}"
            reasons = [
                "later authorized information is relevant, but this soft or contextual dependency remains usable under the profile"
            ]
        else:
            status = "review_required"
            code = f"review_required:finding_{standing['finding_status']}"
            reasons = [
                "a later authorized state changes the standing of the exact selected revision"
            ]
    elif (
        standing["finding_status"] == "withdrawn"
        or standing["decision_status"] == "revoked"
        or standing["verifier_status"] == "revoked"
    ):
        if dependency["role"] in ACTIVE_ROLES:
            status = "blocked"
            code = _blocked_code(standing)
            reasons = [
                "the selected hard, data, or method dependency no longer has the required standing"
            ]
        else:
            status = "review_required"
            code = _review_code(standing)
            reasons = [
                "the selected soft or contextual dependency changed standing and must be reconsidered"
            ]
    else:
        status = "satisfied"
        code = "satisfied:exact_dependency_retains_standing"
        reasons = [
            "the exact selected revision, decision, verifier material, evidence roots, and premise digest retain their required standing"
        ]

    if status not in DEPENDENCY_STATUSES:
        raise AssertionError(f"unexpected dependency status {status}")
    return {
        "schema": RESOLUTION_SCHEMA,
        "projection": "derived_read_only",
        "rebuildable": True,
        "authoritative": False,
        "fact_manifest_root": envelope["fact_manifest_root"],
        "dependency_observation_root": observation_root(dependency),
        "dependency_status": status,
        "code": code,
        "reasons": reasons,
        "role": dependency["role"],
        "selected_parent": _selected_parent(dependency),
        "last_seen": copy.deepcopy(manifest["last_seen"]),
        "delivered": copy.deepcopy(manifest["delivered"]),
        "delivery_inspection_root": inspection["inspection_root"],
        "continuity": {
            "git_relation": inspection_result["git_relation"],
            "event_relation": inspection_result["event_relation"],
            "merge_base": inspection_result["merge_base"],
            "bundle_root": inspection_result["bundle_root"],
            "verification": inspection_result["verification"],
        },
        "change_event": copy.deepcopy(standing["change_event"]),
        "requires_review": status in {"review_required", "blocked"},
        "blocks_consumption": status
        in {"review_required", "blocked", "stale", "forked", "unresolvable"},
        "child_truth": "not_assessed",
        "child_mutation": "none",
        "authority_effect": "none",
        "writes": [],
        "caveats": [
            "This projection classifies the dependency, not the truth of any child result.",
            "The fact manifest must come from separately verified exact Git, Vela, decision, Receipt, and verifier inspection.",
            "No authority event or accepted state is created or changed.",
        ],
    }


def unresolvable_projection(error: ManifestError) -> dict[str, Any]:
    """Stable projection for malformed or unbound input."""

    return {
        "schema": RESOLUTION_SCHEMA,
        "projection": "derived_read_only",
        "rebuildable": True,
        "authoritative": False,
        "fact_manifest_root": None,
        "dependency_observation_root": None,
        "dependency_status": "unresolvable",
        "code": f"unresolvable:{error.code}",
        "reasons": [
            error.detail or "the exact bounded fact manifest is invalid or unavailable"
        ],
        "role": None,
        "selected_parent": None,
        "last_seen": None,
        "delivered": None,
        "delivery_inspection_root": None,
        "continuity": None,
        "change_event": None,
        "requires_review": False,
        "blocks_consumption": True,
        "child_truth": "not_assessed",
        "child_mutation": "none",
        "authority_effect": "none",
        "writes": [],
        "caveats": [
            "This projection classifies the dependency, not the truth of any child result.",
            "Invalid input cannot be promoted into accepted context.",
            "No authority event or accepted state is created or changed.",
        ],
    }


def resolve_bytes(raw: bytes) -> tuple[dict[str, Any] | None, dict[str, Any]]:
    """Parse and resolve bytes, returning a typed fail-closed result."""

    try:
        envelope = parse_envelope(raw)
        return envelope, resolve_envelope(envelope)
    except ManifestError as error:
        return None, unresolvable_projection(error)


def correction_ci_projection(resolution: dict[str, Any]) -> dict[str, Any]:
    """Adapt one resolver result into removable correction-aware CI."""

    status = resolution["dependency_status"]
    if status == "satisfied":
        gate = "pass"
        exit_code = 0
    elif status == "warning":
        gate = "warn"
        exit_code = 0
    elif status == "review_required":
        gate = "review"
        exit_code = 20
    elif status == "blocked":
        gate = "fail"
        exit_code = 21
    elif status == "stale":
        gate = "fail"
        exit_code = 22
    elif status == "forked":
        gate = "fail"
        exit_code = 23
    else:
        gate = "fail"
        exit_code = 24
    return {
        "schema": CI_PROJECTION_SCHEMA,
        "projection": "derived_read_only",
        "rebuildable": True,
        "authoritative": False,
        "fact_manifest_root": resolution["fact_manifest_root"],
        "dependency_observation_root": resolution["dependency_observation_root"],
        "dependency_status": status,
        "code": resolution["code"],
        "gate": gate,
        "suggested_exit_code": exit_code,
        "review_targets": (
            [copy.deepcopy(resolution["selected_parent"])]
            if status in {"review_required", "blocked"}
            and resolution["selected_parent"] is not None
            else []
        ),
        "warning_targets": (
            [copy.deepcopy(resolution["selected_parent"])]
            if status == "warning" and resolution["selected_parent"] is not None
            else []
        ),
        "child_truth": "not_assessed",
        "child_mutation": "none",
        "authority_effect": "none",
        "writes": [],
        "message": (
            "Dependency standing is satisfied."
            if status == "satisfied"
            else (
                "Dependency remains usable with a visible warning; no conclusion about child truth is made."
                if status == "warning"
                else "Dependency standing requires explicit handling; no conclusion about child truth is made."
            )
        ),
    }


def accepted_context_pack_projection(
    envelope: dict[str, Any] | None,
    resolution: dict[str, Any],
) -> dict[str, Any]:
    """Build a compact context pack without silently reactivating stale state."""

    status = resolution["dependency_status"]
    active_context: list[dict[str, Any]] = []
    quarantined_context: list[dict[str, Any]] = []
    context_warnings: list[dict[str, Any]] = []
    if envelope is not None and status in {"satisfied", "warning"}:
        manifest = envelope["fact_manifest"]
        dependency = manifest["dependency"]
        finding = copy.deepcopy(manifest["accepted_finding"])
        finding["links"] = []
        active_context.append(
            {
                "finding": finding,
                "finding_revision_root": dependency["finding_revision_root"],
                "decision_event_id": dependency["decision_event_id"],
                "decision_event_content_root": dependency[
                    "decision_event_content_root"
                ],
                "authority_id": dependency["authority_id"],
                "receipt_roots": copy.deepcopy(dependency["receipt_roots"]),
                "verifier_attachments": copy.deepcopy(
                    dependency["verifier_attachments"]
                ),
                "premise_digest": dependency["premise_digest"],
                "role": dependency["role"],
                "links_omitted_as_mutable_review_surface": True,
                "activation": (
                    "accepted" if status == "satisfied" else "usable_with_warning"
                ),
            }
        )
        if status == "warning":
            context_warnings.append(
                {
                    "code": resolution["code"],
                    "selected_parent": copy.deepcopy(resolution["selected_parent"]),
                }
            )
    elif resolution["selected_parent"] is not None:
        quarantined_context.append(
            {
                "dependency_status": status,
                "selected_parent": copy.deepcopy(resolution["selected_parent"]),
                "code": resolution["code"],
            }
        )
    return {
        "schema": CONTEXT_PACK_SCHEMA,
        "projection": "derived_read_only",
        "rebuildable": True,
        "authoritative": False,
        "fact_manifest_root": resolution["fact_manifest_root"],
        "dependency_observation_root": resolution["dependency_observation_root"],
        "dependency_status": status,
        "active_context": active_context,
        "quarantined_context": quarantined_context,
        "context_warnings": context_warnings,
        "active_context_count": len(active_context),
        "child_truth": "not_assessed",
        "child_mutation": "none",
        "authority_effect": "none",
        "writes": [],
        "caveats": [
            "Only satisfied dependencies and explicitly visible non-blocking warnings enter active context.",
            "Review-required, blocked, stale, forked, and unresolvable dependencies remain quarantined.",
            "This pack is disposable context porcelain and cannot authorize, sign, accept, or apply a claim.",
        ],
    }


def _selected_parent(dependency: dict[str, Any]) -> dict[str, Any]:
    return copy.deepcopy(dependency)


def _blocked_code(standing: dict[str, Any]) -> str:
    if standing["finding_status"] == "withdrawn":
        return "blocked:finding_withdrawn"
    if standing["decision_status"] == "revoked":
        return "blocked:decision_revoked"
    return "blocked:verifier_revoked"


def _review_code(standing: dict[str, Any]) -> str:
    if standing["finding_status"] == "withdrawn":
        return "review_required:finding_withdrawn"
    if standing["decision_status"] == "revoked":
        return "review_required:decision_revoked"
    return "review_required:verifier_revoked"


def _exact_object(value: Any, fields: set[str], label: str) -> None:
    if not isinstance(value, dict):
        raise ManifestError(f"invalid:{label}")
    missing = sorted(fields - set(value))
    if missing:
        raise ManifestError(f"missing:{label}.{missing[0]}")
    unexpected = sorted(set(value) - fields)
    if unexpected:
        raise ManifestError(f"unexpected:{label}.{unexpected[0]}")


def _exact(value: dict[str, Any], field: str, expected: str) -> None:
    if value.get(field) != expected:
        raise ManifestError(f"invalid:{field}")


def _bounded_text(value: Any, label: str) -> None:
    if not isinstance(value, str) or not value.strip():
        raise ManifestError(f"invalid:{label}")
    if len(value.encode("utf-8")) > MAX_TEXT_BYTES:
        raise ManifestError(f"oversized:{label}")


def _root(value: dict[str, Any], field: str) -> None:
    item = value.get(field)
    if not isinstance(item, str) or not SHA256.fullmatch(item):
        raise ManifestError(f"invalid:{field}")


def _git_oid(value: dict[str, Any], field: str) -> None:
    item = value.get(field)
    if not isinstance(item, str) or not GIT_OID.fullmatch(item):
        raise ManifestError(f"invalid:{field}")


def _authority(value: dict[str, Any], field: str) -> None:
    item = value.get(field)
    if (
        not isinstance(item, str)
        or len(item.encode("utf-8")) > 256
        or not AUTHORITY.fullmatch(item)
    ):
        raise ManifestError(f"invalid:{field}")


def _enum(value: dict[str, Any], field: str, allowed: set[str]) -> None:
    if value.get(field) not in allowed:
        raise ManifestError(f"invalid:{field}")


def _root_list(value: dict[str, Any], field: str) -> None:
    items = value.get(field)
    if not isinstance(items, list) or not items or len(items) > MAX_LIST_ITEMS:
        raise ManifestError(f"invalid:{field}")
    if any(not isinstance(item, str) or not SHA256.fullmatch(item) for item in items):
        raise ManifestError(f"invalid:{field}")
    if len(set(items)) != len(items):
        raise ManifestError(f"duplicate:{field}")


def _attachments(value: dict[str, Any], field: str) -> None:
    items = value.get(field)
    if not isinstance(items, list) or not items or len(items) > MAX_LIST_ITEMS:
        raise ManifestError(f"invalid:{field}")
    seen: set[str] = set()
    for item in items:
        if not isinstance(item, dict) or set(item) != {
            "attachment_id",
            "attachment_content_root",
        }:
            raise ManifestError(f"invalid:{field}")
        identifier = item["attachment_id"]
        root = item["attachment_content_root"]
        if (
            not isinstance(identifier, str)
            or not re.fullmatch(r"vva_[0-9a-f]{16}", identifier)
            or not isinstance(root, str)
            or not SHA256.fullmatch(root)
        ):
            raise ManifestError(f"invalid:{field}")
        if identifier in seen:
            raise ManifestError(f"duplicate:{field}")
        seen.add(identifier)


def _require_sorted_roots(items: list[str], label: str) -> None:
    if items != sorted(items):
        raise ManifestError(f"noncanonical:{label}")


def _require_sorted_attachments(items: list[dict[str, str]], label: str) -> None:
    expected = sorted(
        items,
        key=lambda item: (
            item["attachment_id"],
            item["attachment_content_root"],
        ),
    )
    if items != expected:
        raise ManifestError(f"noncanonical:{label}")


def canonical_observation_bytes(value: dict[str, Any]) -> bytes:
    """Expose the exact tuple bytes used by both benchmark representations."""

    try:
        return observation_canonical_bytes(value)
    except ObservationError as error:
        raise ManifestError("invalid:dependency_observation", str(error)) from error
