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


def reseal_all_dependent_roots(
    root: Path, *, preserve_registered_runtime_root: bool = False
) -> None:
    """Model an attacker who recomputes every public transitive root."""

    def read(name: str) -> Any:
        return json.loads((root / name).read_text())

    method = read("method-binding.json")
    evidence = read("evidence-bindings.json")
    selection = read("case-selection.json")
    ledger = read("atom-equivalence.json")
    runtime = read("runtime-binding.json")
    configurations = read("participant-configurations.json")
    schedule = read("assignment-schedule.json")
    registration = read("registration.json")
    hold = read("hold-state.json")
    custody = read("custody-contract.json")
    state = read("prelaunch-state.json")

    runtime["configuration_slots"] = configurations["slots"]
    write_json(root / "runtime-binding.json", runtime)
    slot_roots = {
        slot["slot_id"]: VERIFY.generate.canonical_root(slot)
        for slot in configurations["slots"]
    }
    for row in schedule["rows"]:
        row["configuration_slot_root"] = slot_roots[row["configuration_slot"]]
        row["prompt_root"] = VERIFY.raw_root(
            (root / "prompts" / f"{row['assignment_id']}.txt").read_bytes()
        )
        packet = read(f"packets/{row['assignment_id']}.json")
        row["packet_root"] = VERIFY.generate.canonical_root(packet)
    schedule_body = {
        key: value for key, value in schedule.items() if key != "assignment_root"
    }
    schedule["assignment_root"] = VERIFY.generate.canonical_root(schedule_body)
    write_json(root / "assignment-schedule.json", schedule)

    response_root = VERIFY.raw_root((root / "response.schema.json").read_bytes())
    registration.update(
        {
            "method_binding_root": VERIFY.generate.canonical_root(method),
            "evidence_binding_root": VERIFY.generate.canonical_root(evidence),
            "case_selection_root": VERIFY.generate.canonical_root(selection),
            "assignment_root": schedule["assignment_root"],
            "atom_equivalence_root": VERIFY.generate.canonical_root(ledger),
            "runtime_binding_root": VERIFY.generate.canonical_root(runtime),
            "participant_configurations_root": VERIFY.generate.canonical_root(
                configurations
            ),
            "response_schema_sha256": response_root,
        }
    )
    if preserve_registered_runtime_root:
        registration["runtime_binding_root"] = read("registration.json")[
            "runtime_binding_root"
        ]
    contract_fields = {
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
    contract_root = VERIFY.generate.canonical_root(
        {field: registration[field] for field in contract_fields}
    )
    registration["registration_contract_root"] = contract_root

    rows_by_id = {row["assignment_id"]: row for row in schedule["rows"]}
    permit_roots: dict[str, str] = {}
    for path in sorted((root / "permits").glob("*.permit.json")):
        permit = json.loads(path.read_text())
        row = rows_by_id[permit["assignment_id"]]
        permit.update(
            {
                "registration_contract_root": contract_root,
                "assignment_root": schedule["assignment_root"],
                "configuration_slot_root": row["configuration_slot_root"],
                "packet_root": row["packet_root"],
                "prompt_root": row["prompt_root"],
                "response_schema_sha256": response_root,
            }
        )
        write_json(path, permit)
        permit_roots[permit["assignment_id"]] = VERIFY.generate.canonical_root(permit)

    for item in hold["permits"]:
        item["permit_root"] = permit_roots[item["assignment_id"]]
    hold["registration_contract_root"] = contract_root
    hold["assignment_root"] = schedule["assignment_root"]
    hold["permit_set_root"] = VERIFY.generate.canonical_root(hold["permits"])
    write_json(root / "hold-state.json", hold)

    registration["permit_set_root"] = hold["permit_set_root"]
    registration["hold_state_root"] = VERIFY.generate.canonical_root(hold)
    registration_body = {
        key: value for key, value in registration.items() if key != "registration_root"
    }
    registration["registration_root"] = VERIFY.generate.canonical_root(
        registration_body
    )
    write_json(root / "registration.json", registration)

    custody["registration_root"] = registration["registration_root"]
    write_json(root / "custody-contract.json", custody)
    state.update(
        {
            "registration_root": registration["registration_root"],
            "assignment_root": schedule["assignment_root"],
            "permit_set_root": hold["permit_set_root"],
            "custody_contract_root": VERIFY.generate.canonical_root(custody),
        }
    )
    write_json(root / "prelaunch-state.json", state)
    refresh_manifest(root)


def reseal_invalid_fixture_chain(root: Path) -> None:
    relation_path = root / "invalid-fixture/candidate-relation.json"
    receipt_path = root / "invalid-fixture/witness-failure-receipt.json"
    impact_path = root / "invalid-fixture/impact.json"
    fixture_path = root / "invalid-fixture/fixture.json"
    relation = json.loads(relation_path.read_text())
    receipt = json.loads(receipt_path.read_text())
    impact = json.loads(impact_path.read_text())
    fixture = json.loads(fixture_path.read_text())

    relation_root = VERIFY.generate.canonical_root(relation)
    receipt["relation_record_root"] = relation_root
    write_json(receipt_path, receipt)
    impact["affected"][0]["record_root"] = relation_root.removeprefix("sha256:")
    write_json(impact_path, impact)
    fixture["relation_record_root"] = relation_root
    fixture["witness_failure_receipt_root"] = VERIFY.generate.canonical_root(receipt)
    fixture["impact_root"] = VERIFY.generate.canonical_root(impact)
    write_json(fixture_path, fixture)

    selection_path = root / "case-selection.json"
    selection = json.loads(selection_path.read_text())
    invalid_case = selection["cases"][2]
    for entry in invalid_case["derived_mechanism_atoms"]:
        raw = (root / entry["path"]).read_bytes()
        entry["bytes"] = len(raw)
        entry["sha256"] = VERIFY.raw_root(raw)
    invalid_case["derived_mechanism_root"] = VERIFY.generate.canonical_root(
        invalid_case["derived_mechanism_atoms"]
    )
    write_json(selection_path, selection)

    evidence_path = root / "evidence-bindings.json"
    evidence = json.loads(evidence_path.read_text())
    evidence["invalid_fixture_root"] = VERIFY.generate.canonical_root(fixture)
    write_json(evidence_path, evidence)

    schedule = json.loads((root / "assignment-schedule.json").read_text())
    for row in schedule["rows"]:
        if (
            row["case_id"] == "deliberately-invalid-byte-identity"
            and row["arm"] == "correspondence-assisted"
        ):
            packet_path = root / "packets" / f"{row['assignment_id']}.json"
            packet = json.loads(packet_path.read_text())
            packet["derived_mechanism_atoms"] = invalid_case["derived_mechanism_atoms"]
            write_json(packet_path, packet)
    reseal_all_dependent_roots(root)


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
        for field in (
            "fixed_denominator",
            "held_permits",
            "released_permits",
            "provider_calls",
            "participant_responses",
            "scoring_attempts",
            "key_accesses",
            "stage_b_families_selected",
        ):
            self.assertIs(type(result[field]), int, field)

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
            registration = json.loads((root / "registration.json").read_text())
            registration["runtime_binding_root"] = "sha256:" + "0" * 64
            write_json(root / "registration.json", registration)
            reseal_all_dependent_roots(root, preserve_registered_runtime_root=True)
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

    def test_fully_resealed_erdos_unknown_claims_fail_closed(self) -> None:
        with mutated_root() as root:
            selection = json.loads((root / "case-selection.json").read_text())
            erdos = selection["cases"][0]
            erdos["full_biconditional"] = True
            erdos["scientific_acceptance"] = "claimed"
            erdos["execution_authorized"] = True
            write_json(root / "case-selection.json", selection)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked(
                "case_fields:erdos-730-affirmative-rhs"
            )

    def test_fully_resealed_fc_case_nested_unknown_field_fails_closed(self) -> None:
        with mutated_root() as root:
            selection = json.loads((root / "case-selection.json").read_text())
            selection["cases"][1]["base_atoms"][0]["scientific_acceptance"] = "claimed"
            write_json(root / "case-selection.json", selection)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked(
                "base_atom_fields:fc-leaneval-oeis-303656"
            )

    def test_fully_resealed_invalid_case_type_inflation_fails_closed(self) -> None:
        with mutated_root() as root:
            selection = json.loads((root / "case-selection.json").read_text())
            selection["cases"][2]["allowed_impact_ids"] = [True]
            write_json(root / "case-selection.json", selection)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked(
                "allowed_impact_ids:deliberately-invalid-byte-identity"
            )

    def test_fully_resealed_runtime_nested_unknown_field_fails_closed(self) -> None:
        with mutated_root() as root:
            runtime = json.loads((root / "runtime-binding.json").read_text())
            runtime["rejected_runtime_evidence"]["execution_authorized"] = True
            write_json(root / "runtime-binding.json", runtime)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("rejected_runtime_evidence_fields")

    def test_fully_resealed_runtime_type_inflation_fails_closed(self) -> None:
        with mutated_root() as root:
            runtime = json.loads((root / "runtime-binding.json").read_text())
            runtime["required_participant_configuration_count"] = True
            write_json(root / "runtime-binding.json", runtime)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked(
                "runtime_type:required_participant_configuration_count"
            )

    def test_fully_resealed_runtime_boolean_counter_fails_closed(self) -> None:
        with mutated_root() as root:
            runtime = json.loads((root / "runtime-binding.json").read_text())
            runtime["provider_calls"] = False
            write_json(root / "runtime-binding.json", runtime)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("runtime_type:provider_calls")

    def test_fully_resealed_relation_endpoint_unknown_field_fails_closed(self) -> None:
        with mutated_root() as root:
            path = root / "invalid-fixture/candidate-relation.json"
            relation = json.loads(path.read_text())
            relation["source"]["declaration"]["mathematical_truth"] = "claimed"
            write_json(path, relation)
            reseal_invalid_fixture_chain(root)
            self.assert_full_verification_blocked(
                "fixture_relation_declaration_fields:source"
            )

    def test_fully_resealed_atom_ledger_unknown_field_fails_closed(self) -> None:
        with mutated_root() as root:
            ledger = json.loads((root / "atom-equivalence.json").read_text())
            ledger["cases"][0]["scientific_acceptance"] = "claimed"
            write_json(root / "atom-equivalence.json", ledger)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("atom_ledger_case_fields")

    def test_fully_resealed_assignment_type_inflation_fails_closed(self) -> None:
        with mutated_root() as root:
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            schedule["rows"][0]["ordinal"] = True
            write_json(root / "assignment-schedule.json", schedule)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("assignment_row_type:ordinal")

    def test_fully_resealed_assignment_boolean_attempt_fails_closed(self) -> None:
        with mutated_root() as root:
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            schedule["rows"][0]["attempt"] = True
            write_json(root / "assignment-schedule.json", schedule)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("assignment_row_type:attempt")

    def test_fully_resealed_configuration_unknown_field_fails_closed(self) -> None:
        with mutated_root() as root:
            configurations = json.loads(
                (root / "participant-configurations.json").read_text()
            )
            configurations["information_boundary"]["execution_authorized"] = True
            write_json(root / "participant-configurations.json", configurations)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("configuration_information_boundary")

    def test_fully_resealed_configuration_slot_unknown_field_fails_closed(self) -> None:
        with mutated_root() as root:
            configurations = json.loads(
                (root / "participant-configurations.json").read_text()
            )
            configurations["slots"][0]["execution_authorized"] = True
            write_json(root / "participant-configurations.json", configurations)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("configuration_slot_fields")

    def test_fully_resealed_configuration_boolean_counter_fails_closed(self) -> None:
        with mutated_root() as root:
            configurations = json.loads(
                (root / "participant-configurations.json").read_text()
            )
            configurations["provider_calls"] = False
            write_json(root / "participant-configurations.json", configurations)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("configuration_provider_calls_type")

    def test_fully_resealed_packet_nested_unknown_field_fails_closed(self) -> None:
        with mutated_root() as root:
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            packet_path = (
                root / "packets" / f"{schedule['rows'][0]['assignment_id']}.json"
            )
            packet = json.loads(packet_path.read_text())
            packet["base_semantic_atoms"][0]["scientific_acceptance"] = "claimed"
            write_json(packet_path, packet)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("packet_base_atom_fields")

    def test_fully_resealed_packet_boolean_byte_count_fails_closed(self) -> None:
        with mutated_root() as root:
            schedule = json.loads((root / "assignment-schedule.json").read_text())
            packet_path = (
                root / "packets" / f"{schedule['rows'][0]['assignment_id']}.json"
            )
            packet = json.loads(packet_path.read_text())
            packet["base_semantic_atoms"][0]["bytes"] = False
            write_json(packet_path, packet)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("packet_base_atom_fields")

    def test_fully_resealed_permit_type_inflation_fails_closed(self) -> None:
        with mutated_root() as root:
            permit_path = next((root / "permits").glob("*.permit.json"))
            permit = json.loads(permit_path.read_text())
            permit["attempt"] = True
            write_json(permit_path, permit)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("permit_type:attempt")

    def test_fully_resealed_hold_permit_unknown_field_fails_closed(self) -> None:
        with mutated_root() as root:
            hold = json.loads((root / "hold-state.json").read_text())
            hold["permits"][0]["execution_authorized"] = True
            write_json(root / "hold-state.json", hold)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("hold_permit_fields")

    def test_fully_resealed_hold_boolean_counter_fails_closed(self) -> None:
        with mutated_root() as root:
            hold = json.loads((root / "hold-state.json").read_text())
            hold["provider_calls"] = False
            write_json(root / "hold-state.json", hold)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("hold_closed_values")

    def test_fully_resealed_custody_type_inflation_fails_closed(self) -> None:
        with mutated_root() as root:
            custody = json.loads((root / "custody-contract.json").read_text())
            custody["scoring_semantics"]["one_scoring_attempt"] = 1
            write_json(root / "custody-contract.json", custody)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("custody_scoring_semantics")

    def test_fully_resealed_custody_boolean_counter_fails_closed(self) -> None:
        with mutated_root() as root:
            custody = json.loads((root / "custody-contract.json").read_text())
            custody["provider_calls"] = False
            write_json(root / "custody-contract.json", custody)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("custody_provider_calls_type")

    def test_fully_resealed_custody_decimal_string_type_fails_closed(self) -> None:
        with mutated_root() as root:
            custody = json.loads((root / "custody-contract.json").read_text())
            custody["scoring_semantics"]["decimal_rounding"] = 1
            write_json(root / "custody-contract.json", custody)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("custody_scoring_semantics")

    def test_fully_resealed_qualifier_unknown_field_fails_closed(self) -> None:
        with mutated_root() as root:
            evidence = json.loads((root / "evidence-bindings.json").read_text())
            evidence["maintained_qualifier"]["execution_authorized"] = True
            write_json(root / "evidence-bindings.json", evidence)
            custody = json.loads((root / "custody-contract.json").read_text())
            custody["maintained_qualifier"] = evidence["maintained_qualifier"]
            write_json(root / "custody-contract.json", custody)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("maintained_qualifier_fields")

    def test_fully_resealed_prelaunch_type_inflation_fails_closed(self) -> None:
        with mutated_root() as root:
            state = json.loads((root / "prelaunch-state.json").read_text())
            state["provider_calls"] = False
            write_json(root / "prelaunch-state.json", state)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("state_closed_values")

    def test_fully_resealed_registration_boolean_count_fails_closed(self) -> None:
        with mutated_root() as root:
            registration = json.loads((root / "registration.json").read_text())
            registration["fresh_sessions_per_cell"] = True
            write_json(root / "registration.json", registration)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("registration_closed_design")

    def test_fully_resealed_case_boolean_count_fails_closed(self) -> None:
        with mutated_root() as root:
            selection = json.loads((root / "case-selection.json").read_text())
            selection["fixed_case_count"] = True
            write_json(root / "case-selection.json", selection)
            reseal_all_dependent_roots(root)
            self.assert_full_verification_blocked("case_count_type")

    def test_fully_resealed_review_receipt_boolean_exit_fails_closed(self) -> None:
        with mutated_root() as root:
            path = root / "invalid-fixture/witness-failure-receipt.json"
            receipt = json.loads(path.read_text())
            receipt["observed_exit"] = True
            write_json(path, receipt)
            reseal_invalid_fixture_chain(root)
            self.assert_full_verification_blocked("fixture_receipt_closed_values")


if __name__ == "__main__":
    unittest.main()
