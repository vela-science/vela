#!/usr/bin/env python3
"""Freeze one matched Harbor study for the real Erdős 264 proof repair.

Harbor owns agent execution, isolation, retries, timing, cost, and result
capture. This script only validates exact Git/Vela inputs and materializes two
ordinary Harbor tasks whose sole treatment difference is read-only Vela CLI
availability.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import shlex
import shutil
import subprocess
import sys
from typing import Any, Sequence

SCHEMA = "vela.erdos-264-proof-repair-fixture.v1"
PLAN_SCHEMA = "vela.erdos-264-proof-repair-harbor-plan.v1"
TARGET_ID = "erdos:264:parts-i-proof-repair"
TARGET_PACKET = "targets/erdos-264-parts-i-proof-repair.json"
CORRECTION_CLAIM_ID = (
    "vcl_5a7df5408c6b11aa52745af2ce1203db3b39cb9a9404c27309f4ee490ffb1386"
)
CORRECTION_CLAIM_ROOT = (
    "sha256:4d3f546331886ba10891c1ceb46267993d41b99ed746a19b74d91ccb9448b16e"
)
FORMAL_COMMIT = "e6d6b867dc85eec2f88bc47496b4314c623f9f92"
FORMAL_TREE = "1e24e996a9fee330dc885ec2b314f60bfd508985"
FORMAL_PATH = "FormalConjectures/ErdosProblems/264.lean"
FORMAL_SHA256 = (
    "sha256:c59caaa2524e3edd52944e63f5d9bb0614f1bc36d7fb8a0fec7029c14c266b46"
)
REFERENCE_COMMIT = "68da20b96673899166e94638f5a7fffeb7231d35"
REFERENCE_TREE = "1d42e6d9d0fecef7de0c6c2a6e3cf7d58283bab8"
REFERENCE_PATH = "src/v4.29.1/ErdosProblems/Erdos264.lean"
REFERENCE_SHA256 = (
    "sha256:10c61b6082a51a85d7b0e41bffc7ee0799d46183b6a3848a9816cf9e943fedf2"
)
SOURCE_CORRECTION_BEFORE = "593e6b76702c5dbffaaa91b59f4faaed705d04ce"
SOURCE_CORRECTION_COMMIT = "0598b8f281060a18416d60753fd75621d659bb07"
SOURCE_DIFF_ROOT = (
    "sha256:a1935f112f5e086cac55d0933f6aa5588893aa7452512d5a0319e12fba4a472f"
)
PUBLICATION_ID = "arXiv:2406.17593"
ARMS = ("git-files", "vela-guided")


class MaterializationError(ValueError):
    pass


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def seal(value: dict[str, Any], field: str) -> dict[str, Any]:
    value[field] = sha256(
        canonical_bytes({key: item for key, item in value.items() if key != field})
    )
    return value


def command(argv: Sequence[str], *, cwd: pathlib.Path) -> str:
    try:
        result = subprocess.run(argv, cwd=cwd, capture_output=True, text=True)
    except OSError as error:
        raise MaterializationError(f"cannot execute {argv[0]}: {error}") from error
    if result.returncode != 0:
        raise MaterializationError(
            f"command failed ({result.returncode}): {' '.join(argv)}: {result.stderr.strip()}"
        )
    return result.stdout.strip()


def json_command(argv: Sequence[str], *, cwd: pathlib.Path) -> dict[str, Any]:
    try:
        value = json.loads(command(argv, cwd=cwd))
    except json.JSONDecodeError as error:
        raise MaterializationError(
            f"command returned invalid JSON: {' '.join(argv)}"
        ) from error
    if not isinstance(value, dict):
        raise MaterializationError(
            f"command returned non-object JSON: {' '.join(argv)}"
        )
    return value


def read_json(path: pathlib.Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise MaterializationError(f"cannot read exact JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise MaterializationError(f"expected JSON object at {path}")
    return value


def require_clean(repo: pathlib.Path, label: str) -> tuple[str, str]:
    if command(("git", "status", "--porcelain=v1", "--untracked-files=all"), cwd=repo):
        raise MaterializationError(f"{label} checkout must be clean")
    return (
        command(("git", "rev-parse", "HEAD"), cwd=repo),
        command(("git", "rev-parse", "HEAD^{tree}"), cwd=repo),
    )


def require_source(
    repo: pathlib.Path,
    *,
    label: str,
    commit: str,
    tree: str,
    relative: str,
    root: str,
) -> None:
    observed_commit, observed_tree = require_clean(repo, label)
    if (observed_commit, observed_tree) != (commit, tree):
        raise MaterializationError(f"{label} Git identity differs")
    path = repo / relative
    if not path.is_file() or path.is_symlink() or sha256(path.read_bytes()) != root:
        raise MaterializationError(f"{label} retained source bytes differ")


def validate_target_projection(
    offer: dict[str, Any], packet: dict[str, Any], repository: dict[str, Any]
) -> dict[str, Any]:
    availability = offer.get("availability")
    targets = offer.get("targets")
    if (
        not isinstance(availability, dict)
        or not isinstance(availability.get("configured"), int)
        or availability["configured"] < 1
        or availability.get("stale") != 0
        or availability.get("fresh") != availability["configured"]
        or availability.get("returned") != 1
    ):
        raise MaterializationError(
            "Erdős 264 must be the first current Target in a fully fresh index"
        )
    if not isinstance(targets, list) or len(targets) != 1:
        raise MaterializationError("Vela must return one first-ranked Target")
    target = targets[0]
    packet_locator = target.get("packet") or {}
    if (
        target.get("target_id") != TARGET_ID
        or target.get("rank") != 1
        or packet_locator.get("path") != TARGET_PACKET
        or packet.get("schema") != "erdos-frontier.correction-inheritance-work.v1"
        or packet.get("authority") != "non_authoritative"
        or (packet.get("target") or {}).get("id") != TARGET_ID
        or (packet.get("prerequisite") or {}).get("accepted_claim")
        != {"claim_id": CORRECTION_CLAIM_ID, "claim_root": CORRECTION_CLAIM_ROOT}
    ):
        raise MaterializationError(
            "first Target is not the exact correction-gated repair"
        )
    if packet.get("source") != {
        "repository": "https://github.com/google-deepmind/formal-conjectures.git",
        "commit": FORMAL_COMMIT,
        "tree": FORMAL_TREE,
        "path": FORMAL_PATH,
        "sha256": FORMAL_SHA256,
        "lean_toolchain": "leanprover/lean4:v4.27.0",
        "mathlib_commit": "a3a10db0e9d66acbebf76c5e6a135066525ac900",
        "declaration": "Erdos264.erdos_264.parts.i",
    }:
        raise MaterializationError("Target does not bind the frozen Formal source")
    prior = packet.get("known_prior_evidence") or {}
    if (
        prior.get("revision") != REFERENCE_COMMIT
        or prior.get("path") != REFERENCE_PATH
        or prior.get("file_sha256") != REFERENCE_SHA256
    ):
        raise MaterializationError("Target does not bind the frozen public proof")
    accepted = {
        row.get("claim_id"): row.get("claim_root")
        for row in repository.get("accepted_claims", [])
        if row.get("standing") == "accepted"
    }
    if accepted.get(CORRECTION_CLAIM_ID) != CORRECTION_CLAIM_ROOT:
        raise MaterializationError("correction Claim is not accepted Standing")
    return target


def tree_root(directory: pathlib.Path) -> str:
    rows = [
        {
            "path": path.relative_to(directory).as_posix(),
            "sha256": sha256(path.read_bytes()),
        }
        for path in sorted(item for item in directory.rglob("*") if item.is_file())
    ]
    return sha256(canonical_bytes(rows))


def render(path: pathlib.Path, replacements: dict[str, str]) -> None:
    text = path.read_text()
    for marker, replacement in replacements.items():
        text = text.replace("{{" + marker + "}}", replacement)
    if "{{" in text or "}}" in text:
        raise MaterializationError(f"unresolved template marker in {path}")
    path.write_text(text)


def bundle(repo: pathlib.Path, output: pathlib.Path, revision: str) -> None:
    command(("git", "bundle", "create", str(output), revision), cwd=repo)


def materialize(args: argparse.Namespace) -> dict[str, Any]:
    frontier = args.frontier.resolve()
    formal = args.formal_conjectures.resolve()
    reference = args.reference_proof.resolve()
    vela = args.vela.resolve()
    vela_linux = args.vela_linux.resolve()
    output = args.output.resolve()
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        raise MaterializationError("output must be absent or empty")
    frontier_commit, frontier_tree = require_clean(frontier, "Frontier")
    require_source(
        formal,
        label="Formal Conjectures",
        commit=FORMAL_COMMIT,
        tree=FORMAL_TREE,
        relative=FORMAL_PATH,
        root=FORMAL_SHA256,
    )
    require_source(
        reference,
        label="reference proof",
        commit=REFERENCE_COMMIT,
        tree=REFERENCE_TREE,
        relative=REFERENCE_PATH,
        root=REFERENCE_SHA256,
    )
    if (
        not vela.is_file()
        or not vela_linux.is_file()
        or vela_linux.read_bytes()[:4] != b"\x7fELF"
    ):
        raise MaterializationError("exact local and Linux Vela binaries are required")
    offer = json_command(
        (str(vela), "next", ".", "--limit", "1", "--json"), cwd=frontier
    )
    packet_path = frontier / TARGET_PACKET
    packet = read_json(packet_path)
    repository = read_json(frontier / ".vela" / "repository.json")
    target = validate_target_projection(offer, packet, repository)
    packet_root = sha256(packet_path.read_bytes())
    if (target.get("packet") or {}).get("sha256") != packet_root:
        raise MaterializationError("Target packet root differs from Vela offer")

    episode = {
        "publication": PUBLICATION_ID,
        "source_repository": "https://github.com/google-deepmind/formal-conjectures.git",
        "source_transition": {
            "before_commit": SOURCE_CORRECTION_BEFORE,
            "after_commit": SOURCE_CORRECTION_COMMIT,
            "diff_root": SOURCE_DIFF_ROOT,
        },
        "accepted_correction": {
            "claim_id": CORRECTION_CLAIM_ID,
            "claim_root": CORRECTION_CLAIM_ROOT,
        },
        "native_obligation": {
            "target_id": TARGET_ID,
            "packet_root": packet_root,
            "declaration": "Erdos264.erdos_264.parts.i",
        },
    }
    episode_root = sha256(canonical_bytes(episode))

    output.mkdir(parents=True, exist_ok=True)
    fixture = seal(
        {
            "schema": SCHEMA,
            "fixture_root": "",
            "evidence_level": "real_correction_case",
            "scientific_episode_root": episode_root,
            "scientific_episode": episode,
            "arms": list(ARMS),
            "treatment": "read_only_vela_cli",
            "frontier": {
                "commit": frontier_commit,
                "tree": frontier_tree,
                "repository_root": offer["repository_root"],
                "target_index_root": offer["target_index_root"],
                "target_id": TARGET_ID,
                "packet_root": packet_root,
                "correction_claim_id": CORRECTION_CLAIM_ID,
                "correction_claim_root": CORRECTION_CLAIM_ROOT,
            },
            "formal_conjectures": {
                "commit": FORMAL_COMMIT,
                "tree": FORMAL_TREE,
                "path": FORMAL_PATH,
                "sha256": FORMAL_SHA256,
            },
            "reference_proof": {
                "commit": REFERENCE_COMMIT,
                "tree": REFERENCE_TREE,
                "path": REFERENCE_PATH,
                "sha256": REFERENCE_SHA256,
            },
            "vela": {
                "version": command((str(vela), "--version"), cwd=frontier),
                "sha256": sha256(vela.read_bytes()),
                "linux_sha256": sha256(vela_linux.read_bytes()),
            },
            "custody": {
                "same_repository_bytes": True,
                "same_reference_proof_bytes": True,
                "authority_credentials_available": False,
                "automatic_decision": False,
                "agent_network": "OpenAI OAuth endpoints only",
                "verifier_network": "none",
            },
            "claim_limit": (
                "One real source-assisted semantic proof repair after one human correction Decision; "
                "not new-theorem discovery, statistical performance evidence, or protocol breakthrough."
            ),
        },
        "fixture_root",
    )
    (output / "fixture.json").write_bytes(canonical_bytes(fixture))
    bundles = {
        "frontier.bundle": (frontier, "HEAD"),
        "formal-conjectures.bundle": (formal, "HEAD"),
        "reference-proof.bundle": (reference, "HEAD"),
    }
    for name, (repo, revision) in bundles.items():
        bundle(repo, output / name, revision)

    template = pathlib.Path(__file__).with_name("task")
    task_rows = []
    for arm in ARMS:
        task = output / "tasks" / arm
        shutil.copytree(
            template, task, ignore=shutil.ignore_patterns("__pycache__", "*.pyc")
        )
        for name in bundles:
            shutil.copy2(output / name, task / "environment" / name)
        shutil.copy2(
            output / "formal-conjectures.bundle",
            task / "tests" / "formal-conjectures.bundle",
        )
        shutil.copy2(output / "fixture.json", task / "tests" / "fixture.json")
        shutil.copy2(
            frontier / "execution/erdos-264-proof-repair/verify.py",
            task / "tests" / "verify.py",
        )
        vela_install = ""
        guidance = (
            "Use ordinary Git and file-reading tools to inspect the Frontier. "
            "The `vela` executable is intentionally absent."
        )
        if arm == "vela-guided":
            shutil.copy2(vela_linux, task / "environment" / "vela")
            vela_install = (
                "COPY vela /usr/local/bin/vela\n"
                "RUN chmod 0555 /usr/local/bin/vela && "
                f"test \"$(vela --version)\" = {shlex.quote(fixture['vela']['version'])}"
            )
            guidance = (
                "Use the installed read-only Vela CLI to select and inspect the first Target: "
                "`vela next /workspace/frontier --limit 1 --json` and "
                "`vela start <target-id> --frontier /workspace/frontier --json`."
            )
        replacements = {
            "ARM": arm,
            "CODEX_VERSION": args.codex_version,
            "VELA_INSTALL": vela_install,
            "TOOL_GUIDANCE": guidance,
            "FRONTIER_COMMIT": frontier_commit,
            "FORMAL_COMMIT": FORMAL_COMMIT,
            "REFERENCE_COMMIT": REFERENCE_COMMIT,
        }
        for relative in (
            "task.toml",
            "instruction.md",
            "environment/Dockerfile",
            "tests/Dockerfile",
        ):
            render(task / relative, replacements)
        task_rows.append(
            {
                "arm": arm,
                "path": task.relative_to(output).as_posix(),
                "root": tree_root(task),
            }
        )

    job = json_command(
        (
            "harbor",
            "run",
            "--path",
            "tasks",
            "--agent",
            "codex",
            "--model",
            args.model,
            "--agent-kwarg",
            f"version={args.codex_version}",
            "--n-attempts",
            "1",
            "--n-concurrent",
            "1",
            "--max-retries",
            "0",
            "--job-name",
            args.job_name,
            "--print-config",
        ),
        cwd=output,
    )
    (output / "harbor-job.json").write_bytes(canonical_bytes(job))
    plan = seal(
        {
            "schema": PLAN_SCHEMA,
            "plan_root": "",
            "fixture_root": fixture["fixture_root"],
            "evidence_level": fixture["evidence_level"],
            "scientific_episode_root": episode_root,
            "task_roots": task_rows,
            "harbor_job_root": sha256(canonical_bytes(job)),
            "attempts_per_arm": 1,
            "primary_outcome": "exact native Lean pass for the frozen corrected theorem",
            "secondary_outcomes": [
                "elapsed_seconds",
                "all_in_cost",
                "human_interventions",
            ],
            "claim_credit": {
                "eligible": [
                    "one correction-continuation case",
                    "native Lean exact pass@1 for each arm",
                ],
                "ineligible": [
                    "new theorem discovery",
                    "statistical agent lift",
                    "external adoption",
                    "protocol breakthrough",
                ],
            },
            "automatic_decision": False,
        },
        "plan_root",
    )
    (output / "plan.json").write_bytes(canonical_bytes(plan))
    for name in bundles:
        (output / name).unlink()
    return plan


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--frontier", type=pathlib.Path, required=True)
    result.add_argument("--formal-conjectures", type=pathlib.Path, required=True)
    result.add_argument("--reference-proof", type=pathlib.Path, required=True)
    result.add_argument("--vela", type=pathlib.Path, required=True)
    result.add_argument("--vela-linux", type=pathlib.Path, required=True)
    result.add_argument("--model", required=True)
    result.add_argument("--codex-version", required=True)
    result.add_argument("--job-name", default="vela-erdos-264-proof-repair")
    result.add_argument("--output", type=pathlib.Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    try:
        plan = materialize(parser().parse_args(argv))
    except MaterializationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(
        canonical_bytes(
            {"ok": True, "plan_root": plan["plan_root"], "writes_frontier": False}
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
