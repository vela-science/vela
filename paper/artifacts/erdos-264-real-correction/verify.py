"""Verify the bounded Erdős 264 real-correction evidence package."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

CASE_ROOT = "sha256:06236c11c3d26cdd548a67ae58968b97066e554940837fd512f9b1348899a4f3"


def sha256(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def git(repo: Path, *args: str) -> bytes:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        capture_output=True,
    ).stdout


def git_show(repo: Path, commit: str, path: str) -> bytes:
    return git(repo, "show", f"{commit}:{path}")


def canonical_diff(repo: Path, old_commit: str, new_commit: str, path: str) -> bytes:
    return git(
        repo,
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--full-index",
        old_commit,
        new_commit,
        "--",
        path,
    )


def definition_bytes(source: bytes) -> bytes:
    marker = b"def IsIrrationalitySequence"
    start = source.index(marker)
    end = source.index(b"\n\n/--", start)
    return source[start : end + 1]


def direct_consumers(source: bytes) -> list[str]:
    text = source.decode("utf-8")
    matches = list(re.finditer(r"\btheorem\s+([A-Za-z0-9_.']+)", text))
    consumers: list[str] = []
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        declaration = text[match.start() : end]
        signature = declaration.split(":= by", 1)[0]
        if "IsIrrationalitySequence" in signature:
            consumers.append(f"Erdos264.{match.group(1)}")
    return consumers


def load_json(encoded: bytes, label: str) -> dict[str, Any]:
    value = json.loads(encoded)
    require(isinstance(value, dict), f"{label} must be a JSON object")
    return value


def read_bound_json(
    repo: Path,
    commit: str,
    binding: dict[str, Any],
    label: str,
    *,
    root_key: str = "root",
) -> tuple[bytes, dict[str, Any]]:
    encoded = git_show(repo, commit, binding["path"])
    require(sha256(encoded) == binding[root_key], f"{label} root mismatch")
    return encoded, load_json(encoded, label)


def read_bound_bytes(
    repo: Path,
    commit: str,
    binding: dict[str, Any],
    label: str,
    *,
    root_key: str = "root",
) -> bytes:
    encoded = git_show(repo, commit, binding["path"])
    require(sha256(encoded) == binding[root_key], f"{label} root mismatch")
    return encoded


def decode_authority_record(envelope: dict[str, Any], label: str) -> dict[str, Any]:
    require(
        envelope.get("payloadType") == "application/vnd.vela.authority-record.v1+json",
        f"{label} payload type mismatch",
    )
    payload = envelope.get("payload")
    require(isinstance(payload, str), f"{label} payload missing")
    decoded = base64.b64decode(payload, validate=True)
    return load_json(decoded, f"{label} payload")


def verify_decision(
    evidence_repo: Path,
    evidence_commit: str,
    decision: dict[str, Any],
    *,
    proposal_id: str,
    claim_id: str,
    expected_kind: str,
    label: str,
) -> dict[str, Any]:
    _, review = read_bound_json(
        evidence_repo,
        evidence_commit,
        decision["review_event"],
        f"{label} review event",
        root_key="file_root",
    )
    _, transition = read_bound_json(
        evidence_repo,
        evidence_commit,
        decision["transition_event"],
        f"{label} transition event",
        root_key="file_root",
    )
    _, envelope = read_bound_json(
        evidence_repo,
        evidence_commit,
        decision["authority_record"],
        f"{label} authority record",
        root_key="file_root",
    )
    authority_record = decode_authority_record(envelope, f"{label} authority record")
    content = authority_record.get("content")
    require(isinstance(content, dict), f"{label} authority content missing")

    require(
        review.get("id") == decision["review_event"]["id"],
        f"{label} review id mismatch",
    )
    require(
        transition.get("id") == decision["transition_event"]["id"],
        f"{label} event id mismatch",
    )
    review_content = review.get("content")
    transition_content = transition.get("content")
    require(isinstance(review_content, dict), f"{label} review content missing")
    require(isinstance(transition_content, dict), f"{label} transition content missing")
    review_payload = review_content.get("payload")
    transition_payload = transition_content.get("payload")
    require(isinstance(review_payload, dict), f"{label} review payload missing")
    require(isinstance(transition_payload, dict), f"{label} transition payload missing")

    require(
        review_content.get("kind") == "review.accepted",
        f"{label} review was not accepted",
    )
    require(
        review_payload.get("proposal_id") == proposal_id,
        f"{label} review proposal mismatch",
    )
    require(review_payload.get("verdict") == "accepted", f"{label} verdict mismatch")
    require(
        transition_content.get("kind") == expected_kind,
        f"{label} transition kind mismatch",
    )
    require(
        transition_payload.get("claim_id") == claim_id,
        f"{label} transition Claim mismatch",
    )
    require(
        transition_payload.get("proposal_id") == proposal_id,
        f"{label} transition Proposal mismatch",
    )
    require(
        transition_payload.get("repository_before") == decision["repository_before"],
        f"{label} repository-before mismatch",
    )
    require(
        transition_payload.get("repository_after") == decision["repository_after"],
        f"{label} repository-after mismatch",
    )
    require(
        content.get("sequence") == decision["sequence"],
        f"{label} authority sequence mismatch",
    )
    require(
        content.get("before_event_log_root") == decision["event_log_before"],
        f"{label} event-log-before mismatch",
    )
    require(
        content.get("after_event_log_root") == decision["event_log_after"],
        f"{label} event-log-after mismatch",
    )
    event_ids = content.get("event_ids")
    require(
        isinstance(event_ids, list) and len(event_ids) == 2,
        f"{label} event list missing",
    )
    require(
        set(event_ids)
        == {decision["review_event"]["id"], decision["transition_event"]["id"]},
        f"{label} authority event list mismatch",
    )
    approvals = content.get("semantic_approvals")
    require(
        isinstance(approvals, list) and len(approvals) == 1, f"{label} approval missing"
    )
    require(
        approvals[0].get("action") == "review_accept",
        f"{label} approval action mismatch",
    )
    return {
        "authority_record_id": authority_record.get("record_id"),
        "sequence": content.get("sequence"),
        "event_log_before": content.get("before_event_log_root"),
        "event_log_after": content.get("after_event_log_root"),
        "repository_before": transition_payload.get("repository_before"),
        "repository_after": transition_payload.get("repository_after"),
    }


def verify(case_path: Path, source_repo: Path, evidence_repo: Path) -> dict[str, Any]:
    case_bytes = case_path.read_bytes()
    require(sha256(case_bytes) == CASE_ROOT, "case root mismatch")
    case = load_json(case_bytes, "case")
    require(
        case.get("schema") == "vela.erdos-264-real-correction.v1",
        "case schema mismatch",
    )
    require(
        case.get("authority") == "non_authoritative", "case authority boundary mismatch"
    )

    source = case["source"]
    path = source["path"]
    predecessor = source["predecessor"]
    successor = source["successor"]
    old_commit = predecessor["commit"]
    new_commit = successor["commit"]

    old_tree = git(source_repo, "rev-parse", f"{old_commit}^{{tree}}").decode().strip()
    new_tree = git(source_repo, "rev-parse", f"{new_commit}^{{tree}}").decode().strip()
    require(old_tree == predecessor["tree"], "predecessor tree mismatch")
    require(new_tree == successor["tree"], "successor tree mismatch")
    old_blob = git(source_repo, "rev-parse", f"{old_commit}:{path}").decode().strip()
    new_blob = git(source_repo, "rev-parse", f"{new_commit}:{path}").decode().strip()
    require(old_blob == predecessor["blob"], "predecessor blob mismatch")
    require(new_blob == successor["blob"], "successor blob mismatch")

    old_bytes = git_show(source_repo, old_commit, path)
    new_bytes = git_show(source_repo, new_commit, path)
    require(
        sha256(old_bytes) == predecessor["file_root"], "predecessor file root mismatch"
    )
    require(sha256(new_bytes) == successor["file_root"], "successor file root mismatch")
    require(
        sha256(definition_bytes(old_bytes)) == predecessor["definition_root"],
        "predecessor definition root mismatch",
    )
    require(
        sha256(definition_bytes(new_bytes)) == successor["definition_root"],
        "successor definition root mismatch",
    )
    diff = canonical_diff(source_repo, old_commit, new_commit, path)
    require(sha256(diff) == source["full_index_diff_root"], "source diff root mismatch")
    require(
        b"b : \xe2\x84\x95 \xe2\x86\x92 \xe2\x84\x95" in old_bytes,
        "natural predecessor missing",
    )
    require(
        b"b : \xe2\x84\x95 \xe2\x86\x92 \xe2\x84\xa4" in new_bytes,
        "integer successor missing",
    )
    require(b"BddBelow (Set.range b)" in new_bytes, "successor lower bound missing")
    consumers = direct_consumers(new_bytes)
    require(
        consumers == source["direct_consumers"], "direct consumer inventory mismatch"
    )

    retained = case["retained_evidence"]
    evidence_commit = retained["commit"]
    evidence_tree = (
        git(evidence_repo, "rev-parse", f"{evidence_commit}^{{tree}}").decode().strip()
    )
    require(evidence_tree == retained["tree"], "retained evidence tree mismatch")

    correction = retained["correction"]
    _, correction_artifact = read_bound_json(
        evidence_repo, evidence_commit, correction["artifact"], "correction artifact"
    )
    require(
        correction_artifact.get("predecessor", {}).get("commit") == old_commit,
        "correction artifact predecessor mismatch",
    )
    require(
        correction_artifact.get("successor", {}).get("commit") == new_commit,
        "correction artifact successor mismatch",
    )
    artifact_consumers = [
        item["symbol"]
        for item in correction_artifact["direct_consumer_scope"]["consumers"]
    ]
    require(
        artifact_consumers == consumers,
        "correction artifact consumer inventory mismatch",
    )
    require(
        correction_artifact["transition"]["full_index_diff_sha256"]
        == source["full_index_diff_root"],
        "correction artifact diff mismatch",
    )

    _, correction_submission = read_bound_json(
        evidence_repo,
        evidence_commit,
        correction["submission"],
        "correction Submission",
    )
    _, correction_verification = read_bound_json(
        evidence_repo,
        evidence_commit,
        correction["verification"],
        "correction Verification",
    )
    _, correction_claim = read_bound_json(
        evidence_repo, evidence_commit, correction["claim"], "correction Claim"
    )
    _, correction_proposal = read_bound_json(
        evidence_repo, evidence_commit, correction["proposal"], "correction Proposal"
    )
    require(
        correction_submission.get("submission_id") == correction["submission"]["id"],
        "correction Submission id mismatch",
    )
    require(
        correction_verification.get("verification_record_id")
        == correction["verification"]["id"],
        "correction Verification id mismatch",
    )
    require(
        correction_verification.get("outcome") == "pass",
        "correction Verification did not pass",
    )
    require(
        correction_claim.get("claim_id") == correction["claim"]["id"],
        "correction Claim id mismatch",
    )
    require(
        correction_proposal.get("proposal_id") == correction["proposal"]["id"],
        "correction Proposal id mismatch",
    )
    correction_relations = correction_claim.get("relations")
    require(
        isinstance(correction_relations, list), "correction Claim relations missing"
    )
    require(
        [item.get("kind") for item in correction_relations] == ["supersedes"],
        "correction Claim relations drifted",
    )
    correction_replay = verify_decision(
        evidence_repo,
        evidence_commit,
        correction["decision"],
        proposal_id=correction["proposal"]["id"],
        claim_id=correction["claim"]["id"],
        expected_kind="finding.superseded",
        label="correction",
    )

    repair = retained["accepted_dependent_repair"]
    repair_source = repair["source"]
    repair_tree = (
        git(source_repo, "rev-parse", f"{repair_source['commit']}^{{tree}}")
        .decode()
        .strip()
    )
    require(repair_tree == repair_source["tree"], "repair source tree mismatch")
    repair_blob = (
        git(source_repo, "rev-parse", f"{repair_source['commit']}:{path}")
        .decode()
        .strip()
    )
    require(repair_blob == repair_source["blob"], "repair source blob mismatch")
    repair_source_bytes = git_show(source_repo, repair_source["commit"], path)
    require(
        sha256(repair_source_bytes) == repair_source["file_root"],
        "repair source file root mismatch",
    )
    require(
        sha256(definition_bytes(repair_source_bytes))
        == repair_source["corrected_definition_root"],
        "repair source definition root mismatch",
    )
    require(
        repair_source["corrected_definition_root"] == successor["definition_root"],
        "repair was not bound to corrected definition",
    )
    candidate = read_bound_bytes(
        evidence_repo, evidence_commit, repair["artifact"], "repair artifact"
    )
    require(
        sha256(definition_bytes(candidate)) == successor["definition_root"],
        "repair artifact definition root mismatch",
    )
    require(
        direct_consumers(candidate) == consumers,
        "repair artifact consumer inventory mismatch",
    )
    require(
        b"theorem erdos_264.parts.i : \xc2\xacIsIrrationalitySequence (2 ^ \xc2\xb7) := by\n  set_option"
        in candidate,
        "repair artifact theorem body missing",
    )

    _, capsule = read_bound_json(
        evidence_repo,
        evidence_commit,
        repair["verifier_capsule"],
        "repair verifier capsule",
    )
    require(
        capsule.get("source", {}).get("commit") == repair_source["commit"],
        "repair capsule source mismatch",
    )
    require(
        capsule.get("environment", {}).get("lean_toolchain")
        == "leanprover/lean4:v4.27.0",
        "repair Lean toolchain mismatch",
    )
    _, repair_verification = read_bound_json(
        evidence_repo, evidence_commit, repair["verification"], "repair Verification"
    )
    _, repair_claim = read_bound_json(
        evidence_repo, evidence_commit, repair["claim"], "repair Claim"
    )
    _, repair_proposal = read_bound_json(
        evidence_repo, evidence_commit, repair["proposal"], "repair Proposal"
    )
    require(
        repair_verification.get("verification_record_id")
        == repair["verification"]["id"],
        "repair Verification id mismatch",
    )
    require(
        repair_verification.get("outcome") == "pass", "repair Verification did not pass"
    )
    require(
        repair_claim.get("claim_id") == repair["claim"]["id"],
        "repair Claim id mismatch",
    )
    require(
        repair_proposal.get("proposal_id") == repair["proposal"]["id"],
        "repair Proposal id mismatch",
    )
    require(
        repair_claim.get("relations") == [],
        "repair Claim unexpectedly has Vela dependency relations",
    )
    repair_replay = verify_decision(
        evidence_repo,
        evidence_commit,
        repair["decision"],
        proposal_id=repair["proposal"]["id"],
        claim_id=repair["claim"]["id"],
        expected_kind="finding.asserted",
        label="accepted repair",
    )
    require(
        repair_replay["event_log_before"] == correction_replay["event_log_after"],
        "event-log replay chain is not contiguous",
    )
    require(
        repair_replay["sequence"] == correction_replay["sequence"] + 1,
        "authority sequence is not contiguous",
    )

    ceiling = case["claim_ceiling"]
    nonclaims = " ".join(ceiling["not_established"])
    require(
        "not five Vela Claim depends edges" in nonclaims,
        "Vela dependency-edge limit missing",
    )
    require("support diamond" in nonclaims, "support-diamond limit missing")
    require("general scientific lift" in nonclaims, "general-lift limit missing")

    return {
        "schema": "vela.erdos-264-real-correction-verification.v1",
        "outcome": "pass",
        "case_root": sha256(case_bytes),
        "source": {
            "predecessor_commit": old_commit,
            "successor_commit": new_commit,
            "predecessor_definition_root": sha256(definition_bytes(old_bytes)),
            "successor_definition_root": sha256(definition_bytes(new_bytes)),
            "diff_root": sha256(diff),
            "direct_consumers": consumers,
        },
        "correction": {
            "verification_root": correction["verification"]["root"],
            "claim_root": correction["claim"]["root"],
            "replay": correction_replay,
        },
        "accepted_dependent_repair": {
            "artifact_root": sha256(candidate),
            "verification_root": repair["verification"]["root"],
            "claim_root": repair["claim"]["root"],
            "replay": repair_replay,
        },
        "checks": {
            "source_objects_exact": True,
            "five_direct_source_consumers_exact": True,
            "correction_verification_decision_replay_exact": True,
            "accepted_repair_bound_to_corrected_definition": True,
            "repair_claim_has_no_vela_dependency_edge": True,
            "claim_ceiling_explicit": True,
        },
        "limits": ceiling["not_established"],
        "authority_effect": "none",
        "standing_effect": "none",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-repo", type=Path, required=True)
    parser.add_argument("--evidence-repo", type=Path, required=True)
    parser.add_argument(
        "--case", type=Path, default=Path(__file__).with_name("case.json")
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        result = verify(
            args.case.resolve(),
            args.source_repo.resolve(),
            args.evidence_repo.resolve(),
        )
    except (
        OSError,
        subprocess.CalledProcessError,
        ValueError,
        json.JSONDecodeError,
    ) as error:
        print(f"verification failed: {error}", file=sys.stderr)
        return 1
    encoded = f"{json.dumps(result, sort_keys=True, separators=(',', ':'))}\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
