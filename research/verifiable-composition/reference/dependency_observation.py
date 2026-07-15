"""Strict experiment-only parser for ADR 0004 dependency observations.

This module interprets producer provenance. It does not authorize, accept,
sign, or mutate Vela state.
"""

from __future__ import annotations

import hashlib
import json
import re
from typing import Any


MAX_DOCUMENT_BYTES = 1024 * 1024
MAX_LIST_ITEMS = 256
MAX_AUTHORITY_BYTES = 256
SCHEMA = "vela.experimental-dependency-observation.v0"
ROLES = {"hard", "soft", "data", "method", "contextual"}
REQUIRED = (
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
)
SHA256 = re.compile(r"^sha256:[0-9a-f]{64}$")
GIT_OID = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
FRONTIER_ID = re.compile(r"^vfr_[0-9a-f]{16}$")
FINDING_ID = re.compile(r"^vf_[0-9a-f]{16}$")
EVENT_ID = re.compile(r"^vev_[0-9a-f]{16}$")
ATTACHMENT_ID = re.compile(r"^vva_[0-9a-f]{16}$")
SIGNATURE = re.compile(r"^(?:v1:)?[0-9a-f]{128}$")
AUTHORITY = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:@/+~-]*$")


class ObservationError(ValueError):
    """One stable fail-closed validation result."""


def _object_without_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ObservationError("duplicate:object_name")
        result[key] = value
    return result


def parse_observation(raw: bytes) -> dict[str, Any]:
    if len(raw) > MAX_DOCUMENT_BYTES:
        raise ObservationError("oversized:document")
    try:
        decoded = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ObservationError("invalid:utf8") from error
    try:
        value = json.loads(decoded, object_pairs_hook=_object_without_duplicates)
    except ObservationError:
        raise
    except (json.JSONDecodeError, RecursionError) as error:
        raise ObservationError("invalid:json") from error
    validate_observation(value)
    return value


def validate_observation(value: Any) -> None:
    if not isinstance(value, dict):
        raise ObservationError("invalid:document")
    for field in REQUIRED:
        if field not in value:
            raise ObservationError(f"missing:{field}")
    unexpected = sorted(set(value) - set(REQUIRED))
    if unexpected:
        raise ObservationError(f"unexpected:{unexpected[0]}")

    exact(value, "schema", SCHEMA)
    pattern(value, "parent_frontier_id", FRONTIER_ID)
    pattern(value, "parent_git_commit", GIT_OID)
    pattern(value, "parent_git_tree", GIT_OID)
    if len(value["parent_git_commit"]) != len(value["parent_git_tree"]):
        raise ObservationError("invalid:git_object_format")
    for field in (
        "parent_event_log_root",
        "parent_snapshot_root",
        "finding_revision_root",
        "decision_event_content_root",
        "premise_digest",
    ):
        pattern(value, field, SHA256)
    pattern(value, "finding_id", FINDING_ID)
    pattern(value, "decision_event_id", EVENT_ID)
    pattern(value, "decision_signature", SIGNATURE)
    pattern(value, "authority_id", AUTHORITY, max_bytes=MAX_AUTHORITY_BYTES)
    if value["role"] not in ROLES:
        raise ObservationError("invalid:role")

    receipt_roots = bounded_list(value, "receipt_roots")
    if not receipt_roots:
        raise ObservationError("invalid:receipt_roots")
    for root in receipt_roots:
        if not isinstance(root, str) or not SHA256.fullmatch(root):
            raise ObservationError("invalid:receipt_roots")
    if len(set(receipt_roots)) != len(receipt_roots):
        raise ObservationError("duplicate:receipt_roots")

    attachments = bounded_list(value, "verifier_attachments")
    if not attachments:
        raise ObservationError("invalid:verifier_attachments")
    seen_attachments: set[str] = set()
    for attachment in attachments:
        if not isinstance(attachment, dict):
            raise ObservationError("invalid:verifier_attachments")
        if set(attachment) != {"attachment_id", "attachment_content_root"}:
            raise ObservationError("invalid:verifier_attachments")
        identifier = attachment["attachment_id"]
        root = attachment["attachment_content_root"]
        if not isinstance(identifier, str) or not ATTACHMENT_ID.fullmatch(identifier):
            raise ObservationError("invalid:verifier_attachments")
        if not isinstance(root, str) or not SHA256.fullmatch(root):
            raise ObservationError("invalid:verifier_attachments")
        if identifier in seen_attachments:
            raise ObservationError("duplicate:verifier_attachments")
        seen_attachments.add(identifier)


def exact(value: dict[str, Any], field: str, expected: str) -> None:
    if value[field] != expected:
        raise ObservationError(f"invalid:{field}")


def pattern(
    value: dict[str, Any],
    field: str,
    expected: re.Pattern[str],
    *,
    max_bytes: int | None = None,
) -> None:
    item = value[field]
    if not isinstance(item, str):
        raise ObservationError(f"invalid:{field}")
    if max_bytes is not None and len(item.encode()) > max_bytes:
        raise ObservationError(f"oversized:{field}")
    if not expected.fullmatch(item):
        raise ObservationError(f"invalid:{field}")


def bounded_list(value: dict[str, Any], field: str) -> list[Any]:
    items = value[field]
    if not isinstance(items, list):
        raise ObservationError(f"invalid:{field}")
    if len(items) > MAX_LIST_ITEMS:
        raise ObservationError(f"oversized:{field}")
    return items


def canonical_bytes(value: dict[str, Any]) -> bytes:
    validate_observation(value)
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode()


def observation_root(value: dict[str, Any]) -> str:
    return f"sha256:{hashlib.sha256(canonical_bytes(value)).hexdigest()}"
