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
import tarfile
from pathlib import Path
from typing import Any, Sequence

import contract


ARMS = ("git-files", "vela-guided")
SCENARIOS = (
    "formal-foreign-reference-continuation",
    "quantum-certificate-supersession",
    "erdos-post-decision-continuation",
    "explicit-target-absence",
)
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


def retained_artifact_path(frontier: Path, root: str) -> Path:
    if not contract.ROOT.fullmatch(root):
        fail(f"invalid artifact root: {root}")
    return frontier / "records" / "artifacts" / "sha256" / root.removeprefix("sha256:")


def read_foreign_reference(archive: Path) -> tuple[dict[str, Any], str]:
    try:
        with tarfile.open(archive, "r:*") as bundle:
            members = [member for member in bundle.getmembers() if member.name == "reference.v1.json"]
            if len(members) != 1:
                fail("foreign-reference archive must contain one exact reference.v1.json")
            handle = bundle.extractfile(members[0])
            if handle is None:
                fail("foreign-reference archive reference.v1.json is not a file")
            payload = handle.read()
    except (OSError, tarfile.TarError) as exc:
        raise contract.ContractError(f"cannot read foreign-reference archive {archive}: {exc}") from exc
    try:
        value = json.loads(payload)
    except json.JSONDecodeError as exc:
        raise contract.ContractError(f"foreign-reference manifest is invalid JSON: {exc}") from exc
    if not isinstance(value, dict):
        fail("foreign-reference manifest must be a JSON object")
    return value, contract.sha256_root(payload)


def inspect_current_decision(frontier: Path, vela: Path, proposal_id: str) -> dict[str, Any]:
    """Read one exact current Decision packet without changing the Frontier."""
    frontier, vela = frontier.resolve(), vela.resolve()
    if command(("git", "status", "--porcelain"), cwd=frontier):
        fail("frontier checkout must be clean")

    before_commit = command(("git", "rev-parse", "HEAD"), cwd=frontier)
    before_tree = command(("git", "rev-parse", "HEAD^{tree}"), cwd=frontier)
    next_work = json_command((str(vela), "next", ".", "--limit", "1", "--json"), cwd=frontier)
    inbox = json_command((str(vela), "review", "inbox", str(frontier), "--json"), cwd=frontier)

    if (before_commit, before_tree) != (
        command(("git", "rev-parse", "HEAD"), cwd=frontier),
        command(("git", "rev-parse", "HEAD^{tree}"), cwd=frontier),
    ) or command(("git", "status", "--porcelain"), cwd=frontier):
        fail("read-only inspection changed the Frontier checkout")

    targets = next_work.get("targets")
    availability = next_work.get("availability")
    if (
        not isinstance(targets, list)
        or targets
        or not isinstance(availability, dict)
        or availability.get("configured") != 0
        or availability.get("returned") != 0
    ):
        fail("product-compression study requires exactly zero configured and returned Targets")
    if inbox.get("repository_root") != next_work.get("repository_root"):
        fail("Decision Inbox and continuation inspection disagree on repository root")
    inbox_projection_root = inbox.get("projection_root")
    if not contract.ROOT.fullmatch(inbox_projection_root or ""):
        fail("Decision Inbox has no rooted projection")

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
    return {
        "frontier": frontier,
        "vela": vela,
        "commit": before_commit,
        "tree": before_tree,
        "next_work": next_work,
        "inbox_projection_root": inbox_projection_root,
        "entry": entry,
        "submission": submission,
        "submission_root": submission_root,
        "verifications": verifications,
    }


