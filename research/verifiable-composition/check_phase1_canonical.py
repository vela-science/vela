#!/usr/bin/env python3
"""Focused, offline Phase 1A canonical-custody and decision-evidence checks."""

from __future__ import annotations

import argparse
import copy
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "reference"))

from exact_checkout import (  # noqa: E402
    CompositionError,
    ExactGitCheckout,
    canonical_bytes,
    derived_event_id,
    event_content_root,
    inspect_named_decision_python,
    inspect_canonical_checkout,
    strict_json,
    verify_canonical_materialization,
)


PHASE1A = ROOT / "registration/phase1a-canonical-custody.json"
ABLATION = ROOT / "registration/decision-evidence-ablation.json"
CANONICAL_VECTORS = ROOT / "vectors/canonical-custody-cases.json"
DECISION_VECTORS = ROOT / "vectors/decision-evidence-cases.json"
CANONICAL_FIXTURE = ROOT / "fixtures/canonical-custody/fixture.json"
DECISION_FIXTURE = ROOT / "fixtures/decision-inspection/fixture.json"
EXPECTED_METRICS = {
    "false_verified_count",
    "check_strict_ok",
    "proof_verify_ok",
    "git_commit_exact",
    "git_tree_exact",
    "replay_current_replayed_root_parity",
    "replay_proof_event_log_root_parity",
    "replay_proof_snapshot_root_parity",
    "visible_view_root_parity",
    "lock_root_parity",
    "registered_vector_pass_count",
}
EXPECTED_ABLATION_METRICS = {
    "false_verified_count",
    "rust_python_classification_parity",
    "root_only_classification",
    "retained_preimage_classification",
    "replay_root_equal_between_arms",
    "decision_event_id_equal_between_arms",
    "registered_vector_pass_count",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def read_json(path: Path) -> dict[str, Any]:
    value = strict_json(path.read_bytes(), label=path.name)
    require(isinstance(value, dict), f"{path} must contain one object")
    return value


def validate_registration(
    phase1a: dict[str, Any],
    ablation: dict[str, Any],
    canonical_vectors: dict[str, Any],
    decision_vectors: dict[str, Any],
) -> None:
    require(
        phase1a.get("schema") == "vela.verifiable-composition.phase1a-registration.v0",
        "Phase 1A registration schema drift",
    )
    require(phase1a.get("run_class") == "internal_fixture", "run class widened")
    require(
        phase1a.get("release", {}).get("tag") == "v0.800.13"
        and phase1a.get("release", {}).get("commit")
        == "b3076f8935a38ecaef252e7f062648794cc7cd07",
        "released Vela pin drift",
    )
    require(
        set(phase1a.get("registered_metrics", [])) == EXPECTED_METRICS,
        "unregistered or missing Phase 1A metric",
    )
    canonical_cases = canonical_vectors.get("cases")
    require(isinstance(canonical_cases, list), "canonical vectors missing")
    canonical_ids = [case.get("id") for case in canonical_cases if isinstance(case, dict)]
    require(len(canonical_ids) == len(set(canonical_ids)), "duplicate canonical vector id")
    require(
        canonical_ids == phase1a.get("registered_vectors", {}).get("ids"),
        "unregistered or missing canonical vector",
    )
    require(
        ablation.get("schema")
        == "vela.verifiable-composition.decision-evidence-ablation.v0",
        "ablation registration schema drift",
    )
    require(
        set(ablation.get("registered_metrics", [])) == EXPECTED_ABLATION_METRICS,
        "unregistered or missing ablation metric",
    )
    arms = ablation.get("arms")
    require(
        isinstance(arms, list)
        and [(arm.get("id"), arm.get("name")) for arm in arms]
        == [("R", "root_only"), ("P", "retained_preimage")],
        "unregistered, missing, or reordered ablation arm",
    )
    decision_cases = decision_vectors.get("cases")
    require(isinstance(decision_cases, list), "decision vectors missing")
    decision_ids = [case.get("id") for case in decision_cases if isinstance(case, dict)]
    require(len(decision_ids) == len(set(decision_ids)), "duplicate decision vector id")
    require(
        decision_ids == ablation.get("registered_vectors", {}).get("ids"),
        "unregistered or missing decision vector",
    )


def registration_negative_tests(
    phase1a: dict[str, Any],
    ablation: dict[str, Any],
    canonical_vectors: dict[str, Any],
    decision_vectors: dict[str, Any],
) -> None:
    mutations: list[tuple[str, tuple[dict[str, Any], ...]]] = []
    metric = copy.deepcopy(phase1a)
    metric["registered_metrics"].append("unregistered_metric")
    mutations.append(("metric", (metric, ablation, canonical_vectors, decision_vectors)))
    vector = copy.deepcopy(canonical_vectors)
    vector["cases"].append(
        {"id": "unregistered-vector", "mutation": "none", "expected": "rejected"}
    )
    mutations.append(("vector", (phase1a, ablation, vector, decision_vectors)))
    arm = copy.deepcopy(ablation)
    arm["arms"].append({"id": "X", "name": "unregistered"})
    mutations.append(("arm", (phase1a, arm, canonical_vectors, decision_vectors)))
    for label, arguments in mutations:
        try:
            validate_registration(*arguments)
        except AssertionError:
            continue
        raise AssertionError(f"registration validator accepted unregistered {label}")


def write_json(path: Path, value: Any) -> None:
    path.write_bytes(json.dumps(value, indent=2, sort_keys=True).encode() + b"\n")


def replace_root(path: Path, field: str, root: str) -> None:
    text = path.read_text()
    lines = []
    replaced = False
    for line in text.splitlines():
        if line.startswith(f"{field}:"):
            lines.append(f"{field}: {root}")
            replaced = True
        else:
            lines.append(line)
    require(replaced, f"{field} absent from {path}")
    path.write_text("\n".join(lines) + "\n")


def mutate_canonical(frontier: Path, mutation: str) -> None:
    events = sorted((frontier / ".vela/events").glob("*.json"))
    findings = sorted((frontier / ".vela/findings").glob("*.json"))
    forged = "sha256:" + "f" * 64
    if mutation == "none":
        return
    if mutation == "frontier_json_only":
        value = read_json(frontier / "frontier.json")
        value["frontier"]["name"] = "fabricated-visible-name"
        write_json(frontier / "frontier.json", value)
        return
    if mutation == "canonical_event_content":
        value = read_json(events[0])
        value["reason"] = "mutated canonical event"
        write_json(events[0], value)
        return
    if mutation == "canonical_event_delete":
        events[0].unlink()
        return
    if mutation == "canonical_event_duplicate":
        shutil.copyfile(events[0], frontier / ".vela/events/vev_0000000000000000.json")
        return
    if mutation == "lock_snapshot_root":
        replace_root(frontier / "vela.lock", "snapshot_hash", forged)
        return
    if mutation == "proof_event_root":
        value = read_json(frontier / "proof/latest.json")
        value["event_log_hash"] = forged
        write_json(frontier / "proof/latest.json", value)
        return
    if mutation == "fabricate_derived_views":
        visible = read_json(frontier / "frontier.json")
        visible["_meta"]["event_log_hash"] = forged
        visible["_meta"]["snapshot_hash"] = forged
        write_json(frontier / "frontier.json", visible)
        latest = read_json(frontier / "proof/latest.json")
        latest["event_log_hash"] = forged
        latest["frontier_hash"] = forged
        write_json(frontier / "proof/latest.json", latest)
        hashes = read_json(frontier / "proof/hashes.json")
        hashes["event_log_hash"] = forged
        hashes["snapshot_hash"] = forged
        write_json(frontier / "proof/hashes.json", hashes)
        replace_root(frontier / "vela.lock", "event_log_hash", forged)
        replace_root(frontier / "vela.lock", "snapshot_hash", forged)
        return
    if mutation == "canonical_events_missing":
        for event in events:
            event.unlink()
        return
    if mutation == "canonical_finding_delete":
        findings[0].unlink()
        return
    raise AssertionError(f"unknown canonical mutation {mutation}")


def run_canonical_vectors(
    registration: dict[str, Any],
    vectors: dict[str, Any],
    fixture: dict[str, Any],
    executable: Path,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    checkout = ExactGitCheckout(REPO, registration["inputs"]["repository_commit"])
    require(
        checkout.subtree_oid(fixture["source_path"]) == fixture["source_tree"],
        "canonical fixture Git tree drift",
    )
    outcomes = []
    baseline: dict[str, Any] | None = None
    for case in vectors["cases"]:
        if case["mutation"] in {
            "exact_tree_mismatch",
            "git_symlink_entry",
            "destination_symlink",
            "checkout_oversize",
        }:
            result = run_materializer_vector(
                checkout, fixture, executable, case["mutation"]
            )
            actual = result["code"] if result.get("ok") else "rejected"
            require(
                actual == case["expected"],
                f"canonical vector {case['id']}: expected {case['expected']}, got {result}",
            )
            outcomes.append(
                {
                    "id": case["id"],
                    "expected": case["expected"],
                    "actual": actual,
                    "code": result["code"],
                }
            )
            continue
        with tempfile.TemporaryDirectory(prefix="vela-adr4-canonical-") as directory:
            frontier = Path(directory) / "frontier"
            identity = checkout.materialize_subtree(fixture["source_path"], frontier)
            mutate_canonical(frontier, case["mutation"])
            result = verify_canonical_materialization(frontier, executable)
            actual = result["code"] if result.get("ok") else "rejected"
            require(
                actual == case["expected"],
                f"canonical vector {case['id']}: expected {case['expected']}, got {result}",
            )
            outcomes.append(
                {
                    "id": case["id"],
                    "expected": case["expected"],
                    "actual": actual,
                    "code": result["code"],
                }
            )
            if case["mutation"] == "none":
                baseline = {**result, "git": identity}
    require(baseline is not None and baseline.get("ok") is True, "baseline absent")
    return outcomes, baseline


def run_materializer_vector(
    checkout: ExactGitCheckout,
    fixture: dict[str, Any],
    executable: Path,
    mutation: str,
) -> dict[str, Any]:
    try:
        with tempfile.TemporaryDirectory(prefix="vela-adr4-materializer-") as directory:
            root = Path(directory)
            destination = root / "frontier"
            if mutation == "exact_tree_mismatch":
                return inspect_canonical_checkout(
                    REPO,
                    checkout.commit,
                    fixture["source_path"],
                    destination,
                    executable,
                    expected_frontier_tree="0" * 40,
                )
            if mutation == "destination_symlink":
                outside = root / "outside"
                outside.mkdir()
                os.symlink(outside, destination)
                checkout.materialize_subtree(fixture["source_path"], destination)
            elif mutation == "checkout_oversize":
                checkout.materialize_subtree(
                    fixture["source_path"], destination, max_tree_bytes=1
                )
            elif mutation == "git_symlink_entry":
                source = root / "source"
                subprocess.run(
                    ["git", "init", "-q", str(source)],
                    check=True,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.PIPE,
                )
                frontier = source / "frontier"
                (frontier / ".vela").mkdir(parents=True)
                (frontier / "proof").mkdir()
                (frontier / ".vela/config.toml").write_text("schema = 'fixture'\n")
                (frontier / "frontier.json").write_text("{}\n")
                (frontier / "vela.lock").write_text("schema: fixture\n")
                (frontier / "proof/latest.json").write_text("{}\n")
                os.symlink("frontier.json", frontier / "escape-link")
                subprocess.run(
                    ["git", "-C", str(source), "add", "frontier"],
                    check=True,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.PIPE,
                )
                subprocess.run(
                    [
                        "git",
                        "-C",
                        str(source),
                        "-c",
                        "user.name=ADR004 fixture",
                        "-c",
                        "user.email=fixture@invalid",
                        "commit",
                        "-q",
                        "-m",
                        "fixture",
                    ],
                    check=True,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.PIPE,
                )
                commit = subprocess.run(
                    ["git", "-C", str(source), "rev-parse", "HEAD"],
                    check=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                ).stdout.decode().strip()
                ExactGitCheckout(source, commit).materialize_subtree(
                    "frontier", destination
                )
            else:
                raise AssertionError(f"unknown materializer vector {mutation}")
    except CompositionError as error:
        return {
            "ok": False,
            "status": "rejected",
            "code": error.code,
            "detail": error.detail,
        }
    except (OSError, subprocess.SubprocessError) as error:
        return {
            "ok": False,
            "status": "rejected",
            "code": "fixture:setup_failed",
            "detail": type(error).__name__,
        }
    return {
        "ok": True,
        "status": "verified",
        "code": "canonical_custody_verified",
    }


def sign_event(event: dict[str, Any], seed_hex: str) -> None:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

    fields = (
        "schema",
        "id",
        "kind",
        "target",
        "actor",
        "timestamp",
        "reason",
        "before_hash",
        "after_hash",
        "payload",
        "caveats",
    )
    body = canonical_bytes({field: event[field] for field in fields})
    payload_type = b"application/vnd.vela.event+json"
    framed = (
        b"DSSEv1 "
        + str(len(payload_type)).encode()
        + b" "
        + payload_type
        + b" "
        + str(len(body)).encode()
        + b" "
        + body
    )
    signature = Ed25519PrivateKey.from_private_bytes(bytes.fromhex(seed_hex)).sign(framed)
    event["signature"] = f"v1:{signature.hex()}"


def readdress_decision(case: dict[str, Any]) -> None:
    project = case["project"]
    old_id = case["event_id"]
    event = next(event for event in project["events"] if event["id"] == old_id)
    event["id"] = derived_event_id(event)
    sign_event(event, case["test_key_seed_hex"])
    case["event_id"] = event["id"]
    case["content_root"] = event_content_root(event)


def mutate_decision(case: dict[str, Any], mutation: str) -> None:
    project = case["project"]
    decision = next(event for event in project["events"] if event["id"] == case["event_id"])
    actor = project["actors"][0]
    preimage = case["preimage"]
    forged = "sha256:" + "f" * 64
    if mutation == "none":
        return
    if mutation == "remove_preimage":
        case["preimage"] = None
        return
    if mutation == "decision_event_id":
        case["event_id"] = "vev_0000000000000000"
        return
    if mutation == "decision_content_root":
        case["content_root"] = forged
        return
    if mutation == "event_id_rederivation":
        decision["id"] = "vev_0000000000000000"
        case["event_id"] = decision["id"]
        return
    if mutation == "actor_remove":
        project["actors"] = []
        return
    if mutation == "actor_duplicate":
        project["actors"].append(copy.deepcopy(actor))
        return
    if mutation == "actor_agent":
        decision["actor"] = {"id": "agent:decision-inspection-test", "type": "human"}
        readdress_decision(case)
        return
    if mutation == "actor_namespace":
        actor["id"] = "scientist:decision-inspection-test"
        decision["actor"]["id"] = actor["id"]
        readdress_decision(case)
        return
    if mutation == "actor_registered_after":
        actor["created_at"] = "2100-01-01T00:00:00Z"
        return
    if mutation == "actor_revoked_at":
        actor["revoked_at"] = decision["timestamp"]
        actor["revoked_reason"] = "fixed hostile vector"
        return
    if mutation == "actor_revoked_after":
        actor["revoked_at"] = "2100-01-01T00:00:00Z"
        actor["revoked_reason"] = "fixed valid historical vector"
        return
    if mutation == "actor_public_key":
        actor["public_key"] = "00" * 32
        return
    if mutation == "decision_signature":
        decision["signature"] = "v1:" + "00" * 64
        return
    if mutation == "applied_event_signature":
        applied = next(
            event
            for event in project["events"]
            if event["id"] == decision["payload"]["applied_event_id"]
        )
        applied["signature"] = "v1:" + "00" * 64
        return
    if mutation == "decision_root_remove":
        decision["payload"]["provenance"]["input_refs"] = []
        readdress_decision(case)
        return
    if mutation == "decision_root_duplicate":
        refs = decision["payload"]["provenance"]["input_refs"]
        refs.append(refs[0])
        readdress_decision(case)
        return
    if mutation == "proposal_link":
        decision["target"]["id"] = "vpr_0000000000000000"
        readdress_decision(case)
        return
    if mutation == "applied_event_link":
        decision["payload"]["applied_event_id"] = "vev_0000000000000000"
        readdress_decision(case)
        return
    if mutation == "preimage_version":
        preimage["decision_preimage_version"] = "vela.decision-plan.internal.v2"
        return
    if mutation == "preimage_reason":
        preimage["ordered_answers"][0]["reason"] = "tampered"
        return
    if mutation == "preimage_event_log_root":
        preimage["expected_event_log_root"] = forged
        return
    if mutation == "preimage_proposal_root":
        preimage["ordered_answers"][0]["proposal_root"] = forged
        preimage["consumed_fact_roots"][0]["proposal_root"] = forged
        return
    if mutation == "preimage_receipt_root":
        preimage["consumed_fact_roots"][0]["receipt_root"] = forged
        return
    if mutation == "preimage_verifier_root":
        preimage["consumed_fact_roots"][0]["verifier_snapshot_root"] = forged
        return
    if mutation == "preimage_policy_root":
        preimage["consumed_fact_roots"][0]["policy_input_root"] = forged
        return
    if mutation == "preimage_authority_root":
        preimage["consumed_fact_roots"][0]["reviewer_authority_root"] = forged
        return
    if mutation == "preimage_impact_root":
        preimage["consumed_fact_roots"][0]["downstream_impact_root"] = forged
        return
    if mutation == "post_decision_attachment":
        later = copy.deepcopy(project["events"][0])
        later["timestamp"] = "2100-01-02T00:00:00Z"
        later["reason"] = "post-decision verifier attachment"
        later["target"] = {"type": "verifier_attachment", "id": "vva_postdecision000"}
        later["payload"] = {"attachment_id": "vva_postdecision000"}
        later["id"] = derived_event_id(later)
        project["events"].append(later)
        return
    raise AssertionError(f"unknown decision mutation {mutation}")


def run_decision_vectors(
    vectors: dict[str, Any], fixture: dict[str, Any]
) -> tuple[list[dict[str, str]], dict[str, Any], dict[str, Any]]:
    outcomes = []
    retained: dict[str, Any] | None = None
    root_only: dict[str, Any] | None = None
    for vector in vectors["cases"]:
        case = copy.deepcopy(fixture)
        mutate_decision(case, vector["mutation"])
        preimage = (
            None
            if case["preimage"] is None
            else canonical_bytes(case["preimage"])
        )
        result = inspect_named_decision_python(
            case["project"], case["event_id"], case["content_root"], preimage
        )
        require(
            result["code"] == vector["expected"],
            f"decision vector {vector['id']}: expected {vector['expected']}, got {result}",
        )
        outcomes.append(
            {
                "id": vector["id"],
                "expected": vector["expected"],
                "actual": result["code"],
            }
        )
        if vector["id"] == "retained-preimage-baseline":
            retained = result
        if vector["id"] == "root-only-preimage-absent":
            root_only = result
    require(retained is not None and root_only is not None, "ablation arms absent")
    require(retained["ok"] is True, "retained-preimage arm did not verify")
    require(
        root_only["code"] == "unresolvable:decision_preimage_unavailable",
        "root-only arm did not expose the registered gap",
    )
    require(
        retained["decision_event_id"] == root_only["decision_event_id"]
        and retained["decision_root"] == root_only["decision_root"],
        "ablation changed the signed decision",
    )
    return outcomes, retained, root_only


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true", required=True)
    parser.add_argument(
        "--vela",
        type=Path,
        required=True,
        help=(
            "explicit pre-registered offline Vela 0.800.13 executable; "
            "the release-mismatch check rejects every other version"
        ),
    )
    args = parser.parse_args()

    phase1a = read_json(PHASE1A)
    ablation = read_json(ABLATION)
    canonical_vectors = read_json(CANONICAL_VECTORS)
    decision_vectors = read_json(DECISION_VECTORS)
    canonical_fixture = read_json(CANONICAL_FIXTURE)
    decision_fixture = read_json(DECISION_FIXTURE)
    validate_registration(phase1a, ablation, canonical_vectors, decision_vectors)
    registration_negative_tests(
        phase1a, ablation, canonical_vectors, decision_vectors
    )
    canonical_outcomes, baseline = run_canonical_vectors(
        phase1a, canonical_vectors, canonical_fixture, args.vela
    )
    decision_outcomes, retained, root_only = run_decision_vectors(
        decision_vectors, decision_fixture
    )

    false_verified = sum(
        1
        for outcome in canonical_outcomes
        if outcome["expected"] == "rejected"
        and outcome["actual"] == "canonical_custody_verified"
    ) + sum(
        1
        for outcome in decision_outcomes
        if not outcome["expected"].startswith("verified:")
        and outcome["actual"].startswith("verified:")
    )
    require(false_verified == 0, f"false verified outcomes: {false_verified}")
    report = {
        "schema": "vela.verifiable-composition.phase1a-result.v0",
        "ok": True,
        "false_verified_count": false_verified,
        "canonical_vectors": len(canonical_outcomes),
        "decision_vectors": len(decision_outcomes),
        "canonical_roots": baseline["roots"],
        "git": baseline["git"],
        "runner": baseline["runner"],
        "arms": {
            "R": root_only["code"],
            "P": retained["code"],
            "same_decision_event_id": retained["decision_event_id"],
            "same_decision_root": retained["decision_root"],
        },
        "promotion": {
            "read_only_inspector": "eligible_after_rust_vector_parity",
            "retained_preimage": "experiment_supports_retention_without_authority_change",
            "dependency_or_status_primitive": "do_not_promote",
        },
    }
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
