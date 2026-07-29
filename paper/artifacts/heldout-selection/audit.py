#!/usr/bin/env python3
"""Audit the frozen held-out correction selection rule against exact Git heads."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import subprocess
import sys
import tarfile
from pathlib import Path
from typing import Any

PLAN_ROOT = "sha256:b9dbf4b86b841b7b09a79e865ae0187a3ed6dcead896cc2446edcacb836af6a8"
PLAN_BYTES_SHA256 = "sha256:4efaa34ebd61738ec111ba7245afe8d8ea270b1be7194541f92b8622be386535"
CORRECTION_KINDS = frozenset({"corrects", "narrows", "retracts", "supersedes"})
WRITER_QUALIFICATION = {
    "predecessor": "vcl_5d2858542f6882556bb7652c908708913fadd7ced61014cd5842ae0954ddfe09",
    "successor": "vcl_4bc14401b203218cb7b9de0141747e0c17cea3a6b0cc522639323ab13e432eaf",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def digest(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def git(repo: Path, *args: str, binary: bool = False) -> bytes | str:
    result = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=not binary,
    )
    return result.stdout


def git_text(repo: Path, *args: str) -> str:
    result = git(repo, *args)
    assert isinstance(result, str)
    return result.strip()


def git_json(repo: Path, revision: str, path: str) -> dict[str, Any]:
    encoded = git(repo, "show", f"{revision}:{path}", binary=True)
    assert isinstance(encoded, bytes)
    value = json.loads(encoded)
    require(isinstance(value, dict), f"{revision}:{path} must contain one JSON object")
    return value


def repository_at(repo: Path, revision: str) -> dict[str, Any]:
    value = git_json(repo, revision, ".vela/repository.json")
    require(
        value.get("schema") in {"vela.repository.v2", "vela.repository.v3"},
        f"{repo.name}@{revision} is not a supported Vela repository epoch",
    )
    return value


def accepted_claims(repository: dict[str, Any]) -> dict[str, dict[str, Any]]:
    references = repository.get("accepted_claims")
    require(isinstance(references, list), "repository accepted_claims must be a list")
    result: dict[str, dict[str, Any]] = {}
    for reference in references:
        require(isinstance(reference, dict), "accepted Claim reference must be an object")
        claim_id = reference.get("claim_id")
        require(isinstance(claim_id, str), "accepted Claim reference has no claim_id")
        require(claim_id not in result, f"duplicate accepted Claim {claim_id}")
        result[claim_id] = reference
    return result


def claim_at(
    repo: Path,
    revision: str,
    reference: dict[str, Any],
) -> dict[str, Any]:
    path = reference.get("path")
    root = reference.get("claim_root")
    require(isinstance(path, str), "accepted Claim reference has no path")
    require(isinstance(root, str), "accepted Claim reference has no claim_root")
    encoded = git(repo, "show", f"{revision}:{path}", binary=True)
    assert isinstance(encoded, bytes)
    require(digest(encoded) == root, f"{revision}:{path} does not match {root}")
    value = json.loads(encoded)
    require(isinstance(value, dict), f"{revision}:{path} must contain one Claim object")
    require(value.get("claim_id") == reference.get("claim_id"), f"{path} Claim ID mismatch")
    return value


def accepted_claim_objects(
    repo: Path,
    revision: str,
    references: dict[str, dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    archive = git(
        repo,
        "archive",
        "--format=tar",
        revision,
        "records/claims/sha256",
        binary=True,
    )
    assert isinstance(archive, bytes)
    by_path: dict[str, bytes] = {}
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as bundle:
        for member in bundle.getmembers():
            if member.isfile():
                source = bundle.extractfile(member)
                require(source is not None, f"cannot read archived Claim {member.name}")
                by_path[member.name] = source.read()
    claims: dict[str, dict[str, Any]] = {}
    for claim_id, reference in references.items():
        path = reference.get("path")
        root = reference.get("claim_root")
        require(isinstance(path, str) and path in by_path, f"missing accepted Claim {claim_id}")
        encoded = by_path[path]
        require(isinstance(root, str) and digest(encoded) == root, f"Claim root mismatch: {claim_id}")
        value = json.loads(encoded)
        require(value.get("claim_id") == claim_id, f"Claim identity mismatch: {claim_id}")
        claims[claim_id] = value
    return claims


def compaction_predecessor(repo: Path, head: str, baseline: str) -> str | None:
    origin = git_json(repo, head, ".vela/origin.json")
    predecessor = origin.get("predecessor")
    if isinstance(predecessor, dict):
        candidate = predecessor.get("commit")
        if isinstance(candidate, str):
            is_after = subprocess.run(
                ["git", "-C", str(repo), "merge-base", "--is-ancestor", baseline, candidate],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            ).returncode == 0
            is_before_head = subprocess.run(
                ["git", "-C", str(repo), "merge-base", "--is-ancestor", candidate, head],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            ).returncode == 0
            if is_after and is_before_head:
                return candidate
    return None


def new_authority_events(repo: Path, parent: str, commit: str) -> list[dict[str, Any]]:
    paths = git_text(
        repo,
        "diff-tree",
        "--no-commit-id",
        "--name-only",
        "--diff-filter=AM",
        "-r",
        parent,
        commit,
        "--",
        ".vela/authority/events",
    ).splitlines()
    events = []
    for path in paths:
        if path.endswith(".json"):
            events.append(git_json(repo, commit, path))
    return events


def decision_for_claim(events: list[dict[str, Any]], claim_id: str) -> dict[str, Any] | None:
    applied = []
    decisions = []
    for event in events:
        content = event.get("content")
        if not isinstance(content, dict):
            continue
        payload = content.get("payload")
        if not isinstance(payload, dict):
            continue
        if payload.get("claim_id") == claim_id and content.get("kind") in {
            "finding.corrected",
            "finding.retracted",
            "finding.superseded",
        }:
            applied.append(event)
        if content.get("kind") == "review.accepted":
            decisions.append(event)
    for applied_event in applied:
        proposal_id = applied_event["content"]["payload"].get("proposal_id")
        for decision in decisions:
            if decision["content"]["payload"].get("proposal_id") == proposal_id:
                return {
                    "proposal_id": proposal_id,
                    "applied_event_id": applied_event.get("id"),
                    "decision_event_id": decision.get("id"),
                    "recorded_at": decision["content"].get("timestamp"),
                }
    return None


def topology(
    claims: dict[str, dict[str, Any]],
    predecessor: str,
) -> dict[str, Any]:
    relations: list[dict[str, str]] = []
    for source_id, claim in claims.items():
        for relation in claim.get("relations", []):
            if not isinstance(relation, dict):
                continue
            kind = relation.get("kind")
            target = relation.get("target_claim_id")
            if isinstance(kind, str) and isinstance(target, str):
                relations.append({"source": source_id, "kind": kind, "target": target})
    incoming = [relation for relation in relations if relation["target"] == predecessor]
    hard_dependents = sorted(
        relation["source"] for relation in incoming if relation["kind"] == "depends_on"
    )
    support_sources = {
        relation["source"] for relation in incoming if relation["kind"] == "supports"
    }
    support_diamonds = sorted(
        source
        for source in support_sources
        if any(
            relation["source"] == source
            and relation["kind"] == "supports"
            and relation["target"] != predecessor
            and relation["target"] in claims
            for relation in relations
        )
    )
    discovery_only = sorted(
        relation["source"] for relation in incoming if relation["kind"] == "discovery"
    )
    return {
        "incoming_relation_count": len(incoming),
        "hard_dependents": hard_dependents,
        "support_diamonds": support_diamonds,
        "nonconsequential_discovery_relations": discovery_only,
    }


def audit_frontier(
    repo: Path,
    repository_url: str,
    baseline: str,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    require(not git_text(repo, "status", "--porcelain"), f"{repo} is dirty")
    head = git_text(repo, "rev-parse", "HEAD")
    tree = git_text(repo, "rev-parse", "HEAD^{tree}")
    require(
        subprocess.run(
            ["git", "-C", str(repo), "merge-base", "--is-ancestor", baseline, head],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        ).returncode
        == 0,
        f"{baseline} is not an ancestor of {repo}@{head}",
    )
    compaction_origin_commit = compaction_predecessor(repo, head, baseline)
    commits = git_text(
        repo,
        "rev-list",
        "--reverse",
        "--first-parent",
        f"{baseline}..{head}",
    ).splitlines()
    previous_commit = baseline
    previous_claims = accepted_claims(repository_at(repo, baseline))
    candidates: list[dict[str, Any]] = []
    for commit in commits:
        current_claims = accepted_claims(repository_at(repo, commit))
        added = sorted(set(current_claims) - set(previous_claims))
        events = new_authority_events(repo, previous_commit, commit)
        for claim_id in added:
            claim = claim_at(repo, commit, current_claims[claim_id])
            correction_relations = [
                relation
                for relation in claim.get("relations", [])
                if isinstance(relation, dict)
                and relation.get("kind") in CORRECTION_KINDS
                and relation.get("target_claim_id") in previous_claims
            ]
            for relation in correction_relations:
                predecessor_claim_id = relation["target_claim_id"]
                decision = decision_for_claim(events, claim_id)
                all_claims = accepted_claim_objects(repo, commit, current_claims)
                observed_topology = topology(all_claims, predecessor_claim_id)
                rejections = []
                if decision is None:
                    rejections.append("missing_terminal_accepted_decision")
                if (
                    predecessor_claim_id == WRITER_QUALIFICATION["predecessor"]
                    and claim_id == WRITER_QUALIFICATION["successor"]
                ):
                    rejections.append("overlaps_writer_qualification_case")
                if not observed_topology["hard_dependents"]:
                    rejections.append("no_hard_dependent")
                if not observed_topology["support_diamonds"]:
                    rejections.append("no_support_diamond")
                if not observed_topology["nonconsequential_discovery_relations"]:
                    rejections.append("no_nonconsequential_relation")
                candidates.append(
                    {
                        "frontier_id": repository_at(repo, commit)["frontier_id"],
                        "commit": commit,
                        "successor_claim_id": claim_id,
                        "successor_claim_root": current_claims[claim_id]["claim_root"],
                        "transition_kind": relation["kind"],
                        "predecessor_claim_id": predecessor_claim_id,
                        "predecessor_claim_root": previous_claims[predecessor_claim_id][
                            "claim_root"
                        ],
                        "decision": decision,
                        "topology": observed_topology,
                        "rejections": rejections,
                        "automated_gate_pass": not rejections,
                        "qualification_status": (
                            "rejected"
                            if rejections
                            else "requires_scientific_identity_and_removability_qualification"
                        ),
                        "eligible": False,
                    }
                )
        previous_commit = commit
        previous_claims = current_claims
    return (
        {
            "repository": repository_url,
            "baseline": baseline,
            "head": head,
            "tree": tree,
            "scan_end": head,
            "compaction_predecessor": compaction_origin_commit,
            "commits_scanned": len(commits),
        },
        candidates,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--repos-root",
        type=Path,
        default=Path(__file__).resolve().parents[4],
        help="Directory containing the four Frontier repositories",
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    plan_path = here / "plan.v1.json"
    try:
        plan_bytes = plan_path.read_bytes()
        require(digest(plan_bytes) == PLAN_BYTES_SHA256, "frozen plan bytes changed")
        plan = json.loads(plan_bytes)
        frontiers = []
        candidates = []
        for entry in plan["frontier_baselines"]:
            repository_name = entry["repository"].removesuffix(".git").rsplit("/", 1)[-1]
            repo = args.repos_root.expanduser().resolve() / repository_name
            require(repo.is_dir(), f"missing Frontier repository: {repo}")
            observation, found = audit_frontier(repo, entry["repository"], entry["commit"])
            frontiers.append(observation)
            candidates.extend(found)
        candidates.sort(
            key=lambda candidate: (
                candidate.get("decision", {}).get("recorded_at", "")
                if candidate.get("decision")
                else "",
                candidate["frontier_id"],
                candidate["successor_claim_id"],
            )
        )
        requires_qualification = any(
            candidate["automated_gate_pass"] for candidate in candidates
        )
        selected = None
        result = {
            "schema": "vela.heldout-correction-selection-result.v1",
            "plan_root": PLAN_ROOT,
            "plan_bytes_sha256": PLAN_BYTES_SHA256,
            "outcome": (
                "candidate_requires_qualification"
                if requires_qualification
                else "no_qualifying_candidate"
            ),
            "frontiers": frontiers,
            "candidate_count": len(candidates),
            "candidates": candidates,
            "selected": selected,
            "standing_effect": "none",
            "next_action": (
                "Qualify exact source identity, relation semantics, non-invention, and removability before selecting a candidate."
                if requires_qualification
                else "Record the failed held-out entry gate; do not substitute a synthetic case."
            ),
        }
        encoded = f"{json.dumps(result, sort_keys=True, separators=(',', ':'))}\n"
        if args.output:
            require(not args.output.exists(), "output already exists")
            args.output.write_text(encoded, encoding="utf-8")
        else:
            print(encoded, end="")
        return 0
    except (
        KeyError,
        OSError,
        ValueError,
        subprocess.CalledProcessError,
        json.JSONDecodeError,
        tarfile.TarError,
    ) as error:
        detail = (
            error.stderr.decode().strip()
            if isinstance(error, subprocess.CalledProcessError)
            and isinstance(error.stderr, bytes)
            else error.stderr.strip()
            if isinstance(error, subprocess.CalledProcessError) and error.stderr
            else str(error)
        )
        print(f"held-out selection failed: {detail}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
