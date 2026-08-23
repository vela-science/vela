from __future__ import annotations

import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

sys.dont_write_bytecode = True

SCRIPT_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_ROOT))

import generate  # noqa: E402
import scorer  # noqa: E402
import verify  # noqa: E402

ROOT = SCRIPT_ROOT


def write_json(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )


def refresh_manifest(root: Path) -> None:
    entries = []
    for path in sorted(root.rglob("*")):
        if path.is_file() and path.name != "artifact-manifest.json":
            raw = path.read_bytes()
            entries.append(
                {
                    "bytes": len(raw),
                    "path": path.relative_to(root).as_posix(),
                    "sha256": generate.raw_root(raw),
                }
            )
    write_json(
        root / "artifact-manifest.json",
        {
            "artifact_root": generate.canonical_root(entries),
            "authority_effect": "none",
            "entries": entries,
            "schema": "vela.lean-correspondence-anthropic-open-diagnostic-manifest.v1",
        },
    )


class PackageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="anthropic-diag-test-")
        self.root = Path(self.temporary.name) / "artifact"
        shutil.copytree(ROOT, self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def mutate_json(self, relative: str, mutate: Any) -> None:
        path = self.root / relative
        value = json.loads(path.read_text(encoding="utf-8"))
        mutate(value)
        write_json(path, value)
        refresh_manifest(self.root)

    def assert_rejected(self, relative: str, mutate: Any) -> None:
        self.mutate_json(relative, mutate)
        with self.assertRaises((verify.VerificationError, ValueError)):
            verify.verify_package(self.root, check_external=False)

    def test_valid_package(self) -> None:
        self.assertEqual(
            verify.verify_package(self.root),
            json.loads((self.root / "artifact-manifest.json").read_text())[
                "artifact_root"
            ],
        )

    def test_deterministic_regeneration(self) -> None:
        regenerated = Path(self.temporary.name) / "regenerated"
        generate.generate(regenerated)
        expected = sorted(
            path.relative_to(self.root)
            for path in self.root.rglob("*")
            if path.is_file()
        )
        actual = sorted(
            path.relative_to(regenerated)
            for path in regenerated.rglob("*")
            if path.is_file()
        )
        self.assertEqual(actual, expected)
        for relative in expected:
            self.assertEqual(
                (self.root / relative).read_bytes(),
                (regenerated / relative).read_bytes(),
            )

    def test_denominator_drift_resealed_manifest(self) -> None:
        self.assert_rejected(
            "prelaunch-state.json",
            lambda value: value.__setitem__("fixed_denominator", 5),
        )

    def test_arm_drift_resealed_manifest(self) -> None:
        self.assert_rejected(
            "assignment-schedule.json",
            lambda value: value["rows"][0].__setitem__(
                "arm", "correspondence-assisted"
            ),
        )

    def test_case_drift_resealed_manifest(self) -> None:
        self.assert_rejected(
            "assignment-schedule.json",
            lambda value: value["rows"][0].__setitem__("case_id", "invented-case"),
        )

    def test_configuration_drift_resealed_manifest(self) -> None:
        self.assert_rejected(
            "participant-configuration.json",
            lambda value: value["configuration"].__setitem__("model", "floating-alias"),
        )

    def test_permit_cross_binding_resealed_manifest(self) -> None:
        permits = sorted((self.root / "permits").glob("*.json"))
        first = json.loads(permits[0].read_text())
        second = json.loads(permits[1].read_text())
        first["participant_id"] = second["participant_id"]
        write_json(permits[0], first)
        refresh_manifest(self.root)
        with self.assertRaises((verify.VerificationError, ValueError)):
            verify.verify_package(self.root, check_external=False)

    def test_runtime_root_drift_resealed_manifest(self) -> None:
        self.assert_rejected(
            "runtime-binding.json",
            lambda value: value["artifacts"].__setitem__(
                "image_digest", "sha256:" + "0" * 64
            ),
        )

    def test_claim_ceiling_inflation_resealed_manifest(self) -> None:
        self.assert_rejected(
            "preregistration.json",
            lambda value: value["claim_ceiling"].__setitem__("scientific_lift", True),
        )

    def test_stale_positive_prior_result_rejected(self) -> None:
        self.assert_rejected(
            "roadmap-boundary.json",
            lambda value: value["latest_sealed_36_cell_result"].__setitem__(
                "positive_gate", "supported"
            ),
        )

    def test_g3_or_stage_b_overclaim_rejected(self) -> None:
        self.assert_rejected(
            "preregistration.json",
            lambda value: value["claim_ceiling"].__setitem__(
                "living_frontier_g3_inheritance_advantage", True
            ),
        )

    def test_boolean_as_zero_rejected(self) -> None:
        self.assert_rejected(
            "prelaunch-state.json",
            lambda value: value.__setitem__("provider_calls", False),
        )

    def test_unknown_field_rejected(self) -> None:
        self.assert_rejected(
            "runtime-binding.json",
            lambda value: value["anthropic_configuration"].__setitem__(
                "cross_provider", True
            ),
        )

    def test_prompt_byte_drift_resealed_manifest(self) -> None:
        schedule = json.loads((self.root / "assignment-schedule.json").read_text())
        path = self.root / schedule["rows"][0]["prompt_path"]
        path.write_bytes(path.read_bytes() + b"\n")
        refresh_manifest(self.root)
        with self.assertRaises((verify.VerificationError, ValueError)):
            verify.verify_package(self.root, check_external=False)

    def test_undeclared_file_rejected(self) -> None:
        (self.root / "harmless.txt").write_text("harmless\n")
        with self.assertRaises(verify.VerificationError):
            verify.verify_package(self.root, check_external=False)

    def test_symlink_extra_rejected(self) -> None:
        (self.root / "extra-link").symlink_to("README.md")
        with self.assertRaises(verify.VerificationError):
            verify.verify_package(self.root, check_external=False)

    def test_manifest_extra_entry_rejected(self) -> None:
        manifest = json.loads((self.root / "artifact-manifest.json").read_text())
        manifest["entries"].append(dict(manifest["entries"][0]))
        write_json(self.root / "artifact-manifest.json", manifest)
        with self.assertRaises(verify.VerificationError):
            verify.verify_package(self.root, check_external=False)


def scoring_document(
    root: Path, *, all_equal: bool = False, unsafe: bool = False
) -> dict[str, Any]:
    schedule = json.loads((root / "assignment-schedule.json").read_text())
    registration = json.loads((root / "registration.json").read_text())
    rows = []
    first_raw = True
    for assignment in schedule["rows"]:
        values = {
            "relation_validation_correct": True,
            "change_classification_correct": True,
            "impact_closure_correct": True,
            "no_false_authority_or_scientific_inference": True,
        }
        if assignment["arm"] == "raw-source" and first_raw and not all_equal:
            values["relation_validation_correct"] = False
            first_raw = False
        if assignment["arm"] == "correspondence-assisted" and unsafe:
            values["no_false_authority_or_scientific_inference"] = False
        rows.append(
            {
                "arm": assignment["arm"],
                "case_id": assignment["case_id"],
                "cell_id": assignment["cell_id"],
                **values,
                "restricted_seconds": "10.5",
                "terminal_status": "response",
                "tool_call_count": 2,
            }
        )
    return {
        "fixed_denominator": 6,
        "registration_root": registration["registration_root"],
        "rows": rows,
        "schema": "vela.lean-correspondence-anthropic-open-diagnostic-score-input.v1",
        "score_attempt": 1,
    }


class ScorerTests(unittest.TestCase):
    def test_strict_increment_passes_only_diagnostic_gate(self) -> None:
        result = scorer.score_document(scoring_document(ROOT))
        self.assertTrue(result["diagnostic_gate_pass"])
        self.assertEqual(
            result["claim_ceiling"], "anthropic_reviewer_agent_feasibility_only"
        )

    def test_equality_is_no_lift(self) -> None:
        result = scorer.score_document(scoring_document(ROOT, all_equal=True))
        self.assertFalse(result["diagnostic_gate_pass"])
        self.assertFalse(result["informative_raw"])
        self.assertFalse(result["strict_aggregate_increment"])

    def test_assisted_safety_error_fails(self) -> None:
        result = scorer.score_document(scoring_document(ROOT, unsafe=True))
        self.assertFalse(result["diagnostic_gate_pass"])
        self.assertFalse(result["assisted_zero_safety_authority_errors"])

    def test_boolean_score_attempt_rejected(self) -> None:
        document = scoring_document(ROOT)
        document["score_attempt"] = True
        with self.assertRaises(ValueError):
            scorer.score_document(document)

    def test_duplicate_or_missing_cell_rejected(self) -> None:
        document = scoring_document(ROOT)
        document["rows"][1] = dict(document["rows"][0])
        with self.assertRaises(ValueError):
            scorer.score_document(document)

    def test_second_score_attempt_rejected(self) -> None:
        document = scoring_document(ROOT)
        document["score_attempt"] = 2
        with self.assertRaises(ValueError):
            scorer.score_document(document)


if __name__ == "__main__":
    unittest.main()
