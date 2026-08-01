#!/usr/bin/env python3
"""Materialize one native Harbor product-compression study."""

from __future__ import annotations

import argparse
import hashlib
import json
import shlex
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Sequence

import contract


ARMS = ("git-files", "vela-guided")
SESSIONS = ("git-files-01", "vela-guided-01", "vela-guided-02", "git-files-02")
COMPARISON = {
    "required_repetitions_per_arm": 2,
    "guided_exact_required": 2,
    "exactness_rule": "guided_dominates_or_ties_baseline",
    "efficiency_when_exactness_tied": "median_elapsed_improves_at_least_20_percent",
    "cost_rule": "guided_median_cost_no_regression",
}


def fail(message: str) -> None:
    raise contract.ContractError(message)


def digest(path: Path) -> str:
    try:
        return f"sha256:{hashlib.sha256(path.read_bytes()).hexdigest()}"
    except OSError as exc:
        raise contract.ContractError(f"cannot hash {path}: {exc}") from exc


def command(argv: Sequence[str], *, cwd: Path | None = None) -> str:
    try:
        result = subprocess.run(argv, cwd=cwd, check=False, capture_output=True, text=True)
    except OSError as exc:
        raise contract.ContractError(f"cannot execute {argv[0]}: {exc}") from exc
    if result.returncode != 0:
        fail(f"command failed ({result.returncode}): {' '.join(argv)}: {result.stderr.strip()}")
    return result.stdout.strip()


def json_command(argv: Sequence[str], *, cwd: Path) -> dict[str, Any]:
    raw = command(argv, cwd=cwd)
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise contract.ContractError(f"command returned invalid JSON: {' '.join(argv)}: {exc}") from exc
    if not isinstance(value, dict):
        fail(f"command returned non-object JSON: {' '.join(argv)}")
    return value


def relative_content_path(root: str, category: str) -> Path:
    if not contract.ROOT.fullmatch(root):
        fail(f"invalid {category} root: {root}")
    return Path("records") / category / "sha256" / f"{root.removeprefix('sha256:')}.json"


