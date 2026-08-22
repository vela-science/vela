from __future__ import annotations

import json
import os
import shutil
import tempfile
import unittest
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import Any

import verify as VERIFY

ROOT = Path(__file__).resolve().parent
VELA = Path(os.environ.get("STAGE_A_VELA_REPO", ROOT.parents[2])).resolve()
IMPLEMENTATION = Path(
    os.environ.get("STAGE_A_IMPLEMENTATION_REPO", VELA.parent / "lean-correspondence")
).resolve()
CANDIDATES = Path(
    os.environ.get("STAGE_A_CANDIDATES_REPO", VELA.parent / "lean-proofs")
).resolve()


def write_json(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )


def refresh_manifest(root: Path) -> None:
    entries = []
    for path in sorted(root.rglob("*")):
        if (
            not path.is_file()
            or path.name == "artifact-manifest.json"
            or "__pycache__" in path.parts
        ):
            continue
        raw = path.read_bytes()
        entries.append(
            {
                "path": path.relative_to(root).as_posix(),
                "bytes": len(raw),
                "sha256": VERIFY.raw_root(raw),
            }
        )
    write_json(
        root / "artifact-manifest.json",
        {
            "schema": "vela.lean-correspondence-stage-a-artifact-manifest.v1",
            "entries": entries,
            "artifact_root": VERIFY.generate.canonical_root(entries),
            "authority_effect": "none",
        },
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
    def assert_full_verification_blocked(self, code: str) -> None:
        with self.assertRaisesRegex(VERIFY.VerificationError, code):
            VERIFY.run(VELA, IMPLEMENTATION, CANDIDATES, check_lean=False)

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

    def test_outer_manifest_refresh_cannot_reduce_distinct_provider_requirement(
        self,
    ) -> None:
        with mutated_root() as root:
            runtime = json.loads((root / "runtime-binding.json").read_text())
            runtime["required_distinct_provider_organizations"] = 1
            write_json(root / "runtime-binding.json", runtime)
            refresh_manifest(root)
            self.assert_full_verification_blocked("runtime_distinct_provider_count")

    def test_outer_manifest_refresh_cannot_remove_offline_tool_capability(self) -> None:
        with mutated_root() as root:
            runtime = json.loads((root / "runtime-binding.json").read_text())
            runtime["required_capabilities"].remove(
                "read_only_offline_shell_and_file_tools"
            )
            write_json(root / "runtime-binding.json", runtime)
            refresh_manifest(root)
            self.assert_full_verification_blocked("runtime_capabilities")

    def test_outer_manifest_refresh_cannot_enable_participant_network(self) -> None:
        with mutated_root() as root:
            configurations = json.loads(
                (root / "participant-configurations.json").read_text()
            )
            configurations["information_boundary"]["network_from_participant"] = True
            write_json(root / "participant-configurations.json", configurations)
            refresh_manifest(root)
            self.assert_full_verification_blocked("configuration_information_boundary")

    def test_outer_manifest_refresh_cannot_partially_bind_configuration(self) -> None:
        with mutated_root() as root:
            configurations = json.loads(
                (root / "participant-configurations.json").read_text()
            )
            configurations["slots"][0]["provider_organization"] = "provider-a"
            write_json(root / "participant-configurations.json", configurations)
            runtime = json.loads((root / "runtime-binding.json").read_text())
            runtime["configuration_slots"] = configurations["slots"]
            write_json(root / "runtime-binding.json", runtime)
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            slot_root = VERIFY.generate.canonical_root(configurations["slots"][0])
            for row in schedule["rows"]:
                if row["configuration_slot"] == "configuration-a":
                    row["configuration_slot_root"] = slot_root
            body = {
                key: value
                for key, value in schedule.items()
                if key != "assignment_root"
            }
            schedule["assignment_root"] = VERIFY.generate.canonical_root(body)
            write_json(root / "assignment-schedule.json", schedule)
            refresh_manifest(root)
            self.assert_full_verification_blocked("configuration_partial_binding")

    def test_outer_manifest_refresh_cannot_inflate_erdos_730_claim(self) -> None:
        with mutated_root() as root:
            selection = json.loads((root / "case-selection.json").read_text())
            selection["cases"][0]["claim_ceiling"] = (
                "full theorem and scientific acceptance"
            )
            write_json(root / "case-selection.json", selection)
            refresh_manifest(root)
            self.assert_full_verification_blocked(
                "claim_ceiling:erdos-730-affirmative-rhs"
            )

    def test_outer_manifest_refresh_cannot_hide_stale_registered_runtime_root(
        self,
    ) -> None:
        with mutated_root() as root:
            runtime = json.loads((root / "runtime-binding.json").read_text())
            runtime["rejected_runtime_evidence"]["reason"] += " mutated"
            write_json(root / "runtime-binding.json", runtime)
            refresh_manifest(root)
            self.assert_full_verification_blocked("registration_runtime_binding_root")

    def test_outer_manifest_refresh_cannot_cross_bind_configuration_slot(self) -> None:
        with mutated_root() as root:
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            schedule["rows"][0]["configuration_slot_root"] = schedule["rows"][1][
                "configuration_slot_root"
            ]
            body = {
                key: value
                for key, value in schedule.items()
                if key != "assignment_root"
            }
            schedule["assignment_root"] = VERIFY.generate.canonical_root(body)
            write_json(root / "assignment-schedule.json", schedule)
            refresh_manifest(root)
            self.assert_full_verification_blocked("assignment_configuration_root")

    def test_outer_manifest_refresh_cannot_cross_bind_permit_packet(self) -> None:
        with mutated_root() as root:
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            permit = json.loads(
                (
                    root
                    / "permits"
                    / f"{schedule['rows'][0]['assignment_id']}.permit.json"
                ).read_text()
            )
            permit["packet_root"] = schedule["rows"][1]["packet_root"]
            write_json(
                root
                / "permits"
                / f"{schedule['rows'][0]['assignment_id']}.permit.json",
                permit,
            )
            refresh_manifest(root)
            self.assert_full_verification_blocked("permit_packet_cross_binding")

    def test_outer_manifest_refresh_cannot_hide_stale_custody_root(self) -> None:
        with mutated_root() as root:
            state = json.loads((root / "prelaunch-state.json").read_text())
            state["custody_contract_root"] = "sha256:" + "0" * 64
            write_json(root / "prelaunch-state.json", state)
            refresh_manifest(root)
            self.assert_full_verification_blocked("state_custody_contract_root")


if __name__ == "__main__":
    unittest.main()
