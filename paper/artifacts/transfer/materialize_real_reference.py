#!/usr/bin/env python3
"""Materialize one exact, non-authoritative cross-Frontier reference package.

The source Frontier remains canonical. This script reads the exact retained
compacted tree and the predecessor transition named by that tree's repository
origin. Later source commits cannot silently change the evidence package.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "conformance"))
from readers.python.canonical import canonical_bytes  # noqa: E402


CLAIM_ID = "vcl_4bc14401b203218cb7b9de0141747e0c17cea3a6b0cc522639323ab13e432eaf"
SUBMISSION_ID = "vsb_44cd52724425171f"
PROPOSAL_ID = "vpr_23f32f95d4f073e8"
VERIFICATION_ID = "vvr_ed3383c1cd640d43"
APPLIED_EVENT_ID = "vev_c0b7450dd55a75f6"
DECISION_EVENT_ID = "vev_c9edac512e2b3307"
AUTHORITY_RECORD_ID = "var_64ec1c05368bc2c7"
SOURCE_COMMIT = "81e79f008b4fc653888efda810dd8eb48e50cffa"


def run(*args: str, cwd: Path) -> bytes:
    return subprocess.run(
        args,
        cwd=cwd,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout


def git_text(source: Path, *args: str) -> str:
    return run("git", *args, cwd=source).decode("utf-8").strip()


def git_bytes(source: Path, revision: str, path: str) -> bytes:
    return run("git", "show", f"{revision}:{path}", cwd=source)


def digest_bytes(value: bytes) -> str:
    return f"sha256:{hashlib.sha256(value).hexdigest()}"


def canonical_root(value: object) -> str:
    return digest_bytes(canonical_bytes(value))


def load_json(value: bytes, label: str) -> dict[str, object]:
    parsed = json.loads(value)
    if not isinstance(parsed, dict):
        raise ValueError(f"{label} is not a JSON object")
    if canonical_bytes(parsed) != value:
        raise ValueError(f"{label} is not canonical JSON")
    return parsed


def indexed_path(
    manifest: dict[str, object], field: str, object_id: str
) -> tuple[str, str]:
    entries = manifest.get(field)
    if not isinstance(entries, list):
        raise ValueError(f"transition manifest has no {field} index")
    matches = [
        entry
        for entry in entries
        if isinstance(entry, dict) and entry.get("id") == object_id
    ]
    if len(matches) != 1:
        raise ValueError(f"transition manifest does not index exactly one {object_id}")
    path = matches[0].get("path")
    root = matches[0].get("root")
    if not isinstance(path, str) or not isinstance(root, str):
        raise ValueError(f"transition manifest entry for {object_id} is incomplete")
    return path, root


def accepted_claim_path(
    manifest: dict[str, object], claim_id: str
) -> tuple[str, str]:
    entries = manifest.get("accepted_claims")
    if not isinstance(entries, list):
        raise ValueError("repository manifest has no accepted_claims index")
    matches = [
        entry
        for entry in entries
        if isinstance(entry, dict)
        and entry.get("claim_id") == claim_id
        and entry.get("standing") == "accepted"
    ]
    if len(matches) != 1:
        raise ValueError(f"repository does not retain accepted Claim {claim_id}")
    path = matches[0].get("path")
    root = matches[0].get("claim_root")
    if not isinstance(path, str) or not isinstance(root, str):
        raise ValueError(f"accepted Claim entry for {claim_id} is incomplete")
    return path, root


def write_object(output: Path, package_path: str, value: bytes) -> None:
    destination = output / package_path
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(value)


def resolve_source_snapshot(source: Path, source_ref: str) -> tuple[str, str]:
    commit = git_text(source, "rev-parse", f"{source_ref}^{{commit}}")
    tree = git_text(source, "rev-parse", f"{commit}^{{tree}}")
    return commit, tree


def materialize(
    source: Path,
    output: Path,
    source_ref: str = SOURCE_COMMIT,
) -> dict[str, object]:
    if git_text(source, "status", "--porcelain"):
        raise ValueError("source Frontier is dirty")
    current_commit, current_tree = resolve_source_snapshot(source, source_ref)
    current_manifest_bytes = git_bytes(source, current_commit, ".vela/repository.json")
    current_origin_bytes = git_bytes(source, current_commit, ".vela/origin.json")
    current_manifest = load_json(current_manifest_bytes, "current repository manifest")
    origin = load_json(current_origin_bytes, "repository origin")
    if origin.get("kind") != "compaction":
        raise ValueError("current repository origin is not a compaction")
    predecessor = origin.get("predecessor")
    if not isinstance(predecessor, dict):
        raise ValueError("compaction origin has no predecessor")
    transition_commit = predecessor.get("commit")
    transition_tree = predecessor.get("tree")
    transition_root = predecessor.get("repository_root")
    if not all(isinstance(value, str) for value in (transition_commit, transition_tree, transition_root)):
        raise ValueError("compaction predecessor identity is incomplete")
    if git_text(source, "rev-parse", f"{transition_commit}^{{tree}}") != transition_tree:
        raise ValueError("predecessor Git tree does not match repository origin")

    transition_manifest_bytes = git_bytes(
        source, transition_commit, ".vela/repository.json"
    )
    transition_manifest = load_json(
        transition_manifest_bytes, "transition repository manifest"
    )
    if digest_bytes(transition_manifest_bytes) != transition_root:
        raise ValueError("predecessor repository root does not match retained bytes")
    current_root = digest_bytes(current_manifest_bytes)

    current_claim_path, current_claim_root = accepted_claim_path(
        current_manifest, CLAIM_ID
    )
    transition_claim_path, transition_claim_root = accepted_claim_path(
        transition_manifest, CLAIM_ID
    )
    if (current_claim_path, current_claim_root) != (
        transition_claim_path,
        transition_claim_root,
    ):
        raise ValueError("accepted correction did not survive compaction exactly")

    submission_path, submission_root = indexed_path(
        transition_manifest, "submissions", SUBMISSION_ID
    )
    proposal_path, proposal_root = indexed_path(
        transition_manifest, "proposals", PROPOSAL_ID
    )
    verification_path, verification_root = indexed_path(
        transition_manifest, "verifications", VERIFICATION_ID
    )
    authority_keyset_root = transition_manifest.get("authority_keyset_root")
    if not isinstance(authority_keyset_root, str):
        raise ValueError("transition manifest has no authority keyset root")

    source_paths = {
        "claim": transition_claim_path,
        "submission": submission_path,
        "proposal": proposal_path,
        "verification": verification_path,
        "applied_event": f".vela/authority/events/{APPLIED_EVENT_ID}.json",
        "decision_event": f".vela/authority/events/{DECISION_EVENT_ID}.json",
        "authority_record": (
            f".vela/authority/records/{AUTHORITY_RECORD_ID}.dsse.json"
        ),
        "authority_keyset": (
            ".vela/authority/keysets/"
            f"{authority_keyset_root.removeprefix('sha256:')}.json"
        ),
    }
    retained = {
        role: git_bytes(source, transition_commit, path)
        for role, path in source_paths.items()
    }
    if retained["claim"] != git_bytes(source, current_commit, current_claim_path):
        raise ValueError("current accepted Claim bytes differ from transition bytes")

    applied = load_json(retained["applied_event"], "applied Event")
    decision = load_json(retained["decision_event"], "Decision Event")
    envelope = load_json(retained["authority_record"], "authority record envelope")
    payload = base64.b64decode(envelope["payload"], validate=True)
    authority_record = load_json(payload, "authority record payload")
    applied_semantic_id = (
        decision.get("content", {}).get("payload", {}).get("applied_event_id")
        if isinstance(decision.get("content"), dict)
        else None
    )
    if not isinstance(applied_semantic_id, str):
        raise ValueError("Decision Event has no applied semantic Event ID")

    package_values = {
        "current_repository_manifest": (
            "objects/current/.vela/repository.json",
            "current-repository-manifest",
            current_root,
            current_manifest_bytes,
        ),
        "repository_origin": (
            "objects/current/.vela/origin.json",
            origin["origin_id"],
            digest_bytes(current_origin_bytes),
            current_origin_bytes,
        ),
        "transition_repository_manifest": (
            "objects/transition/.vela/repository.json",
            "transition-repository-manifest",
            transition_root,
            transition_manifest_bytes,
        ),
        "claim": (
            f"objects/transition/{transition_claim_path}",
            CLAIM_ID,
            transition_claim_root,
            retained["claim"],
        ),
        "submission": (
            f"objects/transition/{submission_path}",
            SUBMISSION_ID,
            submission_root,
            retained["submission"],
        ),
        "proposal": (
            f"objects/transition/{proposal_path}",
            PROPOSAL_ID,
            proposal_root,
            retained["proposal"],
        ),
        "verification": (
            f"objects/transition/{verification_path}",
            VERIFICATION_ID,
            verification_root,
            retained["verification"],
        ),
        "applied_event": (
            f"objects/transition/{source_paths['applied_event']}",
            APPLIED_EVENT_ID,
            digest_bytes(retained["applied_event"]),
            retained["applied_event"],
        ),
        "decision_event": (
            f"objects/transition/{source_paths['decision_event']}",
            DECISION_EVENT_ID,
            digest_bytes(retained["decision_event"]),
            retained["decision_event"],
        ),
        "authority_record": (
            f"objects/transition/{source_paths['authority_record']}",
            AUTHORITY_RECORD_ID,
            canonical_root(authority_record),
            retained["authority_record"],
        ),
        "authority_keyset": (
            f"objects/transition/{source_paths['authority_keyset']}",
            "authority-keyset",
            authority_keyset_root,
            retained["authority_keyset"],
        ),
    }
    objects = []
    for role in sorted(package_values):
        package_path, object_id, root, value = package_values[role]
        write_object(output, package_path, value)
        objects.append(
            {
                "role": role,
                "id": object_id,
                "root": root,
                "bytes_root": digest_bytes(value),
                "path": package_path,
            }
        )

    reference = {
        "schema": "vela.foreign-reference.v1",
        "source": {
            "frontier_id": current_manifest["frontier_id"],
            "current_repository": {
                "git_commit": current_commit,
                "git_tree": current_tree,
                "repository_root": current_root,
            },
            "transition_repository": {
                "git_commit": transition_commit,
                "git_tree": transition_tree,
                "repository_root": transition_root,
            },
            "repository_origin": {
                "id": origin["origin_id"],
                "root": digest_bytes(current_origin_bytes),
            },
            "claim": {"id": CLAIM_ID, "root": transition_claim_root},
            "submission": {"id": SUBMISSION_ID, "root": submission_root},
            "proposal": {"id": PROPOSAL_ID, "root": proposal_root},
            "verification": {"id": VERIFICATION_ID, "root": verification_root},
            "decision_event": {
                "id": DECISION_EVENT_ID,
                "root": digest_bytes(retained["decision_event"]),
            },
            "applied_event": {
                "id": APPLIED_EVENT_ID,
                "root": digest_bytes(retained["applied_event"]),
                "semantic_id": applied_semantic_id,
            },
            "authority_record": {
                "id": AUTHORITY_RECORD_ID,
                "root": canonical_root(authority_record),
            },
            "authority_keyset_root": authority_keyset_root,
            "standing": "accepted",
        },
        "objects": objects,
        "object_set_root": canonical_root(objects),
        "completeness": {"status": "complete", "missing_roles": []},
        "authority": {
            "source_standing": "accepted",
            "local_standing_effect": "none",
            "requires_local_decision": True,
        },
        "does_not_establish": [
            "The source Frontier's accepted Standing has no authority in a receiving Frontier.",
            "Retaining this envelope does not verify scientific truth or change local Standing.",
            "First-party transfer does not establish independent adoption, a Registry, or an Atlas.",
        ],
    }
    (output / "reference.json").write_bytes(canonical_bytes(reference))
    return {
        "schema": "vela.foreign-reference-materialization.v1",
        "source_repository": git_text(source, "remote", "get-url", "origin"),
        "reference_root": canonical_root(reference),
        "object_set_root": reference["object_set_root"],
        "object_count": len(objects),
        "current_git_commit": current_commit,
        "current_git_tree": current_tree,
        "current_repository_root": current_root,
        "transition_git_commit": transition_commit,
        "transition_git_tree": transition_tree,
        "transition_repository_root": transition_root,
        "local_standing_effect": "none",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument(
        "--source-ref",
        default=SOURCE_COMMIT,
        help="Exact retained compacted source commit (defaults to the frozen evidence commit)",
    )
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()
    source = args.source.expanduser().resolve()
    output = args.output.expanduser().resolve()
    if output.exists():
        if not args.force:
            raise SystemExit(f"output already exists: {output}")
        shutil.rmtree(output)
    output.mkdir(parents=True)
    try:
        result = materialize(source, output, args.source_ref)
        (output / "materialization.json").write_bytes(canonical_bytes(result) + b"\n")
        print(json.dumps(result, indent=2))
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        shutil.rmtree(output, ignore_errors=True)
        print(f"materialize foreign reference: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
