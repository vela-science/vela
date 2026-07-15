#!/usr/bin/env python3
"""Prove the experimental observation is lossless and Receipt-body bound."""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent
RESOURCES = ROOT.parents[1] / "crates/vela-cli/resources"
sys.path.insert(0, str(RESOURCES))

from reference.dependency_observation import (  # noqa: E402
    observation_root,
    validate_observation,
)
from vela_receipt_v1 import (  # noqa: E402
    artifact,
    attach_statement,
    attestation_binding,
    canonical_json,
    distillation_block,
    make_receipt,
    receipt_body_sha256,
    strict_json_load_bytes,
    validate_receipt,
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def build_receipt(observation: dict[str, object]) -> dict[str, object]:
    graph_path = ROOT / "registration/graph-case.json"
    receipt = make_receipt(
        claim_id="producer:adr-0004-phase-1",
        claim="The experimental child records one structurally valid dependency observation for the registered case.",
        claim_type="computational",
        replayability="exact",
        artifacts=[artifact(graph_path, "input", ROOT)],
        verifier_runs=[
            {
                "method": "ADR 0004 experiment-only observation check",
                "outcome": "pass",
                "log": "local fixture; not scientific acceptance",
            }
        ],
        caveats=[
            "The observation uses placeholder object roots and proves only Receipt byte binding, not root resolution, authority, or outside composition."
        ],
        generated_by="vela-adr-0004-internal-fixture",
        submitter="agent:adr-0004-fixture",
        acceptance_scope="hypothesis_only",
        acceptance_status="not_assessed",
        acceptance_authority="producer",
        acceptance_profile="producer.emission.v1",
        policy_ref="urn:vela:policy:none",
        evidence_level=None,
        distillation=distillation_block(
            status="missing",
            audience="experiment reviewer",
            level="not_assessed",
            rubric="not_assessed",
        ),
        lineage={
            "frontier": None,
            "parents": [],
            "derived_from": [],
            "supersedes": [],
            "source_refs": [],
            "producer_run_id": "adr-0004-phase-1",
        },
        environment={"vela:experimental_dependencies": [observation]},
    )
    receipt["provenance"]["emitted_at"] = "2026-07-15T00:00:00Z"
    attach_statement(receipt)
    return receipt


def main() -> None:
    vectors = json.loads((ROOT / "vectors/observation-cases.json").read_text())
    observation = vectors["base"]
    validate_observation(observation)
    receipt = build_receipt(observation)
    errors = validate_receipt(receipt)
    require(not errors, f"Receipt validation failed: {errors}")
    require(attestation_binding(receipt) == "bound", "Receipt attestation is unbound")

    body_root = receipt_body_sha256(receipt)
    asserted = receipt["attestation"]["statement"]["predicate"]["vela:receipt_body"]["sha256"]
    require(asserted == body_root, "attestation body root does not match Receipt body")
    round_trip = strict_json_load_bytes(canonical_json(receipt))
    require(
        round_trip["environment"]["vela:experimental_dependencies"] == [observation],
        "dependency observation changed during canonical Receipt round-trip",
    )

    changed = copy.deepcopy(receipt)
    changed_observation = changed["environment"]["vela:experimental_dependencies"][0]
    changed_observation["premise_digest"] = (
        "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
    )
    try:
        attestation_binding(changed)
    except ValueError as error:
        require(
            "does not match receipt body" in str(error),
            f"unexpected stale-binding error: {error}",
        )
    else:
        raise AssertionError("mutated dependency observation retained a stale Receipt binding")

    attach_statement(changed)
    changed_root = receipt_body_sha256(changed)
    require(changed_root != body_root, "observation mutation did not change Receipt body root")
    require(not validate_receipt(changed), "rebound mutated Receipt is invalid")
    print(
        "receipt binding: placeholder observation preserved; mutation refused under stale binding; "
        f"body {body_root[:16]}.. -> {changed_root[:16]}..; "
        f"observation {observation_root(observation)[7:23]}.."
    )


if __name__ == "__main__":
    main()