def inspect_terminal_continuation(
    frontier: Path, vela: Path, accepted_claim_id: str,
) -> dict[str, Any]:
    """Read one accepted transition and its current Target without writes."""
    frontier, vela = frontier.resolve(), vela.resolve()
    if command(("git", "status", "--porcelain"), cwd=frontier):
        fail("frontier checkout must be clean")
    commit = command(("git", "rev-parse", "HEAD"), cwd=frontier)
    tree = command(("git", "rev-parse", "HEAD^{tree}"), cwd=frontier)
    next_work = json_command((str(vela), "next", ".", "--limit", "1", "--json"), cwd=frontier)
    why = json_command((str(vela), "why", ".", accepted_claim_id, "--json"), cwd=frontier)
    if (commit, tree) != (
        command(("git", "rev-parse", "HEAD"), cwd=frontier),
        command(("git", "rev-parse", "HEAD^{tree}"), cwd=frontier),
    ) or command(("git", "status", "--porcelain"), cwd=frontier):
        fail("read-only inspection changed the Frontier checkout")
    try:
        [target] = next_work["targets"]
        packet_root = target["packet"]["sha256"]
        packet = contract.read_json(frontier / target["packet"]["path"])
        chain = why["chain"]
        [proposal] = chain["proposals"]
        verification_candidate = chain["verification_records"][0]
        if "verification_record" in verification_candidate:
            verification = verification_candidate
        else:
            repository = contract.read_json(frontier / ".vela" / "repository.json")
            verification_row = next(
                row for row in repository["verifications"]
                if row["id"] == verification_candidate["verification_record_id"]
            )
            verification = {
                "verification_record": verification_candidate,
                "verification_record_root": verification_row["root"],
            }
        decision = next(item for item in chain["authority_events"] if item["event"]["content"]["kind"] == "review.accepted")
        applied = next(item for item in chain["authority_events"] if item["event"]["content"]["kind"] == "finding.asserted")
        accepted = packet["accepted_state"]["latest_bounded_negative"]
        next_range = packet["target"]["next_bounded_range"]
    except (KeyError, TypeError, ValueError, StopIteration) as exc:
        raise contract.ContractError(f"incomplete post-Decision continuation: {exc}") from exc
    if (
        next_work["availability"] != {"configured": 1, "stale": 0, "fresh": 1, "returned": 1}
        or why["frontier_id"] != next_work["frontier_id"]
        or why["repository_root"] != next_work["repository_root"]
        or why["claim_id"] != accepted_claim_id
        or why["standing"] != "accepted"
        or chain["standing_basis"] != "compacted_origin"
        or why["interpretation"] != {
            "submission_is_acceptance": False,
            "verification_is_acceptance": False,
            "standing_is_derived": True,
        }
        or digest(frontier / target["packet"]["path"]) != packet_root
        or accepted["claim_id"] != accepted_claim_id
        or accepted["claim_root"] != why["claim_root"]
        or packet.get("producer_completion") not in (None, {})
        or accepted["range"]["last"] + 1 != next_range["first"]
        or packet["completion_contract"]["duplicate_range_forbidden"] is not True
    ):
        fail("post-Decision continuation does not match current accepted Standing and Target")
    return {
        "frontier": frontier, "vela": vela, "commit": commit, "tree": tree,
        "next_work": next_work, "why": why, "chain": chain, "target": target,
        "packet_root": packet_root, "proposal": proposal, "verification": verification,
        "decision": decision, "applied": applied, "accepted": accepted,
        "next_range": next_range,
    }


