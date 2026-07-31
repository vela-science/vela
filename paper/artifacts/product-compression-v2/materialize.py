#!/usr/bin/env python3
"""Materialize exact product-compression v2 study inputs without authority."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Sequence

import harness


def fail(message: str) -> None:
    raise harness.ContractError(message)


def reject_system_temporary_output(output: Path) -> None:
    resolved = output.resolve()
    candidates = {
        Path("/tmp"), Path("/private/tmp"), Path("/var/tmp"),
        Path("/private/var/tmp"), Path(tempfile.gettempdir()),
    }
    roots = {path.resolve() for path in candidates if path.exists()}
    if any(resolved == root or resolved.is_relative_to(root) for root in roots):
        fail("study materials must not be written under a system temporary root")


def digest(path: Path) -> str:
    try:
        return f"sha256:{hashlib.sha256(path.read_bytes()).hexdigest()}"
    except OSError as exc:
        raise harness.ContractError(f"cannot hash {path}: {exc}") from exc


def command(argv: Sequence[str], *, cwd: Path) -> str:
    try:
        result = subprocess.run(
            argv,
            cwd=cwd,
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as exc:
        raise harness.ContractError(f"cannot execute {argv[0]}: {exc}") from exc
    if result.returncode != 0:
        fail(f"command failed ({result.returncode}): {' '.join(argv)}: {result.stderr.strip()}")
    return result.stdout.strip()


def json_command(argv: Sequence[str], *, cwd: Path) -> dict[str, Any]:
    raw = command(argv, cwd=cwd)
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise harness.ContractError(f"command returned invalid JSON: {' '.join(argv)}: {exc}") from exc
    if not isinstance(value, dict):
        fail(f"command returned non-object JSON: {' '.join(argv)}")
    return value


def relative_content_path(root: str, category: str) -> Path:
    if not harness.ROOT.fullmatch(root):
        fail(f"invalid {category} root: {root}")
    return Path("records") / category / "sha256" / f"{root.removeprefix('sha256:')}.json"


def materialize(
    frontier: Path, vela: Path, attempt_path: Path, proposal_id: str
) -> tuple[dict[str, Any], dict[str, Any], dict[str, bytes]]:
    frontier = frontier.resolve()
    vela = vela.resolve()
    attempt_path = attempt_path.resolve()
    if command(("git", "status", "--porcelain"), cwd=frontier):
        fail("frontier checkout must be clean")

    before_commit = command(("git", "rev-parse", "HEAD"), cwd=frontier)
    before_tree = command(("git", "rev-parse", "HEAD^{tree}"), cwd=frontier)
    remote = command(("git", "remote", "get-url", "origin"), cwd=frontier)
    status = json_command((str(vela), "status", str(frontier), "--json"), cwd=frontier)
    next_work = json_command((str(vela), "next", str(frontier), "--limit", "1", "--json"), cwd=frontier)
    inbox = json_command((str(vela), "review", "inbox", str(frontier), "--json"), cwd=frontier)

    after_commit = command(("git", "rev-parse", "HEAD"), cwd=frontier)
    after_tree = command(("git", "rev-parse", "HEAD^{tree}"), cwd=frontier)
    if (before_commit, before_tree) != (after_commit, after_tree) or command(("git", "status", "--porcelain"), cwd=frontier):
        fail("read-only inspection changed the Frontier checkout")

    attempt = harness.read_json(attempt_path)
    if attempt.get("schema") != "vela.attempt.v8":
        fail("expected one vela.attempt.v8 document")
    if attempt.get("frontier_id") != next_work.get("frontier_id"):
        fail("Attempt and current Frontier identities disagree")
    if status.get("campaign", {}).get("active_attempt_count") != 0:
        fail("study requires the exact completed Attempt, not an active campaign")

    targets = next_work.get("targets")
    if not isinstance(targets, list) or len(targets) != 1:
        fail("study requires exactly one current successor Target")
    target = targets[0]
    if target.get("target_id") != attempt.get("target"):
        fail("successor Target identity differs from completed Attempt target")
    start_packet = attempt.get("starting_target_task_binding", {}).get("packet", {}).get("sha256")
    current_packet = target.get("packet", {}).get("sha256")
    if not harness.ROOT.fullmatch(start_packet or "") or not harness.ROOT.fullmatch(current_packet or ""):
        fail("Attempt or successor packet root is invalid")
    if start_packet == current_packet:
        fail("Target has not advanced beyond the completed Attempt packet")
    packet_path = frontier / target.get("packet", {}).get("path", "")
    if digest(packet_path) != current_packet:
        fail("current Target packet bytes disagree with vela next")

    entries = [entry for entry in inbox.get("entries", []) if entry.get("proposal_id") == proposal_id]
    if len(entries) != 1:
        fail(f"expected one Decision Inbox entry for {proposal_id}")
    entry = entries[0]
    if entry.get("inputs", {}).get("repository_root") != next_work.get("repository_root"):
        fail("Decision Inbox entry is stale against current repository root")
    if entry.get("readiness", {}).get("protocol_gate") != "satisfied" or entry.get("staleness", {}).get("state") != "current":
        fail("Decision Inbox entry is not current and protocol-ready")
    verifications = entry.get("verification_records")
    if not isinstance(verifications, list) or len(verifications) != 1:
        fail("study requires exactly one retained Verification Record")
    verification = verifications[0]

    submission_root = entry.get("inputs", {}).get("submission_root")
    submission_path = frontier / relative_content_path(submission_root, "submissions")
    if digest(submission_path) != submission_root:
        fail("Submission bytes disagree with the Decision Inbox")
    submission = harness.read_json(submission_path)

    receipts = attempt.get("agent_run_receipts")
    if not isinstance(receipts, list) or len(receipts) != 2:
        fail("study requires exactly two root-linked Runs")
    links = attempt.get("agent_run_submission_links")
    if not isinstance(links, list) or len(links) != 1:
        fail("study requires exactly one Run-to-Submission link")
    registered_run_id = links[0].get("run_id")
    if links[0].get("submission_id") != submission.get("submission_id"):
        fail("Attempt link and Submission identity disagree")
    provenance = submission.get("provenance", {})
    if provenance.get("source_attempt") != attempt.get("attempt_id") or provenance.get("source_run") != registered_run_id:
        fail("Submission provenance does not bind the exact Attempt and Run")

    runs: list[dict[str, Any]] = []
    source_files: list[dict[str, Any]] = []
    participant_files: dict[str, bytes] = {}
    sanitized_attempt = copy.deepcopy(attempt)
    for index, (receipt, sanitized_receipt) in enumerate(
        zip(receipts, sanitized_attempt["agent_run_receipts"], strict=True), start=1
    ):
        result = receipt.get("result", {})
        run = result.get("run", {})
        evidence = result.get("evidence_manifest", {})
        run_path, evidence_path = Path(run.get("path", "")), Path(evidence.get("path", ""))
        if digest(run_path) != run.get("sha256") or digest(evidence_path) != evidence.get("sha256"):
            fail("Run or evidence-manifest bytes disagree with the Attempt receipt")
        run_relative = f"campaign/run-{index:02d}/run.json"
        evidence_relative = f"campaign/run-{index:02d}/evidence-manifest.json"
        participant_files[run_relative] = run_path.read_bytes()
        participant_files[evidence_relative] = evidence_path.read_bytes()
        sanitized_receipt["result"]["run"]["path"] = run_relative
        sanitized_receipt["result"]["evidence_manifest"]["path"] = evidence_relative
        registered = run.get("id") == registered_run_id
        runs.append({
            "run_number": receipt.get("run_number"),
            "run_id": run.get("id"),
            "receipt_root": receipt.get("receipt_root"),
            "previous_receipt_root": receipt.get("previous_receipt_root"),
            "evidence_root": evidence.get("root"),
            "submission_state": "registered" if registered else "retained_corroboration",
            "submission_id": submission.get("submission_id") if registered else None,
            "proposal_id": entry.get("proposal_id") if registered else None,
            "claim_id": entry.get("claim_id") if registered else None,
            "verification_id": verification.get("verification_record_id") if registered else None,
        })
        source_files.extend((
            {"kind": "run", "sha256": run.get("sha256"), "size": run.get("size")},
            {"kind": "evidence_manifest", "sha256": evidence.get("sha256"), "root": evidence.get("root"), "size": evidence.get("size")},
        ))

    participant_files["campaign/attempt.json"] = harness.canonical_bytes(sanitized_attempt)
    participant_file_manifest = [
        {"path": path, "size": len(data), "sha256": harness.sha256_root(data)}
        for path, data in sorted(participant_files.items())
    ]

    expected = {
        "schema": "vela.product-compression-answer.v2",
        "work": {
            "frontier_id": next_work["frontier_id"],
            "repository_root": next_work["repository_root"],
            "target_id": target["target_id"],
            "target_index_root": next_work["target_index_root"],
            "packet_sha256": current_packet,
        },
        "campaign": {
            "attempt_id": attempt["attempt_id"],
            "authorization_root": attempt["authorization_root"],
            "state": "completed_target_advanced",
            "completed_target_packet_sha256": start_packet,
            "consequence_ceiling": attempt["consequence_ceiling"],
            "budget": attempt["budget"],
            "usage": {key: attempt["usage"][key] for key in ("runs", "submissions", "verifications", "artifacts", "artifact_bytes")},
            "runs": runs,
            "next_action_code": "start_successor_attempt",
        },
        "review": {
            "proposal_id": entry["proposal_id"],
            "proposal_root": entry["inputs"]["proposal_root"],
            "source_submission_id": submission["submission_id"],
            "target_claim_id": entry["claim_id"],
            "verification_id": verification["verification_record_id"],
            "inbox_projection_root": inbox["projection_root"],
            "inbox_entry_root": entry["entry_root"],
            "protocol_gate": entry["readiness"]["protocol_gate"],
            "human_decision_required": entry["readiness"]["human_decision_required"],
            "verification_is_acceptance": False,
            "standing_transition": entry["standing_diff"]["transition"],
            "accepted_before": entry["standing_diff"]["accepted_before"],
            "accepted_if_accept": entry["standing_diff"]["accepted_if_accept"],
            "accepted_if_reject": entry["standing_diff"]["accepted_if_reject"],
            "staleness": entry["staleness"]["state"],
            "next_if_accept_code": "replay_and_recompute_targets",
            "next_if_reject_code": "replay_without_standing_change",
        },
        "safety": {"authority_action_performed": False, "accepted_state_changed": False},
    }
    harness.validate_answer(expected)

    fixture = {
        "schema": "vela.product-compression-fixture.v2",
        "fixture_root": "",
        "vela": {"version": command((str(vela), "--version"), cwd=frontier), "binary_sha256": digest(vela)},
        "frontier": {
            "frontier_id": next_work["frontier_id"], "remote": remote,
            "git_commit": before_commit, "git_tree": before_tree,
            "repository_root": next_work["repository_root"], "target_index_root": next_work["target_index_root"],
        },
        "sources": {
            "attempt_sha256": digest(attempt_path), "attempt_id": attempt["attempt_id"],
            "run_files": source_files, "submission_root": submission_root,
            "proposal_root": entry["inputs"]["proposal_root"],
            "verification_record_root": verification["verification_record_root"],
            "inbox_projection_root": inbox["projection_root"], "inbox_entry_root": entry["entry_root"],
            "successor_packet_sha256": current_packet,
        },
        "participant_files": participant_file_manifest,
    }
    harness.seal(fixture, "fixture_root")
    answer_key = harness.seal({
        "schema": "vela.product-compression-answer-key.v2",
        "answer_key_root": "", "fixture_root": fixture["fixture_root"], "expected": expected,
    }, "answer_key_root")
    harness.validate_answer_key(answer_key)
    return fixture, answer_key, participant_files


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--frontier", type=Path, required=True)
    result.add_argument("--vela", type=Path, required=True)
    result.add_argument("--attempt", type=Path, required=True)
    result.add_argument("--proposal", required=True)
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        reject_system_temporary_output(args.output)
        fixture, answer_key, participant_files = materialize(
            args.frontier, args.vela, args.attempt, args.proposal
        )
        args.output.mkdir(parents=True, exist_ok=True)
        os.chmod(args.output, 0o700)
        harness.write_json(args.output / "fixture.json", fixture)
        harness.write_json(args.output / "answer-key.json", answer_key)
        os.chmod(args.output / "fixture.json", 0o600)
        os.chmod(args.output / "answer-key.json", 0o600)
        for relative, data in participant_files.items():
            destination = args.output / "participant" / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            os.chmod(destination.parent, 0o700)
            destination.write_bytes(data)
            os.chmod(destination, 0o600)
        sys.stdout.buffer.write(harness.canonical_bytes({
            "ok": True, "fixture_root": fixture["fixture_root"],
            "answer_key_root": answer_key["answer_key_root"], "writes_frontier": False,
        }))
        return 0
    except harness.ContractError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
