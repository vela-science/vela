#!/usr/bin/env python3
"""Freeze the exact read-only baseline for the action-complete campaign."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any, Sequence

import contract
import materialize


CANONICAL_FRONTIERS = (
    "erdos",
    "formal-conjectures",
    "quantum-codes",
    "sidon-sets",
)


def fail(message: str) -> None:
    raise contract.ContractError(message)


def command(argv: Sequence[str], *, cwd: Path | None = None) -> str:
    try:
        result = subprocess.run(argv, cwd=cwd, check=False, capture_output=True, text=True)
    except OSError as exc:
        raise contract.ContractError(f"cannot execute {argv[0]}: {exc}") from exc
    if result.returncode != 0:
        fail(f"command failed ({result.returncode}): {' '.join(argv)}: {result.stderr.strip()}")
    return result.stdout.strip()


def json_command(argv: Sequence[str], *, cwd: Path) -> dict[str, Any]:
    try:
        value = json.loads(command(argv, cwd=cwd))
    except json.JSONDecodeError as exc:
        raise contract.ContractError(f"command returned invalid JSON: {' '.join(argv)}: {exc}") from exc
    if not isinstance(value, dict):
        fail(f"command returned non-object JSON: {' '.join(argv)}")
    return value


def digest(path: Path) -> str:
    try:
        return contract.sha256_root(path.read_bytes())
    except OSError as exc:
        raise contract.ContractError(f"cannot hash {path}: {exc}") from exc


def git_identity(repository: Path) -> dict[str, str]:
    repository = repository.resolve()
    if command(("git", "status", "--porcelain"), cwd=repository):
        fail(f"repository checkout must be clean: {repository}")
    return {
        "git_commit": command(("git", "rev-parse", "HEAD"), cwd=repository),
        "git_tree": command(("git", "rev-parse", "HEAD^{tree}"), cwd=repository),
        "remote": command(("git", "remote", "get-url", "origin"), cwd=repository),
    }


def stable_status(status: dict[str, Any]) -> dict[str, Any]:
    return {
        "frontier": status.get("frontier"),
        "git": status.get("git"),
        "integrity": status.get("integrity"),
        "roots": status.get("roots"),
        "counts": status.get("counts"),
        "work": status.get("work"),
        "decision_inbox": status.get("decision_inbox"),
    }


def stable_offer(offer: dict[str, Any]) -> dict[str, Any]:
    targets = []
    for target in offer.get("targets", []):
        targets.append({
            field: target.get(field)
            for field in (
                "rank", "lane", "target_id", "title", "objective", "why",
                "labels", "packet", "verifier_profile",
            )
        })
    return {
        field: offer.get(field)
        for field in (
            "frontier_id", "origin_id", "repository_root", "target_index_root",
            "availability", "next_action",
        )
    } | {"targets": targets}


def frontier_identity(slug: str, repository: Path, vela: Path) -> dict[str, Any]:
    repository = repository.resolve()
    git = git_identity(repository)
    status = json_command((str(vela), "status", str(repository), "--json"), cwd=repository)
    offer = json_command((str(vela), "next", str(repository), "--limit", "1", "--json"), cwd=repository)
    if command(("git", "status", "--porcelain"), cwd=repository):
        fail(f"read-only inspection changed the Frontier checkout: {slug}")
    if status.get("git") != {"commit": git["git_commit"], "tree": git["git_tree"]}:
        fail(f"status and Git identity disagree: {slug}")
    roots = status.get("roots")
    integrity = status.get("integrity")
    if not isinstance(roots, dict) or not isinstance(integrity, dict):
        fail(f"status lacks roots or integrity: {slug}")
    if offer.get("frontier_id") != status.get("frontier", {}).get("id") or offer.get("repository_root") != roots.get("repository"):
        fail(f"status and next disagree: {slug}")

    stable_status_value = stable_status(status)
    stable_offer_value = stable_offer(offer)
    result: dict[str, Any] = {
        "slug": slug,
        **git,
        "frontier_id": offer["frontier_id"],
        "repository_root": roots["repository"],
        "status_root": contract.sha256_root(contract.canonical_bytes(stable_status_value)),
        "offer_root": contract.sha256_root(contract.canonical_bytes(stable_offer_value)),
        "integrity": {
            "replay": integrity.get("replay"),
            "strict": integrity.get("strict"),
            "blocker_count": integrity.get("blocker_count"),
        },
        "availability": offer.get("availability"),
        "counts": status.get("counts"),
    }
    targets = offer.get("targets")
    if slug == "erdos":
        if not isinstance(targets, list) or len(targets) != 1:
            fail("Erdős must expose exactly one current Target")
        target = targets[0]
        packet = target.get("packet")
        if not isinstance(packet, dict):
            fail("Erdős Target has no packet")
        packet_path = repository / str(packet.get("path", ""))
        if digest(packet_path) != packet.get("sha256"):
            fail("Erdős Target packet bytes disagree with the offer")
        packet_value = contract.read_json(packet_path)
        try:
            next_range = packet_value["target"]["next_bounded_range"]
        except (KeyError, TypeError) as exc:
            raise contract.ContractError(f"Erdős Target packet lacks next range: {exc}") from exc
        result["target"] = {
            "target_id": target.get("target_id"),
            "packet_root": packet.get("sha256"),
            "verifier_profile": target.get("verifier_profile"),
            "next_range": next_range,
        }
    else:
        if targets != []:
            fail(f"{slug} unexpectedly exposes a Target")
        result["next_action"] = offer.get("next_action")
    return result


def observatory_identity(path: Path, frontiers: list[dict[str, Any]]) -> dict[str, Any]:
    manifest = contract.read_json(path)
    try:
        projection = manifest["projection"]
        site = manifest["site"]
        projected_frontiers = projection["source_frontiers"]
    except (KeyError, TypeError) as exc:
        raise contract.ContractError(f"Observatory manifest is incomplete: {exc}") from exc
    by_slug = {item.get("slug"): item for item in projected_frontiers if isinstance(item, dict)}
    rows = []
    for frontier in frontiers:
        projected = by_slug.get(frontier["slug"])
        if not isinstance(projected, dict):
            fail(f"Observatory omits Frontier: {frontier['slug']}")
        row = {
            "slug": frontier["slug"],
            "git_commit": projected.get("commit"),
            "git_tree": projected.get("tree"),
            "repository_root": projected.get("repository_root"),
        }
        if any(row[name] != frontier[name] for name in ("git_commit", "git_tree", "repository_root")):
            fail(f"Observatory source drift: {frontier['slug']}")
        rows.append(row)
    return {
        "url": "https://app.vela.space/.well-known/vela-site.json",
        "manifest_sha256": digest(path),
        "schema": manifest.get("schema"),
        "authority": manifest.get("authority"),
        "site_version": site.get("version"),
        "site_commit": site.get("commit"),
        "projection_schema": projection.get("schema"),
        "read_model_schema": projection.get("read_model_schema"),
        "projection_root": projection.get("release_root"),
        "vela_version": projection.get("vela_version"),
        "vela_binary_sha256": projection.get("vela_binary_sha256"),
        "frontiers": rows,
    }


def contract_roots(directory: Path) -> dict[str, str]:
    files = {
        "contract": directory / "contract.py",
        "freezer": directory / "freeze_campaign.py",
        "materializer": directory / "materialize.py",
        "summary": directory / "summarize.py",
        "answer_schema": directory / "answer.schema.json",
    }
    roots = {name: digest(path) for name, path in files.items()}
    roots["task_template"] = materialize.tree_root(directory / "task")
    return roots


def freeze(
    vela_repository: Path,
    vela: Path,
    frontiers: dict[str, Path],
    observatory_manifest: Path,
    observed_at: str,
) -> dict[str, Any]:
    vela_repository, vela = vela_repository.resolve(), vela.resolve()
    vela_git = git_identity(vela_repository)
    if not vela.is_file():
        fail(f"Vela binary does not exist: {vela}")
    version = command((str(vela), "--version"), cwd=vela_repository)
    frontier_rows = [frontier_identity(slug, frontiers[slug], vela) for slug in CANONICAL_FRONTIERS]
    observatory = observatory_identity(observatory_manifest, frontier_rows)
    if observatory["vela_version"] != version:
        fail("Observatory and campaign binary versions disagree")
    directory = Path(__file__).resolve().parent
    result = contract.seal({
        "schema": "vela.action-complete-campaign-baseline.v1",
        "baseline_root": "",
        "observed_at": observed_at,
        "source_state": {
            "vela": {
                "version": version,
                "binary_sha256": digest(vela),
                "source_identity_root": contract.sha256_root(contract.canonical_bytes(vela_git)),
                **vela_git,
            },
            "harbor": {"version": command(("harbor", "--version"), cwd=vela_repository)},
            "frontiers": frontier_rows,
            "observatory": observatory,
        },
        "benchmark": {
            "implementation": "native_harbor",
            "arms": ["git-files", "vela-guided"],
            "task_classes": list(contract.ACTION_COMPLETE_TASK_CLASSES),
            "contract_roots": contract_roots(directory),
            "custody": {
                "answer_key_available_to_agent": False,
                "authority_credentials_available_to_agent": False,
                "canonical_checkout_mutable": False,
                "automatic_decision": False,
            },
            "instrumentation_pilot": {"repetitions_per_arm": 2, "claim_credit": False},
            "confirmatory_design": {
                "power": 0.8,
                "two_sided_alpha": 0.05,
                "minimum_useful_effect": 0.2,
                "sample_size_rule": "computed_from_blinded_pilot_variance",
            },
            "primary_metrics": ["ETY", "VPAC", "FIE", "CPI", "correction_resilience"],
        },
        "limitations": [
            "This baseline binds current source and reader state; it contains no model output and earns no performance claim.",
            "The controlled correction task is a closed-ground-truth product benchmark, not a real downstream Frontier correction.",
            "Harbor execution evidence and passing Verification never select or imply a scientific Decision.",
        ],
    }, "baseline_root")
    contract.validate_action_complete_baseline(result)
    return result


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--vela-repository", type=Path, required=True)
    result.add_argument("--vela", type=Path, required=True)
    result.add_argument("--frontier", action="append", default=[], metavar="SLUG=PATH")
    result.add_argument("--observatory-manifest", type=Path, required=True)
    result.add_argument("--observed-at", required=True)
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        frontiers: dict[str, Path] = {}
        for item in args.frontier:
            if "=" not in item:
                fail(f"invalid --frontier value: {item}")
            slug, path = item.split("=", 1)
            if slug in frontiers:
                fail(f"duplicate Frontier: {slug}")
            frontiers[slug] = Path(path)
        if set(frontiers) != set(CANONICAL_FRONTIERS):
            fail("--frontier must name erdos, formal-conjectures, quantum-codes, and sidon-sets exactly once")
        result = freeze(
            args.vela_repository,
            args.vela,
            frontiers,
            args.observatory_manifest,
            args.observed_at,
        )
        contract.write_json(args.output, result)
        sys.stdout.buffer.write(contract.canonical_bytes({
            "ok": True,
            "baseline_root": result["baseline_root"],
            "writes_frontiers": False,
            "output": str(args.output),
        }))
        return 0
    except contract.ContractError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