def inspect_target_absence(frontier: Path, vela: Path) -> dict[str, Any]:
    """Read one exact no-work result without inventing a Target."""
    frontier, vela = frontier.resolve(), vela.resolve()
    if command(("git", "status", "--porcelain"), cwd=frontier):
        fail("frontier checkout must be clean")
    commit = command(("git", "rev-parse", "HEAD"), cwd=frontier)
    tree = command(("git", "rev-parse", "HEAD^{tree}"), cwd=frontier)
    if command(("git", "ls-tree", "--name-only", "HEAD", "--", "targets.json"), cwd=frontier):
        fail("target-absence scenario requires no canonical targets.json")
    status = json_command((str(vela), "status", ".", "--json"), cwd=frontier)
    next_work = json_command((str(vela), "next", ".", "--limit", "1", "--json"), cwd=frontier)
    if (commit, tree) != (
        command(("git", "rev-parse", "HEAD"), cwd=frontier),
        command(("git", "rev-parse", "HEAD^{tree}"), cwd=frontier),
    ) or command(("git", "status", "--porcelain"), cwd=frontier):
        fail("read-only inspection changed the Frontier checkout")
    if (
        next_work.get("availability") != {"configured": 0, "stale": 0, "fresh": 0, "returned": 0}
        or next_work.get("targets") != []
        or next_work.get("next_action")
        != "No Target Index is configured; inspect the Frontier before inventing work."
        or status.get("integrity") != {
            "replay": "verified", "strict": "pass", "blocker_count": 0, "blockers_by_code": {},
        }
        or status.get("frontier", {}).get("id") != next_work.get("frontier_id")
        or status.get("roots", {}).get("repository") != next_work.get("repository_root")
    ):
        fail("Frontier does not expose the exact no-work state")
    return {
        "frontier": frontier,
        "vela": vela,
        "commit": commit,
        "tree": tree,
        "status": status,
        "next_work": next_work,
    }


def verification_projection(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        {
            field: item[field]
            for field in (
                "verification_record_id", "verification_record_root", "outcome",
                "property", "verifier", "independent_of_producer",
                "protocol_evidence_role", "satisfies_requirements", "does_not_establish",
            )
        }
        for item in items
    ]


def expected_answer(
    inspection: dict[str, Any], scenario: str, requested_change: dict[str, Any]
) -> dict[str, Any]:
    entry = inspection["entry"]
    submission = inspection["submission"]
    next_work = inspection["next_work"]
    standing_delta = entry["standing_delta"]
    result = {
        "schema": contract.ANSWER_SCHEMA,
        "scenario": scenario,
        "frontier": {
            "frontier_id": next_work["frontier_id"],
            "repository_root": next_work["repository_root"],
            "configured_targets": 0,
        },
        "decision": {
            "proposal_id": entry["proposal_id"],
            "proposal_root": entry["inputs"]["proposal_root"],
            "source_submission_id": submission["submission_id"],
            "source_submission_root": inspection["submission_root"],
            "proposed_claim_id": entry["claim_id"],
            "proposed_claim_root": entry["inputs"]["claim_root"],
            "requested_change": requested_change,
            "assertion": entry["assertion"],
            "conditions": entry["conditions"],
            "limits": entry["limits"],
            "verifications": verification_projection(inspection["verifications"]),
            "verification_set_root": entry["inputs"]["verification_set_root"],
            "inbox_entry_root": entry["entry_root"],
            "protocol_gate": entry["readiness"]["protocol_gate"],
            "blockers": entry["readiness"]["blockers"],
            "human_decision_required": entry["readiness"]["human_decision_required"],
            "verification_is_acceptance": False,
            "standing_delta": standing_delta,
            "staleness": entry["staleness"]["state"],
            "next_obligation": entry["next_obligation"],
        },
    }
    contract.validate_answer(result)
    return result


