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
        self.offline = VERIFY.load_json(VERIFY.OFFLINE)

    def offline_records(self, value=None):
        return VERIFY.validate_offline(self.offline if value is None else value)

    def assert_offline_blocked(self, candidate, message: str) -> None:
        body = dict(candidate)
        body.pop("record_root", None)
        candidate["record_root"] = VERIFY.canonical_root(body)
        with self.assertRaisesRegex(VERIFY.CandidateError, message):
            VERIFY.validate_offline(candidate)

    def assert_registration_blocked(self, candidate, message: str) -> None:
        with self.assertRaisesRegex(VERIFY.CandidateError, message):
            VERIFY.validate_registration(
                candidate, self.offline_records(), check_git=False
            )

    def test_exact_candidate_passes_held_with_credentials_only_blocker(self) -> None:
        receipt = VERIFY.verify(check_credentials=False, check_git=True)
        self.assertEqual(receipt["provider_calls"], 0)
        self.assertEqual(receipt["neutral_calibrations_run"], 0)
        self.assertEqual(receipt["participant_calls"], 0)
        self.assertEqual(receipt["participant_permits_released"], 0)
        self.assertEqual(receipt["authority_effect"], "none")

    def test_boolean_cannot_replace_any_zero_counter_after_reseal(self) -> None:
        for path in (
            ("provider_calls",),
            ("neutral_calibrations_run",),
            ("participant_calls",),
            ("provider_records", 0, "provider_calls"),
            ("provider_records", 0, "qualification_receipt", "provider_calls"),
            ("provider_records", 1, "qualification_receipt", "scientific_sessions"),
            (
                "provider_records",
                1,
                "qualification_receipt",
                "participant_permits_consumed",
            ),
        ):
            with self.subTest(path=path):
                candidate = copy.deepcopy(self.offline)
                target = candidate
                for component in path[:-1]:
                    target = target[component]
                target[path[-1]] = False
                self.assert_offline_blocked(candidate, "counter|provider_calls")

    def test_provider_call_or_scientific_session_inflation_fails_after_reseal(
        self,
    ) -> None:
        for key in (
            "provider_calls",
            "scientific_sessions",
            "participant_permits_consumed",
        ):
            candidate = copy.deepcopy(self.offline)
            candidate["provider_records"][0]["qualification_receipt"][key] = 1
            self.assert_offline_blocked(candidate, "qualifier_counter")

    def test_early_neutral_permit_consumption_fails_after_reseal(self) -> None:
        candidate = copy.deepcopy(self.offline)
        candidate["provider_records"][0]["consumed_neutral_permit_exists"] = True
        self.assert_offline_blocked(candidate, "early_permit_consume")

    def test_image_configuration_tool_and_qualification_cross_binding_fail(
        self,
    ) -> None:
        for key in (
            "image_digest",
            "configuration_root",
            "tool_boundary_root",
            "qualification_root",
        ):
            with self.subTest(key=key):
                candidate = copy.deepcopy(self.offline)
                candidate["provider_records"][0]["qualification_receipt"][key] = (
                    candidate["provider_records"][1]["qualification_receipt"][key]
                )
                self.assert_offline_blocked(candidate, f"qualifier_binding:{key}")

    def test_same_image_substitution_fails_after_reseal(self) -> None:
        candidate = copy.deepcopy(self.offline)
        candidate["provider_records"][1]["qualification_receipt"]["image_digest"] = (
            candidate["provider_records"][0]["qualification_receipt"]["image_digest"]
        )
        self.assert_offline_blocked(candidate, "qualifier_binding:image_digest")

    def test_provider_equivalence_drift_fails_after_reseal(self) -> None:
        candidate = copy.deepcopy(self.offline)
        candidate["provider_records"][1]["qualification_receipt"][
            "provider_equivalence_root"
        ] = "sha256:" + "0" * 64
        self.assert_offline_blocked(candidate, "provider_equivalence_drift")

    def test_false_qualifier_gate_fails_after_reseal(self) -> None:
        candidate = copy.deepcopy(self.offline)
        candidate["provider_records"][0]["qualification_receipt"]["gates"][
            "trust_and_mounts"
        ] = False
        self.assert_offline_blocked(candidate, "qualifier_gate")

    def test_qualifier_commit_tree_or_bytes_substitution_fails(self) -> None:
        for key in ("git_commit", "git_tree", "sha256"):
            with self.subTest(key=key):
                candidate = copy.deepcopy(self.registration)
                candidate["maintained_qualifier"][key] = (
                    "sha256:" + "0" * 64 if key == "sha256" else "0" * 40
                )
                self.assert_registration_blocked(candidate, "qualifier_binding")

    def test_credentials_blocker_and_maintained_schema_registry_cannot_drift(
        self,
    ) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["blockers"] = []
        self.assert_registration_blocked(candidate, "blockers_drift")
        candidate = copy.deepcopy(self.registration)
        candidate["provider_schema_boundary"]["maintained_registry_rules"][0][2] = False
        self.assert_registration_blocked(candidate, "participant_schema_registry")
        candidate = copy.deepcopy(self.registration)
        candidate["provider_schema_boundary"]["participant_provider_derivatives"][0][
            "provider_schema_sha256"
        ] = "sha256:" + "1" * 64
        self.assert_registration_blocked(candidate, "participant_schema_derivatives")

    def test_early_authorization_or_neutral_release_fails(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["authorization"]["neutral_calibration_execution_authorized"] = True
        self.assert_registration_blocked(candidate, "early_authorization")
        candidate = copy.deepcopy(self.registration)
        candidate["neutral_calibration_permits"][0]["status"] = "released"
        self.assert_registration_blocked(candidate, "neutral_permit_released")

    def test_neutral_permit_cross_provider_binding_fails(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["neutral_calibration_permits"][1]["run_id"] = candidate[
            "neutral_calibration_permits"
        ][0]["run_id"]
        self.assert_registration_blocked(candidate, "neutral_permit_cross_binding")
        candidate = copy.deepcopy(self.registration)
        candidate["neutral_calibration_permits"][1]["permit_root"] = candidate[
            "neutral_calibration_permits"
        ][0]["permit_root"]
        self.assert_registration_blocked(candidate, "neutral_permit_root")

    def test_neutral_packet_equivalence_and_request_binding_drift_fails(self) -> None:
        for key, value in (
            ("packet_root", "sha256:" + "0" * 64),
            ("packet_path", "neutral-calibration/prompt.txt"),
            ("information_equivalent", False),
            ("inline_packet_allowed", True),
            ("runner_packet_mount_path", "/input/reconstructed.json"),
            ("request_binding", "decoded object accepted"),
        ):
            with self.subTest(key=key):
                candidate = copy.deepcopy(self.registration)
                candidate["neutral_calibration_content"][key] = value
                self.assert_registration_blocked(
                    candidate, "neutral_content_equivalence"
                )

    def test_retired_neutral_permit_cannot_be_released_reused_or_cross_bound(
        self,
    ) -> None:
        for key, value in (
            ("status", "held"),
            ("releasable", True),
            ("consumed", True),
            ("original_state", "consumed"),
        ):
            with self.subTest(key=key):
                candidate = copy.deepcopy(self.registration)
                candidate["retired_neutral_calibration_permits"][0][key] = value
                self.assert_registration_blocked(candidate, "retired_permit_state")
        candidate = copy.deepcopy(self.registration)
        candidate["retired_neutral_calibration_permits"][1]["successor_permit_root"] = (
            candidate["retired_neutral_calibration_permits"][0]["successor_permit_root"]
        )
        self.assert_registration_blocked(candidate, "retired_permit_state")

    def test_retained_packet_path_or_provider_swap_fails_after_reseal(self) -> None:
        candidate = copy.deepcopy(self.offline)
        candidate["provider_records"][0]["retained"]["neutral_packet"] = copy.deepcopy(
            candidate["provider_records"][0]["retained"]["neutral_prompt"]
        )
        self.assert_offline_blocked(candidate, "neutral_content_retained_binding")
        candidate = copy.deepcopy(self.offline)
        candidate["provider_records"][1]["retained"]["retired_permit"] = copy.deepcopy(
            candidate["provider_records"][0]["retained"]["retired_permit"]
        )
        self.assert_offline_blocked(candidate, "retired_permit_binding")

    def test_launchable_runtime_cross_binding_or_boundary_inflation_fails(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["runtime_boundary"]["runtime_images"][1]["image_digest"] = candidate[
            "runtime_boundary"
        ]["runtime_images"][0]["image_digest"]
        self.assert_registration_blocked(candidate, "runtime_image_binding")
        candidate = copy.deepcopy(self.registration)
        candidate["runtime_boundary"]["participant_network_until_authorized"] = True
        self.assert_registration_blocked(candidate, "runtime_boundary")
        candidate = copy.deepcopy(self.registration)
        candidate["runtime_boundary"]["writes"] = True
        self.assert_registration_blocked(candidate, "runtime_boundary")

    def test_retained_runner_or_launchability_cross_binding_fails_after_reseal(
        self,
    ) -> None:
        for label in (
            "runner",
            "bridge",
            "launchability",
            "provider_contract",
            "build_a",
        ):
            with self.subTest(label=label):
                candidate = copy.deepcopy(self.offline)
                candidate["provider_records"][1]["retained"][label] = copy.deepcopy(
                    candidate["provider_records"][0]["retained"][label]
                )
                self.assert_offline_blocked(
                    candidate,
                    "retained_digest|launchability|oci_launchable|provider_contract|retained_build",
                )

    def test_frozen_provider_parameters_cannot_drift(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["participant_configurations"][0]["parameters"][
            "max_output_tokens"
        ] = 1
        self.assert_registration_blocked(candidate, "frozen_configuration_drift")
        candidate = copy.deepcopy(self.registration)
        candidate["participant_configurations"][1]["api"][
            "anthropic_version_header"
        ] = "2024-01-01"
        self.assert_registration_blocked(candidate, "frozen_configuration_drift")

    def test_stage_a_binding_or_participant_release_fails(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["stage_a_binding"]["participant_permits_released"] = 1
        self.assert_registration_blocked(candidate, "stage_a_binding")
        candidate = copy.deepcopy(self.registration)
        candidate["stage_a_binding"]["pilot_commit"] = "0" * 40
        self.assert_registration_blocked(candidate, "stage_a_binding")

    def test_credential_presence_retention_or_value_observation_fails(self) -> None:
        for key, value in (
            ("presence", "present"),
            ("retained", True),
            ("value_observed", True),
        ):
            candidate = copy.deepcopy(self.registration)
            candidate["credentials"][0][key] = value
            self.assert_registration_blocked(candidate, "credential_state")

    def test_unknown_outer_field_fails_even_after_semantic_checks(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["execution_authorized"] = True
        self.assert_registration_blocked(candidate, "registration_root")


if __name__ == "__main__":
    unittest.main()
