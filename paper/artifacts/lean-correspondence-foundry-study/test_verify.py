from __future__ import annotations

import copy
import importlib.util
import json
import unittest
from pathlib import Path

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


if __name__ == "__main__":
    unittest.main()
