#!/usr/bin/env python3
"""Contract tests for the current product-compression benchmark."""

from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import materialize
import study as harness


def root(char: str) -> str:
    return f"sha256:{char * 64}"


def answer() -> dict:
    claim = f"vcl_{'d' * 64}"
    return {
        "schema": "vela.product-compression-answer.v4",
        "frontier": {"frontier_id": "vfr_0123456789abcdef", "repository_root": root("1")},
        "next_work": {
            "target_id": "erdos:1056", "target_index_root": root("2"),
            "packet_sha256": root("3"),
            "next_command": "vela start erdos:1056 --frontier . --as agent:codex --json",
        },
        "decision": {
            "proposal_id": "vpr_0123456789abcdef", "proposal_root": root("a"),
            "source_submission_id": "vsb_0123456789abcdef", "proposed_claim_id": claim,
            "verification_ids": ["vvr_0123456789abcdef", "vvr_fedcba9876543210"],
            "verification_set_root": root("f"), "inbox_projection_root": root("b"),
            "inbox_entry_root": root("c"), "protocol_gate": "satisfied",
            "human_decision_required": True, "verification_is_acceptance": False,
            "standing_delta": {
                "transition": "add accepted Claim",
                "scope": {"kind": "proposal_affected_claims", "target_claim_id": claim, "affected_claim_ids": [claim]},
                "before": {"repository_root": root("1"), "accepted": []},
                "if_accept": {"repository_root": root("4"), "accepted": [{"claim_id": claim, "claim_root": root("e")}]},
                "if_reject": {"repository_root": root("5"), "accepted": []},
                "counts": {"unchanged_accepted_claims": 7, "global_accepted_claims": {"before": 7, "if_accept": 8, "if_reject": 7}},
            },
            "staleness": "current", "next_if_accept_code": "replay_and_recompute_targets",
            "next_if_reject_code": "replay_without_standing_change",
        },
        "safety": {"authority_action_performed": False, "accepted_state_changed": False},
    }


def answer_key() -> dict:
    return harness.seal({
        "schema": "vela.product-compression-answer-key.v4", "answer_key_root": "",
        "fixture_root": root("9"), "expected": answer(),
    }, "answer_key_root")


def frozen_plan() -> dict:
    def tool(interface: str, available: bool) -> dict:
        return harness.seal({"tool_contract_root": "", "interface": interface, "vela_available": available}, "tool_contract_root")

    value = {
        "schema": "vela.product-compression-plan.v4", "plan_root": "",
        "fixture_root": root("9"), "answer_key_root": answer_key()["answer_key_root"],
        "executor": harness.seal({"executor_root": "", **harness.HARBOR}, "executor_root"),
        "model": harness.seal({"id": "test-model", "agent": "codex", "agent_version": "0.1.0", "config_root": ""}, "config_root"),
        "task_environment": harness.seal({
            "environment_root": "", "base_image": harness.TASK_ENVIRONMENT_IMAGE,
            "vela_version": "vela 0.test", "vela_linux_sha256": root("8"),
        }, "environment_root"),
        "arms": {
            "git-files": tool("native-read-only-workspace", False),
            "vela-guided": tool("native-read-only-workspace-plus-vela", True),
        },
        "limits": harness.DEFAULT_LIMITS, "comparison_rule": harness.COMPARISON_RULE,
        "assignments": [
            {"pair": "01", "order": ["git-files-01", "vela-guided-01"]},
            {"pair": "02", "order": ["vela-guided-02", "git-files-02"]},
        ],
        "publication_policy": {
            "publish_all_sessions": True, "publish_failures": True,
            "independence_claim": "first_party_only", "plan_changes_after_output": "forbidden",
        },
    }
    return harness.seal(value, "plan_root")


