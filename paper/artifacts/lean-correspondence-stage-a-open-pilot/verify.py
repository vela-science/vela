#!/usr/bin/env python3
"""Verify the Stage A prelaunch package without executing a participant."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

import generate
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parent
REQUIRED_TOP_LEVEL = {
    "README.md",
    "artifact-manifest.json",
    "assignment-schedule.json",
    "atom-equivalence.json",
    "case-selection.json",
    "custody-contract.json",
    "evidence-bindings.json",
    "generate.py",
    "hold-state.json",
    "invalid-fixture",
    "method-binding.json",
    "packets",
    "participant-configurations.json",
    "permits",
    "prelaunch-state.json",
    "prompts",
    "registration.json",
    "response.schema.json",
    "runtime-binding.json",
    "test_verify.py",
    "verify.py",
}


class VerificationError(ValueError):
    pass


def require(condition: bool, code: str) -> None:
    if not condition:
        raise VerificationError(code)


def load_json(path: Path) -> Any:
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in items:
            if key in value:
                raise VerificationError(f"duplicate_json_key:{path.name}:{key}")
            value[key] = item
        return value

    try:
        return json.loads(path.read_bytes(), object_pairs_hook=pairs)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise VerificationError(f"json_invalid:{path.name}") from error


def raw_root(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def verify_manifest() -> str:
    manifest = load_json(ROOT / "artifact-manifest.json")
    require(
        set(manifest) == {"schema", "entries", "artifact_root", "authority_effect"},
        "manifest_fields",
    )
    require(
        manifest["schema"] == "vela.lean-correspondence-stage-a-artifact-manifest.v1",
        "manifest_schema",
    )
    require(manifest["authority_effect"] == "none", "manifest_authority")
    actual_paths = {
        path.relative_to(ROOT).as_posix()
        for path in ROOT.rglob("*")
        if path.is_file()
        and path.name != "artifact-manifest.json"
        and "__pycache__" not in path.parts
    }
    entries = manifest["entries"]
    require(isinstance(entries, list) and entries, "manifest_entries")
    paths = [item.get("path") for item in entries]
    require(len(paths) == len(set(paths)), "manifest_duplicate_path")
    require(set(paths) == actual_paths, "manifest_inventory")
    for entry in entries:
        require(set(entry) == {"path", "bytes", "sha256"}, "manifest_entry_fields")
        path = ROOT / entry["path"]
        raw = path.read_bytes()
        require(len(raw) == entry["bytes"], f"manifest_bytes:{entry['path']}")
        require(raw_root(raw) == entry["sha256"], f"manifest_sha256:{entry['path']}")
    require(
        generate.canonical_root(entries) == manifest["artifact_root"],
        "artifact_root",
    )
    return manifest["artifact_root"]


def verify_method_and_evidence(
    vela_repo: Path, implementation: Path, candidates: Path
) -> None:
    method = load_json(ROOT / "method-binding.json")
    evidence = load_json(ROOT / "evidence-bindings.json")
    require(method["method_commit"] == generate.METHOD_COMMIT, "method_commit")
    require(method["method_tree"] == generate.METHOD_TREE, "method_tree")
    require(
        generate.git(vela_repo, "rev-parse", f"{generate.METHOD_COMMIT}^{{tree}}")
        == generate.METHOD_TREE,
        "method_git_tree",
    )
    require(
        method["method_artifact_root"]
        == "sha256:2d909b874eedc765546010e799d6fde709c88f3fcc623b45ab46130c3dfa68e4",
        "method_artifact_root",
    )
    response_raw = (ROOT / "response.schema.json").read_bytes()
    method_response_raw = (
        vela_repo
        / "paper/artifacts/lean-correspondence-foundry-study/response.schema.json"
    ).read_bytes()
    require(response_raw == method_response_raw, "response_schema_method_drift")
    Draft202012Validator.check_schema(load_json(ROOT / "response.schema.json"))
    require(
        method["response_schema_sha256"] == raw_root(response_raw),
        "response_schema_root",
    )
    require(evidence["maintained_qualifier"]["copied"] is False, "qualifier_copied")
    qualifier = vela_repo / evidence["maintained_qualifier"]["path"]
    require(
        raw_root(qualifier.read_bytes()) == evidence["maintained_qualifier"]["sha256"],
        "qualifier_sha256",
    )
    require(
        generate.git(
            vela_repo,
            "rev-parse",
            f"{generate.METHOD_COMMIT}:{evidence['maintained_qualifier']['path']}",
        )
        == evidence["maintained_qualifier"]["blob"],
        "qualifier_blob",
    )
    require(
        generate.git(implementation, "rev-parse", "HEAD")
        == generate.IMPLEMENTATION_COMMIT,
        "implementation_checkout",
    )
    require(
        generate.git(implementation, "rev-parse", "HEAD^{tree}")
        == generate.IMPLEMENTATION_TREE,
        "implementation_tree",
    )
    require(
        generate.git(candidates, "rev-parse", "HEAD") == generate.CANDIDATE_COMMIT,
        "candidate_checkout",
    )
    require(
        generate.git(candidates, "rev-parse", "HEAD^{tree}") == generate.CANDIDATE_TREE,
        "candidate_tree",
    )
    neutral = evidence["neutral_implementation"]
    require(
        neutral["commit"] == generate.IMPLEMENTATION_COMMIT,
        "evidence_implementation_commit",
    )
    require(
        neutral["tree"] == generate.IMPLEMENTATION_TREE, "evidence_implementation_tree"
    )
    require(
        neutral["publication_import_independent_review"] == "PASS",
        "implementation_review",
    )
    require(
        neutral["reviewed_import_sha256"]
        == raw_root(
            (implementation / "source/reviewed-packets/IMPORT.json").read_bytes()
        ),
        "import_sha256",
    )
    candidate = evidence["candidate_packets"]
    require(candidate["commit"] == generate.CANDIDATE_COMMIT, "candidate_commit")
    require(candidate["tree"] == generate.CANDIDATE_TREE, "candidate_evidence_tree")
    require(candidate["independent_review"] == "PASS_GO_FOR_IMPORT", "candidate_review")
    require(evidence["authority_effect"] == "none", "evidence_authority")


def reconstruct_fixture_repository(
    side: str,
) -> tuple[Path, tempfile.TemporaryDirectory[str]]:
    temporary = tempfile.TemporaryDirectory(prefix=f"lc-stage-a-verify-{side}-")
    repo = Path(temporary.name) / side
    shutil.copytree(ROOT / "invalid-fixture" / f"{side}-repo", repo)
    generate.git(repo, "init", "-b", "main")
    generate.git(repo, "config", "user.name", "Stage A invalid fixture")
    generate.git(repo, "config", "user.email", "fixture@invalid.example")
    generate.git(repo, "add", ".")
    environment = os.environ | {
        "GIT_AUTHOR_DATE": "2000-01-01T00:00:00Z",
        "GIT_COMMITTER_DATE": "2000-01-01T00:00:00Z",
    }
    generate.git(
        repo, "commit", "-m", "stage-a-invalid-fixture", environment=environment
    )
    return repo, temporary


def verify_invalid_fixture(check_lean: bool) -> None:
    fixture = load_json(ROOT / "invalid-fixture/fixture.json")
    relation = load_json(ROOT / "invalid-fixture/candidate-relation.json")
    receipt = load_json(ROOT / "invalid-fixture/witness-failure-receipt.json")
    require(fixture["both_declarations_compile"] is True, "fixture_compile_contract")
    require(fixture["distinct_numerals"] is True, "fixture_distinct_contract")
    require(fixture["witness_must_fail"] is True, "fixture_failure_contract")
    require(relation["relation"] == "byte_identity", "fixture_relation")
    require(relation["state"] == "candidate", "fixture_state")
    require(relation["witness"]["kind"] == "lean_command", "fixture_witness_kind")
    require(
        receipt["relation_record_root"] == generate.canonical_root(relation),
        "fixture_receipt_relation_root",
    )
    require(
        receipt["outcome"] == "witness_failed_as_designed", "fixture_receipt_outcome"
    )
    repositories: list[tuple[Path, tempfile.TemporaryDirectory[str]]] = []
    try:
        for side in ("source", "target"):
            repo, temporary = reconstruct_fixture_repository(side)
            repositories.append((repo, temporary))
            expected = fixture["repositories"][side]
            require(
                generate.git(repo, "rev-parse", "HEAD") == expected["commit"],
                f"fixture_commit:{side}",
            )
            require(
                generate.git(repo, "rev-parse", "HEAD^{tree}") == expected["tree"],
                f"fixture_tree:{side}",
            )
            for entry in expected["files"]:
                raw = (repo / entry["path"]).read_bytes()
                require(
                    raw_root(raw) == entry["sha256"],
                    f"fixture_sha256:{side}:{entry['path']}",
                )
                require(
                    generate.git(repo, "rev-parse", f"HEAD:{entry['path']}")
                    == entry["git_blob"],
                    f"fixture_blob:{side}:{entry['path']}",
                )
        if check_lean:
            source_repo = repositories[0][0]
            target_repo = repositories[1][0]
            for repo in (source_repo, target_repo):
                result = subprocess.run(
                    ["lake", "env", "lean", "Basic.lean"],
                    cwd=repo,
                    check=False,
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                require(result.returncode == 0, "fixture_declaration_compile")
            failure = subprocess.run(
                relation["witness"]["command"],
                cwd=source_repo,
                check=False,
                capture_output=True,
                text=True,
                timeout=120,
            )
            require(failure.returncode != 0, "fixture_witness_unexpected_pass")
    finally:
        for _, temporary in repositories:
            temporary.cleanup()


def verify_cases_and_atoms() -> dict[str, dict[str, Any]]:
    selection = load_json(ROOT / "case-selection.json")
    ledger = load_json(ROOT / "atom-equivalence.json")
    require(selection["fixed_case_count"] == 3, "case_count")
    require(
        selection["stage_b_eligibility"] == "permanently_excluded",
        "stage_b_case_exclusion",
    )
    cases = selection["cases"]
    ids = [item["case_id"] for item in cases]
    visible = [item["participant_visible_id"] for item in cases]
    require(len(ids) == len(set(ids)) == 3, "case_id_unique")
    require(len(visible) == len(set(visible)) == 3, "visible_case_id_unique")
    require(
        ids
        == [
            "erdos-730-affirmative-rhs",
            "fc-leaneval-oeis-303656",
            "deliberately-invalid-byte-identity",
        ],
        "case_substitution",
    )
    require(
        all(
            not any(
                token in item.lower()
                for token in ("invalid", "valid", "answer", "gold")
            )
            for item in visible
        ),
        "participant_case_answer_leakage",
    )
    ledger_by_id = {item["case_id"]: item for item in ledger["cases"]}
    require(set(ledger_by_id) == set(ids), "atom_ledger_case_set")
    require(ledger["information_equivalent"] is True, "atom_equivalence_false")
    for case in cases:
        base = case["base_atoms"]
        derived = case["derived_mechanism_atoms"]
        require(
            generate.canonical_root(base) == case["semantic_atom_root"],
            f"semantic_atom_root:{case['case_id']}",
        )
        require(
            generate.canonical_root(derived) == case["derived_mechanism_root"],
            f"derived_atom_root:{case['case_id']}",
        )
        item = ledger_by_id[case["case_id"]]
        require(
            item["semantic_atom_root"] == case["semantic_atom_root"],
            f"ledger_atom_root:{case['case_id']}",
        )
        require(
            item["raw_semantic_atom_root"] == item["assisted_semantic_atom_root"],
            f"arm_atom_mismatch:{case['case_id']}",
        )
        require(
            item["protected_label_present"] is False,
            f"protected_label:{case['case_id']}",
        )
        require(item["answer_key_present"] is False, f"answer_key:{case['case_id']}")
    return {item["case_id"]: item for item in cases}


def verify_external_case_assets(
    cases: dict[str, dict[str, Any]], implementation: Path, candidates: Path
) -> None:
    relation_schema = load_json(implementation / "schemas/relation-v0.2.schema.json")
    invalid_relation = load_json(ROOT / "invalid-fixture/candidate-relation.json")
    errors = list(Draft202012Validator(relation_schema).iter_errors(invalid_relation))
    require(not errors, "invalid_fixture_relation_schema")
    for case_id, case in cases.items():
        if case_id == "deliberately-invalid-byte-identity":
            for entry in case["base_atoms"]:
                path = ROOT / entry["path"]
                require(path.is_file(), f"invalid_base_missing:{entry['path']}")
                raw = path.read_bytes()
                require(
                    len(raw) == entry["bytes"], f"invalid_base_bytes:{entry['path']}"
                )
                require(
                    raw_root(raw) == entry["sha256"],
                    f"invalid_base_sha256:{entry['path']}",
                )
            continue
        for entry in case["base_atoms"]:
            path = candidates / entry["path"]
            require(path.is_file(), f"candidate_atom_missing:{entry['path']}")
            raw = path.read_bytes()
            require(len(raw) == entry["bytes"], f"candidate_atom_bytes:{entry['path']}")
            require(
                raw_root(raw) == entry["sha256"],
                f"candidate_atom_sha256:{entry['path']}",
            )
        for entry in case["derived_mechanism_atoms"]:
            path = implementation / entry["path"]
            require(path.is_file(), f"implementation_atom_missing:{entry['path']}")
            raw = path.read_bytes()
            require(
                len(raw) == entry["bytes"], f"implementation_atom_bytes:{entry['path']}"
            )
            require(
                raw_root(raw) == entry["sha256"],
                f"implementation_atom_sha256:{entry['path']}",
            )


def verify_schedule_packets_prompts(cases: dict[str, dict[str, Any]]) -> dict[str, Any]:
    schedule = load_json(ROOT / "assignment-schedule.json")
    body = {key: value for key, value in schedule.items() if key != "assignment_root"}
    require(
        generate.canonical_root(body) == schedule["assignment_root"], "assignment_root"
    )
    require(
        schedule["fixed_denominator"] == len(schedule["rows"]) == 12,
        "denominator_drift",
    )
    rows = schedule["rows"]
    assignment_ids = [row["assignment_id"] for row in rows]
    require(len(assignment_ids) == len(set(assignment_ids)), "duplicate_assignment_id")
    require([row["ordinal"] for row in rows] == list(range(1, 13)), "assignment_order")
    expected_cross_product = {
        (slot, case_id, arm)
        for slot in ("configuration-a", "configuration-b")
        for case_id in cases
        for arm in ("raw-source", "correspondence-assisted")
    }
    observed = {(row["configuration_slot"], row["case_id"], row["arm"]) for row in rows}
    require(observed == expected_cross_product, "assignment_cross_product")
    common, preambles = generate.extract_prompt_sections()
    packet_by_case_arm: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for row in rows:
        require(
            row["fresh_session"] is True and row["attempt"] == 1,
            "fresh_session_attempt",
        )
        require(row["timeout_seconds"] == 1200, "timeout_drift")
        prompt_raw = (ROOT / "prompts" / f"{row['assignment_id']}.txt").read_bytes()
        expected_prompt = (
            preambles[row["arm"]]
            + "\n"
            + common.replace("[assignment_id]", row["assignment_id"]).replace(
                "[opaque_family_id]", row["participant_visible_case_id"]
            )
        ).encode()
        require(prompt_raw == expected_prompt, f"prompt_drift:{row['assignment_id']}")
        require(
            raw_root(prompt_raw) == row["prompt_root"],
            f"prompt_root:{row['assignment_id']}",
        )
        packet = load_json(ROOT / "packets" / f"{row['assignment_id']}.json")
        require(
            generate.canonical_root(packet) == row["packet_root"],
            f"packet_root:{row['assignment_id']}",
        )
        require(packet["assignment_id"] == row["assignment_id"], "packet_assignment")
        require(packet["arm_exposed_to_participant"] is False, "arm_leakage")
        require(packet["answer_key_present"] is False, "answer_leakage")
        require(packet["authority_effect"] == "none", "packet_authority")
        case = cases[row["case_id"]]
        require(
            packet["semantic_atom_root"] == case["semantic_atom_root"],
            "packet_atom_root",
        )
        if row["arm"] == "raw-source":
            require(packet["derived_mechanism_atoms"] == [], "raw_derived_leakage")
        else:
            require(
                packet["derived_mechanism_atoms"] == case["derived_mechanism_atoms"],
                "assisted_derived_drift",
            )
        packet_by_case_arm.setdefault((row["case_id"], row["arm"]), []).append(packet)
    for case_id in cases:
        raw_packets = packet_by_case_arm[(case_id, "raw-source")]
        assisted_packets = packet_by_case_arm[(case_id, "correspondence-assisted")]
        require(
            len(raw_packets) == len(assisted_packets) == 2,
            "case_arm_configuration_count",
        )
        roots = {
            generate.canonical_root(packet["base_semantic_atoms"])
            for packet in [*raw_packets, *assisted_packets]
        }
        require(
            roots == {cases[case_id]["semantic_atom_root"]},
            f"arm_information_mismatch:{case_id}",
        )
    return schedule


def verify_registration_permits_state(schedule: dict[str, Any]) -> tuple[str, str]:
    runtime = load_json(ROOT / "runtime-binding.json")
    configurations = load_json(ROOT / "participant-configurations.json")
    registration = load_json(ROOT / "registration.json")
    hold = load_json(ROOT / "hold-state.json")
    state = load_json(ROOT / "prelaunch-state.json")
    custody = load_json(ROOT / "custody-contract.json")
    require(
        runtime["status"]
        == "blocked_missing_exact_two_provider_tool_runtime_qualification",
        "runtime_blocker_missing",
    )
    require(
        runtime["maintained_qualifier_receipt_root"] is None,
        "early_qualification_receipt",
    )
    require(
        runtime["provider_calls"] == runtime["participant_calls"] == 0,
        "runtime_call_count",
    )
    require(runtime["execution_authorized"] is False, "runtime_execution_authorized")
    require(configurations["status"] == "blocked_unbound", "configuration_status")
    require(len(configurations["slots"]) == 2, "configuration_slot_count")
    require(
        all(slot["status"] == "unbound" for slot in configurations["slots"]),
        "configuration_early_binding",
    )
    require(
        all(
            slot["qualification_receipt_root"] is None
            for slot in configurations["slots"]
        ),
        "configuration_early_qualification",
    )
    contract_keys = {
        "schema",
        "stage",
        "method_binding_root",
        "evidence_binding_root",
        "case_selection_root",
        "assignment_root",
        "atom_equivalence_root",
        "runtime_binding_root",
        "participant_configurations_root",
        "response_schema_sha256",
        "fixed_denominator",
        "arms",
        "participant_configuration_slots",
        "cases",
        "fresh_sessions_per_cell",
        "timeout_seconds",
        "zero_retries",
        "zero_substitutions",
        "protected_stage_b_material_created",
        "provider_calls_authorized",
        "scoring_authorized",
        "authority_effect",
    }
    contract = {key: registration[key] for key in contract_keys}
    contract_root = generate.canonical_root(contract)
    require(
        registration["registration_contract_root"] == contract_root,
        "registration_contract_root",
    )
    registration_body = {
        key: value for key, value in registration.items() if key != "registration_root"
    }
    require(
        generate.canonical_root(registration_body) == registration["registration_root"],
        "registration_root",
    )
    require(registration["fixed_denominator"] == 12, "registration_denominator")
    require(registration["provider_calls_authorized"] is False, "provider_authorized")
    require(registration["scoring_authorized"] is False, "scoring_authorized")
    require(
        registration["protected_stage_b_material_created"] is False,
        "protected_stage_b_material",
    )
    permit_files = sorted((ROOT / "permits").glob("*.permit.json"))
    require(len(permit_files) == 12, "permit_count")
    permit_rows = []
    seen_assignments: set[str] = set()
    for path in permit_files:
        permit = load_json(path)
        assignment = permit["assignment_id"]
        require(
            assignment not in seen_assignments, "duplicate_reused_permit_assignment"
        )
        seen_assignments.add(assignment)
        require(
            permit["registration_contract_root"] == contract_root, "permit_registration"
        )
        require(
            permit["assignment_root"] == schedule["assignment_root"],
            "permit_assignment_root",
        )
        require(permit["status"] == "held", "permit_not_held")
        require(
            permit["releasable"] is False and permit["consumed"] is False,
            "early_permit_release",
        )
        require(
            permit["runtime_qualification_receipt_root"] is None,
            "permit_early_qualification",
        )
        require(
            permit["attempt"] == 1 and permit["timeout_seconds"] == 1200,
            "permit_contract_drift",
        )
        permit_rows.append(
            {
                "assignment_id": assignment,
                "permit_root": generate.canonical_root(permit),
                "status": "held",
            }
        )
    permit_rows.sort(key=lambda item: item["assignment_id"])
    hold_rows = sorted(hold["permits"], key=lambda item: item["assignment_id"])
    require(permit_rows == hold_rows, "hold_permit_roots")
    permit_set_root = generate.canonical_root(hold["permits"])
    require(
        permit_set_root == hold["permit_set_root"] == registration["permit_set_root"],
        "permit_set_root",
    )
    require(hold["held"] == hold["fixed_denominator"] == 12, "hold_denominator")
    require(
        hold["released"] == hold["consumed"] == hold["terminal"] == 0,
        "hold_state_progress",
    )
    require(
        hold["provider_calls"] == hold["scoring_attempts"] == hold["key_accesses"] == 0,
        "hold_forbidden_action",
    )
    require(state["state"] == runtime["status"], "state_runtime_status")
    require(
        state["registration_root"] == registration["registration_root"],
        "state_registration",
    )
    require(state["assignment_root"] == schedule["assignment_root"], "state_assignment")
    require(state["permit_set_root"] == permit_set_root, "state_permits")
    require(
        state["held_permits"] == 12 and state["released_permits"] == 0, "state_hold"
    )
    require(state["participant_configurations_bound"] == 0, "state_configuration_count")
    require(
        state["runtime_qualification_receipt_root"] is None, "state_early_qualification"
    )
    require(
        state["provider_calls"] == state["participant_responses"] == 0, "state_calls"
    )
    require(state["scoring_attempts"] == state["key_accesses"] == 0, "state_score_key")
    require(state["stage_b_families_selected"] == 0, "stage_b_selection")
    require(state["execution_authorized"] is False, "state_execution_authorized")
    require(
        custody["registration_root"] == registration["registration_root"],
        "custody_registration",
    )
    require(custody["one_permit_outstanding"] is True, "custody_scheduler")
    require(custody["protected_stage_b_key_created"] is False, "custody_stage_b_key")
    require(custody["provider_calls"] == 0, "custody_provider_calls")
    return registration["registration_root"], permit_set_root


def verify_no_forbidden_outputs() -> None:
    forbidden_parts = {
        "captures",
        "responses",
        "scores",
        "scored-result",
        "adjudication-key",
        "protected-adjudication",
        "stage-b-selection",
        "consumed-permits",
    }
    for path in ROOT.rglob("*"):
        require(
            not any(part in forbidden_parts for part in path.parts),
            f"forbidden_output:{path.name}",
        )


def run(
    vela_repo: Path, implementation: Path, candidates: Path, *, check_lean: bool
) -> dict[str, Any]:
    require(
        {path.name for path in ROOT.iterdir() if path.name != "__pycache__"}
        == REQUIRED_TOP_LEVEL,
        "top_level_inventory",
    )
    artifact_root = verify_manifest()
    verify_method_and_evidence(vela_repo, implementation, candidates)
    verify_invalid_fixture(check_lean)
    cases = verify_cases_and_atoms()
    verify_external_case_assets(cases, implementation, candidates)
    schedule = verify_schedule_packets_prompts(cases)
    registration_root, permit_set_root = verify_registration_permits_state(schedule)
    verify_no_forbidden_outputs()
    return {
        "status": "PASS",
        "prelaunch_readiness": "BLOCKED_MISSING_EXACT_TWO_PROVIDER_TOOL_RUNTIME_QUALIFICATION",
        "artifact_root": artifact_root,
        "registration_root": registration_root,
        "assignment_root": schedule["assignment_root"],
        "permit_set_root": permit_set_root,
        "fixed_denominator": 12,
        "held_permits": 12,
        "released_permits": 0,
        "provider_calls": 0,
        "participant_responses": 0,
        "scoring_attempts": 0,
        "key_accesses": 0,
        "stage_b_families_selected": 0,
        "authority_effect": "none",
        "execution_authorized": False,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vela-repo", type=Path, default=ROOT.parents[2])
    parser.add_argument("--implementation", type=Path, required=True)
    parser.add_argument("--candidates", type=Path, required=True)
    parser.add_argument("--check-lean", action="store_true")
    args = parser.parse_args()
    print(
        json.dumps(
            run(
                args.vela_repo.resolve(),
                args.implementation.resolve(),
                args.candidates.resolve(),
                check_lean=args.check_lean,
            ),
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