def formal_foreign_reference_scenario(inspection: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any], str, str]:
    """Qualify the existing Formal foreign-reference continuation scenario."""
    frontier = inspection["frontier"]
    entry = inspection["entry"]
    submission = inspection["submission"]
    artifacts = submission.get("artifacts")
    references = [
        artifact
        for artifact in artifacts or []
        if isinstance(artifact, dict) and artifact.get("kind") == "foreign-reference"
    ]
    if len(references) != 1:
        fail("receiver Submission must bind exactly one foreign-reference artifact")
    archive_root = references[0].get("digest")
    archive_path = retained_artifact_path(frontier, archive_root)
    if digest(archive_path) != archive_root:
        fail("retained foreign-reference archive bytes disagree with the Submission")
    reference, reference_root = read_foreign_reference(archive_path)
    authority = reference.get("authority")
    source = reference.get("source")
    source_claim = source.get("claim") if isinstance(source, dict) else None
    if reference.get("schema") != "vela.foreign-reference.v1":
        fail("receiver artifact is not a Vela foreign-reference manifest")
    if not isinstance(authority, dict) or (
        authority.get("source_standing"),
        authority.get("local_standing_effect"),
        authority.get("requires_local_decision"),
    ) != ("accepted", "none", True):
        fail("foreign reference must preserve accepted source Standing and require a local Decision")
    if not isinstance(source, dict) or not isinstance(source_claim, dict):
        fail("foreign reference has no exact source Claim")
    source_claim_id = source_claim.get("id")
    source_claim_root = source_claim.get("root")
    source_frontier_id = source.get("frontier_id")
    if (
        not isinstance(source_frontier_id, str)
        or len(source_frontier_id) != 20
        or not source_frontier_id.startswith("vfr_")
        or not isinstance(source_claim_id, str)
        or not contract.ROOT.fullmatch(source_claim_root or "")
    ):
        fail("foreign reference source Claim identity is malformed")
    assertion = entry.get("assertion")
    if not isinstance(assertion, str) or source_claim_id not in assertion or reference_root not in assertion:
        fail("receiver Proposal does not bind the accepted source Claim and reference root")

    requested_change = submission.get("requested_change")
    if requested_change != {"kind": "add_claim"}:
        fail("foreign-reference scenario requires an add_claim Submission")
    anchor = {
        "source": {
            "frontier_id": source_frontier_id,
            "reference_root": reference_root,
            "archive_sha256": archive_root,
            "claim_id": source_claim_id,
            "claim_root": source_claim_root,
            "standing": authority["source_standing"],
            "local_standing_effect": authority["local_standing_effect"],
        }
    }
    instruction = (
        "The fixture identifies an accepted source Claim and its exact foreign-reference "
        "archive, but intentionally does not name the receiver Proposal. Identify the one "
        "current receiver Proposal that binds that source anchor. Distinguish accepted source "
        "Standing from pending local Standing and report the exact Decision packet."
    )
    claim_limit = (
        "First-party evidence from one frozen receiver-continuation task; no independent-user, "
        "full correction-inheritance, or general scientific-workflow claim."
    )
    return anchor, expected_answer(inspection, SCENARIOS[0], requested_change), instruction, claim_limit


def quantum_certificate_scenario(inspection: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any], str, str]:
    """Qualify one exact pending quantum-certificate supersession."""
    frontier = inspection["frontier"]
    submission = inspection["submission"]
    requested = submission.get("requested_change")
    target = requested.get("target") if isinstance(requested, dict) else None
    if not isinstance(target, dict) or requested.get("kind") != "supersede_claim":
        fail("quantum scenario requires one exact supersede_claim Submission")
    target_id, target_root = target.get("claim_id"), target.get("claim_root")
    if not isinstance(target_id, str) or not contract.ROOT.fullmatch(target_root or ""):
        fail("quantum supersession target is malformed")
    artifacts = submission.get("artifacts")
    witnesses = [item for item in artifacts or [] if isinstance(item, dict) and item.get("kind") == "witness"]
    if len(witnesses) != 1:
        fail("quantum scenario requires exactly one witness")
    witness = witnesses[0]
    witness_path = frontier / witness.get("path", "")
    if digest(witness_path) != witness.get("digest"):
        fail("quantum witness bytes disagree with the Submission")
    requested_change = {
        "kind": "supersede_claim",
        "target_claim_id": target_id,
        "target_claim_root": target_root,
    }
    anchor = {
        "accepted_predecessor": {"claim_id": target_id, "claim_root": target_root},
        "witness": {
            "root": witness["digest"],
            "kind": witness["kind"],
            "path": witness["path"],
        },
    }
    instruction = (
        "The fixture identifies one accepted predecessor Claim and its retained witness, but "
        "intentionally does not name the pending Proposal. Find the one current Proposal that "
        "supersedes that predecessor. Report the proposed replacement, both distinct Verification "
        "scopes and nonclaims, the exact accept/reject Standing delta, and exact next obligations."
    )
    claim_limit = (
        "First-party evidence from one frozen pending quantum correction. No scientific Decision, "
        "acceptance, post-correction remap, optimality, novelty, external independence, verifier-"
        "soundness, or general scientific-productivity claim."
    )
    return anchor, expected_answer(inspection, SCENARIOS[1], requested_change), instruction, claim_limit


