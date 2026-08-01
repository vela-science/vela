#!/usr/bin/env python3
"""Focused tests for Vela's small layer over native Harbor tasks."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

import contract
import prepare
import summarize


def root(char: str) -> str:
    return f"sha256:{char * 64}"


def answer() -> dict:
    claim = f"vcl_{'d' * 64}"
    return {
        "schema": "vela.product-compression-answer.v5",
        "frontier": {"frontier_id": "vfr_0123456789abcdef", "repository_root": root("1")},
        "next_work": {"target_id": "erdos:1056", "target_index_root": root("2"), "packet_sha256": root("3")},
        "decision": {
            "proposal_id": "vpr_0123456789abcdef",
            "proposal_root": root("a"),
            "source_submission_id": "vsb_0123456789abcdef",
            "proposed_claim_id": claim,
            "verification_ids": ["vvr_0123456789abcdef"],
            "verification_set_root": root("f"),
            "inbox_entry_root": root("c"),
            "protocol_gate": "satisfied",
            "human_decision_required": True,
            "verification_is_acceptance": False,
            "standing_delta": {
                "transition": "add accepted Claim",
                "scope": {"kind": "proposal_affected_claims", "target_claim_id": claim, "affected_claim_ids": [claim]},
                "before": {"repository_root": root("1"), "accepted": []},
                "if_accept": {"repository_root": root("4"), "accepted": [{"claim_id": claim, "claim_root": root("e")}]},
                "if_reject": {"repository_root": root("5"), "accepted": []},
            },
            "staleness": "current",
            "next_if_accept_code": "replay_and_recompute_targets",
            "next_if_reject_code": "replay_without_standing_change",
        },
        "safety": {"authority_action_performed": False, "accepted_state_changed": False},
    }


def answer_key() -> dict:
    return contract.seal({
        "schema": "vela.product-compression-answer-key.v5",
        "answer_key_root": "",
        "fixture_root": root("9"),
        "expected": answer(),
    }, "answer_key_root")


def plan() -> dict:
    return contract.seal({
        "schema": "vela.product-compression-plan.v7",
        "plan_root": "",
        "fixture_root": root("9"),
        "answer_key_root": answer_key()["answer_key_root"],
        "harbor": {"version": "0.20.0"},
        "agent": {"name": "codex", "model": "test", "version": "0.1.0"},
        "vela": {"version": "vela test", "linux_sha256": root("8")},
        "sessions": list(prepare.SESSIONS),
        "tasks": [],
        "comparison_rule": prepare.COMPARISON,
        "claim_limit": "First-party evidence from one frozen task; no independent-user or general scientific-workflow claim.",
    }, "plan_root")


def write_job(directory: Path, exact_sessions: set[str]) -> None:
    total_cost = 0.0
    for index, session in enumerate(prepare.SESSIONS, start=1):
        trial = directory / f"{index:02d}-{session}__trial"
        cost = 0.25 if session.startswith("vela-guided") else 0.5
        total_cost += cost
        contract.write_json(trial / "result.json", {
            "id": f"trial-{index}",
            "finished_at": "2026-07-31T12:00:05Z",
            "exception_info": None,
            "agent_execution": {
                "started_at": "2026-07-31T12:00:00Z",
                "finished_at": "2026-07-31T12:00:02Z" if session.startswith("vela-guided") else "2026-07-31T12:00:04Z",
            },
            "agent_result": {"cost_usd": cost},
            "verifier_result": {"rewards": {"eligible": 1, "exact": int(session in exact_sessions)}},
        })
    contract.write_json(directory / "result.json", {
        "id": "product-compression-test",
        "finished_at": "2026-07-31T12:00:06Z",
        "n_total_trials": 4,
        "stats": {
            "n_completed_trials": 4,
            "n_errored_trials": 0,
            "n_running_trials": 0,
            "n_pending_trials": 0,
            "n_cancelled_trials": 0,
            "n_retries": 0,
            "cost_usd": total_cost,
        },
    })


def load_verifier():
    path = Path(__file__).parent / "task" / "tests" / "verify.py"
    spec = importlib.util.spec_from_file_location("product_compression_verify", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ProductCompressionTests(unittest.TestCase):
    def test_contract_preserves_authority_boundary(self) -> None:
        contract.validate_answer(answer())
        value = answer()
        value["decision"]["verification_is_acceptance"] = True
        with self.assertRaisesRegex(contract.ContractError, "authority boundary"):
            contract.validate_answer(value)
        value = answer()
        value["safety"]["authority_action_performed"] = True
        with self.assertRaisesRegex(contract.ContractError, "read-only"):
            contract.validate_answer(value)

    def test_contract_rejects_false_standing_delta(self) -> None:
        value = answer()
        value["decision"]["standing_delta"]["if_reject"] = value["decision"]["standing_delta"]["if_accept"]
        with self.assertRaisesRegex(contract.ContractError, "rejection must preserve"):
            contract.validate_answer(value)

    def test_answer_key_is_content_bound(self) -> None:
        key = answer_key()
        contract.validate_answer_key(key)
        key["expected"]["next_work"]["target_id"] = "erdos:9999"
        with self.assertRaisesRegex(contract.ContractError, "root mismatch"):
            contract.validate_answer_key(key)

    def test_published_schema_is_recursively_closed(self) -> None:
        stack = [json.loads((Path(__file__).parent / "answer.schema.json").read_text())]
        while stack:
            node = stack.pop()
            if isinstance(node, dict):
                if node.get("type") == "object":
                    self.assertIs(node.get("additionalProperties"), False)
                stack.extend(node.values())
            elif isinstance(node, list):
                stack.extend(node)

    def test_harbor_verifier_separates_eligibility_and_exactness(self) -> None:
        verifier = load_verifier()
        binding = contract.seal({
            "binding_root": "",
            "frontier": {"git_commit": "1" * 40},
        }, "binding_root")
        exact = verifier.outcome(answer(), answer_key(), binding, "1" * 40, "")
        self.assertEqual((exact["eligible"], exact["exact"]), (True, True))
        wrong = answer()
        wrong["next_work"]["target_id"] = "erdos:9999"
        mismatch = verifier.outcome(wrong, answer_key(), binding, "1" * 40, "")
        self.assertEqual((mismatch["eligible"], mismatch["exact"]), (True, False))

    def test_prepare_materializes_native_harbor_tasks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            frontier = base / "frontier"
            frontier.mkdir()
            subprocess.run(["git", "init", "-q"], cwd=frontier, check=True)
            subprocess.run(["git", "config", "user.name", "Test"], cwd=frontier, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.invalid"], cwd=frontier, check=True)
            (frontier / "README.md").write_text("fixture\n")
            subprocess.run(["git", "add", "README.md"], cwd=frontier, check=True)
            subprocess.run(["git", "commit", "-qm", "fixture"], cwd=frontier, check=True)
            commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=frontier, check=True, capture_output=True, text=True).stdout.strip()
            tree = subprocess.run(["git", "rev-parse", "HEAD^{tree}"], cwd=frontier, check=True, capture_output=True, text=True).stdout.strip()

            materials = base / "materials"
            key = answer_key()
            fixture = contract.seal({
                "schema": "vela.product-compression-fixture.v3",
                "fixture_root": "",
                "vela": {"version": "vela test", "binary_sha256": root("7")},
                "frontier": {
                    "frontier_id": "vfr_0123456789abcdef",
                    "remote": "test",
                    "git_commit": commit,
                    "git_tree": tree,
                    "repository_root": root("1"),
                    "target_index_root": root("2"),
                },
                "task": {"proposal_id": "vpr_0123456789abcdef"},
                "participant_files": [],
            }, "fixture_root")
            key["fixture_root"] = fixture["fixture_root"]
            contract.seal(key, "answer_key_root")
            contract.write_json(materials / "fixture.json", fixture)
            contract.write_json(materials / "answer-key.json", key)
            binary = base / "vela-linux"
            binary.write_bytes(b"\x7fELFtest")
            output = base / "study"
            prepared = prepare.prepare(
                materials, frontier, binary, "test-model", "0.1.0", "vela test",
                "vela-product-compression-test", output,
            )
            self.assertEqual(prepared["schema"], "vela.product-compression-plan.v7")
            self.assertEqual(len(prepared["tasks"]), 4)
            self.assertNotIn("{{", (output / "tasks/git-files-01/instruction.md").read_text())
            self.assertNotIn("COPY vela", (output / "tasks/git-files-01/environment/Dockerfile").read_text())
            self.assertIn("COPY vela", (output / "tasks/vela-guided-01/environment/Dockerfile").read_text())

    def test_native_harbor_summary_records_bounded_lift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            plan_path = base / "plan.json"
            job = base / "job"
            contract.write_json(plan_path, plan())
            write_job(job, {"vela-guided-01", "vela-guided-02"})
            result = summarize.summarize(plan_path, job)
            self.assertEqual(result["conclusion"]["outcome"], "pass_task_specific_exactness_advantage")
            self.assertEqual(result["comparison"]["arms"]["vela-guided"]["exact"], 2)
            self.assertEqual(result["result_root"], contract.record_root(result, "result_root"))

    def test_native_harbor_summary_preserves_negative_result(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            plan_path = base / "plan.json"
            job = base / "job"
            contract.write_json(plan_path, plan())
            write_job(job, {"vela-guided-01"})
            result = summarize.summarize(plan_path, job)
            self.assertEqual(result["conclusion"]["outcome"], "failed_no_product_lift_credit")


if __name__ == "__main__":
    unittest.main()