def materialize_fixture(
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
    if not contract.ROOT.fullmatch(packet_root or "") or digest(packet_path) != packet_root:
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
    submission = contract.read_json(submission_path)

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
    contract.validate_answer(expected)

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
    contract.seal(fixture, "fixture_root")
    answer_key = contract.seal({
        "schema": "vela.product-compression-answer-key.v5",
        "answer_key_root": "", "fixture_root": fixture["fixture_root"], "expected": expected,
    }, "answer_key_root")
    contract.validate_answer_key(answer_key)
    return fixture, answer_key


def tree_root(directory: Path) -> str:
    rows = [
        {
            "path": path.relative_to(directory).as_posix(),
            "sha256": contract.sha256_root(path.read_bytes()),
        }
        for path in sorted(item for item in directory.rglob("*") if item.is_file())
    ]
    return contract.sha256_root(contract.canonical_bytes(rows))


def render(path: Path, replacements: dict[str, str]) -> None:
    text = path.read_text(encoding="utf-8")
    for marker, replacement in replacements.items():
        text = text.replace(f"{{{{{marker}}}}}", replacement)
    if "{{" in text or "}}" in text:
        fail(f"unresolved task template marker in {path}")
    path.write_text(text, encoding="utf-8")


def build_study(
    fixture: dict[str, Any],
    answer_key: dict[str, Any],
    frontier: Path,
    vela_linux: Path,
    model: str,
    codex_version: str,
    job_name: str,
    output: Path,
) -> dict[str, Any]:
    """Build the cached Harbor tasks and frozen comparison plan."""
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        fail(f"output must be absent or empty: {output}")
    if fixture.get("fixture_root") != contract.record_root(fixture, "fixture_root"):
        fail("fixture root mismatch")
    contract.validate_answer_key(answer_key)
    if fixture["fixture_root"] != answer_key["fixture_root"]:
        fail("fixture and answer key disagree")

    frontier = frontier.resolve()
    vela_linux = vela_linux.resolve()
    if command(("git", "status", "--porcelain"), cwd=frontier):
        fail("frontier checkout must be clean")
    if command(("git", "rev-parse", "HEAD"), cwd=frontier) != fixture["frontier"]["git_commit"]:
        fail("frontier checkout does not match the fixture")
    if not vela_linux.is_file() or vela_linux.read_bytes()[:4] != b"\x7fELF":
        fail("guided arm requires an exact Linux Vela executable")
    if not all((model, codex_version, job_name)):
        fail("model, Codex version, and job name are required")

    output.mkdir(parents=True, exist_ok=True)
    contract.write_json(output / "fixture.json", fixture)
    contract.write_json(output / "answer-key.json", answer_key)
    tasks = output / "tasks"
    bundle = output / "frontier.bundle"
    command(("git", "bundle", "create", str(bundle), "HEAD"), cwd=frontier)
    template = Path(__file__).with_name("task")
    task_rows = []
    try:
        for session in SESSIONS:
            arm = session.rsplit("-", 1)[0]
            task = tasks / session
            shutil.copytree(template, task)
            environment = task / "environment"
            tests = task / "tests"
            shutil.copy2(bundle, environment / "frontier.bundle")
            shutil.copy2(output / "fixture.json", environment / "fixture.json")
            shutil.copy2(
                Path(__file__).with_name("answer.schema.json"),
                environment / "answer.schema.json",
            )
            shutil.copy2(output / "fixture.json", tests / "fixture.json")
            shutil.copy2(output / "answer-key.json", tests / "answer-key.json")

            vela_install = ""
            guidance = (
                "Use ordinary Git and file-reading tools only. "
                "The `vela` executable is intentionally absent."
            )
            if arm == "vela-guided":
                shutil.copy2(vela_linux, environment / "vela")
                vela_install = (
                    "COPY vela /usr/local/bin/vela\n"
                    "RUN chmod 0555 /usr/local/bin/vela && "
                    f"test \"$(vela --version)\" = {shlex.quote(fixture['vela']['version'])}"
                )
                guidance = (
                    "You may also use the installed read-only `vela` CLI: "
                    "`vela status . --json`, "
                    "`vela next . --json`, `vela show . <id> --json`, and "
                    "`vela review show . <id> --json`."
                )
            render(
                task / "instruction.md",
                {"SESSION_ID": session, "TOOL_GUIDANCE": guidance},
            )
            render(task / "task.toml", {"SESSION_ID": session})
            render(environment / "Dockerfile", {"VELA_INSTALL": vela_install})
            task_rows.append({
                "path": task.relative_to(output).as_posix(),
                "root": tree_root(task),
            })
    finally:
        bundle.unlink(missing_ok=True)

    plan = contract.seal({
        "schema": "vela.product-compression-plan.v7",
        "plan_root": "",
        "fixture_root": fixture["fixture_root"],
        "answer_key_root": answer_key["answer_key_root"],
        "harbor": {"version": command(("harbor", "--version"))},
        "agent": {"name": "codex", "model": model, "version": codex_version},
        "vela": {
            "version": fixture["vela"]["version"],
            "linux_sha256": contract.sha256_root(vela_linux.read_bytes()),
        },
        "sessions": list(SESSIONS),
        "tasks": task_rows,
        "comparison_rule": COMPARISON,
        "claim_limit": (
            "First-party evidence from one frozen task; no independent-user "
            "or general scientific-workflow claim."
        ),
    }, "plan_root")
    contract.write_json(output / "plan.json", plan)
    contract.write_json(output / "harbor-job.json", {
        "job_name": job_name,
        "n_concurrent_trials": 1,
        "retry": {"max_retries": 0},
        "agents": [{
            "name": "codex",
            "model_name": model,
            "kwargs": {"version": codex_version},
        }],
        "tasks": [{"path": row["path"]} for row in task_rows],
    })
    return plan


def materialize(
    frontier: Path,
    vela: Path,
    proposal_id: str,
    vela_linux: Path,
    model: str,
    codex_version: str,
    job_name: str,
    output: Path,
) -> dict[str, Any]:
    fixture, answer_key = materialize_fixture(frontier, vela, proposal_id)
    return build_study(
        fixture, answer_key, frontier, vela_linux, model, codex_version, job_name, output,
    )


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--frontier", type=Path, required=True)
    result.add_argument("--vela", type=Path, required=True)
    result.add_argument("--proposal", required=True)
    result.add_argument("--vela-linux", type=Path, required=True)
    result.add_argument("--model", required=True)
    result.add_argument("--codex-version", required=True)
    result.add_argument("--job-name", default="vela-product-compression")
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        plan = materialize(
            args.frontier,
            args.vela,
            args.proposal,
            args.vela_linux,
            args.model,
            args.codex_version,
            args.job_name,
            args.output,
        )
        sys.stdout.buffer.write(contract.canonical_bytes({
            "ok": True,
            "plan_root": plan["plan_root"],
            "fixture_root": plan["fixture_root"],
            "answer_key_root": plan["answer_key_root"],
            "writes_frontier": False,
        }))
        return 0
    except contract.ContractError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