def terminal_continuation_answer(inspection: dict[str, Any]) -> dict[str, Any]:
    why, chain, target = inspection["why"], inspection["chain"], inspection["target"]
    proposal, verification = inspection["proposal"], inspection["verification"]
    decision, applied = inspection["decision"], inspection["applied"]
    result = {
        "schema": contract.ANSWER_SCHEMA,
        "scenario": SCENARIOS[2],
        "frontier": {
            "frontier_id": why["frontier_id"], "repository_root": why["repository_root"],
            "target_index_root": inspection["next_work"]["target_index_root"], "configured_targets": 1,
        },
        "continuation": {
            "accepted_claim_id": why["claim_id"], "accepted_claim_root": why["claim_root"],
            "standing_basis": chain["standing_basis"], "origin_root": chain["standing_basis_detail"]["origin_root"],
            "archive_bytes_re_read": chain["standing_basis_detail"]["archive_bytes_re_read"],
            "proposal_id": proposal["proposal"]["proposal_id"], "proposal_root": proposal["proposal_root"],
            "submission_id": proposal["proposal"]["producer_package"]["id"], "submission_root": proposal["proposal"]["producer_package"]["root"],
            "verification_id": verification["verification_record"]["verification_record_id"], "verification_root": verification["verification_record_root"],
            "decision_event_id": decision["authority_event_id"], "decision_event_root": decision["authority_event_root"], "decision_actor": decision["event"]["content"]["actor"]["type"],
            "accepted_first": inspection["accepted"]["range"]["first"], "accepted_through": inspection["accepted"]["range"]["last"],
            "next_target_id": target["target_id"], "next_first": inspection["next_range"]["first"], "next_last": inspection["next_range"]["last"],
            "packet_root": inspection["packet_root"], "verifier_profile": target["verifier_profile"],
            "verification_is_acceptance": False, "decision_changes_standing": True,
            "next_target_changes_standing": False,
        },
    }
    contract.validate_answer(result)
    return result


def erdos_post_decision_scenario(
    inspection: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any], str, str]:
    """Qualify one accepted Erdős transition and its exact current continuation."""
    why = inspection["why"]
    anchor = {
        "accepted_claim": {"claim_id": why["claim_id"], "claim_root": why["claim_root"]},
        "target_id": inspection["target"]["target_id"], "packet_root": inspection["packet_root"],
    }
    instruction = (
        "The fixture identifies one accepted bounded Claim and its exact closed Target slice. "
        "From the current checkout, recover the Submission, passing Verification, and human "
        "Decision that give the Claim Standing. Then identify the first current non-overlapping "
        "Target, packet, and verifier, and preserve the limits on "
        "what its execution, Submission, or Verification could change."
    )
    claim_limit = (
        "First-party evidence from one current-head post-Decision Erdős continuation task. It may "
        "measure exact cold continuation from an authorized Decision to the next obligation; it "
        "does not establish correction propagation, external independence, "
        "general scientific productivity, or resolution of Erdős problem 1056."
    )
    return anchor, terminal_continuation_answer(inspection), instruction, claim_limit


