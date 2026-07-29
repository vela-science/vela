#!/usr/bin/env python3
"""Freeze the terminal task instance for the matched Git-versus-Vela pilot."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


PROTOCOL_SCHEMA = "vela.state-lift-study-protocol.v1"
TASK_SCHEMA = "vela.state-lift-task-instance.v1"
ANSWER_KEY_SCHEMA = "vela.state-lift-answer-key.v1"
AMENDMENT_SCHEMA = "vela.state-lift-study-amendment.v1"
PREREGISTRATION_AMENDMENT_ROOT = (
    "sha256:432ba0ac55997130db9b7a4f6004f0ec3bbed7f3e419b4faf2eb75fe0c472c0d"
)

PROPOSAL_ID = "vpr_23f32f95d4f073e8"
PROPOSAL_ROOT = (
    "sha256:38fe9363a278e84c0bf14efc171b2cc3ca3f51cd324ac8a9519cd6b1f0261998"
)
PREDECESSOR_CLAIM_ID = (
    "vcl_5d2858542f6882556bb7652c908708913fadd7ced61014cd5842ae0954ddfe09"
)
PREDECESSOR_CLAIM_ROOT = (
    "sha256:eaca8077dd44632849567faea6074026c76c78eb7e0ea3e8129cfe2d56d54e51"
)
REPLACEMENT_CLAIM_ID = (
    "vcl_4bc14401b203218cb7b9de0141747e0c17cea3a6b0cc522639323ab13e432eaf"
)
REPLACEMENT_CLAIM_ROOT = (
    "sha256:bec914998ce614508cd013f796e3e6e3f2d4950e87ee95858ebbf418bbc85624"
)
SUBMISSION_ID = "vsb_44cd52724425171f"
SUBMISSION_ROOT = (
    "sha256:4cd059848ce06c943e2cafffac0ffa0f14838b5adba022bc4c076df6acc5af12"
)
REGISTRATION_ID = "vrr_6660762458eb85e3"
REGISTRATION_ROOT = (
    "sha256:76f7f627dcdadd3b5431a1b5ad29154b74cac046bf3fdc159531544bc67a9ad7"
)
VERIFICATION_ID = "vvr_ed3383c1cd640d43"
VERIFICATION_ROOT = (
    "sha256:dc4fb781b6bf0817afaad258571419e4fabb1c3868b62dc67415e7d70af99fa5"
)
SOURCE_ARTIFACT_ROOT = (
    "sha256:d18024c4333f77144955adf0036ce831e71b331ea7d9cc9cb69958f960f56d6c"
)

NEXT_ACTION_BY_STANDING = {
    "accepted": "inspect_dependents_and_repair_or_revalidate",
    "rejected": "preserve_predecessor_and_prepare_new_bounded_revision",
}
SCOPE_LIMIT_CODES = [
    "not_erdos_424_proof",
    "not_unique_informal_interpretation",
    "verification_not_acceptance",
]


class MaterializeError(ValueError):
    """Raised when the terminal benchmark input is not exact."""


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


def require(condition: bool, message: str) -> None:
    if not condition:
        raise MaterializeError(message)


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
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise MaterializeError(
            f"command failed ({completed.returncode}): {' '.join(argv)}: {detail}"
        )
    return completed.stdout


def run_json(argv: list[str], *, cwd: Path | None = None) -> dict[str, Any]:
    output = run(argv, cwd=cwd)
    try:
        value = json.loads(output)
    except json.JSONDecodeError as error:
        raise MaterializeError(
            f"command did not emit one JSON object: {' '.join(argv)}"
        ) from error
    require(isinstance(value, dict), f"{' '.join(argv)} output must be an object")
    return value


def require_clean_repository(path: Path, label: str) -> None:
    status = run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=path,
    )
    require(not status.strip(), f"{label} repository is dirty")


def verify_clean_clone(
    frontier: Path,
    vela: Path,
    expected_check: dict[str, Any],
) -> None:
    with tempfile.TemporaryDirectory(prefix="vela-state-lift-") as temporary:
        clone = Path(temporary) / "frontier"
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
        require_clean_repository(clone, "fresh-clone Frontier")
        observed = run_json(
            [str(vela), "check", str(clone), "--strict", "--json"]
        )
        for field in (
            "frontier_id",
            "git_commit",
            "git_tree",
            "repository_root",
            "epoch_id",
            "epoch_root",
        ):
            require(
                observed.get(field) == expected_check.get(field),
                f"fresh-clone {field} drift",
            )


def extract_predicate(statement: Any, label: str) -> str:
    require(isinstance(statement, str), f"{label} statement must be text")
    marker = " ↔ "
    require(marker in statement, f"{label} statement lacks exact biconditional")
    predicate = statement.split(marker, 1)[1].strip()
    if predicate.endswith(" := by"):
        predicate = predicate[: -len(" := by")]
    require(bool(predicate), f"{label} predicate is empty")
    return predicate


def validate_source_transition(
    source_diff: dict[str, Any],
    source_repository: Path,
) -> dict[str, Any]:
    require(
        source_diff.get("schema") == "vela.source-statement-diff.v1",
        "source artifact schema mismatch",
    )
    subject = source_diff.get("subject")
    predecessor = source_diff.get("predecessor")
    successor = source_diff.get("successor")
    require(isinstance(subject, dict), "source artifact subject is missing")
    require(isinstance(predecessor, dict), "source artifact predecessor is missing")
    require(isinstance(successor, dict), "source artifact successor is missing")

    path = subject.get("path")
    require(isinstance(path, str) and path, "source path is missing")
    observed: dict[str, str] = {}
    for label, record in (("predecessor", predecessor), ("successor", successor)):
        commit = record.get("commit")
        require(isinstance(commit, str) and len(commit) == 40, f"{label} commit invalid")
        content = subprocess.run(
            ["git", "show", f"{commit}:{path}"],
            cwd=source_repository,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if content.returncode != 0:
            raise MaterializeError(
                f"cannot read {label} source object {commit}:{path}"
            )
        root = sha256_bytes(content.stdout)
        require(root == record.get("file_sha256"), f"{label} file root drift")
        tree = run(["git", "rev-parse", f"{commit}^{{tree}}"], cwd=source_repository).strip()
        require(tree == record.get("tree"), f"{label} tree drift")
        observed[f"{label}_commit"] = commit
        observed[f"{label}_file_root"] = root
        observed[f"{label}_predicate"] = extract_predicate(
            record.get("statement"),
            label,
        )

    diff = subprocess.run(
        [
            "git",
            "diff",
            observed["predecessor_commit"],
            observed["successor_commit"],
            "--",
            path,
        ],
        cwd=source_repository,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    require(diff.returncode == 0, "cannot reproduce exact source diff")
    return {
        "repository": subject.get("repository"),
        "path": path,
        **observed,
        "diff_root": sha256_bytes(diff.stdout),
    }


def validate_review(review: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    require(review.get("schema") == "vela.review.v1", "review schema mismatch")
    require(review.get("proposal_id") == PROPOSAL_ID, "proposal ID drift")
    require(review.get("proposal_root") == PROPOSAL_ROOT, "proposal root drift")
    require(review.get("standing") in NEXT_ACTION_BY_STANDING, "Decision is not terminal")

    proposal = review.get("proposal")
    submission = review.get("submission")
    claim = review.get("claim")
    decision = review.get("decision")
    records = review.get("verification_records")
    require(isinstance(proposal, dict), "proposal is missing")
    require(isinstance(submission, dict), "submission is missing")
    require(isinstance(claim, dict), "replacement Claim is missing")
    require(isinstance(decision, dict), "terminal Decision is missing")
    require(isinstance(records, list), "Verification Record list is missing")

    require(submission.get("submission_id") == SUBMISSION_ID, "Submission ID drift")
    require(
        proposal.get("producer_package", {}).get("root") == SUBMISSION_ROOT,
        "Submission root drift",
    )
    require(claim.get("claim_id") == REPLACEMENT_CLAIM_ID, "replacement Claim drift")
    require(
        proposal.get("subject", {}).get("root") == REPLACEMENT_CLAIM_ROOT,
        "replacement Claim root drift",
    )
    relations = claim.get("relations")
    require(isinstance(relations, list), "replacement relations are missing")
    require(
        {
            "kind": "supersedes",
            "target_claim_id": PREDECESSOR_CLAIM_ID,
        }
        in relations,
        "replacement does not supersede the exact predecessor",
    )
    evidence = claim.get("evidence")
    require(isinstance(evidence, list), "replacement evidence is missing")
    require(
        any(item.get("artifact_root") == SOURCE_ARTIFACT_ROOT for item in evidence),
        "replacement does not bind the exact source Artifact",
    )

    matching = [
        item
        for item in records
        if isinstance(item, dict)
        and item.get("verification_record_root") == VERIFICATION_ROOT
        and item.get("record", {}).get("verification_record_id") == VERIFICATION_ID
    ]
    require(len(matching) == 1, "exact scoped Verification Record is not imported")
    verification = matching[0]["record"]
    require(verification.get("outcome") == "pass", "Verification did not pass")
    verification_subject = verification.get("subject")
    require(isinstance(verification_subject, dict), "Verification subject missing")
    require(
        verification_subject.get("proposal_id") == PROPOSAL_ID
        and verification_subject.get("claim_id") == REPLACEMENT_CLAIM_ID
        and verification_subject.get("submission_id") == SUBMISSION_ID
        and verification_subject.get("submission_root") == SUBMISSION_ROOT,
        "Verification subject binding drift",
    )

    require(
        decision.get("standing") == review.get("standing"),
        "Decision standing disagrees with review Standing",
    )
    require(
        isinstance(decision.get("event_id"), str)
        and isinstance(decision.get("event_root"), str),
        "Decision event binding is missing",
    )
    return decision, verification


def build_documents(
    *,
    frozen_at: str,
    protocol_root: str,
    preregistration_amendment_root: str,
    scorer_root: str,
    verifier_source_root: str,
    frontier_check: dict[str, Any],
    predecessor_why: dict[str, Any],
    replacement_why: dict[str, Any],
    review: dict[str, Any],
    registration: dict[str, Any],
    source_transition: dict[str, Any],
    vela_version: str,
    vela_binary_root: str,
    runtime_name: str,
    runtime_version: str,
    runtime_binary_root: str,
    model_id: str,
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    decision, verification = validate_review(review)
    require(frontier_check.get("ok") is True, "strict Frontier check failed")
    require(
        frontier_check.get("repository_root") == review.get("repository_root"),
        "review and strict check repository roots disagree",
    )
    require(
        predecessor_why.get("claim_id") == PREDECESSOR_CLAIM_ID
        and predecessor_why.get("claim_root") == PREDECESSOR_CLAIM_ROOT,
        "predecessor Claim identity drift",
    )
    require(
        replacement_why.get("claim_id") == REPLACEMENT_CLAIM_ID
        and replacement_why.get("claim_root") == REPLACEMENT_CLAIM_ROOT,
        "replacement Claim identity drift",
    )
    require(
        replacement_why.get("standing") == review.get("standing"),
        "replacement Standing disagrees with terminal review",
    )
    require(
        registration.get("schema") == "vela.registration-record.v1"
        and registration.get("registration_record_id") == REGISTRATION_ID,
        "registration record identity drift",
    )
    require(
        registration.get("submission_id") == SUBMISSION_ID
        and registration.get("proposal_id") == PROPOSAL_ID
        and registration.get("claim_id") == REPLACEMENT_CLAIM_ID,
        "registration subject binding drift",
    )
    registration_roots = registration.get("roots")
    require(
        registration.get("accepted_state_changed") is False
        and isinstance(registration_roots, dict)
        and registration_roots.get("event_log_before")
        == registration_roots.get("event_log_after"),
        "registration changed accepted state",
    )

    decision_events = [decision["event_id"]]
    if decision.get("applied_event_id"):
        decision_events.append(decision["applied_event_id"])

    task = {
        "schema": TASK_SCHEMA,
        "frozen_at": frozen_at,
        "protocol_root": protocol_root,
        "protocol_amendment_roots": [preregistration_amendment_root],
        "classification": {
            "study": "first-party cold-session pilot",
            "external_participant_credit": False,
            "confirmatory_credit": False,
        },
        "frontier": {
            "frontier_id": frontier_check["frontier_id"],
            "git_commit": frontier_check["git_commit"],
            "git_tree": frontier_check["git_tree"],
            "repository_root": frontier_check["repository_root"],
            "epoch_id": frontier_check["epoch_id"],
            "epoch_root": frontier_check["epoch_root"],
        },
        "correction": {
            "proposal_id": PROPOSAL_ID,
            "proposal_root": PROPOSAL_ROOT,
            "predecessor_claim_id": PREDECESSOR_CLAIM_ID,
            "predecessor_claim_root": PREDECESSOR_CLAIM_ROOT,
            "replacement_claim_id": REPLACEMENT_CLAIM_ID,
            "replacement_claim_root": REPLACEMENT_CLAIM_ROOT,
            "submission_id": SUBMISSION_ID,
            "submission_root": SUBMISSION_ROOT,
            "registration_record_id": REGISTRATION_ID,
            "registration_record_root": REGISTRATION_ROOT,
            "source_artifact_root": SOURCE_ARTIFACT_ROOT,
            "verification_record_id": VERIFICATION_ID,
            "verification_record_root": VERIFICATION_ROOT,
            "verification_profile": verification.get("method", {}).get("profile"),
            "decision_event_id": decision["event_id"],
            "decision_event_root": decision["event_root"],
            "decision_standing": decision["standing"],
            "decision_reason": decision.get("reason"),
            "applied_event_id": decision.get("applied_event_id"),
        },
        "source_transition": source_transition,
        "tools": {
            "vela": {
                "version": vela_version,
                "binary_root": vela_binary_root,
            },
            "runtime": {
                "name": runtime_name,
                "version": runtime_version,
                "binary_root": runtime_binary_root,
                "model_id": model_id,
            },
            "scorer_root": scorer_root,
            "source_verifier_root": verifier_source_root,
        },
        "arms": {
            "git": {
                "vela_available": False,
                "allowed_read_tools": ["git", "jq", "rg"],
            },
            "vela": {
                "vela_available": True,
                "allowed_read_tools": [
                    "git",
                    "jq",
                    "rg",
                    "vela status",
                    "vela show",
                    "vela why",
                    "vela review show",
                    "vela check",
                    "vela log",
                ],
            },
        },
        "limits": {
            "fresh_sessions_per_arm": 4,
            "time_limit_seconds_per_session": 900,
            "observed_token_limit_per_session": 50000,
            "network": "denied",
            "workspace": "read_only",
            "authority_credentials": "absent",
        },
    }
    task_root = sha256_bytes(canonical_bytes(task))

    answer_key = {
        "schema": ANSWER_KEY_SCHEMA,
        "task_instance_root": task_root,
        "expected": {
            "predecessor": {
                "claim_id": PREDECESSOR_CLAIM_ID,
                "claim_root": PREDECESSOR_CLAIM_ROOT,
                "standing": predecessor_why.get("standing"),
            },
            "replacement": {
                "claim_id": REPLACEMENT_CLAIM_ID,
                "claim_root": REPLACEMENT_CLAIM_ROOT,
                "standing": replacement_why.get("standing"),
            },
            "source_transition": {
                "path": source_transition["path"],
                "predecessor_commit": source_transition["predecessor_commit"],
                "predecessor_file_root": source_transition[
                    "predecessor_file_root"
                ],
                "predecessor_predicate": source_transition[
                    "predecessor_predicate"
                ],
                "successor_commit": source_transition["successor_commit"],
                "successor_file_root": source_transition["successor_file_root"],
                "successor_predicate": source_transition["successor_predicate"],
            },
            "evidence": {
                "submission_id": SUBMISSION_ID,
                "submission_root": SUBMISSION_ROOT,
                "verification_ids": [VERIFICATION_ID],
                "decision_id": decision["event_id"],
                "decision_root": decision["event_root"],
                "event_ids": decision_events,
            },
            "accepted_state_delta": {
                "registration": 0,
                "verification": 0,
            },
            "authority": {
                "verification_changed_standing": False,
                "model_or_tool_has_decision_authority": False,
            },
            "next_action_code": NEXT_ACTION_BY_STANDING[decision["standing"]],
            "scope_limit_codes": SCOPE_LIMIT_CODES,
        },
    }
    answer_key_root = sha256_bytes(canonical_bytes(answer_key))

    amendment = {
        "schema": AMENDMENT_SCHEMA,
        "prior_protocol_root": protocol_root,
        "frozen_at": frozen_at,
        "entry_gate": {
            "proposal_id": PROPOSAL_ID,
            "terminal_standing": decision["standing"],
            "strict_clean_clone_required": True,
        },
        "bindings": {
            "task_instance_root": task_root,
            "answer_key_root": answer_key_root,
            "preregistration_amendment_root": preregistration_amendment_root,
            "scorer_root": scorer_root,
            "frontier_repository_root": frontier_check["repository_root"],
            "vela_binary_root": vela_binary_root,
            "runtime_binary_root": runtime_binary_root,
            "model_id": model_id,
        },
        "outcome_rules_frozen_before_decision": {
            "accepted": NEXT_ACTION_BY_STANDING["accepted"],
            "rejected": NEXT_ACTION_BY_STANDING["rejected"],
            "scope_limit_codes": SCOPE_LIMIT_CODES,
        },
        "authorization": {
            "model_calls_authorized_after_this_amendment": True,
            "authority_credentials_available_to_sessions": False,
            "mutating_commands_available_to_sessions": False,
        },
    }
    return task, answer_key, amendment


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--frontier", type=Path, required=True)
    parser.add_argument("--source-repository", type=Path, required=True)
    parser.add_argument("--vela", type=Path, required=True)
    parser.add_argument("--runtime-binary", type=Path, required=True)
    parser.add_argument("--runtime-name", required=True)
    parser.add_argument("--runtime-version", required=True)
    parser.add_argument("--model-id", required=True)
    parser.add_argument("--frozen-at", required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = Path(__file__).resolve().parents[3]
    protocol_path = root / "paper/artifacts/state-lift/protocol.v1.json"
    preregistration_amendment_path = (
        root
        / "paper/artifacts/state-lift/preregistration-amendment-001.v1.json"
    )
    scorer_path = root / "paper/artifacts/state-lift/score.py"
    verifier_path = root / "paper/artifacts/erdos-424/verify_source_transition.py"
    registration_record_path = (
        args.frontier
        / "records/registrations/sha256"
        / f"{REGISTRATION_ROOT.removeprefix('sha256:')}.json"
    )
    source_artifact_path = (
        args.frontier
        / "records/artifacts/sha256"
        / SOURCE_ARTIFACT_ROOT.removeprefix("sha256:")
    )

    try:
        dt.datetime.fromisoformat(args.frozen_at.replace("Z", "+00:00"))
    except ValueError as error:
        raise MaterializeError("--frozen-at must be an RFC3339 timestamp") from error
    require_clean_repository(args.frontier, "Frontier")
    require_clean_repository(args.source_repository, "source")
    require(args.vela.is_file(), "Vela binary is missing")
    require(args.runtime_binary.is_file(), "runtime binary is missing")
    require(source_artifact_path.is_file(), "retained source Artifact is missing")
    require(
        registration_record_path.is_file(),
        "retained registration record is missing",
    )
    require(
        sha256_file(registration_record_path) == REGISTRATION_ROOT,
        "registration record root drift",
    )

    protocol = json.loads(protocol_path.read_text(encoding="utf-8"))
    require(protocol.get("schema") == PROTOCOL_SCHEMA, "protocol schema drift")
    require(
        sha256_file(preregistration_amendment_path)
        == PREREGISTRATION_AMENDMENT_ROOT,
        "preregistration amendment root drift",
    )
    review = run_json(
        [
            str(args.vela),
            "review",
            "show",
            str(args.frontier),
            PROPOSAL_ID,
            "--json",
        ]
    )
    check = run_json(
        [str(args.vela), "check", str(args.frontier), "--strict", "--json"]
    )
    verify_clean_clone(args.frontier, args.vela, check)
    predecessor_why = run_json(
        [
            str(args.vela),
            "why",
            str(args.frontier),
            PREDECESSOR_CLAIM_ID,
            "--json",
        ]
    )
    replacement_why = run_json(
        [
            str(args.vela),
            "why",
            str(args.frontier),
            REPLACEMENT_CLAIM_ID,
            "--json",
        ]
    )
    source_diff = json.loads(source_artifact_path.read_text(encoding="utf-8"))
    registration = json.loads(
        registration_record_path.read_text(encoding="utf-8")
    )
    source_transition = validate_source_transition(
        source_diff,
        args.source_repository,
    )
    vela_version = run([str(args.vela), "--version"]).strip()

    task, answer_key, amendment = build_documents(
        frozen_at=args.frozen_at,
        protocol_root=sha256_file(protocol_path),
        preregistration_amendment_root=PREREGISTRATION_AMENDMENT_ROOT,
        scorer_root=sha256_file(scorer_path),
        verifier_source_root=sha256_file(verifier_path),
        frontier_check=check,
        predecessor_why=predecessor_why,
        replacement_why=replacement_why,
        review=review,
        registration=registration,
        source_transition=source_transition,
        vela_version=vela_version,
        vela_binary_root=sha256_file(args.vela),
        runtime_name=args.runtime_name,
        runtime_version=args.runtime_version,
        runtime_binary_root=sha256_file(args.runtime_binary),
        model_id=args.model_id,
    )

    require(
        not args.output.exists()
        or (args.output.is_dir() and not any(args.output.iterdir())),
        "output directory must be absent or empty",
    )
    args.output.mkdir(parents=True, exist_ok=True)
    outputs = {
        "task-instance.v1.json": task,
        "answer-key.v1.json": answer_key,
        "amendment.v1.json": amendment,
    }
    roots: dict[str, str] = {}
    for name, value in outputs.items():
        path = args.output / name
        path.write_bytes(pretty_bytes(value))
        roots[name] = sha256_file(path)

    result = {
        "schema": "vela.state-lift-materialization-result.v1",
        "ok": True,
        "output": str(args.output.resolve()),
        "canonical_roots": {
            "task_instance": sha256_bytes(canonical_bytes(task)),
            "answer_key": sha256_bytes(canonical_bytes(answer_key)),
            "amendment": sha256_bytes(canonical_bytes(amendment)),
        },
        "file_roots": roots,
    }
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except MaterializeError as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
