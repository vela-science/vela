from __future__ import annotations

import copy
import importlib.util
import sys
import unittest
from pathlib import Path

PACKAGE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "stage_a_runtime_verify", PACKAGE / "verify.py"
)
assert SPEC and SPEC.loader
VERIFY = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = VERIFY
SPEC.loader.exec_module(VERIFY)


class RuntimeQualificationCandidateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.registration = VERIFY.load_json(VERIFY.REGISTRATION)
        self.tool_policy = VERIFY.load_json(VERIFY.TOOL_POLICY)

    def assert_blocked(self, registration, message, tool_policy=None) -> None:
        with self.assertRaisesRegex(VERIFY.CandidateError, message):
            VERIFY.validate_candidate(
                registration,
                self.tool_policy if tool_policy is None else tool_policy,
                check_git=False,
            )

    def test_exact_held_candidate_passes_without_provider_or_permit(self) -> None:
        receipt = VERIFY.validate_candidate(
            self.registration, self.tool_policy, check_git=True
        )
        self.assertEqual(receipt["status"], "pass_exact_held_blocker")
        self.assertEqual(receipt["provider_calls"], 0)
        self.assertEqual(receipt["neutral_calibration_permits"], 0)
        self.assertIsNone(receipt["qualification_receipt_root"])
        self.assertFalse(receipt["neutral_calibration_separately_authorizable"])

    def test_provider_or_model_substitution_fails(self) -> None:
        for field, value in (
            ("provider_organization", "OpenAI"),
            ("model", "gpt-5.6-terra"),
        ):
            with self.subTest(field=field):
                candidate = copy.deepcopy(self.registration)
                candidate["participant_configurations"][1][field] = value
                self.assert_blocked(candidate, "provider_or_model_substitution")

    def test_cross_provider_atom_or_tool_mismatch_fails(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["participant_configurations"][1]["information_boundary_root"] = (
            "sha256:" + "0" * 64
        )
        self.assert_blocked(candidate, "cross_provider_atom_or_tool_mismatch")

    def test_mutable_tool_boundary_fails(self) -> None:
        policy = copy.deepcopy(self.tool_policy)
        policy["filesystem"]["assignment_mount"] = "read_write"
        self.assert_blocked(self.registration, "mutable_tool_boundary", policy)

    def test_unsupported_response_schema_cannot_be_claimed(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["provider_schema_derivation"]["provider_derivatives"] = [
            {"provider": "OpenAI", "root": "sha256:" + "1" * 64}
        ]
        self.assert_blocked(candidate, "unsupported_schema_claimed")

    def test_missing_or_drifted_trust_runtime_image_roots_fail(self) -> None:
        for key, value in (
            ("trust_bundle_root", "sha256:" + "2" * 64),
            ("images", [{"digest": "sha256:" + "3" * 64}]),
            ("absolute_read_only_mounts", [{"source": "/tmp", "read_only": True}]),
        ):
            with self.subTest(key=key):
                candidate = copy.deepcopy(self.registration)
                candidate["image_boundary"][key] = value
                self.assert_blocked(
                    candidate, "missing_or_drifted_trust_runtime_image_roots_not_held"
                )

    def test_stale_qualifier_receipt_fails(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["maintained_qualifier"]["qualification_receipt_root"] = (
            "sha256:" + "4" * 64
        )
        self.assert_blocked(candidate, "stale_or_early_qualifier_receipt")

    def test_credential_retention_fails(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["credentials"][0]["retained"] = True
        self.assert_blocked(candidate, "credential_retention_or_status_drift")

    def test_early_permit_release_fails(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["permits"]["neutral_calibration"] = [
            {"id": "neutral-a", "status": "held"}
        ]
        self.assert_blocked(candidate, "early_permit_creation")

    def test_scheduler_and_retry_paths_fail(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["runtime_contract"]["no_scheduler"] = False
        self.assert_blocked(candidate, "runtime_boundary_mutable:no_scheduler")
        candidate = copy.deepcopy(self.registration)
        candidate["participant_configurations"][0]["parameters"]["retries"] = 1
        self.assert_blocked(candidate, "runtime_parameter_drift")

    def test_qualifier_or_registered_schema_root_drift_fails(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["maintained_qualifier"]["blob"] = "0" * 40
        self.assert_blocked(candidate, "qualifier_root_drift")
        candidate = copy.deepcopy(self.registration)
        candidate["provider_schema_derivation"]["unproved_keywords_present"] = []
        self.assert_blocked(candidate, "unsupported_response_schema_not_held")


if __name__ == "__main__":
    unittest.main()