def target_absence_scenario(
    inspection: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any], str, str]:
    """Qualify one exact absence instead of rewarding invented work."""
    next_work = inspection["next_work"]
    expected = {
        "schema": contract.ANSWER_SCHEMA,
        "scenario": SCENARIOS[3],
        "frontier": {
            "frontier_id": next_work["frontier_id"],
            "repository_root": next_work["repository_root"],
            "configured_targets": 0,
        },
        "absence": {
            "target_index_path": "targets.json",
            "target_index_configured": False,
            "availability": next_work["availability"],
            "returned_target_ids": [],
            "blocker_code": "target_index_not_configured",
            "next_valid_action": "inspect_frontier",
            "may_invent_target": False,
            "standing_changed": False,
            "authority_action_required": False,
        },
    }
    contract.validate_answer(expected)
    instruction = (
        "Determine whether this exact Frontier currently offers any producer Target. "
        "If the repository has no configured Target Index, report the semantic reason "
        "`target_index_not_configured`, the next action `inspect_frontier_before_inventing_work`, "
        "and no target. Do not derive work from accepted Claims, source files, graph position, "
        "or scientific importance."
    )
    claim_limit = (
        "First-party evidence from one exact Frontier no-work state. It may measure refusal to "
        "invent work; it does not establish scientific completeness, adoption, productivity, or "
        "that the Frontier has no valuable unanswered questions."
    )
    return {"canonical_target_index_path": "targets.json"}, expected, instruction, claim_limit


SCENARIO_BUILDERS = {
    SCENARIOS[0]: formal_foreign_reference_scenario,
    SCENARIOS[1]: quantum_certificate_scenario,
}


def materialize_fixture(
    frontier: Path, vela: Path, subject_id: str | None, scenario: str,
) -> tuple[dict[str, Any], dict[str, Any], str, str]:
    """Bind one explicit scenario to one exact current Frontier state."""
    if scenario not in SCENARIOS:
        fail(f"unsupported scenario: {scenario}")
    if scenario == SCENARIOS[3]:
        inspection = inspect_target_absence(frontier, vela)
        anchor, expected, instruction, claim_limit = target_absence_scenario(inspection)
    elif scenario == SCENARIOS[2]:
        if subject_id is None:
            fail("post-Decision scenario requires an accepted Claim")
        inspection = inspect_terminal_continuation(frontier, vela, subject_id)
        anchor, expected, instruction, claim_limit = erdos_post_decision_scenario(inspection)
    else:
        if subject_id is None:
            fail("pending-Decision scenario requires a Proposal")
        inspection = inspect_current_decision(frontier, vela, subject_id)
        anchor, expected, instruction, claim_limit = SCENARIO_BUILDERS[scenario](inspection)
    next_work = inspection["next_work"]
    fixture = contract.seal({
        "schema": contract.FIXTURE_SCHEMA,
        "fixture_root": "",
        "scenario": scenario,
        "vela": {
            "version": command((str(inspection["vela"]), "--version"), cwd=inspection["frontier"]),
            "binary_sha256": digest(inspection["vela"]),
        },
        "frontier": {
            "frontier_id": next_work["frontier_id"],
            "git_commit": inspection["commit"],
            "git_tree": inspection["tree"],
            "repository_root": next_work["repository_root"],
            **(
                {}
                if scenario == SCENARIOS[3]
                else {"configured_targets": next_work["availability"]["configured"]}
            ),
            **(
                {"target_index_root": next_work["target_index_root"]}
                if scenario == SCENARIOS[2]
                else (
                    {}
                    if scenario == SCENARIOS[3]
                    else {"inbox_projection_root": inspection["inbox_projection_root"]}
                )
            ),
        },
        "anchor": anchor,
    }, "fixture_root")
    answer_key = contract.seal({
        "schema": contract.ANSWER_KEY_SCHEMA,
        "answer_key_root": "",
        "fixture_root": fixture["fixture_root"],
        "scenario": scenario,
        "expected": expected,
    }, "answer_key_root")
    contract.validate_answer_key(answer_key)
    return fixture, answer_key, instruction, claim_limit


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
    scenario_instruction: str,
    claim_limit: str,
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
        for arm in ARMS:
            task = tasks / arm
            shutil.copytree(
                template,
                task,
                ignore=shutil.ignore_patterns("__pycache__", "*.pyc"),
            )
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
                    "`vela next . --json`, `vela show . <id> --json`, "
                    "`vela why . <claim-id> --json`, and "
                    "`vela review show . <id> --json`."
                )
            render(
                task / "instruction.md",
                {
                    "TOOL_GUIDANCE": guidance,
                    "SCENARIO_INSTRUCTION": scenario_instruction,
                    "OUTPUT_INSTRUCTION": (
                        "Report the accepted transition, exact next Target, and authority-boundary "
                        "semantics. Do not run "
                        "the Target or act on any pending Proposal."
                        if fixture["scenario"] == SCENARIOS[2]
                        else (
                            "Report the exact absence. Do not invent or recommend a Target and do "
                            "not treat absence as a scientific or authority conclusion."
                            if fixture["scenario"] == SCENARIOS[3]
                            else
                            "Reject any typo or unrelated Proposal. Report its Submission, every scoped "
                            "Verification and nonclaim, the exact conditional Standing change, and all "
                            "three current/accept/reject next obligations. This Frontier has no configured "
                            "Target; do not invent one."
                        )
                    ),
                },
            )
            render(task / "task.toml", {"SCENARIO": fixture["scenario"], "ARM": arm})
            render(
                environment / "Dockerfile",
                {"CODEX_VERSION": codex_version, "VELA_INSTALL": vela_install},
            )
            task_rows.append({
                "path": task.relative_to(output).as_posix(),
                "root": tree_root(task),
            })
    finally:
        bundle.unlink(missing_ok=True)

    job = json_command(
        (
            "harbor", "run",
            "--path", "tasks",
            "--agent", "codex",
            "--model", model,
            "--agent-kwarg", f"version={codex_version}",
            "--n-attempts", "2",
            "--n-concurrent", "1",
            "--max-retries", "0",
            "--job-name", job_name,
            "--print-config",
        ),
        cwd=output,
    )
    contract.write_json(output / "harbor-job.json", job)
    plan = contract.seal({
        "schema": contract.PLAN_SCHEMA,
        "plan_root": "",
        "scenario": fixture["scenario"],
        "fixture_root": fixture["fixture_root"],
        "answer_key_root": answer_key["answer_key_root"],
        "task_roots": task_rows,
        "harbor_job_root": contract.sha256_root(contract.canonical_bytes(job)),
        "comparison_rule": COMPARISON,
        "claim_credit": False,
        "claim_limit": claim_limit,
    }, "plan_root")
    contract.write_json(output / "plan.json", plan)
    return plan


