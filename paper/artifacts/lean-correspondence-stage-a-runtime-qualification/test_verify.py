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

    @staticmethod
    def reroot_configuration(registration, index) -> None:
        configuration = registration["participant_configurations"][index]
        body = copy.deepcopy(configuration)
        body.pop("configuration_root", None)
        configuration["configuration_root"] = VERIFY.canonical_root(body)

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

    def test_reviewer_configuration_weakenings_fail_after_reroot(self) -> None:
        mutations = (
            ("openai_store", 0, "store", True),
            ("openai_effort", 0, "reasoning_effort", "low"),
            ("openai_one_token", 0, "max_output_tokens", 1),
            ("anthropic_thinking", 1, "thinking", "disabled"),
            ("anthropic_tier", 1, "service_tier", "auto"),
        )
        for label, index, key, value in mutations:
            with self.subTest(label=label):
                candidate = copy.deepcopy(self.registration)
                candidate["participant_configurations"][index]["parameters"][key] = (
                    value
                )
                self.reroot_configuration(candidate, index)
                self.assert_blocked(candidate, "configuration_contract_drift")

    def test_api_version_model_and_omission_substitutions_fail(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["participant_configurations"][0]["api"]["provider_api_version"] = (
            "chat-completions-v1"
        )
        self.reroot_configuration(candidate, 0)
        self.assert_blocked(candidate, "configuration_contract_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["participant_configurations"][1]["api"][
            "anthropic_version_header"
        ] = "2024-01-01"
        self.reroot_configuration(candidate, 1)
        self.assert_blocked(candidate, "configuration_contract_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["participant_configurations"][0]["model"] = "gpt-5.6"
        self.reroot_configuration(candidate, 0)
        self.assert_blocked(candidate, "provider_or_model_substitution")

        candidate = copy.deepcopy(self.registration)
        del candidate["participant_configurations"][0]["parameters"]["temperature"]
        self.reroot_configuration(candidate, 0)
        self.assert_blocked(candidate, "runtime_parameter_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["participant_configurations"][1]["parameters"][
            "temperature_was_omitted"
        ] = True
        self.reroot_configuration(candidate, 1)
        self.assert_blocked(candidate, "configuration_contract_drift")

    def test_runtime_capture_empty_reordered_or_custody_weakened_fails(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["runtime_contract"]["capture"] = []
        self.assert_blocked(candidate, "runtime_capture_contract_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["runtime_contract"]["capture"] = list(
            reversed(candidate["runtime_contract"]["capture"])
        )
        self.assert_blocked(candidate, "runtime_capture_contract_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["runtime_contract"]["capture_custody"][
            "ordered_manifest_root_required"
        ] = False
        self.assert_blocked(candidate, "runtime_contract_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["runtime_contract"]["capture_custody"][
            "tool_call_precedes_matching_result"
        ] = False
        self.assert_blocked(candidate, "runtime_contract_drift")

    def test_subscription_oauth_and_credential_substitutions_fail(self) -> None:
        candidate = copy.deepcopy(self.registration)
        credential = candidate["credentials"][0]
        credential["accepted_credential_class"] = credential[
            "available_non_substitutable_class"
        ]
        credential["subscription_oauth_admissible"] = True
        self.assert_blocked(candidate, "credential_admissibility_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["credentials"][1]["provider_organization"] = "OpenAI"
        self.assert_blocked(candidate, "credential_contract_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["credentials"][0]["credential_reference_class"] = (
            "inline_secret_value"
        )
        self.assert_blocked(candidate, "credential_admissibility_drift")

    def test_network_shell_filesystem_and_hidden_answer_weakenings_fail(self) -> None:
        mutations = (
            ("unrestricted_egress", ("network", "runner_egress"), "unrestricted"),
            (
                "hidden_answers",
                ("filesystem", "hidden_answer_paths"),
                "mounted_read_only",
            ),
            (
                "shell_interpolation",
                ("read_only_shell", "invocation"),
                "shell_string_with_interpolation",
            ),
            (
                "path_escape",
                ("tools", 1, "arguments", "path"),
                "arbitrary_absolute_or_relative_path",
            ),
        )
        for label, path, value in mutations:
            with self.subTest(label=label):
                policy = copy.deepcopy(self.tool_policy)
                target = policy
                for component in path[:-1]:
                    target = target[component]
                target[path[-1]] = value
                self.assert_blocked(
                    self.registration, "tool_policy_contract_drift", policy
                )

        policy = copy.deepcopy(self.tool_policy)
        policy["read_only_shell"]["allowed_commands"].append("curl")
        self.assert_blocked(self.registration, "tool_policy_contract_drift", policy)

        policy = copy.deepcopy(self.tool_policy)
        policy["filesystem"]["root_filesystem"] = "read_write"
        self.assert_blocked(self.registration, "mutable_tool_boundary", policy)

    def test_schema_and_custody_substitutions_fail_after_dependent_reroot(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["provider_schema_derivation"]["authoritative_schema_sha256"] = (
            "sha256:" + "5" * 64
        )
        boundary_root = VERIFY.information_boundary_root(
            candidate, candidate["participant_configurations"][0]["tool_policy_sha256"]
        )
        for index, configuration in enumerate(candidate["participant_configurations"]):
            configuration["information_boundary_root"] = boundary_root
            self.reroot_configuration(candidate, index)
        self.assert_blocked(candidate, "information_boundary_contract_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["participant_configurations"][1]["information_boundary_root"] = (
            "sha256:" + "6" * 64
        )
        self.reroot_configuration(candidate, 1)
        self.assert_blocked(candidate, "cross_provider_atom_or_tool_mismatch")

    def test_outer_reroot_cannot_replace_stage_method_or_unchecked_fields(self) -> None:
        candidate = copy.deepcopy(self.registration)
        candidate["stage_a_binding"]["pilot_commit"] = "0" * 40
        candidate["stage_a_binding"]["pilot_tree"] = "1" * 40
        self.assert_blocked(candidate, "stage_a_binding_contract_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["method_binding"]["producer_commit"] = "3" * 40
        candidate["method_binding"]["reviewed_method_directory_tree"] = "4" * 40
        self.assert_blocked(candidate, "method_binding_contract_drift")

        candidate = copy.deepcopy(self.registration)
        candidate["source_references"][0]["claim"] = "rerooted outer claim"
        self.assert_blocked(candidate, "registration_contract_drift")


if __name__ == "__main__":
    unittest.main()
