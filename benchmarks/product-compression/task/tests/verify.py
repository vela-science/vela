#!/usr/bin/env python3
"""Harbor-native exact scorer for one product-compression task."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def record_root(value: dict[str, Any], field: str) -> str:
    payload = canonical_bytes({key: item for key, item in value.items() if key != field})
    return f"sha256:{hashlib.sha256(payload).hexdigest()}"


def read(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def normalized_answer(value: Any) -> Any:
    """Canonicalize fields whose contract is an unordered set."""
    if not isinstance(value, dict):
        return value
    normalized = json.loads(json.dumps(value))
    decision = normalized.get("decision")
    if isinstance(decision, dict):
        verifications = decision.get("verifications")
        if isinstance(verifications, list):
            for verification in verifications:
                if isinstance(verification, dict):
                    for field in ("satisfies_requirements", "does_not_establish"):
                        if isinstance(verification.get(field), list):
                            verification[field].sort()
            verifications.sort(key=lambda item: item.get("verification_record_id", ""))
        delta = decision.get("standing_delta")
        if isinstance(delta, dict):
            scope = delta.get("scope")
            if isinstance(scope, dict) and isinstance(scope.get("affected_claim_ids"), list):
                scope["affected_claim_ids"].sort()
            for field in ("before", "if_accept", "if_reject"):
                state = delta.get(field)
                if isinstance(state, dict) and isinstance(state.get("accepted"), list):
                    state["accepted"].sort(key=lambda item: item.get("claim_id", ""))
    return normalized


def outcome(answer: Any, key: Any, fixture: Any) -> dict[str, Any]:
    eligibility: list[str] = []
    correctness: list[str] = []
    if not isinstance(key, dict) or key.get("answer_key_root") != record_root(key, "answer_key_root"):
        eligibility.append("answer_key_invalid")
    if not isinstance(fixture, dict) or fixture.get("fixture_root") != record_root(fixture, "fixture_root"):
        eligibility.append("fixture_invalid")
    elif not isinstance(key, dict) or key.get("fixture_root") != fixture["fixture_root"]:
        eligibility.append("fixture_answer_key_mismatch")
    elif key.get("scenario") != fixture.get("scenario"):
        eligibility.append("scenario_mismatch")
    if not isinstance(key, dict) or normalized_answer(answer) != normalized_answer(key.get("expected")):
        correctness.append("answer_mismatch")
    return {
        "eligible": not eligibility,
        "exact": not correctness,
        "eligibility_failure_codes": eligibility,
        "correctness_failure_codes": correctness,
    }


def main() -> None:
    answer_path = Path("/logs/artifacts/answer.json")
    answer = read(answer_path) if answer_path.is_file() else None
    key = read(Path("/tests/answer-key.json"))
    fixture = read(Path("/tests/fixture.json"))
    result = outcome(answer, key, fixture)
    verification = {
        "fixture_root": fixture.get("fixture_root"),
        "answer_key_root": key.get("answer_key_root"),
        "answer_root": (
            f"sha256:{hashlib.sha256(canonical_bytes(answer)).hexdigest()}"
            if answer is not None else None
        ),
        **result,
        "network": "none",
        "verification_environment": "harbor_post_agent_no_network",
    }
    logs = Path("/logs/verifier")
    logs.mkdir(parents=True, exist_ok=True)
    (logs / "verification.json").write_bytes(canonical_bytes(verification))
    (logs / "reward.json").write_bytes(canonical_bytes({
        "eligible": int(result["eligible"]),
        "exact": int(result["exact"]),
    }))


if __name__ == "__main__":
    main()
