#!/usr/bin/env python3
"""Materialize two matched Harbor tasks from one exact Vela fixture."""

from __future__ import annotations

import argparse
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


def command(arguments: Sequence[str], *, cwd: Path | None = None) -> str:
    try:
        return subprocess.run(
            arguments, cwd=cwd, check=True, capture_output=True, text=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as exc:
        raise contract.ContractError(f"command failed: {' '.join(arguments)}: {exc}") from exc


def tree_root(directory: Path) -> str:
    rows = []
    for path in sorted(item for item in directory.rglob("*") if item.is_file()):
        rows.append({
            "path": path.relative_to(directory).as_posix(),
            "sha256": contract.sha256_root(path.read_bytes()),
        })
    return contract.sha256_root(contract.canonical_bytes(rows))


def render(path: Path, replacements: dict[str, str]) -> None:
    text = path.read_text(encoding="utf-8")
    for marker, replacement in replacements.items():
        text = text.replace(f"{{{{{marker}}}}}", replacement)
    if "{{" in text or "}}" in text:
        raise contract.ContractError(f"unresolved task template marker in {path}")
    path.write_text(text, encoding="utf-8")


def prepare(
    materials: Path,
    frontier: Path,
    vela_linux: Path,
    model: str,
    codex_version: str,
    vela_version: str,
    job_name: str,
    output: Path,
) -> dict[str, Any]:
    if output.exists() and any(output.iterdir()):
        raise contract.ContractError(f"output must be absent or empty: {output}")
    fixture = contract.read_json(materials / "fixture.json")
    answer_key = contract.read_json(materials / "answer-key.json")
    contract.validate_answer_key(answer_key)
    if fixture.get("fixture_root") != answer_key["fixture_root"]:
        raise contract.ContractError("fixture and answer key disagree")
    if fixture.get("fixture_root") != contract.record_root(fixture, "fixture_root"):
        raise contract.ContractError("fixture root mismatch")
    if command(("git", "status", "--porcelain"), cwd=frontier):
        raise contract.ContractError("frontier checkout must be clean")
    if command(("git", "rev-parse", "HEAD"), cwd=frontier) != fixture["frontier"]["git_commit"]:
        raise contract.ContractError("frontier checkout does not match the fixture")
    if not vela_linux.is_file() or vela_linux.read_bytes()[:4] != b"\x7fELF":
        raise contract.ContractError("guided arm requires an exact Linux Vela executable")
    if not all((model, codex_version, vela_version, job_name)):
        raise contract.ContractError("model, versions, and job name are required")

    output.mkdir(parents=True, exist_ok=True)
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
            binding = contract.seal({
                "schema": "vela.harbor-task-binding.v4",
                "binding_root": "",
                "fixture_root": fixture["fixture_root"],
                "answer_key_root": answer_key["answer_key_root"],
                "session_id": session,
                "arm": arm,
                "frontier": {
                    "git_commit": fixture["frontier"]["git_commit"],
                    "git_tree": fixture["frontier"]["git_tree"],
                    "repository_root": fixture["frontier"]["repository_root"],
                    "bundle_sha256": contract.sha256_root(bundle.read_bytes()),
                },
            }, "binding_root")
            environment = task / "environment"
            tests = task / "tests"
            shutil.copy2(bundle, environment / "frontier.bundle")
            shutil.copy2(materials / "fixture.json", environment / "fixture.json")
            shutil.copy2(Path(__file__).with_name("answer.schema.json"), environment / "answer.schema.json")
            contract.write_json(environment / "task-binding.json", binding)
            contract.write_json(tests / "answer-key.json", answer_key)
            contract.write_json(tests / "task-binding.json", binding)
            vela_install = ""
            guidance = "Use ordinary Git and file-reading tools only. The `vela` executable is intentionally absent."
            if arm == "vela-guided":
                shutil.copy2(vela_linux, environment / "vela")
                vela_install = (
                    "COPY vela /usr/local/bin/vela\n"
                    f"RUN chmod 0555 /usr/local/bin/vela && test \"$(vela --version)\" = '{vela_version}'"
                )
                guidance = (
                    "You may also use the installed read-only `vela` CLI: `vela status . --json`, "
                    "`vela next . --json`, `vela show . <id> --json`, and `vela review show . <id> --json`."
                )
            render(task / "instruction.md", {"SESSION_ID": session, "TOOL_GUIDANCE": guidance})
            render(task / "task.toml", {"SESSION_ID": session})
            render(environment / "Dockerfile", {"VELA_INSTALL": vela_install})
            task_rows.append({"path": task.relative_to(output).as_posix(), "root": tree_root(task)})
    finally:
        bundle.unlink(missing_ok=True)

    plan = contract.seal({
        "schema": "vela.product-compression-plan.v7",
        "plan_root": "",
        "fixture_root": fixture["fixture_root"],
        "answer_key_root": answer_key["answer_key_root"],
        "harbor": {"version": command(("harbor", "--version"))},
        "agent": {"name": "codex", "model": model, "version": codex_version},
        "vela": {"version": vela_version, "linux_sha256": contract.sha256_root(vela_linux.read_bytes())},
        "sessions": list(SESSIONS),
        "tasks": task_rows,
        "comparison_rule": COMPARISON,
        "claim_limit": "First-party evidence from one frozen task; no independent-user or general scientific-workflow claim.",
    }, "plan_root")
    contract.write_json(output / "plan.json", plan)
    contract.write_json(output / "harbor-job.json", {
        "job_name": job_name,
        "n_concurrent_trials": 1,
        "retry": {"max_retries": 0},
        "agents": [{"name": "codex", "model_name": model, "kwargs": {"version": codex_version}}],
        "tasks": [{"path": row["path"]} for row in task_rows],
    })
    return plan


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--materials", type=Path, required=True)
    result.add_argument("--frontier", type=Path, required=True)
    result.add_argument("--vela-linux", type=Path, required=True)
    result.add_argument("--model", required=True)
    result.add_argument("--codex-version", required=True)
    result.add_argument("--vela-version", required=True)
    result.add_argument("--job-name", required=True)
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        plan = prepare(**vars(args))
        sys.stdout.buffer.write(contract.canonical_bytes({"ok": True, "plan_root": plan["plan_root"]}))
        return 0
    except contract.ContractError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
