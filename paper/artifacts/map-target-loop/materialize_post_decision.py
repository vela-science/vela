#!/usr/bin/env python3
"""Materialize the read-only Decision -> remap evidence for the live loop.

The command fails closed until the exact Proposal has a terminal human
Decision. It never invokes a Decision, writes a Frontier, pushes Git, changes
the Observatory release pointer, or reads a signing key.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ARTIFACT_DIR = Path(__file__).resolve().parent
PRE_RUN = ARTIFACT_DIR / "pre-run.v1.json"
POST_VERIFICATION = ARTIFACT_DIR / "post-verification.v1.json"
POST_VERIFICATION_MAP = ARTIFACT_DIR / "post-verification-map.v1.json"
DECISION_PACKET = ARTIFACT_DIR / "decision-packet.v1.json"

TERMINAL_STANDINGS = {"accepted", "rejected"}
ALLOWED_EXACT_PATHS = {".vela/repository.json", "targets.json"}
ALLOWED_PREFIXES = {
    ".vela/authority/events/": ".json",
    ".vela/authority/records/": ".dsse.json",
}


class MaterializeError(ValueError):
    """Raised when the observed terminal loop is not exact."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise MaterializeError(message)


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def pretty_bytes(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    try:
        return sha256_bytes(path.read_bytes())
    except OSError as error:
        raise MaterializeError(f"cannot read {path}: {error}") from error


def load_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise MaterializeError(f"cannot parse {path}: {error}") from error
    require(isinstance(value, dict), f"{path} must contain one JSON object")
    return value


def run(
    argv: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> str:
    completed = subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode != 0:
        detail = (completed.stderr.strip() or completed.stdout.strip())[:1200]
        raise MaterializeError(
            f"command failed ({completed.returncode}): {' '.join(argv)}: {detail}"
        )
    return completed.stdout


def run_json(
    argv: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> dict[str, Any]:
    output = run(argv, cwd=cwd, env=env)
    try:
        value = json.loads(output)
    except json.JSONDecodeError as error:
        raise MaterializeError(
            f"command did not emit one JSON object: {' '.join(argv)}"
        ) from error
    require(isinstance(value, dict), f"{' '.join(argv)} output must be an object")
    return value


def git_text(repository: Path, *args: str) -> str:
    return run(["git", *args], cwd=repository).strip()


def require_clean_synced(repository: Path, label: str) -> tuple[str, str]:
    require(repository.is_dir(), f"{label} repository is missing")
    require(
        not git_text(repository, "status", "--porcelain=v1", "--untracked-files=all"),
        f"{label} repository is dirty",
    )
    head = git_text(repository, "rev-parse", "HEAD")
    remote = git_text(repository, "rev-parse", "origin/main")
    require(head == remote, f"{label} HEAD does not equal origin/main")
    tree = git_text(repository, "rev-parse", "HEAD^{tree}")
    return head, tree


def validate_frozen_inputs(
    pre: dict[str, Any],
    post: dict[str, Any],
    checkpoint: dict[str, Any],
    packet: dict[str, Any],
) -> None:
    require(
        pre.get("schema") == "vela.map-target-loop-pre-run.v1", "pre-run schema drift"
    )
    require(
        post.get("schema") == "vela.map-target-loop-post-verification.v1",
        "post-Verification schema drift",
    )
    require(
        checkpoint.get("schema") == "vela.map-target-loop-post-verification-map.v1",
        "post-Verification map schema drift",
    )
    require(
        packet.get("schema") == "vela.key-free-human-decision-packet.v1",
        "Decision packet schema drift",
    )
    require(
        post.get("status") == "verified_pending_human_decision",
        "post-Verification state is not awaiting a human Decision",
    )
    require(
        checkpoint.get("status") == "verified_candidate_not_activated",
        "post-Verification map checkpoint has the wrong status",
    )
    require(
        checkpoint.get("candidate", {}).get("activated") is False,
        "post-Verification map candidate was recorded as activated",
    )
    require(
        packet.get("status") == "prepared_not_invoked",
        "Decision packet is not the frozen key-free packet",
    )

    producer = post.get("producer", {})
    proposal = packet.get("proposal", {})
    claim = packet.get("claim", {})
    verification = packet.get("verification", {})
    require(
        proposal.get("proposal_id") == producer.get("proposal_id"),
        "Decision packet Proposal ID drift",
    )
    require(
        proposal.get("proposal_root") == producer.get("proposal_root"),
        "Decision packet Proposal root drift",
    )
    require(
        claim.get("claim_id") == producer.get("claim_id"),
        "Decision packet Claim ID drift",
    )
    require(
        claim.get("claim_root") == producer.get("claim_root"),
        "Decision packet Claim root drift",
    )
    require(
        verification.get("record_id") == post.get("verification", {}).get("record_id"),
        "Decision packet Verification ID drift",
    )
    require(
        verification.get("record_root")
        == post.get("verification", {}).get("record_root"),
        "Decision packet Verification root drift",
    )
    require(
        checkpoint.get("frontier", {}).get("commit")
        == post.get("frontier", {}).get("commit"),
        "post-Verification map commit drift",
    )
    require(
        checkpoint.get("frontier", {}).get("repository_root")
        == post.get("frontier", {}).get("repository_root"),
        "post-Verification map repository root drift",
    )
    require(
        checkpoint.get("released_vela", {}).get("binary_sha256")
        == post.get("released_vela", {}).get("binary_sha256"),
        "post-Verification map Vela binary drift",
    )


def validate_status(
    status: dict[str, Any],
    post: dict[str, Any],
) -> dict[str, Any]:
    require(status.get("schema") == "vela.status.v1", "status schema drift")
    require(status.get("ok") is True, "status did not pass")
    require(
        status.get("frontier", {}).get("id")
        == post.get("frontier", {}).get("frontier_id"),
        "Frontier ID drift",
    )
    integrity = status.get("integrity", {})
    require(
        integrity.get("replay") == "verified",
        "Frontier replay is not verified",
    )
    require(integrity.get("strict") == "pass", "Frontier strict state is not passing")
    require(integrity.get("blocker_count") == 0, "Frontier has strict blockers")
    roots = status.get("roots", {})
    counts = status.get("counts", {})
    require(isinstance(roots, dict), "status roots are missing")
    require(isinstance(counts, dict), "status counts are missing")
    count_fields = (
        "accepted_claims",
        "pending_claims",
        "pending_review",
        "accepted_review",
        "rejected_review",
    )
    require(
        all(
            isinstance(counts.get(field), int) and counts[field] >= 0
            for field in count_fields
        ),
        "status carries invalid counts",
    )
    return {
        "commit": status.get("git", {}).get("commit"),
        "tree": status.get("git", {}).get("tree"),
        "repository_root": roots.get("repository"),
        "origin_root": roots.get("origin"),
        "authority_keyset_root": roots.get("authority_keyset"),
        "authority_policy_root": roots.get("authority_policy"),
        "accepted_claims": counts.get("accepted_claims"),
        "pending_claims": counts.get("pending_claims"),
        "pending_review": counts.get("pending_review"),
        "accepted_review": counts.get("accepted_review"),
        "rejected_review": counts.get("rejected_review"),
    }


def validate_review(
    review: dict[str, Any],
    post: dict[str, Any],
) -> dict[str, Any]:
    producer = post["producer"]
    expected_verification = post["verification"]
    require(review.get("schema") == "vela.review.v1", "review schema drift")
    require(review.get("ok") is True, "review inspection did not pass")
    require(
        review.get("frontier_id") == post["frontier"]["frontier_id"],
        "review Frontier ID drift",
    )
    require(
        review.get("proposal_id") == producer["proposal_id"],
        "review Proposal ID drift",
    )
    require(
        review.get("proposal_root") == producer["proposal_root"],
        "review Proposal root drift",
    )
    require(
        review.get("claim", {}).get("claim_id") == producer["claim_id"],
        "review Claim ID drift",
    )
    require(
        review.get("proposal", {}).get("subject", {}).get("root")
        == producer["claim_root"],
        "review Claim root drift",
    )
    records = review.get("verification_records")
    require(isinstance(records, list), "review Verification list is missing")
    matches = [
        item
        for item in records
        if item.get("verification_record_root") == expected_verification["record_root"]
        and item.get("record", {}).get("verification_record_id")
        == expected_verification["record_id"]
        and item.get("record", {}).get("outcome") == expected_verification["outcome"]
    ]
    require(len(matches) == 1, "exact frozen Verification is not retained once")

    standing = review.get("standing")
    require(
        standing in TERMINAL_STANDINGS,
        "Proposal remains pending; a human Decision is required before remapping",
    )
    decision = review.get("decision")
    require(isinstance(decision, dict), "terminal Decision record is missing")
    require(decision.get("standing") == standing, "Decision standing drift")
    require(
        isinstance(decision.get("reason"), str) and decision["reason"].strip(),
        "Decision reason is missing",
    )
    require(
        isinstance(decision.get("decided_at"), str) and decision["decided_at"].strip(),
        "Decision time is missing",
    )
    require(
        isinstance(decision.get("event_id"), str) and decision["event_id"],
        "Decision event ID is missing",
    )
    require(
        isinstance(decision.get("event_root"), str) and decision["event_root"],
        "Decision event root is missing",
    )
    applied = decision.get("applied_event_id")
    if standing == "accepted":
        require(
            isinstance(applied, str) and applied,
            "accepted Decision lacks applied event",
        )
    else:
        require(
            applied is None, "rejected Decision unexpectedly names an applied event"
        )
    return {
        "action": "accept" if standing == "accepted" else "reject",
        "standing": standing,
        "event_id": decision.get("event_id"),
        "event_root": decision.get("event_root"),
        "applied_event_id": applied,
        "decided_at": decision.get("decided_at"),
        "actor": decision.get("actor"),
        "reason": decision.get("reason"),
    }


def changed_paths(frontier: Path, base_commit: str, head: str) -> list[dict[str, str]]:
    require(
        git_text(frontier, "merge-base", "--is-ancestor", base_commit, head) == "",
        "post-Verification commit is not an ancestor of the Decision commit",
    )
    count = int(git_text(frontier, "rev-list", "--count", f"{base_commit}..{head}"))
    require(
        count == 1, "Decision evidence must be exactly one commit after Verification"
    )
    output = git_text(
        frontier,
        "diff",
        "--name-status",
        "--no-renames",
        base_commit,
        head,
    )
    rows: list[dict[str, str]] = []
    for line in output.splitlines():
        change, path = line.split("\t", 1)
        allowed = path in ALLOWED_EXACT_PATHS or any(
            path.startswith(prefix) and path.endswith(suffix)
            for prefix, suffix in ALLOWED_PREFIXES.items()
        )
        require(allowed, f"Decision commit changed unrelated path {path}")
        expected_change = (
            "A"
            if path.startswith(".vela/authority/events/")
            or path.startswith(".vela/authority/records/")
            else "M"
        )
        require(
            change == expected_change,
            f"Decision commit used forbidden change {change} for {path}",
        )
        rows.append({"change": change, "path": path})
    paths = {row["path"] for row in rows}
    require(
        ".vela/repository.json" in paths, "Decision did not update repository manifest"
    )
    require(
        any(path.startswith(".vela/authority/events/") for path in paths),
        "Decision added no authority event",
    )
    require(
        any(path.startswith(".vela/authority/records/") for path in paths),
        "Decision added no covering authority record",
    )
    return rows


def read_authority_evidence(
    frontier: Path,
    paths: list[dict[str, str]],
    decision: dict[str, Any],
) -> dict[str, Any]:
    record_paths = [
        frontier / row["path"]
        for row in paths
        if row["path"].startswith(".vela/authority/records/")
    ]
    event_paths = [
        frontier / row["path"]
        for row in paths
        if row["path"].startswith(".vela/authority/events/")
    ]
    require(len(record_paths) == 1, "Decision must add exactly one authority record")
    envelope = load_object(record_paths[0])
    require(
        envelope.get("payloadType") == "application/vnd.vela.authority-record.v1+json",
        "authority record has unexpected DSSE payload type",
    )
    try:
        payload_bytes = base64.b64decode(envelope["payload"], validate=True)
        payload = json.loads(payload_bytes)
    except (KeyError, ValueError, json.JSONDecodeError) as error:
        raise MaterializeError(f"invalid authority record envelope: {error}") from error
    require(isinstance(payload, dict), "authority record payload is not an object")
    require(
        canonical_bytes(payload) == payload_bytes,
        "authority record payload is not canonical JSON",
    )
    record_id = payload.get("record_id")
    require(
        record_paths[0].name == f"{record_id}.dsse.json",
        "authority record filename does not match its identity",
    )
    content = payload.get("content")
    require(isinstance(content, dict), "authority record content is missing")
    covered_ids = content.get("event_ids")
    require(
        isinstance(covered_ids, list)
        and covered_ids
        and all(isinstance(item, str) for item in covered_ids),
        "authority record event list is missing or invalid",
    )

    events: list[dict[str, Any]] = []
    for path in event_paths:
        value = load_object(path)
        event_id = value.get("id")
        require(path.name == f"{event_id}.json", "authority event filename drift")
        events.append(
            {
                "event_id": event_id,
                "event_root": sha256_bytes(canonical_bytes(value)),
                "kind": value.get("content", {}).get("kind"),
                "path": str(path.relative_to(frontier)),
            }
        )
    observed_ids = {event["event_id"] for event in events}
    require(set(covered_ids) == observed_ids, "authority record coverage drift")
    require(decision["event_id"] in observed_ids, "Decision event is not newly covered")
    if decision["applied_event_id"] is not None:
        require(
            decision["applied_event_id"] in observed_ids,
            "applied scientific event is not newly covered",
        )
    matching = [event for event in events if event["event_id"] == decision["event_id"]]
    require(
        len(matching) == 1 and matching[0]["event_root"] == decision["event_root"],
        "Decision event root does not match retained bytes",
    )
    return {
        "authority_record_id": record_id,
        "authority_record_root": sha256_bytes(canonical_bytes(payload)),
        "authority_record_path": str(record_paths[0].relative_to(frontier)),
        "authentication": content.get("authentication"),
        "operation_id": content.get("operation_id"),
        "transaction_id": content.get("transaction_id"),
        "events": sorted(events, key=lambda item: item["event_id"]),
    }


def validate_offer(offer: dict[str, Any], frontier_id: str) -> dict[str, Any]:
    require(offer.get("schema") == "vela.offer.v1", "offer schema drift")
    require(offer.get("frontier_id") == frontier_id, "offer Frontier ID drift")
    targets = offer.get("targets")
    require(isinstance(targets, list), "offer target list is missing")
    first = targets[0] if targets else None
    if first is None:
        return {
            "rank": None,
            "target_id": None,
            "target_index_root": offer.get("target_index_root"),
            "packet_root": None,
        }
    return {
        "rank": first.get("rank"),
        "target_id": first.get("target_id"),
        "target_index_root": offer.get("target_index_root"),
        "packet_root": first.get("packet", {}).get("sha256"),
    }


def validate_projection(
    projection: dict[str, Any],
    checkpoint: dict[str, Any],
    status: dict[str, Any],
) -> dict[str, Any]:
    require(projection.get("ok") is True, "projection dry-run did not pass")
    require(projection.get("dry_run") is True, "projection command was not a dry-run")
    require(projection.get("activated") is False, "projection dry-run activated state")
    require(
        projection.get("schema") == checkpoint.get("candidate", {}).get("schema"),
        "projection manifest schema drift",
    )
    require(
        projection.get("vela") == "vela 0.950.1",
        "projection used the wrong Vela version",
    )
    frontiers = projection.get("frontiers")
    require(isinstance(frontiers, list), "projection Frontier list is missing")
    matches = [frontier for frontier in frontiers if frontier.get("slug") == "erdos"]
    require(len(matches) == 1, "projection does not contain exactly one Erdős Frontier")
    frontier = matches[0]
    require(frontier.get("commit") == status["commit"], "projection commit drift")
    require(
        frontier.get("repository_root") == status["repository_root"],
        "projection repository root drift",
    )
    for field in (
        "graph_nodes",
        "graph_edges",
        "problems",
        "claims",
        "verifications",
    ):
        require(
            isinstance(frontier.get(field), int) and frontier[field] >= 0,
            f"projection {field} is invalid",
        )
    for field in ("graph_source_root", "graph_layout_root"):
        require(
            isinstance(frontier.get(field), str)
            and frontier[field].startswith("sha256:"),
            f"projection {field} is invalid",
        )
    return {
        "release_root": projection.get("release_root"),
        "schema": projection.get("schema"),
        "graph_source_root": frontier.get("graph_source_root"),
        "graph_layout_root": frontier.get("graph_layout_root"),
        "graph_node_count": frontier.get("graph_nodes"),
        "graph_edge_count": frontier.get("graph_edges"),
        "problem_count": frontier.get("problems"),
        "claim_count": frontier.get("claims"),
        "verification_count": frontier.get("verifications"),
    }


def root_delta(
    checkpoint: dict[str, Any],
    status: dict[str, Any],
    projection: dict[str, Any],
) -> dict[str, list[dict[str, Any]]]:
    before = checkpoint["frontier"]
    pairs = {
        "repository": (before["repository_root"], status["repository_root"]),
        "origin": (before["origin_root"], status["origin_root"]),
        "authority_keyset": (
            before["authority_keyset_root"],
            status["authority_keyset_root"],
        ),
        "authority_policy": (
            before["authority_policy_root"],
            status["authority_policy_root"],
        ),
        "graph_source": (
            before["graph_source_root"],
            projection["graph_source_root"],
        ),
        "graph_layout": (
            before["graph_layout_root"],
            projection["graph_layout_root"],
        ),
        "projection_release": (
            checkpoint["candidate"]["release_root"],
            projection["release_root"],
        ),
    }
    result: dict[str, list[dict[str, Any]]] = {"changed": [], "unchanged": []}
    for name, (prior, after) in pairs.items():
        item = {"name": name, "before": prior, "after": after}
        result["changed" if prior != after else "unchanged"].append(item)
    return result


def semantic_count_delta(
    standing: str,
    before: dict[str, Any],
    after: dict[str, Any],
) -> dict[str, int]:
    deltas = {
        "accepted_claim_count_delta": (
            after["accepted_claims"] - before["accepted_claim_count"]
        ),
        "pending_claim_count_delta": (
            after["pending_claims"] - before["pending_claim_count"]
        ),
        "pending_review_count_delta": (
            after["pending_review"] - before["pending_review_count"]
        ),
        "accepted_review_count_delta": (
            after["accepted_review"] - before["accepted_review_count"]
        ),
        "rejected_review_count_delta": (
            after["rejected_review"] - before["rejected_review_count"]
        ),
    }
    expected = (
        {
            "accepted_claim_count_delta": 1,
            "pending_claim_count_delta": -1,
            "pending_review_count_delta": -1,
            "accepted_review_count_delta": 1,
            "rejected_review_count_delta": 0,
        }
        if standing == "accepted"
        else {
            "accepted_claim_count_delta": 0,
            "pending_claim_count_delta": -1,
            "pending_review_count_delta": -1,
            "accepted_review_count_delta": 0,
            "rejected_review_count_delta": 1,
        }
    )
    require(deltas == expected, "Decision changed unexpected scientific-state counts")
    return deltas


def materialize(
    *,
    frontier: Path,
    vela: Path,
    vela_web: Path,
    frontiers_root: Path,
) -> dict[str, Any]:
    pre = load_object(PRE_RUN)
    post = load_object(POST_VERIFICATION)
    checkpoint = load_object(POST_VERIFICATION_MAP)
    packet = load_object(DECISION_PACKET)
    validate_frozen_inputs(pre, post, checkpoint, packet)

    frontier_head, frontier_tree = require_clean_synced(frontier, "Erdős Frontier")
    web_head, _ = require_clean_synced(vela_web, "vela-web")
    require(
        web_head == checkpoint["implementation"]["commit"],
        "vela-web implementation commit drift",
    )
    require(vela.is_file(), "released Vela binary is missing")
    require(
        sha256_file(vela) == post["released_vela"]["binary_sha256"],
        "released Vela binary root drift",
    )
    require(
        run([str(vela), "--version"]).strip()
        == f"vela {post['released_vela']['version']}",
        "released Vela version drift",
    )

    status_raw = run_json([str(vela), "status", str(frontier), "--json"])
    status = validate_status(status_raw, post)
    require(status["commit"] == frontier_head, "status Git commit drift")
    require(status["tree"] == frontier_tree, "status Git tree drift")
    review_raw = run_json(
        [
            str(vela),
            "review",
            "show",
            str(frontier),
            post["producer"]["proposal_id"],
            "--json",
        ]
    )
    decision = validate_review(review_raw, post)
    require(
        review_raw.get("repository_root") == status["repository_root"],
        "review and status repository roots disagree",
    )
    paths = changed_paths(frontier, post["frontier"]["commit"], frontier_head)
    authority = read_authority_evidence(frontier, paths, decision)
    offer = validate_offer(
        run_json([str(vela), "next", str(frontier), "--limit", "1", "--json"]),
        post["frontier"]["frontier_id"],
    )

    with tempfile.TemporaryDirectory(prefix="vela-map-decision-") as temporary:
        clone = Path(temporary) / "erdos-frontier"
        run(
            [
                "git",
                "clone",
                "--quiet",
                "--no-local",
                "--no-hardlinks",
                str(frontier),
                str(clone),
            ]
        )
        clone_status = validate_status(
            run_json([str(vela), "status", str(clone), "--json"]),
            post,
        )
        require(clone_status == status, "fresh-clone status drift")
        clone_review = validate_review(
            run_json(
                [
                    str(vela),
                    "review",
                    "show",
                    str(clone),
                    post["producer"]["proposal_id"],
                    "--json",
                ]
            ),
            post,
        )
        require(clone_review == decision, "fresh-clone Decision drift")

    projection_env = dict(os.environ)
    projection_env.update(
        {
            "VELA_FRONTIERS_ROOT": str(frontiers_root),
            "VELA_BIN": str(vela),
            "VELA_PROJECTION_DRY_RUN": "1",
        }
    )
    projection_raw = run_json(
        [
            "bun",
            "packages/frontier-data/scripts/refresh-neon-projection.mjs",
        ],
        cwd=vela_web,
        env=projection_env,
    )
    projection = validate_projection(projection_raw, checkpoint, status)

    before_counts = checkpoint["frontier"]
    count_delta = semantic_count_delta(decision["standing"], before_counts, status)
    target_before = {
        "rank": pre["target"]["rank"],
        "target_id": pre["target"]["target_id"],
        "target_index_root": pre["target"]["target_index_root"],
        "packet_root": pre["target"]["packet_sha256"],
    }
    return {
        "schema": "vela.map-target-loop-post-decision.v1",
        "status": "terminal_decision_replayed_and_remapped",
        "recorded_at": decision["decided_at"],
        "released_vela": post["released_vela"],
        "frontier": {
            "frontier_id": post["frontier"]["frontier_id"],
            "before_commit": post["frontier"]["commit"],
            "after_commit": frontier_head,
            "after_tree": frontier_tree,
            "before_repository_root": post["frontier"]["repository_root"],
            "after_repository_root": status["repository_root"],
            "clean_clone_replay": "pass",
            "changed_paths": paths,
        },
        "decision": {**decision, **authority},
        "map": {
            "pre_run": pre["map"],
            "post_verification": checkpoint["frontier"],
            "post_decision": projection,
            "implementation_commit": web_head,
            "layout_authoritative": False,
        },
        "semantic_delta": {
            "claim_id": post["producer"]["claim_id"],
            "claim_standing_before": "pending_review",
            "claim_standing_after": decision["standing"],
            **count_delta,
            "graph_node_count_delta": (
                projection["graph_node_count"] - before_counts["graph_node_count"]
            ),
            "graph_edge_count_delta": (
                projection["graph_edge_count"] - before_counts["graph_edge_count"]
            ),
            "first_target_before": target_before,
            "first_target_after": offer,
        },
        "root_delta": root_delta(checkpoint, status, projection),
        "nonclaims": [
            "The map and its layout are non-authoritative read projections.",
            "Verification passage did not itself change scientific Standing.",
            "The human Decision applies only to the exact bounded Claim and retained evidence.",
            "No bounded result in this loop resolves Erdős problem 1056 or establishes universal nonexistence.",
            "This materializer did not invoke authority, push Git, activate Neon, or mutate a Frontier.",
        ],
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--frontier", type=Path, required=True)
    parser.add_argument("--vela", type=Path, required=True)
    parser.add_argument("--vela-web", type=Path, required=True)
    parser.add_argument("--frontiers-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        output = materialize(
            frontier=args.frontier.resolve(),
            vela=args.vela.resolve(),
            vela_web=args.vela_web.resolve(),
            frontiers_root=args.frontiers_root.resolve(),
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(pretty_bytes(output))
        print(
            json.dumps(
                {
                    "ok": True,
                    "output": str(args.output),
                    "root": sha256_file(args.output),
                    "standing": output["decision"]["standing"],
                    "projection_release_root": output["map"]["post_decision"][
                        "release_root"
                    ],
                },
                sort_keys=True,
            )
        )
        return 0
    except MaterializeError as error:
        print(json.dumps({"ok": False, "error": str(error)}, sort_keys=True))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
