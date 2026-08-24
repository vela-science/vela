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
REQUIRED_RUNTIME_CAPABILITIES = [
    "cold_fresh_session",
    "read_only_offline_shell_and_file_tools",
    "closed_json_response",
    "1200_second_hard_timeout",
    "raw_provider_events_usage_stderr_terminal_teardown_custody",
]
EXPECTED_INFORMATION_BOUNDARY = {
    "network_from_participant": False,
    "source_mounts": "read_only",
    "same_shell_file_tools": True,
    "same_prompt_schema_token_timeout_and_semantic_atoms": True,
}
EXPECTED_RESPONSE_SCHEMA_ROOT = (
    "sha256:b2d9bee1c76bc1f25f134fd50697f4e4a820a36bd61a84081edd5c542d749268"
)
EXPECTED_CASE_CEILINGS = {
    "erdos-730-affirmative-rhs": (
        "affirmative S.Infinite RHS relation only; excludes the full Formal "
        "Conjectures biconditional, mathematical truth, scientific acceptance, "
        "authority, Decision, and Standing"
    ),
    "fc-leaneval-oeis-303656": (
        "deterministic generated-byte lineage and bounded drift calibration only; "
        "excludes mathematical truth, solution, general semantic equivalence, "
        "scientific acceptance, authority, Decision, and Standing"
    ),
    "deliberately-invalid-byte-identity": (
        "semantic invalidity of one fixed distinct-numeral calibration relation "
        "only; excludes general semantic claims, scientific acceptance, authority, "
        "Decision, and Standing"
    ),
}
EXPECTED_IMPACT_IDS = {
    "erdos-730-affirmative-rhs": ["erdos-730.affirmative-rhs-defeq"],
    "fc-leaneval-oeis-303656": [
        "oeis-303656.historical-helper-rename-defeq",
        "oeis-303656.fc-to-leaneval-generated-lineage",
    ],
    "deliberately-invalid-byte-identity": [
        "stage-a-open-calibration.distinct-numerals-byte-identity"
    ],
}
CASE_FIELDS = {
    "erdos-730-affirmative-rhs": {
        "allowed_impact_ids",
        "authority_effect",
        "base_atoms",
        "candidate_packet",
        "candidate_packet_sha256",
        "case_id",
        "claim_ceiling",
        "derived_mechanism_atoms",
        "derived_mechanism_root",
        "impact_path",
        "participant_visible_id",
        "receipt_path",
        "relation_path",
        "semantic_atom_root",
        "source_commit",
        "source_repository",
        "source_tree",
    },
    "fc-leaneval-oeis-303656": {
        "allowed_impact_ids",
        "authority_effect",
        "base_atoms",
        "candidate_packet",
        "candidate_packet_sha256",
        "case_id",
        "claim_ceiling",
        "derived_mechanism_atoms",
        "derived_mechanism_root",
        "impact_path",
        "participant_visible_id",
        "receipt_path",
        "relation_path",
        "semantic_atom_root",
        "source_commit",
        "source_repository",
        "source_tree",
    },
    "deliberately-invalid-byte-identity": {
        "allowed_impact_ids",
        "authority_effect",
        "base_atoms",
        "case_id",
        "claim_ceiling",
        "derived_mechanism_atoms",
        "derived_mechanism_root",
        "participant_visible_id",
        "semantic_atom_root",
        "source_commit",
        "source_repository",
        "source_tree",
        "target_commit",
        "target_tree",
    },
}
EXPECTED_CASE_BINDINGS = {
    "erdos-730-affirmative-rhs": {
        "participant_visible_id": "open-calibration-01",
        "source_repository": "https://github.com/williamjblair/lean-proofs.git",
        "source_commit": generate.CANDIDATE_COMMIT,
        "source_tree": generate.CANDIDATE_TREE,
        "candidate_packet": "lean-correspondence-v0/cases/erdos-730/packet.json",
        "relation_path": "cases/records/erdos-730.affirmative-rhs-defeq.relation.json",
        "receipt_path": "cases/receipts/erdos-730.affirmative-rhs-defeq.receipt.json",
        "impact_path": None,
    },
    "fc-leaneval-oeis-303656": {
        "participant_visible_id": "open-calibration-02",
        "source_repository": "https://github.com/williamjblair/lean-proofs.git",
        "source_commit": generate.CANDIDATE_COMMIT,
        "source_tree": generate.CANDIDATE_TREE,
        "candidate_packet": "lean-correspondence-v0/cases/fc-leaneval-oeis-303656/packet.json",
        "relation_path": "cases/records/oeis-303656.fc-to-leaneval-generated-lineage.relation.json",
        "receipt_path": "cases/receipts/oeis-303656.fc-to-leaneval-generated-lineage.receipt.json",
        "impact_path": "cases/reports/oeis-303656.impact.json",
    },
    "deliberately-invalid-byte-identity": {
        "participant_visible_id": "open-calibration-03",
        "source_repository": "retained deterministic fixture",
    },
}
FILE_ENTRY_FIELDS = {"path", "bytes", "sha256"}
EXPECTED_ATOM_PATHS = {
    "erdos-730-affirmative-rhs": {
        "base": [
            "lean-correspondence-v0/cases/erdos-730/Witness.lean",
            "lean-correspondence-v0/cases/erdos-730/packet.json",
            "lean-toolchain",
            "lake-manifest.json",
        ],
        "derived": [
            "cases/records/erdos-730.affirmative-rhs-defeq.relation.json",
            "cases/receipts/erdos-730.affirmative-rhs-defeq.receipt.json",
        ],
    },
    "fc-leaneval-oeis-303656": {
        "base": [
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
        "derived": [
            "cases/records/oeis-303656.fc-to-leaneval-generated-lineage.relation.json",
            "cases/receipts/oeis-303656.fc-to-leaneval-generated-lineage.receipt.json",
            "cases/reports/oeis-303656.impact.json",
        ],
    },
    "deliberately-invalid-byte-identity": {
        "base": [
            "invalid-fixture/source-repo/Basic.lean",
            "invalid-fixture/source-repo/InvalidWitness.lean",
            "invalid-fixture/source-repo/lakefile.toml",
            "invalid-fixture/source-repo/lean-toolchain",
            "invalid-fixture/target-repo/Basic.lean",
            "invalid-fixture/target-repo/lakefile.toml",
            "invalid-fixture/target-repo/lean-toolchain",
        ],
        "derived": [
            "invalid-fixture/candidate-relation.json",
            "invalid-fixture/witness-failure-receipt.json",
            "invalid-fixture/impact.json",
        ],
    },
}
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


def require_exact_fields(value: Any, fields: set[str], code: str) -> None:
    require(isinstance(value, dict) and set(value) == fields, code)


def require_exact_typed(value: Any, expected: Any, code: str) -> None:
    """Require recursively exact JSON shape, primitive type, and value."""
    require(type(value) is type(expected), code)
    if isinstance(expected, dict):
        require(set(value) == set(expected), code)
        for key, item in expected.items():
            require_exact_typed(value[key], item, f"{code}:{key}")
    elif isinstance(expected, list):
        require(len(value) == len(expected), code)
        for index, (actual_item, expected_item) in enumerate(zip(value, expected)):
            require_exact_typed(actual_item, expected_item, f"{code}:{index}")
    else:
        require(value == expected, code)


def require_string(value: Any, code: str) -> None:
    require(type(value) is str, code)


def require_int(value: Any, code: str) -> None:
    require(type(value) is int, code)


def require_sha256(value: Any, code: str) -> None:
    require_string(value, code)
    require(
        len(value) == 71
        and value.startswith("sha256:")
        and all(character in "0123456789abcdef" for character in value[7:]),
        code,
    )


def require_file_entries(value: Any, code: str) -> None:
    require(type(value) is list, code)
    for index, entry in enumerate(value):
        item_code = f"{code}:{index}"
        require_exact_fields(entry, FILE_ENTRY_FIELDS, item_code)
        require_string(entry["path"], f"{item_code}:path")
        require_int(entry["bytes"], f"{item_code}:bytes")
        require(entry["bytes"] >= 0, f"{item_code}:bytes")
        require_sha256(entry["sha256"], f"{item_code}:sha256")


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
    require(type(entries) is list and entries, "manifest_entries")
    paths = [item.get("path") for item in entries]
    require(len(paths) == len(set(paths)), "manifest_duplicate_path")
    require(set(paths) == actual_paths, "manifest_inventory")
    for entry in entries:
        require(set(entry) == {"path", "bytes", "sha256"}, "manifest_entry_fields")
        require_string(entry["path"], "manifest_entry_path")
        require_int(entry["bytes"], "manifest_entry_bytes_type")
        require_sha256(entry["sha256"], "manifest_entry_sha256_type")
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
    require_exact_fields(
        method,
        {
            "schema",
            "vela_repository",
            "method_commit",
            "method_tree",
            "method_artifact_path",
            "method_artifact_root",
            "prompt_source_sha256",
            "response_schema_sha256",
            "authority_effect",
        },
        "method_fields",
    )
    require_exact_fields(
        evidence,
        {
            "schema",
            "neutral_implementation",
            "candidate_packets",
            "maintained_qualifier",
            "invalid_fixture_root",
            "authority_effect",
        },
        "evidence_fields",
    )
    require(
        method["schema"] == "vela.lean-correspondence-stage-a-method-binding.v1",
        "method_schema",
    )
    require(
        evidence["schema"] == "vela.lean-correspondence-stage-a-evidence-bindings.v1",
        "evidence_schema",
    )
    neutral = evidence["neutral_implementation"]
    candidate = evidence["candidate_packets"]
    qualifier_binding = evidence["maintained_qualifier"]
    require_exact_fields(
        neutral,
        {
            "repository",
            "commit",
            "tree",
            "publication_branch",
            "reviewed_import_blob",
            "reviewed_import_sha256",
            "cases_root_file_sha256",
            "cases_manifest_sha256",
            "publication_import_independent_review",
        },
        "neutral_implementation_fields",
    )
    require_exact_fields(
        candidate,
        {"repository", "commit", "tree", "subtree", "independent_review"},
        "candidate_packet_fields",
    )
    require_exact_fields(
        qualifier_binding,
        {"path", "blob", "sha256", "copied"},
        "maintained_qualifier_fields",
    )
    require_exact_typed(
        {
            "vela_repository": method["vela_repository"],
            "method_artifact_path": method["method_artifact_path"],
            "authority_effect": method["authority_effect"],
        },
        {
            "vela_repository": "https://github.com/vela-science/vela.git",
            "method_artifact_path": "paper/artifacts/lean-correspondence-foundry-study",
            "authority_effect": "none",
        },
        "method_closed_values",
    )
    require_exact_typed(
        {
            "repository": neutral["repository"],
            "commit": neutral["commit"],
            "tree": neutral["tree"],
            "publication_branch": neutral["publication_branch"],
            "reviewed_import_blob": neutral["reviewed_import_blob"],
            "publication_import_independent_review": neutral[
                "publication_import_independent_review"
            ],
        },
        {
            "repository": "https://github.com/vela-science/lean-correspondence.git",
            "commit": generate.IMPLEMENTATION_COMMIT,
            "tree": generate.IMPLEMENTATION_TREE,
            "publication_branch": "main",
            "reviewed_import_blob": "b2aff34cbbdd8e56ff47ac4dc4b14fd09edf0388",
            "publication_import_independent_review": "PASS",
        },
        "neutral_implementation_closed_values",
    )
    require_exact_typed(
        candidate,
        {
            "repository": "https://github.com/williamjblair/lean-proofs.git",
            "commit": generate.CANDIDATE_COMMIT,
            "tree": generate.CANDIDATE_TREE,
            "subtree": "lean-correspondence-v0",
            "independent_review": "PASS_GO_FOR_IMPORT",
        },
        "candidate_packet_closed_values",
    )
    require_exact_typed(
        qualifier_binding,
        {
            "path": "tools/evidence_qualification/qualification.py",
            "blob": generate.QUALIFIER_BLOB,
            "sha256": f"sha256:{generate.QUALIFIER_SHA256}",
            "copied": False,
        },
        "maintained_qualifier_closed_values",
    )
    for field in (
        "reviewed_import_sha256",
        "cases_root_file_sha256",
        "cases_manifest_sha256",
    ):
        require_sha256(neutral[field], f"neutral_implementation_{field}_type")
    require_sha256(evidence["invalid_fixture_root"], "invalid_fixture_root_type")
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
    prompt_raw = (
        vela_repo / "paper/artifacts/lean-correspondence-foundry-study/PROMPT.md"
    ).read_bytes()
    require(
        method["prompt_source_sha256"]
        == raw_root(prompt_raw)
        == "sha256:111f9eb8c03a18d2c08f692591da2f27f2c13a1f098486f6aeb8d40e4f5fd6db",
        "prompt_source_root",
    )
    response_raw = (ROOT / "response.schema.json").read_bytes()
    method_response_raw = (
        vela_repo
        / "paper/artifacts/lean-correspondence-foundry-study/response.schema.json"
    ).read_bytes()
    require(response_raw == method_response_raw, "response_schema_method_drift")
    Draft202012Validator.check_schema(load_json(ROOT / "response.schema.json"))
    require(
        method["response_schema_sha256"]
        == raw_root(response_raw)
        == EXPECTED_RESPONSE_SCHEMA_ROOT,
        "response_schema_root",
    )
    require(qualifier_binding["copied"] is False, "qualifier_copied")
    qualifier = vela_repo / qualifier_binding["path"]
    require(
        raw_root(qualifier.read_bytes()) == qualifier_binding["sha256"],
        "qualifier_sha256",
    )
    require(
        generate.git(
            vela_repo,
            "rev-parse",
            f"{generate.METHOD_COMMIT}:{qualifier_binding['path']}",
        )
        == qualifier_binding["blob"],
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
    require(
        neutral["cases_root_file_sha256"]
        == raw_root((implementation / "cases/ROOTS.json").read_bytes()),
        "cases_root_file_sha256",
    )
    require(
        neutral["cases_manifest_sha256"]
        == raw_root((implementation / "cases/MANIFEST.sha256").read_bytes()),
        "cases_manifest_sha256",
    )
    require(candidate["commit"] == generate.CANDIDATE_COMMIT, "candidate_commit")
    require(candidate["tree"] == generate.CANDIDATE_TREE, "candidate_evidence_tree")
    require(candidate["independent_review"] == "PASS_GO_FOR_IMPORT", "candidate_review")
    require(
        evidence["invalid_fixture_root"]
        == generate.canonical_root(load_json(ROOT / "invalid-fixture/fixture.json")),
        "invalid_fixture_root",
    )
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
    impact = load_json(ROOT / "invalid-fixture/impact.json")
    require_exact_fields(
        fixture,
        {
            "schema",
            "case_id",
            "participant_visible_id",
            "claimed_relation",
            "both_declarations_compile",
            "distinct_numerals",
            "witness_must_fail",
            "repositories",
            "relation_record_root",
            "witness_failure_receipt_root",
            "impact_root",
            "witness_source_sha256",
            "authority_effect",
        },
        "fixture_fields",
    )
    require_exact_fields(
        fixture["repositories"], {"source", "target"}, "fixture_repositories_fields"
    )
    for side in ("source", "target"):
        repository = fixture["repositories"][side]
        require_exact_fields(
            repository,
            {"repository_id", "commit", "tree", "files"},
            f"fixture_repository_fields:{side}",
        )
        require_exact_typed(
            repository["repository_id"],
            f"fixture/stage-a-invalid-{side}",
            f"fixture_repository_id:{side}",
        )
        require_string(repository["commit"], f"fixture_commit_type:{side}")
        require_string(repository["tree"], f"fixture_tree_type:{side}")
        require(type(repository["files"]) is list, f"fixture_files_type:{side}")
        for index, entry in enumerate(repository["files"]):
            code = f"fixture_file:{side}:{index}"
            require_exact_fields(entry, {"path", "bytes", "sha256", "git_blob"}, code)
            require_string(entry["path"], f"{code}:path")
            require_int(entry["bytes"], f"{code}:bytes")
            require_sha256(entry["sha256"], f"{code}:sha256")
            require_string(entry["git_blob"], f"{code}:git_blob")
    require_exact_fields(
        relation,
        {
            "schema_version",
            "relation_id",
            "relation",
            "state",
            "source",
            "target",
            "assumptions",
            "adapters",
            "witness",
            "depends_on",
            "invalidation",
        },
        "fixture_relation_fields",
    )
    for side in ("source", "target"):
        endpoint = relation[side]
        require_exact_fields(
            endpoint,
            {
                "repository",
                "file",
                "file_sha256",
                "declaration",
                "declaration_sha256",
                "environment",
            },
            f"fixture_relation_endpoint_fields:{side}",
        )
        require_exact_fields(
            endpoint["repository"],
            {"repository_id", "commit"},
            f"fixture_relation_repository_fields:{side}",
        )
        require_exact_fields(
            endpoint["declaration"],
            {"name", "kind", "start_line", "end_line"},
            f"fixture_relation_declaration_fields:{side}",
        )
        require_exact_fields(
            endpoint["environment"],
            {"lean_toolchain", "locked_files", "environment_root"},
            f"fixture_relation_environment_fields:{side}",
        )
        require(type(endpoint["environment"]["locked_files"]) is list, "locked_files")
        for item in endpoint["environment"]["locked_files"]:
            require_exact_fields(
                item,
                {"path", "sha256"},
                f"fixture_relation_locked_file_fields:{side}",
            )
            require_string(item["path"], f"fixture_relation_locked_path:{side}")
            require_string(item["sha256"], f"fixture_relation_locked_sha256:{side}")
    require_exact_fields(
        relation["witness"],
        {
            "kind",
            "repository_role",
            "command",
            "timeout_seconds",
            "expected_stdout_sha256",
            "scope",
            "repositories",
        },
        "fixture_relation_witness_fields",
    )
    require_exact_fields(
        relation["invalidation"], {"status", "reason"}, "fixture_invalidation_fields"
    )
    require_exact_fields(
        receipt,
        {
            "schema",
            "relation_record_root",
            "command",
            "expected_exit",
            "observed_exit",
            "stdout_sha256",
            "stderr_contract",
            "outcome",
            "authority_effect",
        },
        "fixture_receipt_fields",
    )
    require_exact_fields(
        impact,
        {"schema_version", "changed", "scope", "affected", "authority_claim"},
        "fixture_impact_fields",
    )
    require(
        type(impact["affected"]) is list and len(impact["affected"]) == 1,
        "fixture_impact_affected",
    )
    require_exact_fields(
        impact["affected"][0],
        {
            "relation_id",
            "record_root",
            "distance",
            "source_repository_id",
            "source_commit",
        },
        "fixture_impact_affected_fields",
    )
    require_exact_typed(
        {
            "schema": fixture["schema"],
            "case_id": fixture["case_id"],
            "participant_visible_id": fixture["participant_visible_id"],
            "claimed_relation": fixture["claimed_relation"],
            "authority_effect": fixture["authority_effect"],
        },
        {
            "schema": "vela.lean-correspondence-stage-a-invalid-fixture.v1",
            "case_id": "deliberately-invalid-byte-identity",
            "participant_visible_id": "open-calibration-03",
            "claimed_relation": "byte_identity",
            "authority_effect": "none",
        },
        "fixture_closed_values",
    )
    require_exact_typed(
        {
            "schema_version": relation["schema_version"],
            "relation_id": relation["relation_id"],
            "relation": relation["relation"],
            "state": relation["state"],
            "assumptions": relation["assumptions"],
            "adapters": relation["adapters"],
            "depends_on": relation["depends_on"],
            "invalidation": relation["invalidation"],
        },
        {
            "schema_version": "lean-correspondence/relation/v0.2",
            "relation_id": "stage-a-open-calibration.distinct-numerals-byte-identity",
            "relation": "byte_identity",
            "state": "candidate",
            "assumptions": [],
            "adapters": [],
            "depends_on": [],
            "invalidation": {"status": "current", "reason": None},
        },
        "fixture_relation_closed_values",
    )
    require_exact_typed(
        relation["witness"],
        {
            "kind": "lean_command",
            "repository_role": "source",
            "command": ["lake", "env", "lean", "InvalidWitness.lean"],
            "timeout_seconds": 120,
            "expected_stdout_sha256": None,
            "scope": None,
            "repositories": [],
        },
        "fixture_witness_closed_values",
    )
    for side, numeral, environment_root in (
        (
            "source",
            11,
            "157b87707fd890d628b254585c566b77b9cf80f29caf0c56bd7d31ee2d5d0a3c",
        ),
        (
            "target",
            12,
            "c6f8cf856b6e6608e7fa7ea7da6b0bd3ea60dbae65c0fde0ead617f3351850ad",
        ),
    ):
        repository = fixture["repositories"][side]
        basic = next(
            item for item in repository["files"] if item["path"] == "Basic.lean"
        )
        locked = [
            {
                "path": item["path"],
                "sha256": item["sha256"].removeprefix("sha256:"),
            }
            for item in repository["files"]
            if item["path"] in {"lakefile.toml", "lean-toolchain"}
        ]
        require_exact_typed(
            relation[side],
            {
                "repository": {
                    "repository_id": repository["repository_id"],
                    "commit": repository["commit"],
                },
                "file": "Basic.lean",
                "file_sha256": basic["sha256"].removeprefix("sha256:"),
                "declaration": {
                    "name": "calibrationValue",
                    "kind": "def",
                    "start_line": 1,
                    "end_line": 1,
                },
                "declaration_sha256": raw_root(
                    f"def calibrationValue : Nat := {numeral}\n".encode()
                ).removeprefix("sha256:"),
                "environment": {
                    "lean_toolchain": "leanprover/lean4:v4.19.0",
                    "locked_files": locked,
                    "environment_root": environment_root,
                },
            },
            f"fixture_relation_endpoint_values:{side}",
        )
    require_exact_typed(
        {
            "schema_version": impact["schema_version"],
            "changed": impact["changed"],
            "scope": impact["scope"],
            "authority_claim": impact["authority_claim"],
        },
        {
            "schema_version": "lean-correspondence/impact/v0.1",
            "changed": ["stage-a-open-calibration.distinct-numerals-byte-identity"],
            "scope": "explicit_record_dependencies_only",
            "authority_claim": "none",
        },
        "fixture_impact_closed_values",
    )
    require_exact_typed(
        impact["affected"][0],
        {
            "relation_id": "stage-a-open-calibration.distinct-numerals-byte-identity",
            "record_root": generate.canonical_root(relation).removeprefix("sha256:"),
            "distance": 0,
            "source_repository_id": fixture["repositories"]["source"]["repository_id"],
            "source_commit": fixture["repositories"]["source"]["commit"],
        },
        "fixture_impact_affected_values",
    )
    require_exact_typed(
        receipt,
        {
            "schema": "vela.lean-correspondence-stage-a-invalid-witness-receipt.v1",
            "relation_record_root": generate.canonical_root(relation),
            "command": ["lake", "env", "lean", "InvalidWitness.lean"],
            "expected_exit": "nonzero",
            "observed_exit": 1,
            "stdout_sha256": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "stderr_contract": "Lean type mismatch proving 11 is not 12; exact stderr is runtime-local and not scored",
            "outcome": "witness_failed_as_designed",
            "authority_effect": "none",
        },
        "fixture_receipt_closed_values",
    )
    source_witness = next(
        item
        for item in fixture["repositories"]["source"]["files"]
        if item["path"] == "InvalidWitness.lean"
    )
    require_exact_typed(
        {
            "both_declarations_compile": fixture["both_declarations_compile"],
            "distinct_numerals": fixture["distinct_numerals"],
            "witness_must_fail": fixture["witness_must_fail"],
            "witness_source_sha256": fixture["witness_source_sha256"],
        },
        {
            "both_declarations_compile": True,
            "distinct_numerals": True,
            "witness_must_fail": True,
            "witness_source_sha256": source_witness["sha256"],
        },
        "fixture_execution_closed_values",
    )
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
        fixture["relation_record_root"] == generate.canonical_root(relation),
        "fixture_relation_record_root",
    )
    require(
        fixture["witness_failure_receipt_root"] == generate.canonical_root(receipt),
        "fixture_witness_failure_receipt_root",
    )
    require(
        fixture["impact_root"] == generate.canonical_root(impact),
        "fixture_impact_root",
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
    require_exact_fields(
        selection,
        {
            "schema",
            "stage",
            "fixed_case_count",
            "stage_b_eligibility",
            "cases",
            "authority_effect",
        },
        "case_selection_fields",
    )
    require(
        selection["schema"] == "vela.lean-correspondence-stage-a-case-selection.v1",
        "case_selection_schema",
    )
    require(selection["stage"] == "A_open_non_ceiling", "case_selection_stage")
    require(selection["authority_effect"] == "none", "case_selection_authority")
    require_exact_fields(
        ledger,
        {"schema", "cases", "information_equivalent", "authority_effect"},
        "atom_ledger_fields",
    )
    require(
        ledger["schema"] == "vela.lean-correspondence-stage-a-atom-equivalence.v1",
        "atom_ledger_schema",
    )
    require(ledger["authority_effect"] == "none", "atom_ledger_authority")
    require_int(selection["fixed_case_count"], "case_count_type")
    require(selection["fixed_case_count"] == 3, "case_count")
    require(
        selection["stage_b_eligibility"] == "permanently_excluded",
        "stage_b_case_exclusion",
    )
    cases = selection["cases"]
    require(type(cases) is list and len(cases) == 3, "case_list")
    require(
        type(ledger["cases"]) is list and len(ledger["cases"]) == 3,
        "atom_ledger_case_list",
    )
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
    ledger_item_fields = {
        "case_id",
        "semantic_atom_root",
        "raw_semantic_atom_root",
        "assisted_semantic_atom_root",
        "assisted_only_derived_root",
        "derivation_rule",
        "protected_label_present",
        "answer_key_present",
    }
    for item in ledger["cases"]:
        require_exact_fields(item, ledger_item_fields, "atom_ledger_case_fields")
        require_exact_typed(
            {
                "derivation_rule": item["derivation_rule"],
                "protected_label_present": item["protected_label_present"],
                "answer_key_present": item["answer_key_present"],
            },
            {
                "derivation_rule": "Lean Correspondence v0.2 relation verification, receipt, recheck, and explicit dependency impact over the exact base atoms",
                "protected_label_present": False,
                "answer_key_present": False,
            },
            f"atom_ledger_case_closed_values:{item['case_id']}",
        )
        for field in (
            "semantic_atom_root",
            "raw_semantic_atom_root",
            "assisted_semantic_atom_root",
            "assisted_only_derived_root",
        ):
            require_sha256(
                item[field], f"atom_ledger_root_type:{item['case_id']}:{field}"
            )
    ledger_by_id = {item["case_id"]: item for item in ledger["cases"]}
    require(set(ledger_by_id) == set(ids), "atom_ledger_case_set")
    require_exact_typed(
        [item["case_id"] for item in ledger["cases"]],
        ids,
        "atom_ledger_case_order",
    )
    require(ledger["information_equivalent"] is True, "atom_equivalence_false")
    for case in cases:
        case_id = case["case_id"]
        require_exact_fields(case, CASE_FIELDS[case_id], f"case_fields:{case_id}")
        require_exact_typed(
            {field: case[field] for field in EXPECTED_CASE_BINDINGS[case_id]},
            EXPECTED_CASE_BINDINGS[case_id],
            f"case_binding_values:{case_id}",
        )
        require_file_entries(case["base_atoms"], f"base_atom_fields:{case_id}")
        require_file_entries(
            case["derived_mechanism_atoms"], f"derived_atom_fields:{case_id}"
        )
        require_exact_typed(
            [item["path"] for item in case["base_atoms"]],
            EXPECTED_ATOM_PATHS[case_id]["base"],
            f"base_atom_inventory:{case_id}",
        )
        require_exact_typed(
            [item["path"] for item in case["derived_mechanism_atoms"]],
            EXPECTED_ATOM_PATHS[case_id]["derived"],
            f"derived_atom_inventory:{case_id}",
        )
        require_sha256(case["semantic_atom_root"], f"semantic_atom_root_type:{case_id}")
        require_sha256(
            case["derived_mechanism_root"], f"derived_mechanism_root_type:{case_id}"
        )
        require(
            type(case["claim_ceiling"]) is str
            and case["claim_ceiling"] == EXPECTED_CASE_CEILINGS[case_id],
            f"claim_ceiling:{case_id}",
        )
        require_exact_typed(
            case["allowed_impact_ids"],
            EXPECTED_IMPACT_IDS[case_id],
            f"allowed_impact_ids:{case_id}",
        )
        require(case["authority_effect"] == "none", f"case_authority:{case_id}")
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
        if "candidate_packet" in case:
            packet_atom = next(
                entry for entry in base if entry["path"] == case["candidate_packet"]
            )
            require(
                case["candidate_packet_sha256"] == packet_atom["sha256"],
                f"candidate_packet_sha256:{case_id}",
            )
        else:
            fixture = load_json(ROOT / "invalid-fixture/fixture.json")
            require_exact_typed(
                {
                    "source_commit": case["source_commit"],
                    "source_tree": case["source_tree"],
                    "target_commit": case["target_commit"],
                    "target_tree": case["target_tree"],
                },
                {
                    "source_commit": fixture["repositories"]["source"]["commit"],
                    "source_tree": fixture["repositories"]["source"]["tree"],
                    "target_commit": fixture["repositories"]["target"]["commit"],
                    "target_tree": fixture["repositories"]["target"]["tree"],
                },
                "invalid_case_repository_binding",
            )
        item = ledger_by_id[case["case_id"]]
        require(
            item["raw_semantic_atom_root"] == item["assisted_semantic_atom_root"],
            f"arm_atom_mismatch:{case_id}",
        )
        require(
            item["semantic_atom_root"]
            == item["raw_semantic_atom_root"]
            == item["assisted_semantic_atom_root"]
            == case["semantic_atom_root"],
            f"ledger_atom_root:{case['case_id']}",
        )
        require(
            item["assisted_only_derived_root"] == case["derived_mechanism_root"],
            f"ledger_derived_root:{case_id}",
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
            for entry in [*case["base_atoms"], *case["derived_mechanism_atoms"]]:
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
    configurations = load_json(ROOT / "participant-configurations.json")
    require_exact_fields(
        schedule,
        {
            "schema",
            "seed_sha256",
            "fixed_denominator",
            "rows",
            "zero_retries",
            "zero_substitutions",
            "authority_effect",
            "assignment_root",
        },
        "assignment_fields",
    )
    require(
        schedule["schema"] == "vela.lean-correspondence-stage-a-assignment-schedule.v1",
        "assignment_schema",
    )
    require(
        schedule["seed_sha256"] == raw_root(generate.SEED_TEXT.encode()),
        "assignment_seed",
    )
    require(
        schedule["zero_retries"] is True and schedule["zero_substitutions"] is True,
        "assignment_retry_substitution",
    )
    require(schedule["authority_effect"] == "none", "assignment_authority")
    require_int(schedule["fixed_denominator"], "assignment_denominator_type")
    body = {key: value for key, value in schedule.items() if key != "assignment_root"}
    require(
        generate.canonical_root(body) == schedule["assignment_root"], "assignment_root"
    )
    require(
        schedule["fixed_denominator"] == len(schedule["rows"]) == 12,
        "denominator_drift",
    )
    rows = schedule["rows"]
    require(type(rows) is list, "assignment_rows_type")
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
    expected_sequence = [
        (slot, case_id, arm)
        for case_id in cases
        for arm in ("raw-source", "correspondence-assisted")
        for slot in ("configuration-a", "configuration-b")
    ]
    require_exact_typed(
        [(row["configuration_slot"], row["case_id"], row["arm"]) for row in rows],
        expected_sequence,
        "assignment_sequence",
    )
    slot_roots = {
        slot["slot_id"]: generate.canonical_root(slot)
        for slot in configurations["slots"]
    }
    common, preambles = generate.extract_prompt_sections()
    packet_by_case_arm: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for row in rows:
        require_exact_fields(
            row,
            {
                "ordinal",
                "assignment_id",
                "configuration_slot",
                "configuration_slot_root",
                "case_id",
                "participant_visible_case_id",
                "arm",
                "fresh_session",
                "attempt",
                "timeout_seconds",
                "prompt_root",
                "packet_root",
            },
            "assignment_row_fields",
        )
        for field in (
            "assignment_id",
            "configuration_slot",
            "configuration_slot_root",
            "case_id",
            "participant_visible_case_id",
            "arm",
            "prompt_root",
            "packet_root",
        ):
            require_string(row[field], f"assignment_row_type:{field}")
        for field in ("ordinal", "attempt", "timeout_seconds"):
            require_int(row[field], f"assignment_row_type:{field}")
        require(type(row["fresh_session"]) is bool, "assignment_row_type:fresh_session")
        assignment_key = f"{row['configuration_slot']}|{row['participant_visible_case_id']}|{row['arm']}"
        expected_assignment_id = (
            f"lc-a-{row['ordinal']:02d}-"
            + hashlib.sha256(
                (generate.SEED_TEXT + assignment_key).encode()
            ).hexdigest()[:12]
        )
        require_exact_typed(
            row["assignment_id"], expected_assignment_id, "assignment_id_derivation"
        )
        require(
            row["fresh_session"] is True and row["attempt"] == 1,
            "fresh_session_attempt",
        )
        require(row["timeout_seconds"] == 1200, "timeout_drift")
        require(
            row["participant_visible_case_id"]
            == cases[row["case_id"]]["participant_visible_id"],
            "assignment_visible_case_binding",
        )
        require(
            row["configuration_slot_root"] == slot_roots[row["configuration_slot"]],
            f"assignment_configuration_root:{row['assignment_id']}",
        )
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
        require_exact_fields(
            packet,
            {
                "schema",
                "assignment_id",
                "participant_visible_case_id",
                "arm_exposed_to_participant",
                "base_semantic_atoms",
                "semantic_atom_root",
                "derived_mechanism_atoms",
                "allowed_impact_ids",
                "response_schema_sha256",
                "read_only",
                "answer_key_present",
                "authority_effect",
            },
            "packet_fields",
        )
        require(
            packet["schema"] == "vela.lean-correspondence-stage-a-packet-manifest.v1",
            "packet_schema",
        )
        for field in (
            "assignment_id",
            "participant_visible_case_id",
            "semantic_atom_root",
            "response_schema_sha256",
            "authority_effect",
        ):
            require_string(packet[field], f"packet_type:{field}")
        require(
            type(packet["allowed_impact_ids"]) is list, "packet_type:allowed_impact_ids"
        )
        require(type(packet["base_semantic_atoms"]) is list, "packet_type:base_atoms")
        require(
            type(packet["derived_mechanism_atoms"]) is list, "packet_type:derived_atoms"
        )
        require_file_entries(
            packet["base_semantic_atoms"],
            f"packet_base_atom_fields:{row['assignment_id']}",
        )
        require_file_entries(
            packet["derived_mechanism_atoms"],
            f"packet_derived_atom_fields:{row['assignment_id']}",
        )
        require(
            generate.canonical_root(packet) == row["packet_root"],
            f"packet_root:{row['assignment_id']}",
        )
        require(packet["assignment_id"] == row["assignment_id"], "packet_assignment")
        require(
            packet["participant_visible_case_id"] == row["participant_visible_case_id"],
            "packet_visible_case_binding",
        )
        require(packet["arm_exposed_to_participant"] is False, "arm_leakage")
        require(packet["answer_key_present"] is False, "answer_leakage")
        require(packet["read_only"] is True, "packet_read_only")
        require(packet["authority_effect"] == "none", "packet_authority")
        case = cases[row["case_id"]]
        require(
            packet["base_semantic_atoms"] == case["base_atoms"], "packet_base_atoms"
        )
        require(
            packet["semantic_atom_root"] == case["semantic_atom_root"],
            "packet_atom_root",
        )
        require(
            packet["allowed_impact_ids"] == case["allowed_impact_ids"],
            "packet_impact_ids",
        )
        require(
            packet["response_schema_sha256"]
            == raw_root((ROOT / "response.schema.json").read_bytes()),
            "packet_response_schema",
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
    method = load_json(ROOT / "method-binding.json")
    evidence = load_json(ROOT / "evidence-bindings.json")
    selection = load_json(ROOT / "case-selection.json")
    ledger = load_json(ROOT / "atom-equivalence.json")
    response_root = raw_root((ROOT / "response.schema.json").read_bytes())
    require_exact_fields(
        registration,
        {
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
            "registration_contract_root",
            "permit_set_root",
            "hold_state_root",
            "status",
            "registration_root",
        },
        "registration_fields",
    )
    require(
        registration["schema"]
        == "vela.lean-correspondence-stage-a-registration-contract.v1",
        "registration_schema",
    )
    for field in (
        "method_binding_root",
        "evidence_binding_root",
        "case_selection_root",
        "assignment_root",
        "atom_equivalence_root",
        "runtime_binding_root",
        "participant_configurations_root",
        "response_schema_sha256",
        "registration_contract_root",
        "permit_set_root",
        "hold_state_root",
        "registration_root",
    ):
        require_sha256(registration[field], f"registration_root_type:{field}")
    require(
        registration["status"] == "blocked_prelaunch_review_only", "registration_status"
    )
    require_exact_fields(
        hold,
        {
            "schema",
            "registration_contract_root",
            "assignment_root",
            "fixed_denominator",
            "held",
            "released",
            "consumed",
            "terminal",
            "permit_set_root",
            "permits",
            "provider_calls",
            "scoring_attempts",
            "key_accesses",
            "authority_effect",
        },
        "hold_fields",
    )
    require(
        hold["schema"] == "vela.lean-correspondence-stage-a-hold-state.v1",
        "hold_schema",
    )
    require_exact_fields(
        state,
        {
            "schema",
            "state",
            "registration_root",
            "assignment_root",
            "permit_set_root",
            "custody_contract_root",
            "fixed_denominator",
            "held_permits",
            "released_permits",
            "terminal_captures",
            "participant_configurations_bound",
            "runtime_qualification_receipt_root",
            "independent_prelaunch_review",
            "provider_calls",
            "participant_responses",
            "scoring_attempts",
            "key_accesses",
            "stage_b_families_selected",
            "protected_stage_b_key_created",
            "execution_authorized",
            "authority_effect",
        },
        "state_fields",
    )
    require(
        state["schema"] == "vela.lean-correspondence-stage-a-prelaunch-state.v1",
        "state_schema",
    )
    require_exact_typed(
        {
            "state": state["state"],
            "fixed_denominator": state["fixed_denominator"],
            "held_permits": state["held_permits"],
            "released_permits": state["released_permits"],
            "terminal_captures": state["terminal_captures"],
            "participant_configurations_bound": state[
                "participant_configurations_bound"
            ],
            "runtime_qualification_receipt_root": state[
                "runtime_qualification_receipt_root"
            ],
            "independent_prelaunch_review": state["independent_prelaunch_review"],
            "provider_calls": state["provider_calls"],
            "participant_responses": state["participant_responses"],
            "scoring_attempts": state["scoring_attempts"],
            "key_accesses": state["key_accesses"],
            "stage_b_families_selected": state["stage_b_families_selected"],
            "protected_stage_b_key_created": state["protected_stage_b_key_created"],
            "execution_authorized": state["execution_authorized"],
            "authority_effect": state["authority_effect"],
        },
        {
            "state": "blocked_missing_exact_two_provider_tool_runtime_qualification",
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
        },
        "state_closed_values",
    )
    require_exact_fields(
        custody,
        {
            "schema",
            "registration_root",
            "maintained_qualifier",
            "required_terminal_files",
            "raw_response_preserved",
            "closed_response_schema",
            "one_permit_outstanding",
            "capture_commit_before_next_release",
            "zero_retries",
            "zero_substitutions",
            "scoring_semantics",
            "protected_stage_b_key_created",
            "provider_calls",
            "authority_effect",
        },
        "custody_fields",
    )
    require(
        custody["schema"] == "vela.lean-correspondence-stage-a-custody-contract.v1",
        "custody_schema",
    )

    require_exact_fields(
        runtime,
        {
            "schema",
            "status",
            "required_participant_configuration_count",
            "required_distinct_provider_organizations",
            "required_capabilities",
            "configuration_slots",
            "rejected_runtime_evidence",
            "independent_runtime_review_receipt_root",
            "maintained_qualifier_receipt_root",
            "provider_calls",
            "participant_calls",
            "execution_authorized",
            "authority_effect",
        },
        "runtime_fields",
    )
    require(
        runtime["schema"] == "vela.lean-correspondence-stage-a-runtime-binding.v1",
        "runtime_schema",
    )
    require_exact_fields(
        runtime["rejected_runtime_evidence"],
        {"prior_image_digest", "prior_review_commit", "reason"},
        "rejected_runtime_evidence_fields",
    )
    require_exact_typed(
        runtime["rejected_runtime_evidence"],
        {
            "prior_image_digest": "sha256:f75ed4428ee3ab3f3275db0378e7375c1364f8b9f06d2f1bb4158502a84d4fc1",
            "prior_review_commit": "4a06ac8aa9a5f07abd019a375d755bfe5f0031aa",
            "reason": "single OpenAI/Codex provider, tools disabled, different prompt and response contract; transitive reuse would violate the reviewed Stage A method",
        },
        "rejected_runtime_evidence_values",
    )
    for field in (
        "required_participant_configuration_count",
        "required_distinct_provider_organizations",
        "provider_calls",
        "participant_calls",
    ):
        require_int(runtime[field], f"runtime_type:{field}")
    require(
        type(runtime["execution_authorized"]) is bool,
        "runtime_type:execution_authorized",
    )
    require(
        runtime["required_participant_configuration_count"] == 2,
        "runtime_configuration_count",
    )
    require(
        runtime["required_distinct_provider_organizations"] == 2,
        "runtime_distinct_provider_count",
    )
    require(
        type(runtime["required_capabilities"]) is list
        and all(type(item) is str for item in runtime["required_capabilities"])
        and runtime["required_capabilities"] == REQUIRED_RUNTIME_CAPABILITIES,
        "runtime_capabilities",
    )
    require(
        runtime["provider_calls"] == runtime["participant_calls"] == 0,
        "runtime_call_count",
    )
    require(runtime["execution_authorized"] is False, "runtime_execution_authorized")
    require(runtime["authority_effect"] == "none", "runtime_authority")

    require_exact_fields(
        configurations,
        {
            "schema",
            "status",
            "slots",
            "information_boundary",
            "provider_calls",
            "authority_effect",
        },
        "configuration_fields",
    )
    require(
        configurations["schema"]
        == "vela.lean-correspondence-stage-a-participant-configurations.v1",
        "configuration_schema",
    )
    require_exact_typed(
        configurations["information_boundary"],
        EXPECTED_INFORMATION_BOUNDARY,
        "configuration_information_boundary",
    )
    require_int(configurations["provider_calls"], "configuration_provider_calls_type")
    require(configurations["provider_calls"] == 0, "configuration_provider_calls")
    require(configurations["authority_effect"] == "none", "configuration_authority")
    require(len(configurations["slots"]) == 2, "configuration_slot_count")
    require(
        type(runtime["configuration_slots"]) is list
        and type(configurations["slots"]) is list
        and runtime["configuration_slots"] == configurations["slots"],
        "runtime_configuration_cross_binding",
    )
    slots = configurations["slots"]
    require(
        [slot.get("slot_id") for slot in slots]
        == ["configuration-a", "configuration-b"],
        "configuration_slot_ids",
    )
    for slot in slots:
        require_exact_fields(
            slot,
            {
                "slot_id",
                "provider_organization",
                "immutable_model_snapshot",
                "configuration_root",
                "qualification_receipt_root",
                "status",
            },
            "configuration_slot_fields",
        )
        require_string(slot["slot_id"], "configuration_slot_id_type")
        require_string(slot["status"], "configuration_slot_status_type")
    all_unbound = all(
        slot["status"] == "unbound"
        and slot["provider_organization"] is None
        and slot["immutable_model_snapshot"] is None
        and slot["configuration_root"] is None
        and slot["qualification_receipt_root"] is None
        for slot in slots
    )
    # This frozen artifact contains no runtime or review receipt bytes. Therefore
    # only the fully unbound prequalification state is valid here. A later bound
    # artifact must add and verify the exact receipt bytes, not merely insert
    # plausible-looking roots into this verifier's current schema.
    require(all_unbound, "configuration_partial_binding")
    require(configurations["status"] == "blocked_unbound", "configuration_status")
    require(
        runtime["status"]
        == "blocked_missing_exact_two_provider_tool_runtime_qualification",
        "runtime_blocker_missing",
    )
    require(
        runtime["independent_runtime_review_receipt_root"] is None
        and runtime["maintained_qualifier_receipt_root"] is None,
        "early_qualification_receipt",
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
    expected_transitive_roots = {
        "method_binding_root": generate.canonical_root(method),
        "evidence_binding_root": generate.canonical_root(evidence),
        "case_selection_root": generate.canonical_root(selection),
        "assignment_root": schedule["assignment_root"],
        "atom_equivalence_root": generate.canonical_root(ledger),
        "runtime_binding_root": generate.canonical_root(runtime),
        "participant_configurations_root": generate.canonical_root(configurations),
        "response_schema_sha256": response_root,
    }
    for field, expected in expected_transitive_roots.items():
        require(registration[field] == expected, f"registration_{field}")
    registration_body = {
        key: value for key, value in registration.items() if key != "registration_root"
    }
    require(
        generate.canonical_root(registration_body) == registration["registration_root"],
        "registration_root",
    )
    require(registration["fixed_denominator"] == 12, "registration_denominator")
    require_exact_typed(
        {
            "stage": registration["stage"],
            "arms": registration["arms"],
            "participant_configuration_slots": registration[
                "participant_configuration_slots"
            ],
            "cases": registration["cases"],
            "fixed_denominator": registration["fixed_denominator"],
            "fresh_sessions_per_cell": registration["fresh_sessions_per_cell"],
            "timeout_seconds": registration["timeout_seconds"],
            "zero_retries": registration["zero_retries"],
            "zero_substitutions": registration["zero_substitutions"],
        },
        {
            "stage": "A_open_non_ceiling",
            "arms": ["raw-source", "correspondence-assisted"],
            "participant_configuration_slots": 2,
            "cases": 3,
            "fixed_denominator": 12,
            "fresh_sessions_per_cell": 1,
            "timeout_seconds": 1200,
            "zero_retries": True,
            "zero_substitutions": True,
        },
        "registration_closed_design",
    )
    require(registration["provider_calls_authorized"] is False, "provider_authorized")
    require(registration["scoring_authorized"] is False, "scoring_authorized")
    require(
        registration["protected_stage_b_material_created"] is False,
        "protected_stage_b_material",
    )
    require(registration["authority_effect"] == "none", "registration_authority")
    permit_files = sorted((ROOT / "permits").glob("*.permit.json"))
    require(len(permit_files) == 12, "permit_count")
    permit_rows = []
    seen_assignments: set[str] = set()
    schedule_by_assignment = {row["assignment_id"]: row for row in schedule["rows"]}
    for path in permit_files:
        permit = load_json(path)
        require_exact_fields(
            permit,
            {
                "schema",
                "registration_contract_root",
                "assignment_root",
                "assignment_id",
                "configuration_slot_root",
                "packet_root",
                "prompt_root",
                "response_schema_sha256",
                "attempt",
                "timeout_seconds",
                "status",
                "releasable",
                "consumed",
                "runtime_qualification_receipt_root",
                "authority_effect",
            },
            "permit_fields",
        )
        for field in (
            "schema",
            "registration_contract_root",
            "assignment_root",
            "assignment_id",
            "configuration_slot_root",
            "packet_root",
            "prompt_root",
            "response_schema_sha256",
            "status",
            "authority_effect",
        ):
            require_string(permit[field], f"permit_type:{field}")
        require_int(permit["attempt"], "permit_type:attempt")
        require_int(permit["timeout_seconds"], "permit_type:timeout_seconds")
        require(type(permit["releasable"]) is bool, "permit_type:releasable")
        require(type(permit["consumed"]) is bool, "permit_type:consumed")
        assignment = permit["assignment_id"]
        require(
            assignment not in seen_assignments, "duplicate_reused_permit_assignment"
        )
        seen_assignments.add(assignment)
        require(assignment in schedule_by_assignment, "permit_unknown_assignment")
        require(path.name == f"{assignment}.permit.json", "permit_filename_binding")
        row = schedule_by_assignment[assignment]
        require(
            permit["registration_contract_root"] == contract_root, "permit_registration"
        )
        require(
            permit["assignment_root"] == schedule["assignment_root"],
            "permit_assignment_root",
        )
        require(
            permit["configuration_slot_root"] == row["configuration_slot_root"],
            "permit_configuration_cross_binding",
        )
        require(
            permit["packet_root"] == row["packet_root"], "permit_packet_cross_binding"
        )
        require(
            permit["prompt_root"] == row["prompt_root"], "permit_prompt_cross_binding"
        )
        require(
            permit["response_schema_sha256"] == response_root, "permit_response_schema"
        )
        require(permit["status"] == "held", "permit_not_held")
        require(
            permit["releasable"] is False and permit["consumed"] is False,
            "early_permit_release",
        )
        require(
            permit["runtime_qualification_receipt_root"] is None,
            "permit_early_or_cross_qualification",
        )
        require(
            permit["attempt"] == 1 and permit["timeout_seconds"] == 1200,
            "permit_contract_drift",
        )
        require(permit["authority_effect"] == "none", "permit_authority")
        permit_rows.append(
            {
                "assignment_id": assignment,
                "permit_root": generate.canonical_root(permit),
                "status": "held",
            }
        )
    require(seen_assignments == set(schedule_by_assignment), "permit_assignment_set")
    permit_rows.sort(key=lambda item: item["assignment_id"])
    require(type(hold["permits"]) is list, "hold_permits_type")
    for item in hold["permits"]:
        require_exact_fields(
            item,
            {"assignment_id", "permit_root", "status"},
            "hold_permit_fields",
        )
        require_string(item["assignment_id"], "hold_permit_assignment_type")
        require_sha256(item["permit_root"], "hold_permit_root_type")
        require_exact_typed(item["status"], "held", "hold_permit_status")
    hold_rows = sorted(hold["permits"], key=lambda item: item["assignment_id"])
    require(permit_rows == hold_rows, "hold_permit_roots")
    permit_set_root = generate.canonical_root(hold["permits"])
    require(
        permit_set_root == hold["permit_set_root"] == registration["permit_set_root"],
        "permit_set_root",
    )
    require(
        hold["registration_contract_root"] == contract_root,
        "hold_registration_contract_root",
    )
    require(
        hold["assignment_root"] == schedule["assignment_root"], "hold_assignment_root"
    )
    require(
        registration["hold_state_root"] == generate.canonical_root(hold),
        "registration_hold_state_root",
    )
    require_exact_typed(
        {
            "fixed_denominator": hold["fixed_denominator"],
            "held": hold["held"],
            "released": hold["released"],
            "consumed": hold["consumed"],
            "terminal": hold["terminal"],
            "provider_calls": hold["provider_calls"],
            "scoring_attempts": hold["scoring_attempts"],
            "key_accesses": hold["key_accesses"],
        },
        {
            "fixed_denominator": 12,
            "held": 12,
            "released": 0,
            "consumed": 0,
            "terminal": 0,
            "provider_calls": 0,
            "scoring_attempts": 0,
            "key_accesses": 0,
        },
        "hold_closed_values",
    )
    require(hold["authority_effect"] == "none", "hold_authority")
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
    require(
        state["participant_configurations_bound"] == 0,
        "state_configuration_count",
    )
    require(
        state["runtime_qualification_receipt_root"]
        == runtime["maintained_qualifier_receipt_root"],
        "state_runtime_qualification",
    )
    require(
        state["provider_calls"] == state["participant_responses"] == 0, "state_calls"
    )
    require(state["scoring_attempts"] == state["key_accesses"] == 0, "state_score_key")
    require(state["stage_b_families_selected"] == 0, "stage_b_selection")
    require(state["terminal_captures"] == 0, "state_terminal_captures")
    require(state["protected_stage_b_key_created"] is False, "state_stage_b_key")
    require(state["execution_authorized"] is False, "state_execution_authorized")
    require(
        custody["registration_root"] == registration["registration_root"],
        "custody_registration",
    )
    require(custody["one_permit_outstanding"] is True, "custody_scheduler")
    require_exact_typed(
        custody["required_terminal_files"],
        [
            "consumed-permit.json",
            "launch.json",
            "provider-events.jsonl",
            "provider-stderr.txt",
            "participant-response.raw.json",
            "usage.json",
            "terminal-receipt.json",
            "teardown.json",
        ],
        "custody_terminal_files",
    )
    require_exact_typed(
        custody["maintained_qualifier"],
        evidence["maintained_qualifier"],
        "custody_qualifier",
    )
    require(
        custody["closed_response_schema"] == response_root, "custody_response_schema"
    )
    require(custody["raw_response_preserved"] is True, "custody_raw_response")
    require(
        custody["capture_commit_before_next_release"] is True, "custody_capture_commit"
    )
    require(
        custody["zero_retries"] is True and custody["zero_substitutions"] is True,
        "custody_retry_substitution",
    )
    expected_scoring = {
        "relation_validation": "exact closed label against the fixed open calibration adjudication",
        "change_classification": "exact closed label against the fixed open calibration adjudication",
        "impact_closure": "closed unique set: every allowed item exactly once, no missing duplicate or unknown id, exact disposition and nonempty supplied evidence bindings",
        "false_inference": "any authority or scientific claim above the registered case ceiling is an error",
        "composite_exact": "all three correctness components and no false inference",
        "failure_timeout_malformed": "retained in the fixed denominator",
        "restricted_seconds": "min(actual_elapsed_seconds, 1200); missing or nonterminal failure assigned 1200",
        "one_scoring_attempt": True,
        "decimal_rounding": "ROUND_HALF_EVEN",
    }
    require_exact_typed(
        custody["scoring_semantics"], expected_scoring, "custody_scoring_semantics"
    )
    require(custody["protected_stage_b_key_created"] is False, "custody_stage_b_key")
    require_int(custody["provider_calls"], "custody_provider_calls_type")
    require(custody["provider_calls"] == 0, "custody_provider_calls")
    require(custody["authority_effect"] == "none", "custody_authority")
    require(
        state["custody_contract_root"] == generate.canonical_root(custody),
        "state_custody_contract_root",
    )
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
