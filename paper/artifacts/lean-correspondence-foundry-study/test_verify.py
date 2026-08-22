from __future__ import annotations

import copy
import importlib.util
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("verify_protocol", ROOT / "verify.py")
assert SPEC is not None and SPEC.loader is not None
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class ProtocolVerificationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.contract = json.loads((ROOT / "study-contract.json").read_text())

    def assert_rejected(self, contract: dict[str, object]) -> None:
        with self.assertRaises(VERIFY.VerificationError):
            VERIFY.verify_contract(contract)

    def test_current_artifact_passes(self) -> None:
        result = VERIFY.run(ROOT.parents[2])
        self.assertEqual(result["status"], "PASS")
        self.assertFalse(result["scientific_result"])
        self.assertFalse(result["execution_authorized"])

    def test_rejects_selected_held_out_family(self) -> None:
        mutated = copy.deepcopy(self.contract)
        mutated["selected_confirmatory_families"] = ["forbidden-family"]
        self.assert_rejected(mutated)

    def test_rejects_provider_authorization(self) -> None:
        mutated = copy.deepcopy(self.contract)
        mutated["provider_calls_authorized"] = True
        self.assert_rejected(mutated)

    def test_rejects_denominator_drift(self) -> None:
        mutated = copy.deepcopy(self.contract)
        mutated["stage_b"]["primary_fixed_denominator"] = 71
        self.assert_rejected(mutated)

    def test_rejects_retry(self) -> None:
        mutated = copy.deepcopy(self.contract)
        mutated["stage_b"]["zero_retries"] = False
        self.assert_rejected(mutated)

    def test_rejects_equality_as_lift(self) -> None:
        mutated = copy.deepcopy(self.contract)
        mutated["aggregate_gates"]["equality_counts_as_positive_lift"] = True
        self.assert_rejected(mutated)

    def test_rejects_safety_relaxation(self) -> None:
        mutated = copy.deepcopy(self.contract)
        mutated["aggregate_gates"]["assisted_false_inference_maximum"] = 1
        self.assert_rejected(mutated)

    def test_decimal_and_strict_lift_boundaries(self) -> None:
        restricted, mean = VERIFY.restricted_mean(
            ["10.000000000", "1300.000000000", None], "1200.000000000"
        )
        self.assertEqual(
            restricted, ["10.000000000", "1200.000000000", "1200.000000000"]
        )
        self.assertEqual(mean, "803.333333333")
        self.assertTrue(VERIFY.strict_lift(6, 5, 1))
        self.assertFalse(VERIFY.strict_lift(5, 5, 1))

    def test_stage_b_permit_requires_exact_independent_binding_review(self) -> None:
        state = json.loads((ROOT / "prelaunch-state.json").read_text())
        machine = json.loads((ROOT / "prelaunch-state-machine.json").read_text())
        schema = json.loads((ROOT / "prelaunch-state.schema.json").read_text())
        state.update(
            {
                "state": "ready_for_stage_b_permit_creation",
                "selected_family_count": 6,
                "family_assignment_prelaunch_binding_root": "sha256:" + "1" * 64,
                "qualification_receipt_root": "sha256:" + "4" * 64,
                "stage_b_permits_created": True,
                "stage_b_permits_releasable": True,
            }
        )
        self.assertTrue(list(Draft202012Validator(schema).iter_errors(state)))
        with self.assertRaisesRegex(
            VERIFY.VerificationError, "selected_binding_review_status"
        ):
            VERIFY.verify_prelaunch_state(state, machine)

        state.update(
            {
                "selected_binding_review": {
                    "status": "PASS",
                    "reviewed_binding_root": "sha256:" + "2" * 64,
                    "review_commit": "3" * 40,
                    "independent": True,
                },
            }
        )
        self.assertFalse(list(Draft202012Validator(schema).iter_errors(state)))
        with self.assertRaisesRegex(
            VERIFY.VerificationError, "selected_binding_review_root_mismatch"
        ):
            VERIFY.verify_prelaunch_state(state, machine)

    def test_pre_runtime_qualification_receipt_is_rejected(self) -> None:
        initial = json.loads((ROOT / "prelaunch-state.json").read_text())
        machine = json.loads((ROOT / "prelaunch-state-machine.json").read_text())
        schema = json.loads((ROOT / "prelaunch-state.schema.json").read_text())
        binding_root = "sha256:" + "1" * 64
        review = {
            "status": "PASS",
            "reviewed_binding_root": binding_root,
            "review_commit": "2" * 40,
            "independent": True,
        }
        mutations = {
            "method_frozen": {},
            "stage_a_passed": {},
            "selection_frozen_pending_independent_review": {
                "selected_family_count": 6,
                "family_assignment_prelaunch_binding_root": binding_root,
            },
            "selected_binding_independent_review_passed": {
                "selected_family_count": 6,
                "family_assignment_prelaunch_binding_root": binding_root,
                "selected_binding_review": review,
            },
        }
        for state_name, fields in mutations.items():
            with self.subTest(state=state_name):
                state = copy.deepcopy(initial)
                state.update(fields)
                state["state"] = state_name
                state["qualification_receipt_root"] = "sha256:" + "3" * 64
                self.assertTrue(list(Draft202012Validator(schema).iter_errors(state)))
                with self.assertRaisesRegex(
                    VERIFY.VerificationError, "pre_runtime_qualification_receipt"
                ):
                    VERIFY.verify_prelaunch_state(state, machine)

    def test_configuration_relation_reversal_blocks_flagship(self) -> None:
        fixture = json.loads((ROOT / "scoring-fixtures.json").read_text())
        case = next(
            item
            for item in fixture["flagship_cases"]
            if item["name"] == "aggregate_passes_but_configuration_relation_reverses"
        )
        summary = VERIFY.expand_score_fixture_case(case)
        self.assertTrue(
            VERIFY.aggregate_flagship_gates_pass(summary["aggregate"], self.contract)
        )
        self.assertTrue(
            VERIFY.family_flagship_gates_pass(summary["families"], self.contract)
        )
        self.assertFalse(
            VERIFY.configuration_flagship_gates_pass(
                summary["configurations"], self.contract
            )
        )
        self.assertFalse(VERIFY.flagship_pass(summary, self.contract))

    def test_configuration_change_and_impact_reversals_block_flagship(self) -> None:
        fixture = json.loads((ROOT / "scoring-fixtures.json").read_text())
        cases = {item["name"]: item for item in fixture["flagship_cases"]}
        for name in (
            "aggregate_passes_but_configuration_change_reverses",
            "aggregate_passes_but_configuration_impact_reverses",
        ):
            with self.subTest(case=name):
                case = cases[name]
                summary = VERIFY.expand_score_fixture_case(case)
                VERIFY.verify_score_summary_feasibility(summary, self.contract)
                self.assertTrue(
                    VERIFY.aggregate_flagship_gates_pass(
                        summary["aggregate"], self.contract
                    )
                )
                self.assertTrue(
                    VERIFY.family_flagship_gates_pass(
                        summary["families"], self.contract
                    )
                )
                self.assertFalse(
                    VERIFY.configuration_flagship_gates_pass(
                        summary["configurations"], self.contract
                    )
                )
                self.assertFalse(VERIFY.flagship_pass(summary, self.contract))

    def test_realizable_positive_configuration_null_and_overall_null(self) -> None:
        fixture = json.loads((ROOT / "scoring-fixtures.json").read_text())
        cases = {item["name"]: item for item in fixture["flagship_cases"]}
        expected = {
            "realizable_registered_positive": True,
            "realizable_configuration_null_with_aggregate_lift": True,
            "realizable_overall_null": False,
        }
        for name, expected_pass in expected.items():
            with self.subTest(case=name):
                summary = VERIFY.expand_score_fixture_case(cases[name])
                VERIFY.verify_score_summary_feasibility(summary, self.contract)
                self.assertIs(
                    VERIFY.flagship_pass(summary, self.contract), expected_pass
                )

    def test_score_summary_feasibility_mutations_are_rejected(self) -> None:
        fixture = json.loads((ROOT / "scoring-fixtures.json").read_text())
        positive = next(
            item
            for item in fixture["flagship_cases"]
            if item["name"] == "realizable_registered_positive"
        )
        positive = VERIFY.expand_score_fixture_case(positive)
        mutations = []

        impossible_composite = copy.deepcopy(positive)
        impossible_composite["families"][0]["assisted"]["relation_correct"] = 4
        mutations.append(
            (impossible_composite, "score_composite_component_feasibility")
        )

        inconsistent_total = copy.deepcopy(positive)
        inconsistent_total["aggregate"]["raw"]["relation_correct"] = 25
        mutations.append((inconsistent_total, "score_family_aggregate_sum"))

        wrong_partition = copy.deepcopy(positive)
        wrong_partition["partition_counts"]["assisted"] = 35
        mutations.append((wrong_partition, "score_partition_counts"))

        wrong_denominator = copy.deepcopy(positive)
        wrong_denominator["configurations"][0]["assisted"]["denominator"] = 17
        mutations.append((wrong_denominator, "score_denominator"))

        for summary, error in mutations:
            with (
                self.subTest(error=error),
                self.assertRaisesRegex(VERIFY.VerificationError, error),
            ):
                VERIFY.verify_score_summary_feasibility(summary, self.contract)


if __name__ == "__main__":
    unittest.main()
