#!/usr/bin/env python3
"""Independent stdlib Reader C for the ADR 0004 fact-manifest profile.

This file intentionally does not import the Vela implementation, the reference
resolver, or ``fact_manifest.py``.  It independently parses the frozen profile,
rederives its roots and continuity facts, and returns only dependency standing.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import stat
import sys
from pathlib import Path
from typing import Any


ENVELOPE_SCHEMA = "vela.verifiable-composition.fact-envelope.v0"
MANIFEST_SCHEMA = "vela.verifiable-composition.fact-manifest.v0"
OBSERVATION_SCHEMA = "vela.experimental-dependency-observation.v0"
INSPECTION_ENVELOPE_SCHEMA = (
    "vela.verifiable-composition.delivery-inspection-envelope.v0"
)
INSPECTION_RESULT_SCHEMA = "vela.verifiable-composition.delivery-inspection.v0"
MAX_BYTES = 1024 * 1024
MAX_SAFE_INTEGER = 2**53 - 1
SHA = re.compile(r"^sha256:[0-9a-f]{64}$")
GIT = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
EVENT_ID = re.compile(r"^vev_[0-9a-f]{16}$")
ATTACHMENT_ID = re.compile(r"^vva_[0-9a-f]{16}$")
SIGNATURE = re.compile(r"^(?:v1:)?[0-9a-f]{128}$")
AUTHORITY = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:@/+~-]*$")
ROLES = {"hard", "soft", "data", "method", "contextual"}
HARD_ROLES = {"hard", "data", "method"}
SOFT_ROLES = {"soft", "contextual"}
EVENT_CORE = (
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


class ReaderError(ValueError):
    pass


def _object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ReaderError("duplicate")
        value[key] = item
    return value


def _constant(_: str) -> None:
    raise ReaderError("nonfinite")


def _safe(value: Any) -> None:
    if value is None or isinstance(value, (bool, str)):
        return
    if isinstance(value, int):
        if abs(value) > MAX_SAFE_INTEGER:
            raise ReaderError("unsafe_integer")
        return
    if isinstance(value, float):
        raise ReaderError("float")
    if isinstance(value, list):
        for item in value:
            _safe(item)
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise ReaderError("key")
            _safe(item)
        return
    raise ReaderError("type")


def canonical(value: Any) -> bytes:
    _safe(value)
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()


def digest(value: Any) -> str:
    return f"sha256:{hashlib.sha256(canonical(value)).hexdigest()}"


def _exact(value: Any, fields: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != fields:
        raise ReaderError("shape")
    return value


def _sha(value: Any) -> str:
    if not isinstance(value, str) or not SHA.fullmatch(value):
        raise ReaderError("sha")
    return value


def _git(value: Any) -> str:
    if not isinstance(value, str) or not GIT.fullmatch(value):
        raise ReaderError("git")
    return value


def _event_root(event: Any) -> str:
    event = _exact(event, set(EVENT_CORE) | {"id", "signature"})
    if not isinstance(event["id"], str) or not EVENT_ID.fullmatch(event["id"]):
        raise ReaderError("event_id")
    if not isinstance(event["signature"], str) or not SIGNATURE.fullmatch(
        event["signature"]
    ):
        raise ReaderError("signature")
    root = digest({field: event[field] for field in EVENT_CORE})
    if event["id"] != f"vev_{root[7:23]}":
        raise ReaderError("event_binding")
    return root


def _event_roots(events: Any) -> list[str]:
    if not isinstance(events, list) or len(events) > 4096:
        raise ReaderError("events")
    roots = [_event_root(event) for event in events]
    if len(roots) != len(set(roots)):
        raise ReaderError("event_duplicate")
    return roots


def _event_log(events: list[dict[str, Any]]) -> str:
    stripped = []
    identifiers = set()
    for event in events:
        _event_root(event)
        if event["id"] in identifiers:
            raise ReaderError("event_id_duplicate")
        identifiers.add(event["id"])
        item = copy.deepcopy(event)
        item.pop("signature")
        stripped.append(item)
    stripped.sort(key=lambda item: item["id"])
    return digest(stripped)


def _sequence_relation(left: list[str], right: list[str]) -> str:
    if left == right:
        return "same"
    if right[: len(left)] == left:
        return "descendant"
    if left[: len(right)] == right:
        return "ancestor"
    return "forked"


def _git_relation(left: str, right: str, merge_base: str) -> str:
    if left == right:
        if merge_base != left:
            raise ReaderError("merge_base")
        return "same"
    if merge_base == left:
        return "descendant"
    if merge_base == right:
        return "ancestor"
    return "forked"


def _observation(value: Any) -> dict[str, Any]:
    fields = {
        "schema",
        "parent_frontier_id",
        "parent_git_commit",
        "parent_git_tree",
        "parent_event_log_root",
        "parent_snapshot_root",
        "finding_id",
        "finding_revision_root",
        "decision_event_id",
        "decision_event_content_root",
        "decision_signature",
        "authority_id",
        "receipt_roots",
        "verifier_attachments",
        "premise_digest",
        "role",
    }
    value = _exact(value, fields)
    if value["schema"] != OBSERVATION_SCHEMA or value["role"] not in ROLES:
        raise ReaderError("observation")
    for field in ("parent_git_commit", "parent_git_tree"):
        _git(value[field])
    if len(value["parent_git_commit"]) != len(value["parent_git_tree"]):
        raise ReaderError("git_format")
    for field in (
        "parent_event_log_root",
        "parent_snapshot_root",
        "finding_revision_root",
        "decision_event_content_root",
        "premise_digest",
    ):
        _sha(value[field])
    if (
        not isinstance(value["decision_event_id"], str)
        or not EVENT_ID.fullmatch(value["decision_event_id"])
        or value["decision_event_id"]
        != f"vev_{value['decision_event_content_root'][7:23]}"
    ):
        raise ReaderError("decision_id")
    if not isinstance(value["decision_signature"], str) or not SIGNATURE.fullmatch(
        value["decision_signature"]
    ):
        raise ReaderError("decision_signature")
    if not isinstance(value["authority_id"], str) or not AUTHORITY.fullmatch(
        value["authority_id"]
    ):
        raise ReaderError("authority")
    receipts = value["receipt_roots"]
    if (
        not isinstance(receipts, list)
        or not receipts
        or receipts != sorted(receipts)
        or len(receipts) != len(set(receipts))
    ):
        raise ReaderError("receipts")
    for root in receipts:
        _sha(root)
    attachments = value["verifier_attachments"]
    if not isinstance(attachments, list) or not attachments:
        raise ReaderError("attachments")
    expected = sorted(
        attachments,
        key=lambda item: (
            item.get("attachment_id", ""),
            item.get("attachment_content_root", ""),
        ),
    )
    if attachments != expected:
        raise ReaderError("attachment_order")
    seen = set()
    for item in attachments:
        _exact(item, {"attachment_id", "attachment_content_root"})
        if (
            not isinstance(item["attachment_id"], str)
            or not ATTACHMENT_ID.fullmatch(item["attachment_id"])
            or item["attachment_id"] in seen
        ):
            raise ReaderError("attachment")
        seen.add(item["attachment_id"])
        _sha(item["attachment_content_root"])
    return value


def _state(value: Any) -> dict[str, str]:
    value = _exact(
        value,
        {"git_commit", "git_tree", "event_log_root", "snapshot_root"},
    )
    _git(value["git_commit"])
    _git(value["git_tree"])
    if len(value["git_commit"]) != len(value["git_tree"]):
        raise ReaderError("state_git")
    _sha(value["event_log_root"])
    _sha(value["snapshot_root"])
    return value


def _inspection(
    value: Any,
    last_seen: dict[str, str],
    delivered: dict[str, str],
    dependency: dict[str, Any],
) -> tuple[str, dict[str, Any], list[str], list[str]]:
    value = _exact(value, {"schema", "inspection_root", "result"})
    if value["schema"] != INSPECTION_ENVELOPE_SCHEMA:
        raise ReaderError("inspection_schema")
    _sha(value["inspection_root"])
    result = _exact(
        value["result"],
        {
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
        },
    )
    if (
        result["schema"] != INSPECTION_RESULT_SCHEMA
        or result["verification"] != "verified"
        or value["inspection_root"] != digest(result)
    ):
        raise ReaderError("inspection_root")
    _sha(result["bundle_root"])
    for field in (
        "last_seen_git_commit",
        "last_seen_git_tree",
        "delivered_git_commit",
        "delivered_git_tree",
        "merge_base",
    ):
        _git(result[field])
    for field in (
        "last_seen_state_document_root",
        "delivered_state_document_root",
    ):
        _sha(result[field])
    if not isinstance(result["last_seen_snapshot"], dict) or not isinstance(
        result["delivered_snapshot"], dict
    ):
        raise ReaderError("snapshot")
    left_roots = _event_roots(result["last_seen_events"])
    right_roots = _event_roots(result["delivered_events"])
    derived_left = {
        "git_commit": result["last_seen_git_commit"],
        "git_tree": result["last_seen_git_tree"],
        "event_log_root": _event_log(result["last_seen_events"]),
        "snapshot_root": digest(result["last_seen_snapshot"]),
    }
    derived_right = {
        "git_commit": result["delivered_git_commit"],
        "git_tree": result["delivered_git_tree"],
        "event_log_root": _event_log(result["delivered_events"]),
        "snapshot_root": digest(result["delivered_snapshot"]),
    }
    if derived_left != last_seen or derived_right != delivered:
        raise ReaderError("inspection_state")
    git_relation = _git_relation(
        result["last_seen_git_commit"],
        result["delivered_git_commit"],
        result["merge_base"],
    )
    event_relation = _sequence_relation(left_roots, right_roots)
    if (
        result["git_relation"] != git_relation
        or result["event_relation"] != event_relation
    ):
        raise ReaderError("inspection_relation")
    compatible = {
        "same": {"same"},
        "descendant": {"same", "descendant"},
        "ancestor": {"same", "ancestor"},
        "forked": {"same", "descendant", "ancestor", "forked"},
    }
    if event_relation not in compatible[git_relation]:
        raise ReaderError("inspection_continuity")
    decisions = [
        event
        for event in result["last_seen_events"]
        if _event_root(event) == dependency["decision_event_content_root"]
    ]
    if len(decisions) != 1:
        raise ReaderError("decision_missing")
    decision = decisions[0]
    actor = decision.get("actor")
    if (
        decision["id"] != dependency["decision_event_id"]
        or decision["signature"] != dependency["decision_signature"]
        or not isinstance(actor, dict)
        or actor.get("id") != dependency["authority_id"]
        or dependency["decision_event_content_root"] not in right_roots
    ):
        raise ReaderError("decision_binding")
    return git_relation, result, left_roots, right_roots


def _finding(value: Any, dependency: dict[str, Any]) -> None:
    if not isinstance(value, dict) or value.get("id") != dependency["finding_id"]:
        raise ReaderError("finding")
    if not isinstance(value.get("links"), list):
        raise ReaderError("links")
    assertion = value.get("assertion")
    conditions = value.get("conditions")
    flags = value.get("flags")
    if (
        not isinstance(assertion, dict)
        or not isinstance(assertion.get("text"), str)
        or not assertion["text"].strip()
        or not isinstance(conditions, dict)
        or not isinstance(flags, dict)
        or not isinstance(flags.get("retracted"), bool)
    ):
        raise ReaderError("finding_shape")
    hashable = copy.deepcopy(value)
    hashable["links"] = []
    if digest(hashable) != dependency["finding_revision_root"]:
        raise ReaderError("finding_root")


def _standing(
    value: Any,
    dependency: dict[str, Any],
    inspection_root: str,
    result: dict[str, Any],
    left_roots: list[str],
    right_roots: list[str],
) -> dict[str, Any]:
    fields = {
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
    }
    value = _exact(value, fields)
    bindings = {
        "selected_finding_revision_root": "finding_revision_root",
        "decision_event_content_root": "decision_event_content_root",
        "authority_id": "authority_id",
        "receipt_roots": "receipt_roots",
        "verifier_attachments": "verifier_attachments",
        "premise_digest": "premise_digest",
    }
    for target, source in bindings.items():
        if value[target] != dependency[source]:
            raise ReaderError("standing_binding")
    if value["receipt_roots"] != sorted(value["receipt_roots"]):
        raise ReaderError("standing_receipts")
    if value["verifier_attachments"] != sorted(
        value["verifier_attachments"],
        key=lambda item: (
            item["attachment_id"],
            item["attachment_content_root"],
        ),
    ):
        raise ReaderError("standing_attachments")
    if value["finding_status"] not in {
        "accepted",
        "corrected",
        "superseded",
        "withdrawn",
    }:
        raise ReaderError("finding_status")
    if value["decision_status"] not in {"valid", "revoked", "missing", "invalid"}:
        raise ReaderError("decision_status")
    if value["verifier_status"] not in {"valid", "revoked", "missing", "invalid"}:
        raise ReaderError("verifier_status")
    if value["evidence_status"] not in {"available", "missing", "invalid"}:
        raise ReaderError("evidence_status")
    changed_dimensions = sum(
        (
            value["finding_status"] != "accepted",
            value["decision_status"] == "revoked",
            value["verifier_status"] == "revoked",
        )
    )
    if changed_dimensions > 1:
        raise ReaderError("standing_multiple_changes")
    changed = changed_dimensions == 1
    change = value["change_event"]
    if not changed:
        if change is not None:
            raise ReaderError("unexpected_change")
        return value
    change = _exact(
        change,
        {
            "event_id",
            "event_content_root",
            "event_signature",
            "authority_id",
            "effect",
            "inspection_result_root",
        },
    )
    if change["inspection_result_root"] != inspection_root:
        raise ReaderError("change_inspection")
    _sha(change["event_content_root"])
    if (
        not isinstance(change["event_id"], str)
        or change["event_id"] != f"vev_{change['event_content_root'][7:23]}"
        or not isinstance(change["event_signature"], str)
        or not SIGNATURE.fullmatch(change["event_signature"])
        or change["event_content_root"] in left_roots
    ):
        raise ReaderError("change_shape")
    matches = [
        event
        for event in result["delivered_events"]
        if _event_root(event) == change["event_content_root"]
    ]
    if len(matches) != 1:
        raise ReaderError("change_history")
    event = matches[0]
    actor = event.get("actor")
    payload = event.get("payload")
    if (
        event["id"] != change["event_id"]
        or event["signature"] != change["event_signature"]
        or not isinstance(actor, dict)
        or actor.get("id") != change["authority_id"]
        or not isinstance(payload, dict)
        or payload.get("dependency_effect") != change["effect"]
        or change["event_content_root"] not in right_roots
    ):
        raise ReaderError("change_binding")
    expected = (
        value["finding_status"]
        if value["finding_status"] != "accepted"
        else "decision_revoked"
        if value["decision_status"] == "revoked"
        else "verifier_revoked"
    )
    if change["effect"] != expected:
        raise ReaderError("change_effect")
    return value


def read_bytes(raw: bytes) -> dict[str, Any]:
    try:
        if len(raw) > MAX_BYTES:
            raise ReaderError("oversized")
        value = json.loads(
            raw.decode(),
            object_pairs_hook=_object,
            parse_constant=_constant,
        )
        _safe(value)
        envelope = _exact(value, {"schema", "fact_manifest_root", "fact_manifest"})
        if envelope["schema"] != ENVELOPE_SCHEMA:
            raise ReaderError("envelope_schema")
        _sha(envelope["fact_manifest_root"])
        manifest = _exact(
            envelope["fact_manifest"],
            {
                "schema",
                "dependency",
                "accepted_finding",
                "last_seen",
                "delivered",
                "delivery_inspection",
                "standing",
            },
        )
        if (
            manifest["schema"] != MANIFEST_SCHEMA
            or digest(manifest) != envelope["fact_manifest_root"]
        ):
            raise ReaderError("manifest_root")
        dependency = _observation(manifest["dependency"])
        _finding(manifest["accepted_finding"], dependency)
        last_seen = _state(manifest["last_seen"])
        delivered = _state(manifest["delivered"])
        for dependency_field, state_field in {
            "parent_git_commit": "git_commit",
            "parent_git_tree": "git_tree",
            "parent_event_log_root": "event_log_root",
            "parent_snapshot_root": "snapshot_root",
        }.items():
            if dependency[dependency_field] != last_seen[state_field]:
                raise ReaderError("parent_binding")
        relation, inspection, left_roots, right_roots = _inspection(
            manifest["delivery_inspection"],
            last_seen,
            delivered,
            dependency,
        )
        standing = _standing(
            manifest["standing"],
            dependency,
            manifest["delivery_inspection"]["inspection_root"],
            inspection,
            left_roots,
            right_roots,
        )
        if relation == "ancestor":
            status, code = "stale", "stale:delivered_root_precedes_last_seen"
        elif relation == "forked":
            status, code = (
                "forked",
                "forked:delivered_root_outside_selected_lineage",
            )
        elif standing["evidence_status"] != "available":
            status = "unresolvable"
            code = f"unresolvable:evidence_{standing['evidence_status']}"
        elif standing["decision_status"] in {"missing", "invalid"}:
            status = "unresolvable"
            code = f"unresolvable:decision_{standing['decision_status']}"
        elif standing["verifier_status"] in {"missing", "invalid"}:
            status = "unresolvable"
            code = f"unresolvable:verifier_{standing['verifier_status']}"
        elif standing["finding_status"] in {"corrected", "superseded"}:
            if dependency["role"] in SOFT_ROLES:
                status = "warning"
                code = f"warning:finding_{standing['finding_status']}"
            else:
                status = "review_required"
                code = f"review_required:finding_{standing['finding_status']}"
        elif (
            standing["finding_status"] == "withdrawn"
            or standing["decision_status"] == "revoked"
            or standing["verifier_status"] == "revoked"
        ):
            if dependency["role"] in HARD_ROLES:
                status = "blocked"
                code = (
                    "blocked:finding_withdrawn"
                    if standing["finding_status"] == "withdrawn"
                    else "blocked:decision_revoked"
                    if standing["decision_status"] == "revoked"
                    else "blocked:verifier_revoked"
                )
            else:
                status = "review_required"
                code = (
                    "review_required:finding_withdrawn"
                    if standing["finding_status"] == "withdrawn"
                    else "review_required:decision_revoked"
                    if standing["decision_status"] == "revoked"
                    else "review_required:verifier_revoked"
                )
        else:
            status, code = (
                "satisfied",
                "satisfied:exact_dependency_retains_standing",
            )
        return {
            "dependency_status": status,
            "code": code,
            "fact_manifest_root": envelope["fact_manifest_root"],
            "dependency_observation_root": digest(dependency),
            "child_truth": "not_assessed",
            "authority_effect": "none",
        }
    except (
        ReaderError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        RecursionError,
        TypeError,
        ValueError,
    ):
        return {
            "dependency_status": "unresolvable",
            "code": "unresolvable:reader_c_invalid",
            "fact_manifest_root": None,
            "dependency_observation_root": None,
            "child_truth": "not_assessed",
            "authority_effect": "none",
        }


def main() -> int:
    parser = argparse.ArgumentParser(description="Independent ADR 0004 Reader C")
    parser.add_argument("--manifest", required=True, type=Path)
    arguments = parser.parse_args()
    try:
        metadata = arguments.manifest.lstat()
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > MAX_BYTES:
            raise OSError
        raw = arguments.manifest.read_bytes()
        if len(raw) != metadata.st_size:
            raise OSError
    except OSError:
        raw = b""
    result = read_bytes(raw)
    print(json.dumps(result, separators=(",", ":"), sort_keys=True))
    return 1 if result["dependency_status"] == "unresolvable" else 0


if __name__ == "__main__":
    sys.exit(main())
