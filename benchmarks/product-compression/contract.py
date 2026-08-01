#!/usr/bin/env python3
"""Small, Vela-specific contract helpers for the Harbor benchmark."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
from typing import Any


ROOT = re.compile(r"^sha256:[0-9a-f]{64}$")


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
        if value["schema"] != "vela.product-compression-answer.v6":
            raise ContractError("$.schema: wrong answer schema")
        frontier = value["frontier"]
        work = value["next_work"]
        decision = value["decision"]
        delta = decision["standing_delta"]
        scope = delta["scope"]
    except (KeyError, TypeError) as exc:
        raise ContractError(f"answer is missing a required field: {exc}") from exc

    require_root(frontier.get("repository_root"), "$.frontier.repository_root")
    require_root(work.get("target_index_root"), "$.next_work.target_index_root")
    require_root(work.get("packet_sha256"), "$.next_work.packet_sha256")
    for field in ("proposal_root", "verification_set_root", "inbox_entry_root"):
        require_root(decision.get(field), f"$.decision.{field}")

    required_boundary = (
        decision.get("human_decision_required"),
        decision.get("verification_is_acceptance"),
        decision.get("next_if_accept_code"),
        decision.get("next_if_reject_code"),
    )
    if required_boundary != (
        True,
        False,
        "replay_and_recompute_targets",
        "replay_without_standing_change",
    ):
        raise ContractError("$.decision: authority boundary is misstated")
    if decision.get("protocol_gate") not in {"satisfied", "blocked"}:
        raise ContractError("$.decision.protocol_gate: invalid state")
    if decision.get("staleness") not in {"current", "stale"}:
        raise ContractError("$.decision.staleness: invalid state")

    claim_id = decision.get("proposed_claim_id")
    if delta.get("transition") != "add accepted Claim":
        raise ContractError("$.decision.standing_delta.transition: unsupported transition")
    if scope != {
        "kind": "proposal_affected_claims",
        "target_claim_id": claim_id,
        "affected_claim_ids": [claim_id],
    }:
        raise ContractError("$.decision.standing_delta.scope: must bind only the proposed Claim")
    before = delta.get("before", {}).get("accepted")
    accepted = delta.get("if_accept", {}).get("accepted")
    rejected = delta.get("if_reject", {}).get("accepted")
    if not all(isinstance(items, list) for items in (before, accepted, rejected)):
        raise ContractError("$.decision.standing_delta: accepted sets must be arrays")
    if rejected != before:
        raise ContractError("$.decision.standing_delta.if_reject: rejection must preserve scoped Standing")
    additions = [item for item in accepted if item not in before]
    if len(accepted) != len(before) + 1 or len(additions) != 1 or additions[0].get("claim_id") != claim_id:
        raise ContractError("$.decision.standing_delta.if_accept: must add exactly the proposed Claim")
    if delta.get("before", {}).get("repository_root") != frontier.get("repository_root"):
        raise ContractError("$.decision.standing_delta.before: does not bind the inspected repository")


def validate_answer_key(value: Any) -> None:
    if not isinstance(value, dict) or value.get("schema") != "vela.product-compression-answer-key.v6":
        raise ContractError("answer key has the wrong schema")
    require_root(value.get("fixture_root"), "$.fixture_root")
    require_root(value.get("answer_key_root"), "$.answer_key_root")
    validate_answer(value.get("expected"))
    if value["answer_key_root"] != record_root(value, "answer_key_root"):
        raise ContractError("$.answer_key_root: root mismatch")
