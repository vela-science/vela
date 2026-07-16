#!/usr/bin/env python3
"""Validate the complete first-party handoff rehearsal result and artifacts."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
RESULT_ROOT = ROOT / "results/first-party-handoff-rehearsal-2026-07-16"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def root(raw: bytes) -> str:
    return f"sha256:{hashlib.sha256(raw).hexdigest()}"


def main() -> None:
    value: dict[str, Any] = json.loads((RESULT_ROOT / "result.json").read_text())
    require(value["schema"] == "vela.first-party-handoff-rehearsal-result.v1", "schema")
    require(value["run_class"] == "first_party_internal_fixture", "run class")
    require(value["result"] == "pass", "result")
    require(value["producer_a"]["v1_v2_parity"] is True, "parent parity")
    require(value["producer_a"]["lrat_verified"] is True, "parent LRAT")
    require(value["pending_handoff"]["authority_status"] == "pending_review", "pending status")
    require(value["pending_handoff"]["vela_route"] == "deferred", "Vela route")
    require(value["pending_handoff"]["accepted_event_delta"] == 0, "accepted event delta")
    require(value["pending_handoff"]["receipt_root"].startswith("sha256:"), "receipt root")
    require(value["pending_handoff"]["proposal_id"].startswith("vpr_"), "proposal ID")
    require(value["pending_handoff"]["hard_dependency_usable"] is False, "pending dependency")
    require(value["child"]["vertices"] == 23, "child size")
    require(value["child"]["chromatic_number"] == 5, "child chromatic number")
    require(value["child"]["v1_v2_parity"] is True, "child parity")
    require(value["child"]["lrat_verified"] is True, "child LRAT")
    replay = value["correction_replay"]
    require(replay["vectors"] == replay["reader_c_parity"] == 54, "Reader C parity")
    require(
        set(replay["status_distribution"])
        == {"satisfied", "warning", "review_required", "blocked", "stale", "forked", "unresolvable"},
        "standing vocabulary",
    )
    require(replay["child_truth"] == "not_assessed", "child truth inference")
    require(value["standards_baseline"] == {"vectors": 13, "passed": 13}, "standards vectors")
    require(
        value["authority"]
        == {
            "human_key_access": False,
            "authority_attempts": 0,
            "accepted_state_claim": False,
            "historical_event_rewrites": 0,
        },
        "authority boundary",
    )
    require(not any(value["credit"].values()), "credit widened")
    require(set(value["gap_verdicts"].values()) == {"not_reproduced"}, "unsupported gap")
    paths = set()
    for artifact in value["artifacts"]:
        path = RESULT_ROOT / artifact["path"]
        require(path.is_file() and not path.is_symlink(), f"artifact missing: {path}")
        raw = path.read_bytes()
        require(artifact["sha256"] == root(raw) and artifact["bytes"] == len(raw), f"artifact drift: {path}")
        require(artifact["path"] not in paths, "duplicate artifact")
        paths.add(artifact["path"])
    require(value["measurements"]["repair_count"] == 0, "repairs")
    require(value["measurements"]["maintainer_semantic_interventions"] == 0, "interventions")
    require(value["measurements"]["network_requests"] == 0, "network")
    print(
        "first-party handoff: parent and child LRAT verified; V1/V2 parity; "
        "54/54 Reader C vectors; 13/13 standards vectors; zero authority"
    )


if __name__ == "__main__":
    main()
