#!/usr/bin/env python3
"""Verify the retained map -> Verification -> Decision -> remap trace offline."""

from __future__ import annotations

import ast
import base64
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path
from typing import Any

ARTIFACT_DIR = Path(__file__).resolve().parent
REPOSITORY_ROOT = ARTIFACT_DIR.parents[2]
MANIFEST_PATH = ARTIFACT_DIR / "manifest.json"


class ReproductionError(ValueError):
    """Raised when retained controller/substrate evidence does not match."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ReproductionError(message)


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return "sha256:" + hashlib.sha256(value).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise ReproductionError(f"cannot parse {path}: {error}") from error
    require(isinstance(value, dict), f"{path} must contain one JSON object")
    return value


def run(argv: list[str], *, cwd: Path | None = None) -> str:
    completed = subprocess.run(
        argv,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = (completed.stderr.strip() or completed.stdout.strip())[:1200]
        raise ReproductionError(
            f"command failed ({completed.returncode}): {' '.join(argv)}: {detail}"
        )
    return completed.stdout


def git(repository: Path, *args: str) -> str:
    return run(["git", *args], cwd=repository).strip()


def verify_manifest(manifest: dict[str, Any]) -> str:
    require(
        manifest.get("schema") == "vela.controller-substrate-reproduction.v1",
        "manifest schema drift",
    )
    files = manifest.get("files")
    require(isinstance(files, list) and files, "manifest files are missing")
    observed_paths: set[str] = set()
    for entry in files:
        require(isinstance(entry, dict), "manifest file entry must be an object")
        relative = entry.get("path")
        require(isinstance(relative, str) and relative, "manifest path is missing")
        candidate = Path(relative)
        require(not candidate.is_absolute(), f"absolute manifest path: {relative}")
        require(".." not in candidate.parts, f"escaping manifest path: {relative}")
        require(relative not in observed_paths, f"duplicate manifest path: {relative}")
        observed_paths.add(relative)
        path = REPOSITORY_ROOT / candidate
        try:
            content = path.read_bytes()
        except OSError as error:
            raise ReproductionError(f"cannot read {relative}: {error}") from error
        require(len(content) == entry.get("size"), f"size drift: {relative}")
        require(sha256_bytes(content) == entry.get("sha256"), f"hash drift: {relative}")

    internal = {
        path.relative_to(REPOSITORY_ROOT).as_posix()
        for path in ARTIFACT_DIR.rglob("*")
        if path.is_file() and path != MANIFEST_PATH
    }
    listed_internal = {
        path
        for path in observed_paths
        if path.startswith("paper/artifacts/controller-")
    }
    require(
        internal == listed_internal, "manifest does not bind every local artifact file"
    )
    root = sha256_bytes(canonical_bytes(sorted(files, key=lambda item: item["path"])))
    require(root == manifest.get("artifact_root"), "artifact root drift")
    return root


def reconstruct_bundle(
    bundle: Path,
    specification: dict[str, Any],
    destination: Path,
) -> None:
    run(["git", "bundle", "verify", str(bundle)])
    run(["git", "init", "--quiet", str(destination)])
    run(["git", "bundle", "unbundle", str(bundle)], cwd=destination)
    boundary = specification.get("shallow_boundary")
    require(isinstance(boundary, str), "bundle shallow boundary is missing")
    (destination / ".git" / "shallow").write_text(boundary + "\n", encoding="utf-8")
    tip = specification.get("tip")
    reference = specification.get("ref")
    require(
        isinstance(tip, str) and isinstance(reference, str), "bundle ref is missing"
    )
    git(destination, "update-ref", reference, tip)
    git(destination, "fsck", "--full")

    commits = specification.get("commits")
    require(isinstance(commits, list) and commits, "bundle commits are missing")
    actual = git(destination, "rev-list", "--reverse", reference).splitlines()
    expected = [entry["commit"] for entry in commits]
    require(actual == expected, "bundle commit sequence drift")
    for entry in commits:
        commit = entry["commit"]
        require(
            git(destination, "rev-parse", f"{commit}^{{tree}}") == entry["tree"],
            f"tree drift at {commit}",
        )
        commit_object = git(destination, "cat-file", "-p", commit).splitlines()
        parent_lines = [
            line[7:] for line in commit_object if line.startswith("parent ")
        ]
        require(parent_lines == entry["object_parents"], f"parent drift at {commit}")


def git_json(repository: Path, commit: str, path: str) -> tuple[dict[str, Any], bytes]:
    raw = run(["git", "show", f"{commit}:{path}"], cwd=repository).encode("utf-8")
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ReproductionError(f"invalid retained JSON {commit}:{path}") from error
    require(isinstance(value, dict), f"retained object must be a map: {path}")
    return value, raw


def repository_stage(
    repository: Path,
    stage: dict[str, Any],
    claim_id: str,
) -> dict[str, Any]:
    value, raw = git_json(repository, stage["commit"], ".vela/repository.json")
    require(sha256_bytes(raw) == stage["repository_root"], "repository root drift")
    accepted = value.get("accepted_claims", [])
    pending = value.get("pending_claims", [])
    require(len(accepted) == stage["accepted_claims"], "accepted count drift")
    require(len(pending) == stage["pending_claims"], "pending count drift")
    standings = {
        item["standing"]
        for item in [*accepted, *pending]
        if item.get("claim_id") == claim_id
    }
    require(standings == set(stage["claim_standings"]), "Claim Standing drift")
    return value


def changed_paths(repository: Path, before: str, after: str) -> list[dict[str, str]]:
    output = git(repository, "diff", "--name-status", before, after)
    rows = []
    for line in output.splitlines():
        change, path = line.split("\t", 1)
        rows.append({"change": change, "path": path})
    return rows


def read_authority_record(
    repository: Path,
    commit: str,
    path: str,
) -> tuple[dict[str, Any], str]:
    envelope, raw = git_json(repository, commit, path)
    require(
        envelope.get("payloadType") == "application/vnd.vela.authority-record.v1+json",
        "authority payload type drift",
    )
    try:
        payload = json.loads(base64.b64decode(envelope["payload"], validate=True))
    except (KeyError, ValueError, json.JSONDecodeError) as error:
        raise ReproductionError("invalid retained authority envelope") from error
    require(isinstance(payload, dict), "authority payload must be an object")
    content = payload.get("content")
    require(isinstance(content, dict), "authority content is missing")
    return content, sha256_bytes(raw)


def verify_controller_boundary(materializer_path: Path) -> None:
    source = materializer_path.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(materializer_path))
    vela_commands: list[tuple[int, list[str]]] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        if not isinstance(node.func, ast.Name) or node.func.id not in {
            "run",
            "run_json",
        }:
            continue
        if not node.args or not isinstance(node.args[0], ast.List):
            continue
        tokens = [
            element.value
            for element in node.args[0].elts
            if isinstance(element, ast.Constant) and isinstance(element.value, str)
        ]
        if any(
            isinstance(element, ast.Call)
            and isinstance(element.func, ast.Name)
            and element.func.id == "str"
            and element.args
            and isinstance(element.args[0], ast.Name)
            and element.args[0].id == "vela"
            for element in node.args[0].elts
        ):
            vela_commands.append((node.lineno, tokens))

    require(
        [tokens for _, tokens in sorted(vela_commands)]
        == [
            ["--version"],
            ["status", "--json"],
            ["review", "show", "--json"],
            ["next", "--limit", "1", "--json"],
            ["status", "--json"],
            ["review", "show", "--json"],
        ],
        "materializer Vela command surface is not the frozen read-only set",
    )
    require(
        "SSH_AUTH_SOCK" not in source, "materializer reads an authority-agent socket"
    )
    require('git", "push' not in source, "materializer can push Git")


def verify_trace(
    manifest: dict[str, Any],
    erdos: Path,
    web: Path,
) -> dict[str, Any]:
    expected = manifest["trace"]
    claim_id = expected["claim_id"]
    stages = expected["repository_stages"]
    snapshots = [repository_stage(erdos, stage, claim_id) for stage in stages]
    pre, submitted, verified, decided = snapshots

    require(
        stages[2]["accepted_claims"] == stages[0]["accepted_claims"],
        "Verification changed accepted Standing",
    )
    require(
        stages[3]["accepted_claims"] == stages[2]["accepted_claims"] + 1,
        "Decision did not cause the sole accepted delta",
    )
    require(
        stages[3]["pending_claims"] == stages[2]["pending_claims"] - 1,
        "Decision pending delta drift",
    )

    observed_diffs = [
        changed_paths(erdos, stages[index]["commit"], stages[index + 1]["commit"])
        for index in range(3)
    ]
    require(observed_diffs == expected["changed_paths"], "stage path delta drift")
    require(
        not any(
            row["path"].startswith(".vela/authority/events/")
            for rows in observed_diffs[:2]
            for row in rows
        ),
        "producer or verifier emitted a Standing event",
    )

    authority = expected["authority_records"]
    for name in ("submission", "verification"):
        item = authority[name]
        content, root = read_authority_record(erdos, item["commit"], item["path"])
        require(root == item["root"], f"{name} authority record root drift")
        require(
            content["principal"]["principal_class"] == "agent", f"{name} class drift"
        )
        require(
            content["semantic_approvals"] == [], f"{name} gained semantic authority"
        )
        require(content["event_ids"] == [], f"{name} emitted semantic events")

    decision_spec = authority["decision"]
    decision_record, decision_root = read_authority_record(
        erdos, decision_spec["commit"], decision_spec["path"]
    )
    require(decision_root == decision_spec["root"], "Decision authority root drift")
    require(
        decision_record["principal"]["principal_class"] == "human",
        "Decision principal is not human",
    )
    require(
        [item["action"] for item in decision_record["semantic_approvals"]]
        == ["review_accept"],
        "Decision approval drift",
    )
    require(
        decision_record["event_ids"] == decision_spec["event_ids"],
        "Decision event coverage drift",
    )

    for event_spec in expected["decision_events"]:
        event, raw = git_json(erdos, stages[3]["commit"], event_spec["path"])
        require(sha256_bytes(raw) == event_spec["root"], "Decision event root drift")
        content = event.get("content", {})
        require(content.get("actor", {}).get("type") == "human", "event actor drift")
        require(
            content.get("principal_id") == decision_record["principal"]["principal_id"],
            "event principal drift",
        )

    target_documents = [
        git_json(erdos, stages[index]["commit"], "targets.json")[0]
        for index in (0, 2, 3)
    ]
    target_packets = [
        document["targets"][0]["packet"]["sha256"] for document in target_documents
    ]
    require(len(set(target_packets)) == 1, "retained Target packet was not stale")
    require(
        target_packets[0] == expected["stale_target"]["packet_root"],
        "Target root drift",
    )

    map_root = REPOSITORY_ROOT / "paper/artifacts/map-target-loop"
    pre_artifact = load_json(map_root / "pre-run.json")
    post_verification = load_json(map_root / "post-verification.json")
    checkpoint = load_json(map_root / "post-verification-map.json")
    packet = load_json(map_root / "decision-packet.json")
    post_decision = load_json(map_root / "post-decision.json")
    require(
        pre_artifact["frontier"]["commit"] == stages[0]["commit"],
        "pre-run commit drift",
    )
    require(
        post_verification["frontier"]["repository_root"]
        == stages[2]["repository_root"],
        "post-Verification root drift",
    )
    require(
        packet["status"] == "prepared_not_invoked", "Decision packet invoked authority"
    )
    require(packet["proposal"]["standing"] == "pending_review", "packet Standing drift")
    require(
        checkpoint["frontier"]["accepted_claim_count"] == stages[2]["accepted_claims"],
        "projection accepted count drift",
    )
    require(
        checkpoint["candidate"]["activated"] is False,
        "post-Verification projection was activated",
    )
    require(
        post_decision["frontier"]["after_commit"] == stages[3]["commit"],
        "post-Decision commit drift",
    )
    require(
        post_decision["semantic_delta"]["accepted_claim_count_delta"] == 1,
        "post-Decision semantic delta drift",
    )
    require(
        post_decision["semantic_delta"]["next_target_packet_advanced"] is False,
        "stale-Target defect was erased",
    )
    require(
        post_decision["map"]["implementation_commit"]
        == manifest["bundles"]["vela_web"]["tip"],
        "remap implementation commit drift",
    )
    require(
        git(web, "rev-parse", f"{manifest['bundles']['vela_web']['tip']}^{{tree}}")
        == manifest["bundles"]["vela_web"]["commits"][-1]["tree"],
        "remap implementation tree drift",
    )

    verify_controller_boundary(map_root / "materialize_post_decision.py")
    require(
        pre is not submitted and verified is not decided, "stage snapshots collapsed"
    )
    return {
        "accepted_before": stages[0]["accepted_claims"],
        "accepted_after_verification": stages[2]["accepted_claims"],
        "accepted_after_decision": stages[3]["accepted_claims"],
        "claim_standing_after": "accepted",
        "decision_actor_class": "human",
        "stale_target_packet_root": target_packets[0],
        "authority_effect": "none",
    }


def verify_all() -> dict[str, Any]:
    manifest = load_json(MANIFEST_PATH)
    artifact_root = verify_manifest(manifest)
    with tempfile.TemporaryDirectory(
        prefix="vela-controller-reproduction-"
    ) as temporary:
        root = Path(temporary)
        erdos = root / "erdos"
        web = root / "vela-web"
        reconstruct_bundle(
            ARTIFACT_DIR / "bundles/erdos-map-target-loop.bundle",
            manifest["bundles"]["erdos"],
            erdos,
        )
        reconstruct_bundle(
            ARTIFACT_DIR / "bundles/vela-web-map-implementation.bundle",
            manifest["bundles"]["vela_web"],
            web,
        )
        result = verify_trace(manifest, erdos, web)
    return {
        "schema": "vela.controller-substrate-reproduction-result.v1",
        "status": "verified",
        "artifact_root": artifact_root,
        "trace": result,
        "claim_ceiling": manifest["claim_ceiling"],
    }


def main() -> int:
    try:
        print(json.dumps(verify_all(), ensure_ascii=False, sort_keys=True))
        return 0
    except ReproductionError as error:
        print(json.dumps({"status": "failed", "error": str(error)}, sort_keys=True))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
