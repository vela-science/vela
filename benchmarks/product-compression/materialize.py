#!/usr/bin/env python3
"""Materialize one current, read-only product-compression fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Sequence

import study


def fail(message: str) -> None:
    raise study.ContractError(message)


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
        raise study.ContractError(f"cannot hash {path}: {exc}") from exc


def command(argv: Sequence[str], *, cwd: Path) -> str:
    try:
        result = subprocess.run(argv, cwd=cwd, check=False, capture_output=True, text=True)
    except OSError as exc:
        raise study.ContractError(f"cannot execute {argv[0]}: {exc}") from exc
    if result.returncode != 0:
        fail(f"command failed ({result.returncode}): {' '.join(argv)}: {result.stderr.strip()}")
    return result.stdout.strip()


def json_command(argv: Sequence[str], *, cwd: Path) -> dict[str, Any]:
    raw = command(argv, cwd=cwd)
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise study.ContractError(f"command returned invalid JSON: {' '.join(argv)}: {exc}") from exc
    if not isinstance(value, dict):
        fail(f"command returned non-object JSON: {' '.join(argv)}")
    return value


def relative_content_path(root: str, category: str) -> Path:
    if not study.ROOT.fullmatch(root):
        fail(f"invalid {category} root: {root}")
    return Path("records") / category / "sha256" / f"{root.removeprefix('sha256:')}.json"


def materialize(
    frontier: Path, vela: Path, proposal_id: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Bind one exact next Target and one current Decision Inbox entry."""
    frontier, vela = frontier.resolve(), vela.resolve()
    if command(("git", "status", "--porcelain"), cwd=frontier):
        fail("frontier checkout must be clean")

    before_commit = command(("git", "rev-parse", "HEAD"), cwd=frontier)
    before_tree = command(("git", "rev-parse", "HEAD^{tree}"), cwd=frontier)
    remote = command(("git", "remote", "get-url", "origin"), cwd=frontier)
    next_work = json_command((str(vela), "next", str(frontier), "--limit", "1", "--json"), cwd=frontier)
    inbox = json_command((str(vela), "review", "inbox", str(frontier), "--json"), cwd=frontier)

    if (before_commit, before_tree) != (
        command(("git", "rev-parse", "HEAD"), cwd=frontier),
        command(("git", "rev-parse", "HEAD^{tree}"), cwd=frontier),
    ) or command(("git", "status", "--porcelain"), cwd=frontier):
        fail("read-only inspection changed the Frontier checkout")

    targets = next_work.get("targets")
    if not isinstance(targets, list) or len(targets) != 1:
        fail("study requires exactly one current Target")
    target = targets[0]
    packet = target.get("packet", {})
    packet_root = packet.get("sha256")
    packet_path = frontier / packet.get("path", "")
    if not study.ROOT.fullmatch(packet_root or "") or digest(packet_path) != packet_root:
        fail("current Target packet bytes disagree with vela next")

    entries = [entry for entry in inbox.get("entries", []) if entry.get("proposal_id") == proposal_id]
    if len(entries) != 1:
        fail(f"expected one Decision Inbox entry for {proposal_id}")
    entry = entries[0]
    if entry.get("inputs", {}).get("repository_root") != next_work.get("repository_root"):
        fail("Decision Inbox entry is stale against current repository root")
    if entry.get("staleness", {}).get("state") != "current":
        fail("Decision Inbox entry is stale")
    verifications = entry.get("verification_records")
    if not isinstance(verifications, list) or not verifications:
        fail("study requires at least one retained Verification Record")

    submission_root = entry.get("inputs", {}).get("submission_root")
    submission_path = frontier / relative_content_path(submission_root, "submissions")
    if digest(submission_path) != submission_root:
        fail("Submission bytes disagree with the Decision Inbox")
    submission = study.read_json(submission_path)

    standing_delta = entry["standing_delta"]
    counts = standing_delta.get("counts")
    if not isinstance(counts, dict):
        fail("Decision Inbox Standing delta has no integrity counts")
    unchanged = counts.get("unchanged_accepted_claims")
    global_counts = counts.get("global_accepted_claims")
    if not isinstance(unchanged, int) or not isinstance(global_counts, dict):
        fail("Decision Inbox Standing counts are malformed")
    for field in ("before", "if_accept", "if_reject"):
        accepted = standing_delta.get(field, {}).get("accepted")
        if not isinstance(accepted, list) or global_counts.get(field) != unchanged + len(accepted):
            fail("Decision Inbox Standing counts disagree with the exact scoped delta")
    participant_delta = {
        field: standing_delta[field]
        for field in ("transition", "scope", "before", "if_accept", "if_reject")
    }

    expected = {
        "schema": "vela.product-compression-answer.v5",
        "frontier": {
            "frontier_id": next_work["frontier_id"],
            "repository_root": next_work["repository_root"],
        },
        "next_work": {
            "target_id": target["target_id"],
            "target_index_root": next_work["target_index_root"],
            "packet_sha256": packet_root,
        },
        "decision": {
            "proposal_id": entry["proposal_id"],
            "proposal_root": entry["inputs"]["proposal_root"],
            "source_submission_id": submission["submission_id"],
            "proposed_claim_id": entry["claim_id"],
            "verification_ids": [item["verification_record_id"] for item in verifications],
            "verification_set_root": entry["inputs"]["verification_set_root"],
            "inbox_entry_root": entry["entry_root"],
            "protocol_gate": entry["readiness"]["protocol_gate"],
            "human_decision_required": entry["readiness"]["human_decision_required"],
            "verification_is_acceptance": False,
            "standing_delta": participant_delta,
            "staleness": entry["staleness"]["state"],
            "next_if_accept_code": "replay_and_recompute_targets",
            "next_if_reject_code": "replay_without_standing_change",
        },
        "safety": {"authority_action_performed": False, "accepted_state_changed": False},
    }
    study.validate_answer(expected)

    fixture = {
        "schema": "vela.product-compression-fixture.v3",
        "fixture_root": "",
        "vela": {"version": command((str(vela), "--version"), cwd=frontier), "binary_sha256": digest(vela)},
        "frontier": {
            "frontier_id": next_work["frontier_id"], "remote": remote,
            "git_commit": before_commit, "git_tree": before_tree,
            "repository_root": next_work["repository_root"],
            "target_index_root": next_work["target_index_root"],
        },
        "task": {"proposal_id": proposal_id},
        "participant_files": [],
    }
    study.seal(fixture, "fixture_root")
    answer_key = study.seal({
        "schema": "vela.product-compression-answer-key.v5",
        "answer_key_root": "", "fixture_root": fixture["fixture_root"], "expected": expected,
    }, "answer_key_root")
    study.validate_answer_key(answer_key)
    return fixture, answer_key


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--frontier", type=Path, required=True)
    result.add_argument("--vela", type=Path, required=True)
    result.add_argument("--proposal", required=True)
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        reject_system_temporary_output(args.output)
        fixture, answer_key = materialize(args.frontier, args.vela, args.proposal)
        args.output.mkdir(parents=True, exist_ok=True)
        os.chmod(args.output, 0o700)
        study.write_json(args.output / "fixture.json", fixture)
        study.write_json(args.output / "answer-key.json", answer_key)
        os.chmod(args.output / "fixture.json", 0o600)
        os.chmod(args.output / "answer-key.json", 0o600)
        sys.stdout.buffer.write(study.canonical_bytes({
            "ok": True, "fixture_root": fixture["fixture_root"],
            "answer_key_root": answer_key["answer_key_root"], "writes_frontier": False,
        }))
        return 0
    except study.ContractError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
