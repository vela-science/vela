from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import Any

import verify as VERIFY

ROOT = Path(__file__).resolve().parent
VELA = ROOT.parents[2]
IMPLEMENTATION = VELA.parent / "lean-correspondence"
CANDIDATES = VELA.parent / "lean-proofs"


def write_json(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )


@contextmanager
def mutated_root() -> Iterator[Path]:
    with tempfile.TemporaryDirectory(prefix="lc-stage-a-test-") as temporary:
        target = Path(temporary) / "artifact"
        shutil.copytree(ROOT, target, ignore=shutil.ignore_patterns("__pycache__"))
        prior = VERIFY.ROOT
        VERIFY.ROOT = target
        try:
            yield target
        finally:
            VERIFY.ROOT = prior


class StageAPrelaunchTests(unittest.TestCase):
    def test_current_artifact_passes_but_is_not_launch_ready(self) -> None:
        result = VERIFY.run(VELA, IMPLEMENTATION, CANDIDATES, check_lean=False)
        self.assertEqual(result["status"], "PASS")
        self.assertTrue(result["prelaunch_readiness"].startswith("BLOCKED_"))
        self.assertEqual(result["held_permits"], 12)
        self.assertEqual(result["released_permits"], 0)
        self.assertFalse(result["execution_authorized"])
        self.assertEqual(result["authority_effect"], "none")

    def test_rejects_stale_assignment_root(self) -> None:
        with mutated_root() as root:
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            schedule["assignment_root"] = "sha256:" + "0" * 64
            write_json(root / "assignment-schedule.json", schedule)
            cases = VERIFY.verify_cases_and_atoms()
            with self.assertRaisesRegex(VERIFY.VerificationError, "assignment_root"):
                VERIFY.verify_schedule_packets_prompts(cases)

    def test_rejects_missing_packet_root_binding(self) -> None:
        with mutated_root() as root:
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            schedule["rows"][0]["packet_root"] = "sha256:" + "0" * 64
            body = {
                key: value
                for key, value in schedule.items()
                if key != "assignment_root"
            }
            schedule["assignment_root"] = VERIFY.generate.canonical_root(body)
            write_json(root / "assignment-schedule.json", schedule)
            cases = VERIFY.verify_cases_and_atoms()
            with self.assertRaisesRegex(VERIFY.VerificationError, "packet_root"):
                VERIFY.verify_schedule_packets_prompts(cases)

    def test_rejects_case_substitution(self) -> None:
        with mutated_root() as root:
            selection = json.loads((root / "case-selection.json").read_text())
            selection["cases"][0]["case_id"] = "substituted-case"
            write_json(root / "case-selection.json", selection)
            with self.assertRaisesRegex(VERIFY.VerificationError, "case_substitution"):
                VERIFY.verify_cases_and_atoms()

    def test_rejects_answer_leakage(self) -> None:
        with mutated_root() as root:
            selection = json.loads((root / "case-selection.json").read_text())
            selection["cases"][2]["participant_visible_id"] = "invalid-answer"
            write_json(root / "case-selection.json", selection)
            with self.assertRaisesRegex(
                VERIFY.VerificationError, "participant_case_answer_leakage"
            ):
                VERIFY.verify_cases_and_atoms()

    def test_rejects_atom_mismatch(self) -> None:
        with mutated_root() as root:
            ledger = json.loads((root / "atom-equivalence.json").read_text())
            ledger["cases"][0]["assisted_semantic_atom_root"] = "sha256:" + "0" * 64
            write_json(root / "atom-equivalence.json", ledger)
            with self.assertRaisesRegex(VERIFY.VerificationError, "arm_atom_mismatch"):
                VERIFY.verify_cases_and_atoms()

    def test_rejects_duplicate_assignment_ids(self) -> None:
        with mutated_root() as root:
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            schedule["rows"][1]["assignment_id"] = schedule["rows"][0]["assignment_id"]
            body = {
                key: value
                for key, value in schedule.items()
                if key != "assignment_root"
            }
            schedule["assignment_root"] = VERIFY.generate.canonical_root(body)
            write_json(root / "assignment-schedule.json", schedule)
            cases = VERIFY.verify_cases_and_atoms()
            with self.assertRaisesRegex(
                VERIFY.VerificationError, "duplicate_assignment_id"
            ):
                VERIFY.verify_schedule_packets_prompts(cases)

    def test_rejects_duplicate_reused_permit_id(self) -> None:
        with mutated_root() as root:
            permit_files = sorted((root / "permits").glob("*.permit.json"))
            first = json.loads(permit_files[0].read_text())
            second = json.loads(permit_files[1].read_text())
            second["assignment_id"] = first["assignment_id"]
            write_json(permit_files[1], second)
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            with self.assertRaisesRegex(
                VERIFY.VerificationError, "duplicate_reused_permit_assignment"
            ):
                VERIFY.verify_registration_permits_state(schedule)

    def test_rejects_denominator_drift(self) -> None:
        with mutated_root() as root:
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            schedule["fixed_denominator"] = 11
            body = {
                key: value
                for key, value in schedule.items()
                if key != "assignment_root"
            }
            schedule["assignment_root"] = VERIFY.generate.canonical_root(body)
            write_json(root / "assignment-schedule.json", schedule)
            cases = VERIFY.verify_cases_and_atoms()
            with self.assertRaisesRegex(VERIFY.VerificationError, "denominator_drift"):
                VERIFY.verify_schedule_packets_prompts(cases)

    def test_rejects_early_qualification(self) -> None:
        with mutated_root() as root:
            runtime = json.loads((root / "runtime-binding.json").read_text())
            runtime["maintained_qualifier_receipt_root"] = "sha256:" + "1" * 64
            write_json(root / "runtime-binding.json", runtime)
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            with self.assertRaisesRegex(
                VERIFY.VerificationError, "early_qualification"
            ):
                VERIFY.verify_registration_permits_state(schedule)

    def test_rejects_early_permit_release(self) -> None:
        with mutated_root() as root:
            permit_file = next((root / "permits").glob("*.permit.json"))
            permit = json.loads(permit_file.read_text())
            permit["status"] = "released"
            permit["releasable"] = True
            write_json(permit_file, permit)
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            with self.assertRaisesRegex(VERIFY.VerificationError, "permit_not_held"):
                VERIFY.verify_registration_permits_state(schedule)


if __name__ == "__main__":
    unittest.main()