def materialize(
    frontier: Path,
    vela: Path,
    subject_id: str | None,
    scenario: str,
    vela_linux: Path,
    model: str,
    codex_version: str,
    job_name: str,
    output: Path,
) -> dict[str, Any]:
    fixture, answer_key, instruction, claim_limit = materialize_fixture(
        frontier, vela, subject_id, scenario,
    )
    return build_study(
        fixture, answer_key, instruction, claim_limit, frontier, vela_linux,
        model, codex_version, job_name, output,
    )


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--frontier", type=Path, required=True)
    result.add_argument("--vela", type=Path, required=True)
    identity = result.add_mutually_exclusive_group()
    identity.add_argument("--proposal")
    identity.add_argument("--accepted-claim")
    identity.add_argument("--absence", action="store_true")
    result.add_argument("--scenario", choices=SCENARIOS, required=True)
    result.add_argument("--vela-linux", type=Path, required=True)
    result.add_argument("--model", required=True)
    result.add_argument("--codex-version", required=True)
    result.add_argument("--job-name", default="vela-product-compression")
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        expected_identity = (
            "accepted_claim" if args.scenario == SCENARIOS[2]
            else "absence" if args.scenario == SCENARIOS[3]
            else "proposal"
        )
        supplied = [
            name for name, present in (
                ("proposal", args.proposal is not None),
                ("accepted_claim", args.accepted_claim is not None),
                ("absence", args.absence),
            ) if present
        ]
        if supplied != [expected_identity]:
            fail(f"{args.scenario} requires --{expected_identity.replace('_', '-')}")
        plan = materialize(
            args.frontier,
            args.vela,
            args.proposal or args.accepted_claim,
            args.scenario,
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
