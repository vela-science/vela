from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("neutral_controller.py")
SPEC = importlib.util.spec_from_file_location("neutral_controller", MODULE_PATH)
assert SPEC and SPEC.loader
CONTROLLER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CONTROLLER)


class ProviderCallDerivationTests(unittest.TestCase):
    def test_zero_pre_request_failure_is_zero_everywhere(self) -> None:
        self.assertEqual(
            CONTROLLER.derive_provider_calls(
                [], bridge=0, runner=0, terminal=0, custody=0
            ),
            0,
        )

    def test_sequential_endpoint_receipts_are_only_source(self) -> None:
        receipts = [
            {"type": "endpoint_attempt", "provider_calls": 1},
            {"type": "endpoint_attempt", "provider_calls": 2},
        ]
        self.assertEqual(
            CONTROLLER.derive_provider_calls(
                receipts, bridge=2, runner=2, terminal=2, custody=2
            ),
            2,
        )

    def test_hardcoded_boolean_sequence_and_cross_layer_drift_fail(self) -> None:
        cases = (
            ([{"type": "endpoint_attempt", "provider_calls": True}], 1, 1, 1, 1),
            ([{"type": "endpoint_attempt", "provider_calls": 2}], 1, 1, 1, 1),
            ([{"type": "endpoint_attempt", "provider_calls": 1}], 1, 0, 1, 1),
            ([], 0, False, 0, 0),
        )
        for receipts, bridge, runner, terminal, custody in cases:
            with (
                self.subTest(receipts=receipts, runner=runner),
                self.assertRaises(ValueError),
            ):
                CONTROLLER.derive_provider_calls(
                    receipts,
                    bridge=bridge,
                    runner=runner,
                    terminal=terminal,
                    custody=custody,
                )


if __name__ == "__main__":
    unittest.main()
