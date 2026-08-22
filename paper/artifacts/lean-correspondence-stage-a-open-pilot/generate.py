#!/usr/bin/env python3
"""Generate the deterministic, held Stage A prelaunch package.

This script performs no provider, scoring, key, authority, or permit-release action.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
METHOD = ROOT.parent / "lean-correspondence-foundry-study"
METHOD_COMMIT = "8a999f5e8ca543531f5e1241fbdef391c78d068a"
METHOD_TREE = "85b43ec7023d16cac404a6f8b6c8d8117584027a"
IMPLEMENTATION_COMMIT = "01d0b3253227bc41d2edc13e5cb318bdae53fc88"
IMPLEMENTATION_TREE = "b91c967537126404011e9a628f98e9d0378f73e5"
CANDIDATE_COMMIT = "148e18cce542f397ccb60b21a896ba063f6d6cca"
CANDIDATE_TREE = "0d2e5533f41d2e54575a2d4b6e1ec9feabf998e7"
QUALIFIER_BLOB = "be1982fc09c8d859b7da131c242de243e6f989b8"
QUALIFIER_SHA256 = "628ac203a48ef19c649dd64dedc010d104d728eb0edbb66392e93955fab872b9"
SEED_TEXT = "vela-lean-correspondence-stage-a-open-pilot-v1\n"


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def canonical_root(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_bytes(value)).hexdigest()


def raw_root(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )


def git(repo: Path, *args: str, environment: dict[str, str] | None = None) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        capture_output=True,
        text=True,
        env=environment,
    )
    return result.stdout.strip()


def checked_object(repo: Path, commit: str, tree: str) -> None:
    if git(repo, "rev-parse", f"{commit}^{{tree}}") != tree:
        raise ValueError(f"tree mismatch for {commit}")


def extract_prompt_sections() -> tuple[str, dict[str, str]]:
    text = (METHOD / "PROMPT.md").read_text(encoding="utf-8")
    common_match = re.search(r"## Common prompt\n\n```text\n(.*?)```", text, re.DOTALL)
    if common_match is None:
        raise ValueError("method common prompt missing")
    common = common_match.group(1)
    preambles: dict[str, str] = {}
    for arm in ("raw-source", "correspondence-assisted"):
        match = re.search(rf"`{re.escape(arm)}`:\n\n```text\n(.*?)```", text, re.DOTALL)
        if match is None:
            raise ValueError(f"method preamble missing: {arm}")
        preambles[arm] = match.group(1)
    return common, preambles


def materialize_invalid_fixture() -> dict[str, Any]:
    fixture = ROOT / "invalid-fixture"
    if fixture.exists():
        shutil.rmtree(fixture)
    source_files = {
        "Basic.lean": b"def calibrationValue : Nat := 11\n#eval calibrationValue\n",
        "InvalidWitness.lean": b"example : (11 : Nat) = 12 := by decide\n",
        "lakefile.toml": b'name = "stage_a_invalid_source"\nversion = "0.1.0"\n\n[[lean_lib]]\nname = "Basic"\n',
        "lean-toolchain": b"leanprover/lean4:v4.19.0\n",
    }
    target_files = {
        "Basic.lean": b"def calibrationValue : Nat := 12\n#eval calibrationValue\n",
        "lakefile.toml": b'name = "stage_a_invalid_target"\nversion = "0.1.0"\n\n[[lean_lib]]\nname = "Basic"\n',
        "lean-toolchain": b"leanprover/lean4:v4.19.0\n",
    }
    repositories: dict[str, Any] = {}
    with tempfile.TemporaryDirectory(prefix="lc-stage-a-invalid-") as temporary:
        temporary_root = Path(temporary)
        for name, files in (("source", source_files), ("target", target_files)):
            retained = fixture / f"{name}-repo"
            repo = temporary_root / name
            retained.mkdir(parents=True)
            repo.mkdir(parents=True)
            for relative, raw in files.items():
                (retained / relative).write_bytes(raw)
                (repo / relative).write_bytes(raw)
            git(repo, "init", "-b", "main")
            git(repo, "config", "user.name", "Stage A invalid fixture")
            git(repo, "config", "user.email", "fixture@invalid.example")
            git(repo, "add", ".")
            environment = os.environ | {
                "GIT_AUTHOR_DATE": "2000-01-01T00:00:00Z",
                "GIT_COMMITTER_DATE": "2000-01-01T00:00:00Z",
            }
            git(
                repo, "commit", "-m", "stage-a-invalid-fixture", environment=environment
            )
            entries = []
            for relative in sorted(files):
                entries.append(
                    {
                        "path": relative,
                        "bytes": len(files[relative]),
                        "sha256": raw_root(files[relative]),
                        "git_blob": git(repo, "rev-parse", f"HEAD:{relative}"),
                    }
                )
            repositories[name] = {
                "repository_id": f"fixture/stage-a-invalid-{name}",
                "commit": git(repo, "rev-parse", "HEAD"),
                "tree": git(repo, "rev-parse", "HEAD^{tree}"),
                "files": entries,
            }

    source = repositories["source"]
    target = repositories["target"]
    source_basic = next(
        item for item in source["files"] if item["path"] == "Basic.lean"
    )
    target_basic = next(
        item for item in target["files"] if item["path"] == "Basic.lean"
    )
    witness_file = next(
        item for item in source["files"] if item["path"] == "InvalidWitness.lean"
    )
    record = {
        "schema_version": "lean-correspondence/relation/v0.2",
        "relation_id": "stage-a-open-calibration.distinct-numerals-byte-identity",
        "relation": "byte_identity",
        "state": "candidate",
        "source": {
            "repository": {
                "repository_id": source["repository_id"],
                "commit": source["commit"],
            },
            "file": "Basic.lean",
            "file_sha256": source_basic["sha256"].removeprefix("sha256:"),
            "declaration": {
                "name": "calibrationValue",
                "kind": "def",
                "start_line": 1,
                "end_line": 1,
            },
            "declaration_sha256": raw_root(
                b"def calibrationValue : Nat := 11\n"
            ).removeprefix("sha256:"),
            "environment": {
                "lean_toolchain": "leanprover/lean4:v4.19.0",
                "locked_files": [
                    {
                        "path": item["path"],
                        "sha256": item["sha256"].removeprefix("sha256:"),
                    }
                    for item in source["files"]
                    if item["path"] in {"lakefile.toml", "lean-toolchain"}
                ],
                "environment_root": "pending-kernel-derived",
            },
        },
        "target": {
            "repository": {
                "repository_id": target["repository_id"],
                "commit": target["commit"],
            },
            "file": "Basic.lean",
            "file_sha256": target_basic["sha256"].removeprefix("sha256:"),
            "declaration": {
                "name": "calibrationValue",
                "kind": "def",
                "start_line": 1,
                "end_line": 1,
            },
            "declaration_sha256": raw_root(
                b"def calibrationValue : Nat := 12\n"
            ).removeprefix("sha256:"),
            "environment": {
                "lean_toolchain": "leanprover/lean4:v4.19.0",
                "locked_files": [
                    {
                        "path": item["path"],
                        "sha256": item["sha256"].removeprefix("sha256:"),
                    }
                    for item in target["files"]
                    if item["path"] in {"lakefile.toml", "lean-toolchain"}
                ],
                "environment_root": "pending-kernel-derived",
            },
        },
        "assumptions": [],
        "adapters": [],
        "witness": {
            "kind": "lean_command",
            "scope": None,
            "command": ["lake", "env", "lean", "InvalidWitness.lean"],
            "repository_role": "source",
            "repositories": [],
            "expected_stdout_sha256": None,
            "timeout_seconds": 120,
        },
        "depends_on": [],
        "invalidation": {"status": "current", "reason": None},
    }
    # The environment-root algorithm is the kernel's canonical root over these fields.
    for endpoint in (record["source"], record["target"]):
        endpoint["environment"]["environment_root"] = canonical_root(
            {
                "lean_toolchain": endpoint["environment"]["lean_toolchain"],
                "locked_files": sorted(
                    endpoint["environment"]["locked_files"],
                    key=lambda item: item["path"],
                ),
            }
        ).removeprefix("sha256:")
    record_root = canonical_root(record)
    write_json(fixture / "candidate-relation.json", record)
    failure_receipt = {
        "schema": "vela.lean-correspondence-stage-a-invalid-witness-receipt.v1",
        "relation_record_root": record_root,
        "command": record["witness"]["command"],
        "expected_exit": "nonzero",
        "observed_exit": 1,
        "stdout_sha256": raw_root(b""),
        "stderr_contract": "Lean type mismatch proving 11 is not 12; exact stderr is runtime-local and not scored",
        "outcome": "witness_failed_as_designed",
        "authority_effect": "none",
    }
    write_json(fixture / "witness-failure-receipt.json", failure_receipt)
    impact = {
        "schema_version": "lean-correspondence/impact/v0.1",
        "changed": [record["relation_id"]],
        "affected": [
            {
                "relation_id": record["relation_id"],
                "record_root": record_root.removeprefix("sha256:"),
                "distance": 0,
                "source_repository_id": source["repository_id"],
                "source_commit": source["commit"],
            }
        ],
        "scope": "explicit_record_dependencies_only",
        "authority_claim": "none",
    }
    write_json(fixture / "impact.json", impact)
    metadata = {
        "schema": "vela.lean-correspondence-stage-a-invalid-fixture.v1",
        "case_id": "deliberately-invalid-byte-identity",
        "participant_visible_id": "open-calibration-03",
        "repositories": repositories,
        "relation_record_root": record_root,
        "witness_source_sha256": witness_file["sha256"],
        "witness_failure_receipt_root": canonical_root(failure_receipt),
        "impact_root": canonical_root(impact),
        "both_declarations_compile": True,
        "distinct_numerals": True,
        "claimed_relation": "byte_identity",
        "witness_must_fail": True,
        "authority_effect": "none",
    }
    write_json(fixture / "fixture.json", metadata)
    return metadata


def file_entry(repo: Path, path: str) -> dict[str, Any]:
    raw = (repo / path).read_bytes()
    return {"path": path, "bytes": len(raw), "sha256": raw_root(raw)}


def build_cases(
    implementation: Path, candidates: Path, invalid: dict[str, Any]
) -> dict[str, Any]:
    case_specs = [
        {
            "case_id": "erdos-730-affirmative-rhs",
            "participant_visible_id": "open-calibration-01",
            "candidate_packet": "lean-correspondence-v0/cases/erdos-730/packet.json",
            "candidate_packet_sha256": "sha256:3276bfe9762ed81cf2cd700eb1e75a7dd67e68308659d499209879b16758b0ed",
            "base_paths": [
                "lean-correspondence-v0/cases/erdos-730/Witness.lean",
                "lean-correspondence-v0/cases/erdos-730/packet.json",
                "lean-toolchain",
                "lake-manifest.json",
            ],
            "relation_path": "cases/records/erdos-730.affirmative-rhs-defeq.relation.json",
            "receipt_path": "cases/receipts/erdos-730.affirmative-rhs-defeq.receipt.json",
            "impact_path": None,
            "claim_ceiling": "affirmative S.Infinite RHS only; not the answer(sorry) biconditional",
            "allowed_impact_ids": ["erdos-730.affirmative-rhs-defeq"],
        },
        {
            "case_id": "fc-leaneval-oeis-303656",
            "participant_visible_id": "open-calibration-02",
            "candidate_packet": "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/packet.json",
            "candidate_packet_sha256": "sha256:c2300410edf5aa255223d5685adc2ea4cad8ce95873f26b246c0bb125134e7cd",
            "base_paths": [
                "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/HistoricalRenameWitness.lean",
                "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/packet.json",
                "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/frozen/context/OeisA303656_conjecture.lean",
                "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/frozen/generated/Challenge.lean",
                "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/frozen/generated/ChallengeDeps.lean",
                "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/frozen/generated/lakefile.toml",
                "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/frozen/generated/lean-toolchain",
                "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/frozen/request.json",
                "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/frozen/generated-files.sha256",
                "lean-toolchain",
                "lake-manifest.json",
            ],
            "relation_path": "cases/records/oeis-303656.fc-to-leaneval-generated-lineage.relation.json",
            "receipt_path": "cases/receipts/oeis-303656.fc-to-leaneval-generated-lineage.receipt.json",
            "impact_path": "cases/reports/oeis-303656.impact.json",
            "claim_ceiling": "deterministic generated-byte lineage and bounded drift calibration; not truth, solution, or general semantic equivalence",
            "allowed_impact_ids": [
                "oeis-303656.historical-helper-rename-defeq",
                "oeis-303656.fc-to-leaneval-generated-lineage",
            ],
        },
    ]
    cases: list[dict[str, Any]] = []
    for spec in case_specs:
        base_atoms = [file_entry(candidates, path) for path in spec.pop("base_paths")]
        derived = [
            file_entry(implementation, spec["relation_path"]),
            file_entry(implementation, spec["receipt_path"]),
        ]
        if spec["impact_path"]:
            derived.append(file_entry(implementation, spec["impact_path"]))
        semantic_atom_root = canonical_root(base_atoms)
        cases.append(
            {
                **spec,
                "source_repository": "https://github.com/williamjblair/lean-proofs.git",
                "source_commit": CANDIDATE_COMMIT,
                "source_tree": CANDIDATE_TREE,
                "base_atoms": base_atoms,
                "semantic_atom_root": semantic_atom_root,
                "derived_mechanism_atoms": derived,
                "derived_mechanism_root": canonical_root(derived),
                "authority_effect": "none",
            }
        )
    invalid_base = []
    for side in ("source", "target"):
        for entry in invalid["repositories"][side]["files"]:
            invalid_base.append(
                {
                    "path": f"invalid-fixture/{side}-repo/{entry['path']}",
                    "bytes": entry["bytes"],
                    "sha256": entry["sha256"],
                }
            )
    invalid_derived = [
        file_entry(ROOT, "invalid-fixture/candidate-relation.json"),
        file_entry(ROOT, "invalid-fixture/witness-failure-receipt.json"),
        file_entry(ROOT, "invalid-fixture/impact.json"),
    ]
    cases.append(
        {
            "case_id": "deliberately-invalid-byte-identity",
            "participant_visible_id": "open-calibration-03",
            "source_repository": "retained deterministic fixture",
            "source_commit": invalid["repositories"]["source"]["commit"],
            "source_tree": invalid["repositories"]["source"]["tree"],
            "target_commit": invalid["repositories"]["target"]["commit"],
            "target_tree": invalid["repositories"]["target"]["tree"],
            "base_atoms": sorted(invalid_base, key=lambda item: item["path"]),
            "semantic_atom_root": canonical_root(
                sorted(invalid_base, key=lambda item: item["path"])
            ),
            "derived_mechanism_atoms": invalid_derived,
            "derived_mechanism_root": canonical_root(invalid_derived),
            "claim_ceiling": "semantic invalidity of one fixed distinct-numeral calibration relation only",
            "allowed_impact_ids": [
                "stage-a-open-calibration.distinct-numerals-byte-identity"
            ],
            "authority_effect": "none",
        }
    )
    value = {
        "schema": "vela.lean-correspondence-stage-a-case-selection.v1",
        "stage": "A_open_non_ceiling",
        "fixed_case_count": 3,
        "stage_b_eligibility": "permanently_excluded",
        "cases": cases,
        "authority_effect": "none",
    }
    write_json(ROOT / "case-selection.json", value)
    return value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--implementation", type=Path, required=True)
    parser.add_argument("--candidates", type=Path, required=True)
    args = parser.parse_args()
    implementation = args.implementation.resolve()
    candidates = args.candidates.resolve()
    checked_object(implementation, IMPLEMENTATION_COMMIT, IMPLEMENTATION_TREE)
    checked_object(candidates, CANDIDATE_COMMIT, CANDIDATE_TREE)
    if git(implementation, "rev-parse", "HEAD") != IMPLEMENTATION_COMMIT:
        raise ValueError("implementation checkout not exact")
    if git(candidates, "rev-parse", "HEAD") != CANDIDATE_COMMIT:
        raise ValueError("candidate checkout not exact")

    invalid = materialize_invalid_fixture()
    cases = build_cases(implementation, candidates, invalid)
    common_prompt, preambles = extract_prompt_sections()
    response_schema_raw = (METHOD / "response.schema.json").read_bytes()
    (ROOT / "response.schema.json").write_bytes(response_schema_raw)

    method_binding = {
        "schema": "vela.lean-correspondence-stage-a-method-binding.v1",
        "vela_repository": "https://github.com/vela-science/vela.git",
        "method_commit": METHOD_COMMIT,
        "method_tree": METHOD_TREE,
        "method_artifact_path": "paper/artifacts/lean-correspondence-foundry-study",
        "method_artifact_root": "sha256:2d909b874eedc765546010e799d6fde709c88f3fcc623b45ab46130c3dfa68e4",
        "prompt_source_sha256": "sha256:111f9eb8c03a18d2c08f692591da2f27f2c13a1f098486f6aeb8d40e4f5fd6db",
        "response_schema_sha256": raw_root(response_schema_raw),
        "authority_effect": "none",
    }
    write_json(ROOT / "method-binding.json", method_binding)
    evidence = {
        "schema": "vela.lean-correspondence-stage-a-evidence-bindings.v1",
        "neutral_implementation": {
            "repository": "https://github.com/vela-science/lean-correspondence.git",
            "commit": IMPLEMENTATION_COMMIT,
            "tree": IMPLEMENTATION_TREE,
            "publication_branch": "main",
            "reviewed_import_blob": "b2aff34cbbdd8e56ff47ac4dc4b14fd09edf0388",
            "reviewed_import_sha256": raw_root(
                (implementation / "source/reviewed-packets/IMPORT.json").read_bytes()
            ),
            "cases_root_file_sha256": raw_root(
                (implementation / "cases/ROOTS.json").read_bytes()
            ),
            "cases_manifest_sha256": raw_root(
                (implementation / "cases/MANIFEST.sha256").read_bytes()
            ),
            "publication_import_independent_review": "PASS",
        },
        "candidate_packets": {
            "repository": "https://github.com/williamjblair/lean-proofs.git",
            "commit": CANDIDATE_COMMIT,
            "tree": CANDIDATE_TREE,
            "subtree": "lean-correspondence-v0",
            "independent_review": "PASS_GO_FOR_IMPORT",
        },
        "maintained_qualifier": {
            "path": "tools/evidence_qualification/qualification.py",
            "blob": QUALIFIER_BLOB,
            "sha256": f"sha256:{QUALIFIER_SHA256}",
            "copied": False,
        },
        "invalid_fixture_root": canonical_root(invalid),
        "authority_effect": "none",
    }
    write_json(ROOT / "evidence-bindings.json", evidence)

    runtime = {
        "schema": "vela.lean-correspondence-stage-a-runtime-binding.v1",
        "status": "blocked_missing_exact_two_provider_tool_runtime_qualification",
        "required_participant_configuration_count": 2,
        "required_distinct_provider_organizations": 2,
        "required_capabilities": [
            "cold_fresh_session",
            "read_only_offline_shell_and_file_tools",
            "closed_json_response",
            "1200_second_hard_timeout",
            "raw_provider_events_usage_stderr_terminal_teardown_custody",
        ],
        "configuration_slots": [
            {
                "slot_id": "configuration-a",
                "provider_organization": None,
                "immutable_model_snapshot": None,
                "configuration_root": None,
                "qualification_receipt_root": None,
                "status": "unbound",
            },
            {
                "slot_id": "configuration-b",
                "provider_organization": None,
                "immutable_model_snapshot": None,
                "configuration_root": None,
                "qualification_receipt_root": None,
                "status": "unbound",
            },
        ],
        "rejected_runtime_evidence": {
            "prior_image_digest": "sha256:f75ed4428ee3ab3f3275db0378e7375c1364f8b9f06d2f1bb4158502a84d4fc1",
            "prior_review_commit": "4a06ac8aa9a5f07abd019a375d755bfe5f0031aa",
            "reason": "single OpenAI/Codex provider, tools disabled, different prompt and response contract; transitive reuse would violate the reviewed Stage A method",
        },
        "maintained_qualifier_receipt_root": None,
        "provider_calls": 0,
        "participant_calls": 0,
        "execution_authorized": False,
        "authority_effect": "none",
    }
    write_json(ROOT / "runtime-binding.json", runtime)

    configs = {
        "schema": "vela.lean-correspondence-stage-a-participant-configurations.v1",
        "status": "blocked_unbound",
        "slots": runtime["configuration_slots"],
        "information_boundary": {
            "network_from_participant": False,
            "source_mounts": "read_only",
            "same_shell_file_tools": True,
            "same_prompt_schema_token_timeout_and_semantic_atoms": True,
        },
        "provider_calls": 0,
        "authority_effect": "none",
    }
    write_json(ROOT / "participant-configurations.json", configs)
    slot_roots = {slot["slot_id"]: canonical_root(slot) for slot in configs["slots"]}

    arms = ("raw-source", "correspondence-assisted")
    schedule_rows = []
    prompt_roots: dict[str, str] = {}
    packet_roots: dict[str, str] = {}
    assignment_index = 0
    for case in cases["cases"]:
        for arm in arms:
            for slot in ("configuration-a", "configuration-b"):
                assignment_index += 1
                key = f"{slot}|{case['participant_visible_id']}|{arm}"
                assignment_id = (
                    f"lc-a-{assignment_index:02d}-"
                    + hashlib.sha256((SEED_TEXT + key).encode()).hexdigest()[:12]
                )
                prompt = preambles[arm] + "\n" + common_prompt
                prompt = prompt.replace("[assignment_id]", assignment_id).replace(
                    "[opaque_family_id]", case["participant_visible_id"]
                )
                prompt_path = ROOT / "prompts" / f"{assignment_id}.txt"
                prompt_path.parent.mkdir(parents=True, exist_ok=True)
                prompt_path.write_text(prompt, encoding="utf-8")
                prompt_root = raw_root(prompt.encode("utf-8"))
                prompt_roots[assignment_id] = prompt_root
                packet = {
                    "schema": "vela.lean-correspondence-stage-a-packet-manifest.v1",
                    "assignment_id": assignment_id,
                    "participant_visible_case_id": case["participant_visible_id"],
                    "arm_exposed_to_participant": False,
                    "base_semantic_atoms": case["base_atoms"],
                    "semantic_atom_root": case["semantic_atom_root"],
                    "derived_mechanism_atoms": (
                        [] if arm == "raw-source" else case["derived_mechanism_atoms"]
                    ),
                    "allowed_impact_ids": case["allowed_impact_ids"],
                    "response_schema_sha256": raw_root(response_schema_raw),
                    "read_only": True,
                    "answer_key_present": False,
                    "authority_effect": "none",
                }
                packet_path = ROOT / "packets" / f"{assignment_id}.json"
                write_json(packet_path, packet)
                packet_root = canonical_root(packet)
                packet_roots[assignment_id] = packet_root
                schedule_rows.append(
                    {
                        "ordinal": assignment_index,
                        "assignment_id": assignment_id,
                        "configuration_slot": slot,
                        "configuration_slot_root": slot_roots[slot],
                        "case_id": case["case_id"],
                        "participant_visible_case_id": case["participant_visible_id"],
                        "arm": arm,
                        "fresh_session": True,
                        "attempt": 1,
                        "timeout_seconds": 1200,
                        "prompt_root": prompt_root,
                        "packet_root": packet_root,
                    }
                )

    schedule_body = {
        "schema": "vela.lean-correspondence-stage-a-assignment-schedule.v1",
        "seed_sha256": raw_root(SEED_TEXT.encode()),
        "fixed_denominator": 12,
        "rows": schedule_rows,
        "zero_retries": True,
        "zero_substitutions": True,
        "authority_effect": "none",
    }
    schedule = {**schedule_body, "assignment_root": canonical_root(schedule_body)}
    write_json(ROOT / "assignment-schedule.json", schedule)

    atom_ledger = {
        "schema": "vela.lean-correspondence-stage-a-atom-equivalence.v1",
        "cases": [
            {
                "case_id": item["case_id"],
                "semantic_atom_root": item["semantic_atom_root"],
                "raw_semantic_atom_root": item["semantic_atom_root"],
                "assisted_semantic_atom_root": item["semantic_atom_root"],
                "assisted_only_derived_root": item["derived_mechanism_root"],
                "derivation_rule": "Lean Correspondence v0.2 relation verification, receipt, recheck, and explicit dependency impact over the exact base atoms",
                "protected_label_present": False,
                "answer_key_present": False,
            }
            for item in cases["cases"]
        ],
        "information_equivalent": True,
        "authority_effect": "none",
    }
    write_json(ROOT / "atom-equivalence.json", atom_ledger)

    registration_contract = {
        "schema": "vela.lean-correspondence-stage-a-registration-contract.v1",
        "stage": "A_open_non_ceiling",
        "method_binding_root": canonical_root(method_binding),
        "evidence_binding_root": canonical_root(evidence),
        "case_selection_root": canonical_root(cases),
        "assignment_root": schedule["assignment_root"],
        "atom_equivalence_root": canonical_root(atom_ledger),
        "runtime_binding_root": canonical_root(runtime),
        "participant_configurations_root": canonical_root(configs),
        "response_schema_sha256": raw_root(response_schema_raw),
        "fixed_denominator": 12,
        "arms": list(arms),
        "participant_configuration_slots": 2,
        "cases": 3,
        "fresh_sessions_per_cell": 1,
        "timeout_seconds": 1200,
        "zero_retries": True,
        "zero_substitutions": True,
        "protected_stage_b_material_created": False,
        "provider_calls_authorized": False,
        "scoring_authorized": False,
        "authority_effect": "none",
    }
    registration_contract_root = canonical_root(registration_contract)
    permits = []
    permit_dir = ROOT / "permits"
    if permit_dir.exists():
        shutil.rmtree(permit_dir)
    permit_dir.mkdir()
    for row in schedule_rows:
        permit = {
            "schema": "vela.lean-correspondence-stage-a-held-permit.v1",
            "registration_contract_root": registration_contract_root,
            "assignment_root": schedule["assignment_root"],
            "assignment_id": row["assignment_id"],
            "configuration_slot_root": row["configuration_slot_root"],
            "packet_root": row["packet_root"],
            "prompt_root": row["prompt_root"],
            "response_schema_sha256": raw_root(response_schema_raw),
            "attempt": 1,
            "timeout_seconds": 1200,
            "status": "held",
            "releasable": False,
            "consumed": False,
            "runtime_qualification_receipt_root": None,
            "authority_effect": "none",
        }
        write_json(permit_dir / f"{row['assignment_id']}.permit.json", permit)
        permits.append(
            {
                "assignment_id": row["assignment_id"],
                "permit_root": canonical_root(permit),
                "status": "held",
            }
        )
    permit_set_root = canonical_root(permits)
    hold_state = {
        "schema": "vela.lean-correspondence-stage-a-hold-state.v1",
        "registration_contract_root": registration_contract_root,
        "assignment_root": schedule["assignment_root"],
        "fixed_denominator": 12,
        "held": 12,
        "released": 0,
        "consumed": 0,
        "terminal": 0,
        "permit_set_root": permit_set_root,
        "permits": permits,
        "provider_calls": 0,
        "scoring_attempts": 0,
        "key_accesses": 0,
        "authority_effect": "none",
    }
    write_json(ROOT / "hold-state.json", hold_state)
    registration_body = {
        **registration_contract,
        "registration_contract_root": registration_contract_root,
        "permit_set_root": permit_set_root,
        "hold_state_root": canonical_root(hold_state),
        "status": "blocked_prelaunch_review_only",
    }
    registration = {
        **registration_body,
        "registration_root": canonical_root(registration_body),
    }
    write_json(ROOT / "registration.json", registration)

    custody = {
        "schema": "vela.lean-correspondence-stage-a-custody-contract.v1",
        "registration_root": registration["registration_root"],
        "maintained_qualifier": evidence["maintained_qualifier"],
        "required_terminal_files": [
            "consumed-permit.json",
            "launch.json",
            "provider-events.jsonl",
            "provider-stderr.txt",
            "participant-response.raw.json",
            "usage.json",
            "terminal-receipt.json",
            "teardown.json",
        ],
        "raw_response_preserved": True,
        "closed_response_schema": raw_root(response_schema_raw),
        "one_permit_outstanding": True,
        "capture_commit_before_next_release": True,
        "zero_retries": True,
        "zero_substitutions": True,
        "scoring_semantics": {
            "relation_validation": "exact closed label against the fixed open calibration adjudication",
            "change_classification": "exact closed label against the fixed open calibration adjudication",
            "impact_closure": "closed unique set: every allowed item exactly once, no missing duplicate or unknown id, exact disposition and nonempty supplied evidence bindings",
            "false_inference": "any authority or scientific claim above the registered case ceiling is an error",
            "composite_exact": "all three correctness components and no false inference",
            "failure_timeout_malformed": "retained in the fixed denominator",
            "restricted_seconds": "min(actual_elapsed_seconds, 1200); missing or nonterminal failure assigned 1200",
            "one_scoring_attempt": True,
            "decimal_rounding": "ROUND_HALF_EVEN",
        },
        "protected_stage_b_key_created": False,
        "provider_calls": 0,
        "authority_effect": "none",
    }
    write_json(ROOT / "custody-contract.json", custody)
    state = {
        "schema": "vela.lean-correspondence-stage-a-prelaunch-state.v1",
        "state": "blocked_missing_exact_two_provider_tool_runtime_qualification",
        "registration_root": registration["registration_root"],
        "assignment_root": schedule["assignment_root"],
        "permit_set_root": permit_set_root,
        "fixed_denominator": 12,
        "held_permits": 12,
        "released_permits": 0,
        "terminal_captures": 0,
        "participant_configurations_bound": 0,
        "runtime_qualification_receipt_root": None,
        "independent_prelaunch_review": "not_requested",
        "provider_calls": 0,
        "participant_responses": 0,
        "scoring_attempts": 0,
        "key_accesses": 0,
        "stage_b_families_selected": 0,
        "protected_stage_b_key_created": False,
        "execution_authorized": False,
        "authority_effect": "none",
    }
    write_json(ROOT / "prelaunch-state.json", state)

    # Manifest every retained file except the manifest itself.
    manifest_entries = []
    for path in sorted(ROOT.rglob("*")):
        if (
            not path.is_file()
            or path.name == "artifact-manifest.json"
            or "__pycache__" in path.parts
        ):
            continue
        raw = path.read_bytes()
        manifest_entries.append(
            {
                "path": path.relative_to(ROOT).as_posix(),
                "bytes": len(raw),
                "sha256": raw_root(raw),
            }
        )
    manifest = {
        "schema": "vela.lean-correspondence-stage-a-artifact-manifest.v1",
        "entries": manifest_entries,
        "artifact_root": canonical_root(manifest_entries),
        "authority_effect": "none",
    }
    write_json(ROOT / "artifact-manifest.json", manifest)
    print(
        json.dumps(
            {
                "artifact_root": manifest["artifact_root"],
                "registration_root": registration["registration_root"],
                "assignment_root": schedule["assignment_root"],
                "permit_set_root": permit_set_root,
                "state": state["state"],
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
