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
    def custody(self) -> dict[str, object]:
        return {
            "schema": "vela.lossless-provider-request-custody.v1",
            "content_type": "application/json",
            "bytes": 42,
            "sha256": "sha256:" + "a" * 64,
            "payload_encoding": "base64-rfc4648-canonical",
            "decode_count": 1,
            "provider_schema_bytes": 7,
            "provider_schema_sha256": "sha256:" + "b" * 64,
            "provider_schema_occurrences": 1,
            "endpoint_write_prepared": True,
        }

    def test_zero_pre_request_failure_is_zero_everywhere(self) -> None:
        self.assertEqual(
            CONTROLLER.derive_provider_calls(
                [], bridge=0, runner=0, terminal=0, custody=0
            ),
            0,
        )

    def test_sequential_endpoint_receipts_are_only_source(self) -> None:
        receipts = [
            {
                "type": "endpoint_attempt",
                "provider_calls": 1,
                "request_custody": self.custody(),
            },
            {
                "type": "endpoint_attempt",
                "provider_calls": 2,
                "request_custody": self.custody(),
            },
        ]
        self.assertEqual(
            CONTROLLER.derive_provider_calls(
                receipts, bridge=2, runner=2, terminal=2, custody=2
            ),
            2,
        )

    def test_hardcoded_boolean_sequence_and_cross_layer_drift_fail(self) -> None:
        cases = (
            (
                [
                    {
                        "type": "endpoint_attempt",
                        "provider_calls": True,
                        "request_custody": self.custody(),
                    }
                ],
                1,
                1,
                1,
                1,
            ),
            (
                [
                    {
                        "type": "endpoint_attempt",
                        "provider_calls": 2,
                        "request_custody": self.custody(),
                    }
                ],
                1,
                1,
                1,
                1,
            ),
            (
                [
                    {
                        "type": "endpoint_attempt",
                        "provider_calls": 1,
                        "request_custody": self.custody(),
                    }
                ],
                1,
                0,
                1,
                1,
            ),
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

    def test_request_custody_is_closed_and_exact(self) -> None:
        mutations = []
        for key, value in (
            ("decode_count", True),
            ("provider_schema_occurrences", 0),
            ("endpoint_write_prepared", False),
            ("payload_encoding", "json.RawMessage"),
        ):
            custody = self.custody()
            custody[key] = value
            mutations.append(custody)
        extra = self.custody()
        extra["semantic_equality"] = True
        mutations.append(extra)
        for custody in mutations:
            with self.subTest(custody=custody), self.assertRaises(ValueError):
                CONTROLLER.derive_provider_calls(
                    [
                        {
                            "type": "endpoint_attempt",
                            "provider_calls": 1,
                            "request_custody": custody,
                        }
                    ],
                    bridge=1,
                    runner=1,
                    terminal=1,
                    custody=1,
                )


if __name__ == "__main__":
    unittest.main()
