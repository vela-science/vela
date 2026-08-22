from __future__ import annotations

import importlib.util
import json
import os
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("anthropic_v3_verify", HERE / "verify.py")
assert SPEC and SPEC.loader
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class VerificationTests(unittest.TestCase):
    def test_valid_artifact(self) -> None:
        result = VERIFY.verify()
        self.assertEqual(result["status"], "PASS_STOPPED_NON_RESULT")
        self.assertFalse(result["positive_qualification"])

    def mutate(self, relative: str, function) -> str:
        with tempfile.TemporaryDirectory() as temporary:
            copy = Path(temporary) / "artifact"
            shutil.copytree(HERE, copy)
            function(copy / relative)
            with (
                mock.patch.object(VERIFY, "PACKAGE", copy),
                self.assertRaises(VERIFY.VerificationError) as raised,
            ):
                VERIFY.verify()
            return str(raised.exception)

    def reseal(self, root: Path) -> None:
        with mock.patch.object(VERIFY, "PACKAGE", root):
            (root / "artifact-root.json").write_text(
                json.dumps(VERIFY.seal_manifest(root), indent=2, sort_keys=True) + "\n"
            )

    def mutate_json(self, relative: str, change) -> str:
        def action(path: Path) -> None:
            value = json.loads(path.read_text())
            change(value)
            path.write_text(json.dumps(value, separators=(",", ":")) + "\n")
            self.reseal(path.parents[len(Path(relative).parts) - 1])

        return self.mutate(relative, action)

    def test_cross_layer_boolean_call_count_rejected_after_reseal(self) -> None:
        self.assertIn(
            "raw_original_bytes",
            self.mutate_json(
                "raw/endpoint-contact-receipt.json",
                lambda x: x.__setitem__("provider_calls", True),
            ),
        )

    def test_cross_layer_count_drift_rejected_after_reseal(self) -> None:
        self.assertIn(
            "raw_original_bytes",
            self.mutate_json(
                "raw/attempt-terminal.json",
                lambda x: x.__setitem__("bridge_provider_calls", 0),
            ),
        )

    def test_request_drift_rejected_after_reseal(self) -> None:
        self.assertIn(
            "raw_original_bytes",
            self.mutate_json(
                "raw/request.raw.json", lambda x: x.__setitem__("model", "drift")
            ),
        )

    def test_response_authority_inflation_rejected_after_reseal(self) -> None:
        self.assertIn(
            "raw_original_bytes",
            self.mutate_json(
                "raw/response.raw.json",
                lambda x: x["authority_scientific_inference"].__setitem__(
                    "repository_authority_effect", "repository_local_decision_evidenced"
                ),
            ),
        )

    def test_usage_binding_drift_rejected_after_reseal(self) -> None:
        self.assertIn(
            "raw_original_bytes",
            self.mutate_json(
                "raw/provider-usage-0001.json",
                lambda x: x.__setitem__(
                    "provider_response_sha256", "sha256:" + "0" * 64
                ),
            ),
        )

    def test_permit_swap_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            path.write_bytes(
                (
                    VERIFY.RUNTIME
                    / "offline-qualification-assets/openai-held_permit.json"
                ).read_bytes()
            )
            self.reseal(path.parents[1])

        self.assertIn(
            "consumed_permit_exact_held_bytes",
            self.mutate(
                "permit/neutral-calibration-anthropic-json-v3-replacement.permit.consumed.json",
                action,
            ),
        )

    def test_extra_file_rejected(self) -> None:
        self.assertIn(
            "artifact_file_set",
            self.mutate(
                "README.md", lambda path: (path.parent / "extra").write_text("x")
            ),
        )

    def test_symlink_rejected(self) -> None:
        def action(path: Path) -> None:
            os.symlink("README.md", path.parent / "link")

        self.assertIn("symbolic_path", self.mutate("README.md", action))

    def test_hardlink_rejected(self) -> None:
        def action(path: Path) -> None:
            os.link(path, path.parent / "hard")

        self.assertIn("file_link_count", self.mutate("README.md", action))

    def test_positive_qualification_contradiction_rejected_after_reseal(self) -> None:
        self.assertIn(
            "classification",
            self.mutate_json(
                "terminal-outcome.json",
                lambda x: x["classification"].__setitem__(
                    "positive_qualification", True
                ),
            ),
        )

    def test_provider_success_contradiction_rejected_after_reseal(self) -> None:
        self.assertIn(
            "classification",
            self.mutate_json(
                "terminal-outcome.json",
                lambda x: x["classification"].__setitem__(
                    "provider_response", "failed"
                ),
            ),
        )

    def test_byte_equality_inflation_rejected_after_reseal(self) -> None:
        self.assertIn(
            "comparison_truth",
            self.mutate_json(
                "terminal-outcome.json",
                lambda x: x["request_comparison"].__setitem__("byte_equal", True),
            ),
        )

    def test_semantic_inequality_inflation_rejected_after_reseal(self) -> None:
        self.assertIn(
            "comparison_truth",
            self.mutate_json(
                "terminal-outcome.json",
                lambda x: x["request_comparison"].__setitem__("semantic_equal", False),
            ),
        )

    def test_schema_occurrence_drift_rejected_after_reseal(self) -> None:
        self.assertIn(
            "comparison_occurrences",
            self.mutate_json(
                "terminal-outcome.json",
                lambda x: x["request_comparison"]["schema_occurrences"].__setitem__(
                    "actual_frozen_schema", 1
                ),
            ),
        )

    def test_retry_or_reuse_inflation_rejected_after_reseal(self) -> None:
        self.assertIn(
            "outcome_attempt",
            self.mutate_json(
                "terminal-outcome.json",
                lambda x: x["attempt"].__setitem__("no_reuse", False),
            ),
        )

    def test_lossless_transport_claim_rejected_after_reseal(self) -> None:
        self.assertIn(
            "cause",
            self.mutate_json(
                "terminal-outcome.json",
                lambda x: x["cause"].__setitem__(
                    "prospective_lossless_byte_payload_transport", "implemented"
                ),
            ),
        )

    def test_actual_transmitted_body_drift_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            path.write_bytes(path.read_bytes() + b" ")
            self.reseal(path.parents[1])

        self.assertIn(
            "actual_request_extraction",
            self.mutate("raw/actual-transmitted-body.raw.json", action),
        )

    def test_provider_event_duplicate_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            raw = path.read_bytes().replace(
                b'{"type":"endpoint_attempt",',
                b'{"type":"endpoint_attempt","type":"endpoint_attempt",',
                1,
            )
            path.write_bytes(raw)
            self.reseal(path.parents[1])

        self.assertIn(
            "raw_original_bytes",
            self.mutate("raw/provider-events.raw.jsonl", action),
        )

    def test_usage_boolean_rejected_after_reseal(self) -> None:
        self.assertIn(
            "raw_original_bytes",
            self.mutate_json(
                "raw/provider-usage-0001.json",
                lambda x: x["usage"].__setitem__("cache_read_input_tokens", False),
            ),
        )

    def test_credential_receipt_unknown_field_rejected_after_reseal(self) -> None:
        self.assertIn(
            "raw_original_bytes",
            self.mutate_json(
                "raw/credential-nonretention.json",
                lambda x: x.__setitem__("credential_value", "absent"),
            ),
        )

    def test_terminal_body_drift_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            raw = path.read_bytes().replace(
                b'"relation_validation":"valid"', b'"relation_validation":"invalid"', 1
            )
            path.write_bytes(raw)
            self.reseal(path.parents[1])

        self.assertIn(
            "raw_original_bytes",
            self.mutate("raw/bridge-to-runner.raw.jsonl", action),
        )

    def test_outcome_unknown_field_rejected_after_reseal(self) -> None:
        self.assertIn(
            "outcome_shape",
            self.mutate_json(
                "terminal-outcome.json",
                lambda x: x.__setitem__("qualification", "positive"),
            ),
        )

    def test_outcome_duplicate_field_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            raw = path.read_bytes().replace(
                b'{"attempt":', b'{"schema":"duplicate","attempt":', 1
            )
            path.write_bytes(raw)
            self.reseal(path.parent)

        self.assertIn(
            "duplicate_json_field", self.mutate("terminal-outcome.json", action)
        )

    def test_provider_response_drift_rejected_after_reseal(self) -> None:
        self.assertIn(
            "raw_original_bytes",
            self.mutate_json(
                "raw/provider-response-0001.raw.json",
                lambda x: x["usage"].__setitem__("input_tokens", 1892),
            ),
        )

    def test_transcript_copy_drift_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            path.write_bytes(
                path.read_bytes().replace(
                    b'"provider_calls":1', b'"provider_calls":0', 1
                )
            )
            self.reseal(path.parents[1])

        self.assertIn(
            "raw_original_bytes",
            self.mutate("raw/provider-events.raw.jsonl", action),
        )

    def test_credential_contradiction_rejected_after_reseal(self) -> None:
        self.assertIn(
            "raw_original_bytes",
            self.mutate_json(
                "raw/credential-nonretention.json",
                lambda x: x.__setitem__("credential_retained", True),
            ),
        )


if __name__ == "__main__":
    unittest.main()
