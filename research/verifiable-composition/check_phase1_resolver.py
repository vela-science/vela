#!/usr/bin/env python3
"""Focused offline checks over a shape-compatible synthetic aggregate."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / "reference"))
SEMANTIC_ROOT_FIELDS = {
    "decision_event_content_root",
    "finding_revision_root",
    "parent_event_log_root",
    "parent_git_tree",
    "parent_snapshot_root",
    "premise_digest",
}
SEMANTIC_ROOT_OPERATIONS = {
    "attachment_root",
    "observation_commit",
    "receipt_root",
}

from exact_checkout import (  # noqa: E402
    CompositionError,
    SELECTION_SCHEMA,
    canonical_receipt_json_bytes,
    derived_attachment_id,
    derived_event_id,
    encode_observation,
    event_log_root,
    finding_revision_root,
    resolve_observation,
    sha256_bytes,
    sha256_json,
    snapshot_candidate_root,
)
from vela_receipt_v1 import (  # noqa: E402
    attach_statement,
    distillation_block,
    make_receipt,
    validate_receipt,
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def run(command: list[str], *, cwd: Path | None = None) -> str:
    result = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        raise RuntimeError(f"command failed: {command!r}: {result.stderr.strip()}")
    return result.stdout.strip()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def make_parent_receipt(
    claim: str, premise_path: str, premise: bytes
) -> dict[str, Any]:
    receipt = make_receipt(
        claim_id="producer:adr-0004-phase-1-parent",
        claim=claim,
        claim_type="computational",
        replayability="exact",
        artifacts=[
            {
                "path": premise_path,
                "kind": "input",
                "sha256": hashlib.sha256(premise).hexdigest(),
            }
        ],
        verifier_runs=[
            {
                "method": "fixture exact arithmetic",
                "outcome": "pass",
                "log": "internal fixture only",
            }
        ],
        caveats=[
            "Internal fixture; no authority or scientific acceptance is inferred."
        ],
        generated_by="vela-adr-0004-resolver-fixture",
        submitter="agent:adr-0004-resolver-fixture",
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
            "producer_run_id": "adr-0004-resolver-fixture",
        },
        environment={"vela:canonicalization_probe": 1e-7},
    )
    receipt["provenance"]["emitted_at"] = "2026-07-15T00:00:00Z"
    attach_statement(receipt)
    errors = validate_receipt(receipt)
    require(not errors, f"fixture receipt invalid: {errors}")
    return receipt


def build_fixture(repo: Path) -> tuple[str, str, bytes, dict[str, Any]]:
    premise_path = "inputs/exact-premise.json"
    premise = b'{"case":"phase1","value":17}\n'
    claim = "The exact registered premise evaluates to seventeen."
    receipt = make_parent_receipt(claim, premise_path, premise)
    receipt_root = sha256_bytes(canonical_receipt_json_bytes(receipt))
    require(
        receipt_root != sha256_json(receipt),
        "fixture must distinguish Receipt JCS from Vela canonical JSON",
    )
    receipt_path = f"records/receipts/sha256/{receipt_root[7:]}.json"

    finding_id = "vf_1111111111111111"
    finding = {
        "id": finding_id,
        "version": 1,
        "previous_version": None,
        "assertion": {"text": claim, "type": "theoretical"},
        "evidence": {"type": "computational"},
        "conditions": {"text": "internal fixture"},
        "confidence": {"score": 1.0},
        "provenance": {"source_type": "internal_fixture"},
        "flags": {"retracted": False},
        "links": [],
        "created": "2026-07-15T00:00:00Z",
        "updated": None,
    }
    finding_root = finding_revision_root(finding)
    claim_digest = hashlib.sha256(claim.strip().encode()).hexdigest()[:16]
    attachment = {
        "schema": "vela.verifier_attachment.v0.1",
        "id": "",
        "target": finding_id,
        "claim_digest": claim_digest,
        "verifier_method": "exact_arithmetic_recompute",
        "solver_id": "python-integer-fixture",
        "independent_of": [],
        "match_to_claim": {"matches": True, "checker_actor": "checker:fixture"},
        "adversarial_probes": [{"kind": "formalism_fidelity", "result": "survived"}],
        "outcome": "passed",
        "method_integrity": "sound",
        "verifier_actor": "verifier:fixture",
        "note": "Shape-valid internal fixture; one attachment does not clear G1.",
        "implementation_id": "fixture-v1",
        "toolchain_hash": "fixture-toolchain",
    }
    attachment["id"] = derived_attachment_id(attachment)

    proposal_id = "vpr_2222222222222222"
    domain_event = {
        "schema": "vela.event.v0.1",
        "id": "",
        "kind": "finding.asserted",
        "target": {"type": "finding", "id": finding_id},
        "actor": {"type": "human", "id": "reviewer:fixture"},
        "timestamp": "2026-07-15T01:00:00Z",
        "reason": "Accept the exact internal fixture finding",
        "before_hash": "sha256:null",
        "after_hash": finding_root,
        "payload": {"proposal_id": proposal_id},
        "caveats": ["Internal fixture only"],
    }
    domain_event["id"] = derived_event_id(domain_event)
    proposal = {
        "schema": "vela.proposal.v0.1",
        "id": proposal_id,
        "kind": "finding.add",
        "target": {"type": "finding", "id": finding_id},
        "actor": {"type": "agent", "id": "agent:fixture"},
        "created_at": "2026-07-15T00:30:00Z",
        "reason": "Fixture proposal",
        "payload": {
            "finding": finding,
            "vela_submission": {
                "schema": "vela.submission-links.internal.v1",
                "receipt_root": receipt_root,
                "receipt_path": receipt_path,
                "record_id": "vrc_3333333333333333",
                "operation_id": "vop_" + "4" * 64,
            },
        },
        "source_refs": [],
        "status": "applied",
        "reviewed_by": "reviewer:fixture",
        "reviewed_at": "2026-07-15T01:00:01Z",
        "decision_reason": "Fixture scope checked",
        "applied_event_id": domain_event["id"],
        "caveats": ["Internal fixture only"],
    }
    decision_event = {
        "schema": "vela.event.v0.1",
        "id": "",
        "kind": "review.accepted",
        "target": {"type": "proposal", "id": proposal_id},
        "actor": {"type": "human", "id": "reviewer:fixture"},
        "timestamp": "2026-07-15T01:00:01Z",
        "reason": "Fixture scope checked",
        "before_hash": "sha256:null",
        "after_hash": "sha256:null",
        "payload": {
            "proposal_id": proposal_id,
            "proposal_kind": "finding.add",
            "verdict": "accepted",
            "applied_event_id": domain_event["id"],
        },
        "caveats": [],
        "signature": "55" * 64,
    }
    decision_event["id"] = derived_event_id(decision_event)
    events = [decision_event, domain_event]
    frontier = {
        "_warning": "Shape-compatible synthetic aggregate; not canonical state or accepted science.",
        "frontier_id": "vfr_6666666666666666",
        "frontier": {"name": "ADR 0004 internal fixture"},
        "findings": [finding],
        "proposals": [proposal],
        "events": events,
        "verifier_attachments": [attachment],
        "proof_state": {},
    }
    frontier["_meta"] = {
        "schema": "vela.frontier_state_meta.v0.1",
        "event_log_hash": event_log_root(events),
        "snapshot_hash": snapshot_candidate_root(frontier),
        "vela_reducer": "vela@0.800.12",
    }

    (repo / premise_path).parent.mkdir(parents=True)
    (repo / premise_path).write_bytes(premise)
    (repo / "inputs/symlink-premise.json").symlink_to("exact-premise.json")
    write_json(repo / receipt_path, receipt)
    write_json(repo / "frontier.json", frontier)
    selection = {
        "schema": SELECTION_SCHEMA,
        "frontier_path": ".",
        "finding_id": finding_id,
        "decision_event_id": decision_event["id"],
        "verifier_attachment_ids": [attachment["id"]],
        "premise_path": premise_path,
        "role": "hard",
    }
    selection_raw = json.dumps(
        selection, separators=(",", ":"), sort_keys=True
    ).encode()
    run(["git", "init", "-q", "-b", "main"], cwd=repo)
    run(["git", "add", "."], cwd=repo)
    run(
        [
            "git",
            "-c",
            "user.name=ADR4 Fixture",
            "-c",
            "user.email=adr4@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "exact fixture",
        ],
        cwd=repo,
    )
    commit = run(["git", "rev-parse", "HEAD"], cwd=repo)
    (repo / "wrong-commit-marker.txt").write_text(
        "This produces a second full commit with a different root tree.\n"
    )
    run(["git", "add", "wrong-commit-marker.txt"], cwd=repo)
    run(
        [
            "git",
            "-c",
            "user.name=ADR4 Fixture",
            "-c",
            "user.email=adr4@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "wrong full commit fixture",
        ],
        cwd=repo,
    )
    wrong_commit = run(["git", "rev-parse", "HEAD"], cwd=repo)
    require(
        wrong_commit != commit and len(wrong_commit) == len(commit),
        "wrong commit fixture must be a distinct full object ID",
    )
    return commit, wrong_commit, selection_raw, selection


def wrong_hex(value: str) -> str:
    prefix = "sha256:" if value.startswith("sha256:") else ""
    body = value[len(prefix) :]
    replacement = "0" if body[0] != "0" else "1"
    return prefix + replacement + body[1:]


def rust_gate_parse_if_available(repo: Path) -> str:
    vela = ROOT.parents[1] / "target/debug/vela"
    if not vela.is_file():
        return "not_available"
    frontier = json.loads((repo / "frontier.json").read_text())
    attachments = repo / "attachments-for-rust-gate.json"
    write_json(attachments, frontier["verifier_attachments"])
    result = subprocess.run(
        [
            str(vela),
            "gate",
            "check",
            "--claim",
            frontier["findings"][0]["assertion"]["text"],
            "--attachments",
            str(attachments),
            "--json",
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    require(result.returncode == 1, "one-attachment Rust gate should fail closed")
    payload = json.loads(result.stdout)
    require(
        payload.get("status") == "needs_verification",
        f"unexpected Rust gate status: {payload}",
    )
    require(
        any(str(reason).startswith("G1:") for reason in payload.get("reasons", [])),
        f"Rust gate did not report the expected independence gap: {payload}",
    )
    attachments.unlink()
    return "needs_verification"


def mutate(observation: dict[str, Any], case: dict[str, Any]) -> dict[str, Any]:
    value = copy.deepcopy(observation)
    operation = case["operation"]
    if operation == "observation_root":
        value[case["field"]] = wrong_hex(value[case["field"]])
    elif operation == "observation_identifier":
        value[case["field"]] = "vfr_7777777777777777"
    elif operation == "observation_signature":
        value[case["field"]] = "88" * 64
    elif operation == "observation_authority":
        value[case["field"]] = "reviewer:other-fixture"
    elif operation == "receipt_root":
        value["receipt_roots"][0] = wrong_hex(value["receipt_roots"][0])
    elif operation == "attachment_root":
        attachment = value["verifier_attachments"][0]
        attachment["attachment_content_root"] = wrong_hex(
            attachment["attachment_content_root"]
        )
    else:
        raise RuntimeError(f"unknown mutation {operation}")
    return value


def main() -> None:
    vectors = json.loads((ROOT / "vectors/phase1-resolver-cases.json").read_text())
    cases = vectors["cases"]
    require(
        SEMANTIC_ROOT_FIELDS
        <= {
            case["field"]
            for case in cases
            if case.get("operation") == "observation_root"
        },
        "resolver vectors do not cover every scalar full-root mismatch",
    )
    require(
        SEMANTIC_ROOT_OPERATIONS <= {case.get("operation") for case in cases},
        "resolver vectors do not cover commit, Receipt, and attachment roots",
    )
    with tempfile.TemporaryDirectory(prefix="vela-adr4-phase1-") as directory:
        repo = Path(directory)
        commit, wrong_commit, selection_raw, selection = build_fixture(repo)
        rust_gate_status = rust_gate_parse_if_available(repo)
        observation = encode_observation(repo, commit, selection_raw)

        # Exact checkout, not the mutable worktree, supplies both files.
        (repo / selection["premise_path"]).write_text("mutated worktree\n")
        (repo / "frontier.json").write_text("{}\n")
        second = encode_observation(repo, commit, selection_raw)
        require(
            second == observation, "worktree mutation changed exact-commit encoding"
        )

        baseline = resolve_observation(
            repo,
            observation,
            frontier_path=".",
            premise_path=selection["premise_path"],
        )
        require(baseline["ok"] is False, "resolver must not emit an authority verdict")
        require(baseline["status"] == "unresolvable", "baseline must fail closed")
        require(
            baseline["code"] == "unresolvable:authority_snapshot_porcelain_missing",
            f"unexpected baseline code {baseline['code']}",
        )
        require(
            any(
                check.get("detail", "").startswith("derived_view_not_canonical_state")
                for check in baseline["checks"]
            ),
            "baseline omitted the derived-view/canonical-state blocker",
        )

        passed = 0
        for case in cases:
            if case["operation"] == "commit_input":
                try:
                    encode_observation(repo, case["value"], selection_raw)
                except CompositionError as error:
                    code = error.code
                else:
                    raise RuntimeError(f"{case['id']} unexpectedly passed")
            elif case["operation"] == "selection_premise":
                changed_selection = copy.deepcopy(selection)
                changed_selection["premise_path"] = case["value"]
                changed_raw = json.dumps(
                    changed_selection, separators=(",", ":"), sort_keys=True
                ).encode()
                try:
                    encode_observation(repo, commit, changed_raw)
                except CompositionError as error:
                    code = error.code
                else:
                    raise RuntimeError(f"{case['id']} unexpectedly passed")
            elif case["operation"] == "observation_commit":
                changed_observation = copy.deepcopy(observation)
                changed_observation["parent_git_commit"] = wrong_commit
                result = resolve_observation(
                    repo,
                    changed_observation,
                    frontier_path=".",
                    premise_path=selection["premise_path"],
                )
                code = result["code"]
            else:
                result = resolve_observation(
                    repo,
                    mutate(observation, case),
                    frontier_path=".",
                    premise_path=selection["premise_path"],
                )
                code = result["code"]
            require(
                code == case["expected_code"],
                f"{case['id']}: got {code}, expected {case['expected_code']}",
            )
            passed += 1
    print(
        f"phase1 exact-checkout resolver: {passed}/{passed} hostile vectors pass; "
        f"shape-compatible synthetic aggregate; worktree ignored; Receipt JCS pinned; "
        f"Rust attachment gate={rust_gate_status}; authority remains unresolved"
    )


if __name__ == "__main__":
    main()
