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
        self.assertEqual(VERIFY.verify()["status"], "PASS")

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
            "endpoint_calls",
            self.mutate_json(
                "raw/endpoint-contact-receipt.json",
                lambda x: x.__setitem__("provider_calls", True),
            ),
        )

    def test_cross_layer_count_drift_rejected_after_reseal(self) -> None:
        self.assertIn(
            "cross_layer_calls",
            self.mutate_json(
                "raw/attempt-terminal.json",
                lambda x: x.__setitem__("bridge_provider_calls", 0),
            ),
        )

    def test_request_drift_rejected_after_reseal(self) -> None:
        self.assertIn(
            "request_exact",
            self.mutate_json(
                "raw/request.raw.json", lambda x: x.__setitem__("model", "drift")
            ),
        )

    def test_response_authority_inflation_rejected_after_reseal(self) -> None:
        self.assertIn(
            "response_root",
            self.mutate_json(
                "raw/response.raw.json",
                lambda x: x["authority_scientific_inference"].__setitem__(
                    "repository_authority_effect", "repository_local_decision_evidenced"
                ),
            ),
        )

    def test_usage_binding_drift_rejected_after_reseal(self) -> None:
        self.assertIn(
            "usage_binding",
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


if __name__ == "__main__":
    unittest.main()
