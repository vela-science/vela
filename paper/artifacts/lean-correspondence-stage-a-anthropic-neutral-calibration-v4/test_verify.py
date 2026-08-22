from __future__ import annotations

import base64
import importlib.util
import json
import os
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("anthropic_v4_verify", HERE / "verify.py")
assert SPEC and SPEC.loader
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class VerificationTests(unittest.TestCase):
    def test_valid_artifact(self) -> None:
        result = VERIFY.verify()
        self.assertEqual(result["status"], "PASS_INDEPENDENTLY_QUALIFIED_ANTHROPIC_V4")
        self.assertEqual(result["provider_calls"], 1)
        self.assertEqual(result["retries"], 0)
        self.assertTrue(result["positive_qualification"])

    def mutate(self, relative: str, action) -> str:
        with tempfile.TemporaryDirectory() as temporary:
            copy = Path(temporary) / "artifact"
            shutil.copytree(HERE, copy)
            action(copy / relative)
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

    def mutate_frame(self, change) -> str:
        def action(path: Path) -> None:
            root = path.parents[1]
            value = json.loads(path.read_text())
            change(value)
            raw = (json.dumps(value, separators=(",", ":")) + "\n").encode()
            path.write_bytes(raw)
            (root / "raw/provider-request-frame.raw.jsonl").write_bytes(raw)
            self.reseal(root)

        return self.mutate("raw/runner-to-bridge.raw.jsonl", action)

    def test_frame_base64_noncanonical_rejected_after_reseal(self) -> None:
        self.mutate_frame(
            lambda x: x["payload"].__setitem__("base64", x["payload"]["base64"] + "\n")
        )

    def test_frame_base64_double_encoding_rejected_after_reseal(self) -> None:
        self.mutate_frame(
            lambda x: x["payload"].__setitem__(
                "base64", base64.b64encode(x["payload"]["base64"].encode()).decode()
            )
        )

    def test_rawmessage_semantic_fallback_rejected_after_reseal(self) -> None:
        def change(value: dict) -> None:
            payload = value.pop("payload")
            value["request"] = json.loads(base64.b64decode(payload["base64"]))

        self.mutate_frame(change)

    def test_frame_request_length_drift_rejected_after_reseal(self) -> None:
        self.mutate_frame(lambda x: x["payload"].__setitem__("bytes", 4277))

    def test_frame_request_root_drift_rejected_after_reseal(self) -> None:
        self.mutate_frame(
            lambda x: x["payload"].__setitem__("sha256", "sha256:" + "0" * 64)
        )

    def test_frame_schema_occurrence_drift_rejected_after_reseal(self) -> None:
        self.mutate_frame(
            lambda x: x["payload"].__setitem__("provider_schema_occurrences", 0)
        )

    def test_actual_network_body_reformat_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            path.write_text(json.dumps(json.loads(path.read_text())) + "\n")
            self.reseal(path.parents[1])

        self.mutate("raw/actual-network-body-0001.raw.json", action)

    def test_network_custody_unknown_field_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/lossless-network-request-custody.json",
            lambda x: x.__setitem__("semantic_only", True),
        )

    def test_network_custody_boolean_count_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/lossless-network-request-custody.json",
            lambda x: x.__setitem__("frame_decode_count", True),
        )

    def test_endpoint_boolean_call_count_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/endpoint-contact-receipt.json",
            lambda x: x.__setitem__("provider_calls", True),
        )

    def test_attempt_cross_layer_count_drift_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/attempt-terminal.json",
            lambda x: x.__setitem__("bridge_provider_calls", 0),
        )

    def test_attempt_unknown_field_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/attempt-terminal.json",
            lambda x: x.__setitem__("retry_authorized", True),
        )

    def test_attempt_retry_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/attempt-terminal.json", lambda x: x.__setitem__("retries", 1)
        )

    def test_permit_reuse_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/permit-release.json", lambda x: x.__setitem__("attempt", 2)
        )

    def test_positive_qualification_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "terminal-outcome.json",
            lambda x: x.__setitem__("positive_qualification", True),
        )

    def test_missing_post_review_classification_rejected(self) -> None:
        self.mutate("post-review-classification.json", lambda path: path.unlink())

    def test_wrong_review_commit_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "post-review-classification.json",
            lambda x: x["independent_review"].__setitem__("commit", VERIFY.PRODUCER),
        )

    def test_wrong_review_tree_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "post-review-classification.json",
            lambda x: x["independent_review"].__setitem__("tree", "0" * 40),
        )

    def test_wrong_review_parent_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "post-review-classification.json",
            lambda x: x["independent_review"].__setitem__("sole_parent", "0" * 40),
        )

    def test_review_report_digest_drift_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "post-review-classification.json",
            lambda x: x["independent_review"]["report"].__setitem__(
                "sha256", "sha256:" + "0" * 64
            ),
        )

    def test_review_verdict_digest_drift_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "post-review-classification.json",
            lambda x: x["independent_review"]["verdict"].__setitem__(
                "sha256", "sha256:" + "0" * 64
            ),
        )

    def test_review_verdict_value_drift_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "post-review-classification.json",
            lambda x: x["independent_review"]["verdict"].__setitem__(
                "value", "BLOCKED"
            ),
        )

    def test_post_review_boolean_as_counter_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "post-review-classification.json",
            lambda x: x["amendment_actions"].__setitem__("provider_calls", False),
        )

    def test_post_review_raw_execution_rewrite_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "post-review-classification.json",
            lambda x: x["amendment_actions"].__setitem__(
                "raw_execution_bytes_modified", True
            ),
        )

    def test_post_review_overclaims_rejected_after_reseal(self) -> None:
        cases = (
            ("openai_qualification", True),
            ("participant_execution_authorized", True),
            ("participant_permit_release_authorized", True),
            ("scoring_authorized", True),
            ("stage_b_selection_authorized", True),
            ("scientific_claim_authorized", True),
            ("protocol_or_core_effect", "modified"),
            ("authority_decision_or_standing_effect", "created"),
        )
        for key, value in cases:
            with self.subTest(key=key):
                self.mutate_json(
                    "post-review-classification.json",
                    lambda x, key=key, value=value: x["claim_ceiling"].__setitem__(
                        key, value
                    ),
                )

    def test_post_review_unknown_field_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "post-review-classification.json",
            lambda x: x["classification"].__setitem__("full_study_qualified", True),
        )

    def test_reviewed_raw_byte_drift_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            path.write_bytes(path.read_bytes() + b" ")
            self.reseal(path.parents[1])

        self.mutate("raw/actual-network-body-0001.raw.json", action)

    def test_bridge_adapter_build_binding_drift_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "execution-build.json",
            lambda x: x["build_parameters"]["anthropic_host_bridge"].__setitem__(
                "provider_adapter", "unbound"
            ),
        )

    def test_binary_identity_drift_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "execution-build.json",
            lambda x: x["binaries"]["anthropic_host_bridge"].__setitem__(
                "sha256", "sha256:" + "0" * 64
            ),
        )

    def test_execution_source_drift_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            path.write_bytes(path.read_bytes() + b"\n")
            root = path.parents[1]
            build_path = root / "execution-build.json"
            build = json.loads(build_path.read_text())
            build["sources"]["controller.py"] = VERIFY.digest(path.read_bytes())
            build_path.write_text(json.dumps(build, indent=2, sort_keys=True) + "\n")
            self.reseal(root)

        self.mutate("execution-sources/controller.py", action)

    def test_provider_event_copy_drift_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            path.write_bytes(path.read_bytes() + b"\n")
            self.reseal(path.parents[1])

        self.mutate("raw/provider-events.raw.jsonl", action)

    def test_provider_response_drift_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/provider-response-0001.raw.json",
            lambda x: x.__setitem__("stop_reason", "tool_use"),
        )

    def test_provider_usage_boolean_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/provider-usage-0001.json",
            lambda x: x["usage"].__setitem__("input_tokens", True),
        )

    def test_terminal_count_drift_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/terminal.json", lambda x: x.__setitem__("provider_calls", 0)
        )

    def test_response_authority_inflation_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/response.raw.json",
            lambda x: x["authority_scientific_inference"].__setitem__(
                "repository_authority_effect", "repository_local_decision_evidenced"
            ),
        )

    def test_credential_metadata_contradiction_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/credential-nonretention.json",
            lambda x: x.__setitem__("credential_retained", True),
        )

    def test_credential_metadata_extra_rejected_after_reseal(self) -> None:
        self.mutate_json(
            "raw/credential-nonretention.json",
            lambda x: x.__setitem__("credential_copy", "none"),
        )

    def test_duplicate_json_key_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            raw = path.read_bytes().rstrip(b"\n")
            path.write_bytes(raw[:-1] + b',"provider_calls":1}\n')
            self.reseal(path.parents[1])

        self.mutate("raw/terminal.json", action)

    def test_secret_shaped_bytes_rejected_after_reseal(self) -> None:
        def action(path: Path) -> None:
            path.write_bytes(
                path.read_bytes() + b"\n" + b"ANTHROPIC_" + b"API_KEY" + b"=x\n"
            )
            self.reseal(path.parent)

        self.mutate("README.md", action)

    def test_extra_file_rejected(self) -> None:
        self.mutate(
            "README.md", lambda path: (path.parent / "undeclared").write_text("x")
        )

    def test_extra_directory_rejected(self) -> None:
        self.mutate("README.md", lambda path: (path.parent / "cache").mkdir())

    def test_symlink_rejected(self) -> None:
        self.mutate(
            "README.md", lambda path: os.symlink("README.md", path.parent / "link")
        )

    def test_hardlink_rejected(self) -> None:
        self.mutate("README.md", lambda path: os.link(path, path.parent / "hardlink"))

    def test_manifest_omission_rejected(self) -> None:
        def action(path: Path) -> None:
            value = json.loads(path.read_text())
            value["files"].pop()
            path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")

        self.mutate("artifact-root.json", action)

    def test_manifest_extra_rejected(self) -> None:
        def action(path: Path) -> None:
            value = json.loads(path.read_text())
            value["files"].append(
                {"path": "ghost", "bytes": 0, "sha256": VERIFY.digest(b"")}
            )
            path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")

        self.mutate("artifact-root.json", action)


if __name__ == "__main__":
    unittest.main()
