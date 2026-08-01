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
import materialize
import summarize


def root(char: str) -> str:
    return f"sha256:{char * 64}"


def answer() -> dict:
    claim = f"vcl_{'d' * 64}"
    return {
        "schema": "vela.product-compression-answer.v6",
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
    }


def answer_key(fixture_root: str = root("9")) -> dict:
    return contract.seal({
        "schema": "vela.product-compression-answer-key.v6",
        "answer_key_root": "",
        "fixture_root": fixture_root,
        "expected": answer(),
    }, "answer_key_root")


def fixture(commit: str = "1" * 40, tree: str = "2" * 40) -> dict:
    return contract.seal({
        "schema": "vela.product-compression-fixture.v4",
        "fixture_root": "",
        "vela": {"version": "vela test", "binary_sha256": root("7")},
        "frontier": {
            "frontier_id": "vfr_0123456789abcdef",
            "git_commit": commit,
            "git_tree": tree,
            "repository_root": root("1"),
            "target_index_root": root("2"),
        },
        "task": {"proposal_id": "vpr_0123456789abcdef"},
    }, "fixture_root")


def plan() -> dict:
    return contract.seal({
        "schema": "vela.product-compression-plan.v8",
        "plan_root": "",
        "fixture_root": root("9"),
        "answer_key_root": answer_key()["answer_key_root"],
        "task_roots": [],
        "harbor_job_root": root("8"),
        "comparison_rule": materialize.COMPARISON,
        "claim_limit": "First-party evidence from one frozen task; no independent-user or general scientific-workflow claim.",
    }, "plan_root")


def write_job(directory: Path, exact_trials: set[tuple[str, int]]) -> None:
    total_cost = 0.0
    rows = [(arm, attempt) for attempt in range(1, 3) for arm in materialize.ARMS]
    for index, (arm, attempt) in enumerate(rows, start=1):
        trial = directory / f"native-{index}"
        cost = 0.25 if arm == "vela-guided" else 0.5
        total_cost += cost
        contract.write_json(trial / "result.json", {
            "id": f"trial-{index}",
            "task_name": f"vela/product-compression-{arm}",
            "finished_at": "2026-07-31T12:00:05Z",
            "exception_info": None,
            "agent_execution": {
                "started_at": "2026-07-31T12:00:00Z",
                "finished_at": "2026-07-31T12:00:02Z" if arm == "vela-guided" else "2026-07-31T12:00:04Z",
            },
            "agent_result": {"cost_usd": cost},
            "verifier_result": {"rewards": {"eligible": 1, "exact": int((arm, attempt) in exact_trials)}},
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
        source = fixture()
        key = answer_key(source["fixture_root"])
        exact = verifier.outcome(answer(), key, source)
        self.assertEqual((exact["eligible"], exact["exact"]), (True, True))
        wrong = answer()
        wrong["next_work"]["target_id"] = "erdos:9999"
        mismatch = verifier.outcome(wrong, key, source)
        self.assertEqual((mismatch["eligible"], mismatch["exact"]), (True, False))
        source["frontier"]["git_tree"] = "3" * 40
        invalid = verifier.outcome(answer(), key, source)
        self.assertFalse(invalid["eligible"])
        self.assertIn("fixture_invalid", invalid["eligibility_failure_codes"])

    def test_materializer_builds_native_harbor_study(self) -> None:
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
            commit = subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=frontier, check=True,
                capture_output=True, text=True,
            ).stdout.strip()
            tree = subprocess.run(
                ["git", "rev-parse", "HEAD^{tree}"], cwd=frontier, check=True,
                capture_output=True, text=True,
            ).stdout.strip()

            source = fixture(commit, tree)
            key = answer_key(source["fixture_root"])
            binary = base / "vela-linux"
            binary.write_bytes(b"\x7fELFtest")
            output = base / "study"
            prepared = materialize.build_study(
                source, key, frontier, binary, "test-model", "0.1.0",
                "vela-product-compression-test", output,
            )
            self.assertEqual(prepared["schema"], "vela.product-compression-plan.v8")
            self.assertEqual(len(prepared["task_roots"]), 2)
            baseline = output / "tasks/git-files"
            guided = output / "tasks/vela-guided"
            self.assertNotIn("{{", (baseline / "instruction.md").read_text())
            self.assertNotIn("COPY vela", (baseline / "environment/Dockerfile").read_text())
            self.assertIn("COPY vela", (guided / "environment/Dockerfile").read_text())
            self.assertEqual(contract.read_json(output / "fixture.json"), source)
            self.assertEqual(contract.read_json(output / "answer-key.json"), key)
            self.assertEqual(contract.read_json(output / "harbor-job.json")["n_attempts"], 2)
            self.assertEqual(
                {path.name for path in (baseline / "environment").iterdir()},
                {"Dockerfile", "answer.schema.json", "fixture.json", "frontier.bundle"},
            )
            resolved = subprocess.run(
                [
                    "harbor", "run", "--config",
                    str(output / "harbor-job.json"), "--print-config",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(resolved.returncode, 0, resolved.stderr)
            self.assertEqual(json.loads(resolved.stdout)["n_attempts"], 2)
            self.assertIn(
                'environment_mode = "separate"',
                (guided / "task.toml").read_text(),
            )

    def test_native_harbor_summary_records_bounded_lift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            plan_path = base / "plan.json"
            job = base / "job"
            contract.write_json(plan_path, plan())
            write_job(job, {("vela-guided", 1), ("vela-guided", 2)})
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
            write_job(job, {("vela-guided", 1)})
            result = summarize.summarize(plan_path, job)
            self.assertEqual(result["conclusion"]["outcome"], "failed_no_product_lift_credit")


if __name__ == "__main__":
    unittest.main()