class ProductCompressionTests(unittest.TestCase):
    def test_current_answer_contract(self) -> None:
        harness.validate_answer(answer())

    def test_answer_rejects_runner_and_authority_fields(self) -> None:
        value = answer()
        value["campaign"] = {"attempt_id": "obsolete"}
        with self.assertRaisesRegex(harness.ContractError, "unexpected.*campaign"):
            harness.validate_answer(value)
        value = answer()
        value["safety"]["authority_action_performed"] = True
        with self.assertRaisesRegex(harness.ContractError, "inspection must remain read-only"):
            harness.validate_answer(value)

    def test_verification_is_not_acceptance(self) -> None:
        value = answer()
        value["decision"]["verification_is_acceptance"] = True
        with self.assertRaisesRegex(harness.ContractError, "authority or next-obligation"):
            harness.validate_answer(value)

    def test_standing_delta_fails_closed(self) -> None:
        value = answer()
        value["decision"]["standing_delta"]["if_reject"] = value["decision"]["standing_delta"]["if_accept"]
        with self.assertRaisesRegex(harness.ContractError, "rejection must preserve"):
            harness.validate_answer(value)
        value = answer()
        value["decision"]["verification_ids"].append(value["decision"]["verification_ids"][0])
        with self.assertRaisesRegex(harness.ContractError, "duplicate Verification"):
            harness.validate_answer(value)

    def test_answer_key_and_plan_are_rooted(self) -> None:
        key, plan = answer_key(), frozen_plan()
        harness.validate_answer_key(key)
        harness.validate_plan(plan)
        key["expected"]["next_work"]["target_id"] = "erdos:9999"
        with self.assertRaisesRegex(harness.ContractError, "answer_key_root"):
            harness.validate_answer_key(key)
        plan["limits"]["elapsed_ms"] += 1
        with self.assertRaisesRegex(harness.ContractError, "plan_root"):
            harness.validate_plan(plan)

    def test_answer_schema_is_recursively_closed(self) -> None:
        stack = [json.loads((Path(__file__).parent / "answer.schema.json").read_text())]
        while stack:
            node = stack.pop()
            if isinstance(node, dict):
                if node.get("type") == "object":
                    self.assertIs(node.get("additionalProperties"), False)
                stack.extend(node.values())
            elif isinstance(node, list):
                stack.extend(node)

    def test_secret_materials_fail_closed_under_system_temporary_roots(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(harness.ContractError, "system temporary root"):
                materialize.reject_system_temporary_output(Path(directory) / "materials")

    def test_offline_verifier_separates_eligibility_from_exactness(self) -> None:
        binding = harness.seal({
            "binding_root": "", "frontier": {"git_commit": "1" * 40},
        }, "binding_root")
        outcome = harness.harbor_verifier_outcome(answer(), answer_key(), binding, "1" * 40, "")
        self.assertTrue(outcome["eligible"])
        self.assertTrue(outcome["exact"])
        wrong = answer()
        wrong["next_work"]["target_id"] = "erdos:9999"
        outcome = harness.harbor_verifier_outcome(wrong, answer_key(), binding, "1" * 40, "")
        self.assertTrue(outcome["eligible"])
        self.assertFalse(outcome["exact"])

    def test_materializer_binds_current_target_and_decision_without_attempt(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root_dir = Path(directory)
            frontier = root_dir / "frontier"
            frontier.mkdir()
            vela = root_dir / "vela"
            vela.write_bytes(b"vela-binary")
            packet = frontier / "targets" / "erdos-1056.json"
            packet.parent.mkdir()
            packet.write_bytes(b"current-packet\n")
            packet_root = materialize.digest(packet)

            submission = {"submission_id": "vsb_0123456789abcdef"}
            submission_bytes = harness.canonical_bytes(submission)
            submission_root = harness.sha256_root(submission_bytes)
            submission_path = frontier / materialize.relative_content_path(submission_root, "submissions")
            submission_path.parent.mkdir(parents=True)
            submission_path.write_bytes(submission_bytes)

            claim = f"vcl_{'d' * 64}"
            proposal = "vpr_0123456789abcdef"
            next_work = {
                "frontier_id": "vfr_0123456789abcdef", "repository_root": root("1"),
                "target_index_root": root("2"),
                "targets": [{
                    "target_id": "erdos:1056", "packet": {"path": "targets/erdos-1056.json", "sha256": packet_root},
                    "next_command": "vela start erdos:1056 --frontier . --as agent:codex --json",
                }],
            }
            entry = {
                "proposal_id": proposal, "claim_id": claim,
                "inputs": {
                    "repository_root": root("1"), "proposal_root": root("a"),
                    "submission_root": submission_root, "verification_set_root": root("f"),
                },
                "readiness": {"protocol_gate": "satisfied", "human_decision_required": True},
                "staleness": {"state": "current"},
                "verification_records": [
                    {"verification_record_id": "vvr_0123456789abcdef"},
                    {"verification_record_id": "vvr_fedcba9876543210"},
                ],
                "standing_delta": answer()["decision"]["standing_delta"],
                "entry_root": root("c"),
            }
            inbox = {"projection_root": root("b"), "entries": [entry]}

            def fake_command(argv: tuple[str, ...], *, cwd: Path) -> str:
                if argv == ("git", "status", "--porcelain"):
                    return ""
                if argv == ("git", "rev-parse", "HEAD"):
                    return "1" * 40
                if argv == ("git", "rev-parse", "HEAD^{tree}"):
                    return "2" * 40
                if argv == ("git", "remote", "get-url", "origin"):
                    return "https://example.invalid/frontier.git"
                if argv == (str(vela.resolve()), "--version"):
                    return "vela 0.test"
                raise AssertionError(argv)

            def fake_json(argv: tuple[str, ...], *, cwd: Path) -> dict:
                return next_work if argv[1] == "next" else inbox

            with mock.patch.object(materialize, "command", side_effect=fake_command), mock.patch.object(materialize, "json_command", side_effect=fake_json):
                fixture, key = materialize.materialize(frontier, vela, proposal)
            harness.validate_answer_key(key)
            self.assertEqual(fixture["participant_files"], [])
            self.assertEqual(key["expected"]["next_work"]["packet_sha256"], packet_root)
            self.assertEqual(len(key["expected"]["decision"]["verification_ids"]), 2)
            self.assertNotIn("campaign", key["expected"])


if __name__ == "__main__":
    unittest.main()
