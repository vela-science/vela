"""Fail-closed verifier for the held Anthropic-only diagnostic package."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

import generate  # noqa: E402
import scorer  # noqa: E402

ROOT = Path(__file__).resolve().parent
QUALIFIER_ROOT = Path("/private/tmp/vela-anthropic-open-diagnostic-held-v2")
QUALIFIER = Path(
    "/private/tmp/vela-stage-a-runtime-qualification-maintained-v2/"
    "tools/evidence_qualification/qualification.py"
)
QUALIFIER_PYTHON = Path(
    "/private/tmp/vela-stage-a-runtime-qualification-python-v1/.venv/bin/python"
)
SHA256 = re.compile(r"sha256:[0-9a-f]{64}")


class VerificationError(ValueError):
    pass


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise VerificationError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_pairs)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise VerificationError(f"invalid JSON: {path}") from error


def typed_equal(actual: Any, expected: Any, label: str = "value") -> None:
    if type(actual) is not type(expected):
        raise VerificationError(f"{label}: exact type mismatch")
    if type(expected) is dict:
        if set(actual) != set(expected):
            raise VerificationError(f"{label}: closed key set mismatch")
        for key in expected:
            typed_equal(actual[key], expected[key], f"{label}.{key}")
    elif type(expected) is list:
        if len(actual) != len(expected):
            raise VerificationError(f"{label}: list length mismatch")
        for index, (actual_item, expected_item) in enumerate(
            zip(actual, expected, strict=True)
        ):
            typed_equal(actual_item, expected_item, f"{label}[{index}]")
    elif actual != expected:
        raise VerificationError(f"{label}: value mismatch")


def git_bytes(commit: str, path: str) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(generate.VELA), "show", f"{commit}:{path}"],
        check=True,
        capture_output=True,
    )
    return result.stdout


def verify_external_bindings() -> None:
    commands = [
        (
            "git",
            "-C",
            str(generate.VELA),
            "rev-parse",
            f"{generate.V4_AMENDMENT_COMMIT}^{{tree}}",
        ),
        (
            "git",
            "-C",
            str(generate.VELA),
            "rev-parse",
            f"{generate.V4_REVIEW_COMMIT}^{{tree}}",
        ),
        (
            "git",
            "-C",
            str(generate.VELA),
            "rev-parse",
            f"{generate.PRIOR_RESULT_REVIEW_COMMIT}^{{tree}}",
        ),
        (
            "git",
            "-C",
            str(generate.VELA),
            "rev-parse",
            f"{generate.PRIOR_RESULT_COMMIT}^{{tree}}",
        ),
        (
            "git",
            "-C",
            str(generate.VELA),
            "rev-parse",
            f"{generate.STAGE_A_COMMIT}^{{tree}}",
        ),
        (
            "git",
            "-C",
            str(generate.VELA),
            "rev-parse",
            f"{generate.METHOD_COMMIT}^{{tree}}",
        ),
    ]
    expected = [
        generate.V4_AMENDMENT_TREE,
        generate.V4_REVIEW_TREE,
        generate.PRIOR_RESULT_REVIEW_TREE,
        generate.PRIOR_RESULT_TREE,
        generate.STAGE_A_TREE,
        generate.METHOD_TREE,
    ]
    for command, wanted in zip(commands, expected, strict=True):
        observed = subprocess.run(
            command, check=True, capture_output=True, text=True
        ).stdout.strip()
        if observed != wanted:
            raise VerificationError("bound Git tree mismatch")
    report = git_bytes(
        generate.V4_REVIEW_COMMIT,
        "paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v4-review/REPORT.md",
    )
    verdict = git_bytes(
        generate.V4_REVIEW_COMMIT,
        "paper/artifacts/lean-correspondence-stage-a-anthropic-neutral-calibration-v4-review/verdict.json",
    )
    if (
        generate.raw_root(report) != generate.V4_REVIEW_REPORT
        or generate.raw_root(verdict) != generate.V4_REVIEW_VERDICT
    ):
        raise VerificationError("independent review byte binding mismatch")
    prior_report = git_bytes(
        generate.PRIOR_RESULT_REVIEW_COMMIT,
        "reviews/inherited-correction-benchmark/order-result-4524c8f7-report.md",
    )
    prior_verdict = git_bytes(
        generate.PRIOR_RESULT_REVIEW_COMMIT,
        "reviews/inherited-correction-benchmark/order-result-4524c8f7-verdict.json",
    )
    if (
        generate.raw_root(prior_report) != generate.PRIOR_RESULT_REVIEW_REPORT
        or generate.raw_root(prior_verdict) != generate.PRIOR_RESULT_REVIEW_VERDICT
    ):
        raise VerificationError("prior result independent review byte binding mismatch")
    prior_result = git_bytes(
        generate.PRIOR_RESULT_COMMIT,
        "paper/artifacts/inherited-correction-held-out-order-replacement-result/scored-result.json",
    )
    if generate.raw_root(prior_result) != generate.PRIOR_RESULT_BYTES:
        raise VerificationError("prior 36-cell result byte binding mismatch")
    bindings = load_json(generate.FOUNDRY / "evidence-bindings.json")[
        "prior_36_cell_negative_result"
    ]
    expected_prior = {
        "fixed_denominator": 36,
        "git_documents_exact": "12/12",
        "positive_gate": "not_supported",
        "state_wrapper_exact": "12/12",
        "vela_authority_errors": 1,
        "vela_exact": "11/12",
    }
    for key, value in expected_prior.items():
        typed_equal(bindings[key], value, f"prior_36_cell_negative_result.{key}")
    stage_a_manifest = load_json(generate.STAGE_A / "artifact-manifest.json")
    if stage_a_manifest["artifact_root"] != generate.STAGE_A_ARTIFACT_ROOT:
        raise VerificationError("frozen Stage A artifact root mismatch")
    method_manifest = load_json(generate.FOUNDRY / "artifact-manifest.json")
    if method_manifest["artifact_root"] != generate.METHOD_ARTIFACT_ROOT:
        raise VerificationError("frozen method artifact root mismatch")
    stage_a_hold = load_json(generate.STAGE_A / "hold-state.json")
    if (
        type(stage_a_hold["held"]) is not int
        or stage_a_hold["held"] != 12
        or type(stage_a_hold["released"]) is not int
        or stage_a_hold["released"] != 0
        or stage_a_hold["permit_set_root"] != generate.STAGE_A_PERMIT_SET_ROOT
    ):
        raise VerificationError("frozen Stage A hold state mismatch")
    runtime_root = load_json(generate.RUNTIME / "artifact-root.json")
    if runtime_root["artifact_root"] != generate.RUNTIME_ARTIFACT_ROOT:
        raise VerificationError("runtime artifact root mismatch")
    runtime_record = load_json(generate.RUNTIME / "offline-qualification.json")
    if runtime_record["record_root"] != generate.RUNTIME_RECORD_ROOT:
        raise VerificationError("runtime offline qualification root mismatch")
    v4_root = load_json(generate.V4 / "artifact-root.json")
    if v4_root["artifact_root"] != generate.V4_AMENDED_ARTIFACT_ROOT:
        raise VerificationError("Anthropic v4 amended artifact root mismatch")
    v4_classification = load_json(generate.V4 / "post-review-classification.json")
    if (
        v4_classification["classification"]["positive_qualification"] is not True
        or v4_classification["independent_review"]["commit"]
        != generate.V4_REVIEW_COMMIT
        or v4_classification["claim_ceiling"][
            "positive_anthropic_neutral_runtime_qualification"
        ]
        is not True
    ):
        raise VerificationError("Anthropic v4 qualification classification mismatch")


def exact_inventory(root: Path, manifest: Any) -> None:
    if type(manifest) is not dict or set(manifest) != {
        "artifact_root",
        "authority_effect",
        "entries",
        "schema",
    }:
        raise VerificationError("manifest shape mismatch")
    entries = manifest["entries"]
    if type(entries) is not list:
        raise VerificationError("manifest entries must be a list")
    expected_paths = []
    for entry in entries:
        if type(entry) is not dict or set(entry) != {"bytes", "path", "sha256"}:
            raise VerificationError("manifest entry shape mismatch")
        if (
            type(entry["bytes"]) is not int
            or type(entry["path"]) is not str
            or type(entry["sha256"]) is not str
        ):
            raise VerificationError("manifest entry type mismatch")
        path = root / entry["path"]
        if path.is_symlink() or not path.is_file() or path.stat().st_nlink != 1:
            raise VerificationError(f"unsafe or missing artifact file: {entry['path']}")
        raw = path.read_bytes()
        if len(raw) != entry["bytes"] or generate.raw_root(raw) != entry["sha256"]:
            raise VerificationError(f"artifact byte mismatch: {entry['path']}")
        expected_paths.append(entry["path"])
    if expected_paths != sorted(set(expected_paths)):
        raise VerificationError("manifest paths must be sorted and unique")
    actual_paths = sorted(
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_file() or path.is_symlink()
    )
    if actual_paths != sorted(expected_paths + ["artifact-manifest.json"]):
        raise VerificationError("on-disk file set differs from manifest")
    expected_directories = {"."}
    for relative in expected_paths:
        parent = Path(relative).parent
        while parent.as_posix() != ".":
            expected_directories.add(parent.as_posix())
            parent = parent.parent
    actual_directories = {"."} | {
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_dir() and not path.is_symlink()
    }
    if actual_directories != expected_directories:
        raise VerificationError(
            "on-disk directory set differs from manifest-derived closure"
        )
    if manifest["artifact_root"] != generate.canonical_root(entries):
        raise VerificationError("artifact root mismatch")
    if (
        manifest["authority_effect"] != "none"
        or manifest["schema"]
        != "vela.lean-correspondence-anthropic-open-diagnostic-manifest.v1"
    ):
        raise VerificationError("manifest semantics mismatch")


def verify_package(root: Path = ROOT, *, check_external: bool = True) -> str:
    root = root.resolve()
    manifest = load_json(root / "artifact-manifest.json")
    exact_inventory(root, manifest)
    if check_external:
        verify_external_bindings()

    with tempfile.TemporaryDirectory(
        prefix="vela-anthropic-diag-expected-"
    ) as temporary:
        expected_root = Path(temporary)
        generate.generate(expected_root)
        expected_manifest = load_json(expected_root / "artifact-manifest.json")
        typed_equal(manifest, expected_manifest, "artifact-manifest")
        for entry in expected_manifest["entries"]:
            relative = entry["path"]
            actual_raw = (root / relative).read_bytes()
            expected_raw = (expected_root / relative).read_bytes()
            if actual_raw != expected_raw:
                raise VerificationError(f"deterministic byte mismatch: {relative}")
            if relative.endswith(".json"):
                typed_equal(
                    load_json(root / relative),
                    load_json(expected_root / relative),
                    relative,
                )

    schedule = load_json(root / "assignment-schedule.json")
    rows = schedule["rows"]
    if (
        len(rows) != 6
        or len({row["cell_id"] for row in rows}) != 6
        or len({row["participant_id"] for row in rows}) != 6
    ):
        raise VerificationError("fresh six-cell denominator mismatch")
    cells = {(row["case_id"], row["arm"]) for row in rows}
    expected_cases = {
        "erdos-730-affirmative-rhs",
        "fc-leaneval-oeis-303656",
        "deliberately-invalid-byte-identity",
    }
    if (
        len(cells) != 6
        or {row["case_id"] for row in rows} != expected_cases
        or {row["configuration_root"] for row in rows}
        != {generate.ANTHROPIC_CONFIGURATION_ROOT}
    ):
        raise VerificationError("case-arm balance or configuration mismatch")
    packet_pairs: dict[str, dict[str, Any]] = {}
    permit_roots = {}
    for row in rows:
        packet = load_json(root / row["packet_path"])
        if (
            packet["assignment_id"] != row["source_assignment_id"]
            or packet["arm_exposed_to_participant"] is not False
            or packet["answer_key_present"] is not False
            or packet["response_schema_sha256"] != generate.SCHEMA_ROOT
        ):
            raise VerificationError("frozen packet assignment or arm boundary drift")
        if (
            generate.raw_root((root / row["prompt_path"]).read_bytes())
            != row["prompt_root"]
        ):
            raise VerificationError("prompt root mismatch")
        if (
            generate.raw_root((root / row["packet_path"]).read_bytes())
            != row["packet_root"]
        ):
            raise VerificationError("packet root mismatch")
        execution_raw = (root / row["execution_packet_path"]).read_bytes()
        derived = (
            generate.packet_derivation.canonical(
                generate.packet_derivation.parse(
                    (root / row["packet_path"]).read_bytes()
                )
            )
            + b"\n"
        )
        receipt = load_json(root / row["packet_derivation_receipt_path"])
        if (
            execution_raw != derived
            or generate.raw_root(execution_raw) != row["execution_packet_root"]
            or generate.raw_root(
                (root / row["packet_derivation_receipt_path"]).read_bytes()
            )
            != row["packet_derivation_receipt_root"]
            or receipt["source_packet_sha256"] != row["packet_root"]
            or receipt["execution_packet_sha256"] != row["execution_packet_root"]
            or receipt["parsed_semantic_equality"] is not True
        ):
            raise VerificationError("source-to-execution packet derivation drift")
        permit = load_json(root / "permits" / f"{row['cell_id']}.permit.json")
        expected_permit_fields = {
            "schema",
            "registration_id",
            "assignment_id",
            "participant_id",
            "run_id",
            "condition",
            "attempt",
            "runner_version",
            "runtime_source_root",
            "configuration_root",
            "image_digest",
            "registered_schema_bytes",
            "provider_schema_bytes",
            "prompt_root",
            "packet_root",
            "tool_boundary_root",
            "tool_policy_root",
            "workspace_content_root",
            "evidence_manifest_root",
            "workspace_preflight_root",
            "timeout_seconds",
            "status",
            "issued_at",
            "consumed_at",
        }
        if (
            type(permit) is not dict
            or set(permit) != expected_permit_fields
            or permit["schema"] != "vela.tooling.closed-launch-permit.v1"
            or permit["registration_id"] != "anthropic-open-diagnostic-registration-v2"
            or permit["assignment_id"] != row["cell_id"]
            or permit["participant_id"] != row["participant_id"]
            or permit["run_id"] != row["cell_id"]
            or permit["condition"] != row["arm"]
            or type(permit["attempt"]) is not int
            or permit["attempt"] != 1
            or permit["runner_version"] != "neutral-runner/1"
            or permit["runtime_source_root"] != generate.RUNTIME_SOURCE_ROOT
            or permit["configuration_root"] != generate.ANTHROPIC_CONFIGURATION_ROOT
            or permit["image_digest"] != generate.ANTHROPIC_IMAGE_DIGEST
            or permit["registered_schema_bytes"] != generate.SCHEMA_ROOT
            or permit["provider_schema_bytes"] != generate.ANTHROPIC_PROVIDER_SCHEMA
            or permit["prompt_root"] != row["prompt_root"]
            or permit["packet_root"] != row["execution_packet_root"]
            or not SHA256.fullmatch(permit["tool_boundary_root"])
            or permit["tool_policy_root"] != generate.ANTHROPIC_TOOL_POLICY_ROOT
            or not SHA256.fullmatch(permit["workspace_content_root"])
            or not SHA256.fullmatch(permit["evidence_manifest_root"])
            or not SHA256.fullmatch(permit["workspace_preflight_root"])
            or type(permit["timeout_seconds"]) is not int
            or permit["timeout_seconds"] != 1200
            or permit["status"] != "held"
            or permit["consumed_at"] is not None
        ):
            raise VerificationError("maintained held permit drift")
        permit_roots[row["cell_id"]] = generate.maintained_root(permit)
        packet_pairs.setdefault(row["case_id"], {})[row["arm"]] = packet
    for case_id, arms in packet_pairs.items():
        raw = arms["raw-source"]
        assisted = arms["correspondence-assisted"]
        if (
            raw["base_semantic_atoms"] != assisted["base_semantic_atoms"]
            or raw["semantic_atom_root"] != assisted["semantic_atom_root"]
            or raw["participant_visible_case_id"]
            != assisted["participant_visible_case_id"]
            or raw["derived_mechanism_atoms"] != []
            or not assisted["derived_mechanism_atoms"]
        ):
            raise VerificationError(f"arm atom-information contract drift: {case_id}")

    registry = load_json(root / "execution-bundle-registry.json")
    if (
        registry["fixed_denominator"] != 6
        or registry["provider_calls"] != 0
        or registry["status"] != "held_offline_qualified"
        or len(registry["bundles"]) != 6
    ):
        raise VerificationError("execution bundle registry state drift")
    for item in registry["bundles"]:
        cell_id = item["cell_id"]
        bundle = root / item["bundle_path"]
        observed_root, entries = generate.inventory_root(bundle)
        receipt = load_json(bundle / "execution/qualification-receipt.json")
        offline = load_json(
            bundle / "execution/offline-evidence/offline-pre-request-validation.json"
        )
        if (
            cell_id not in permit_roots
            or item["bundle_root"] != observed_root
            or item["entry_count"] != len(entries)
            or item["participant_permit_root"] != permit_roots[cell_id]
            or receipt["participant_permit_root"] != permit_roots[cell_id]
            or receipt["qualification_root"] != item["qualification_root"]
            or receipt["status"] != "qualified_hold"
            or receipt["provider_calls"] != 0
            or offline["status"] != "pass"
            or offline["provider_calls"] != 0
            or offline["endpoint_write_receipts"] != 0
        ):
            raise VerificationError("execution bundle or offline qualification drift")

    hold = load_json(root / "hold-state.json")
    state = load_json(root / "prelaunch-state.json")
    if any(
        type(hold[key]) is not int or hold[key] != value
        for key, value in {
            "held": 6,
            "released": 0,
            "consumed": 0,
            "provider_calls": 0,
            "scoring_attempts": 0,
        }.items()
    ):
        raise VerificationError("held permit counter drift")
    if any(
        type(state[key]) is not int or state[key] != value
        for key, value in {
            "held_permits": 6,
            "released_permits": 0,
            "provider_calls": 0,
            "participant_responses": 0,
            "scoring_attempts": 0,
            "credential_content_accesses": 0,
        }.items()
    ):
        raise VerificationError("prelaunch counter drift")
    if (
        state["execution_authorized"] is not False
        or state["state"] != "held_pending_independent_exact_prelaunch_review"
    ):
        raise VerificationError("execution state drift")
    forbidden = {"responses", "scores", "results", "stage-b", "keys"}
    if any((root / name).exists() for name in forbidden):
        raise VerificationError("forbidden execution or scoring artifact exists")
    if scorer.COMPONENTS != (
        "relation_validation_correct",
        "change_classification_correct",
        "impact_closure_correct",
        "no_false_authority_or_scientific_inference",
    ):
        raise VerificationError("scorer component drift")
    return manifest["artifact_root"]


def verify_maintained_qualifier(root: Path = ROOT) -> None:
    if not QUALIFIER.is_file() or not QUALIFIER_PYTHON.is_file():
        raise VerificationError("fixed maintained qualifier environment missing")
    if QUALIFIER_ROOT.exists():
        shutil.rmtree(QUALIFIER_ROOT)
    shutil.copytree(root / "execution-bundles", QUALIFIER_ROOT)
    try:
        registry = load_json(root / "execution-bundle-registry.json")
        for item in registry["bundles"]:
            bundle = QUALIFIER_ROOT / item["cell_id"]
            result = subprocess.run(
                [str(QUALIFIER_PYTHON), str(QUALIFIER), "--bundle", str(bundle)],
                check=True,
                capture_output=True,
            )
            observed = json.loads(result.stdout, object_pairs_hook=_pairs)
            frozen = load_json(
                root / item["bundle_path"] / "execution/qualification-receipt.json"
            )
            typed_equal(observed, frozen, f"maintained qualifier {item['cell_id']}")
    finally:
        shutil.rmtree(QUALIFIER_ROOT)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--skip-external", action="store_true")
    parser.add_argument("--maintained-qualifier", action="store_true")
    args = parser.parse_args()
    try:
        root = verify_package(args.root, check_external=not args.skip_external)
        if args.maintained_qualifier:
            verify_maintained_qualifier(args.root.resolve())
    except (
        VerificationError,
        ValueError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print(f"PASS {root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
