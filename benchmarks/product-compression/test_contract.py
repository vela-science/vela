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


def verification(record_id: str = "vvr_0123456789abcdef", role: str = "requirement_satisfying") -> dict:
    return {
        "verification_record_id": record_id,
        "verification_record_root": root("6" if role == "requirement_satisfying" else "7"),
        "outcome": "pass",
        "property": "Replay the exact retained artifact.",
        "verifier": "verifier:test-v1",
        "independent_of_producer": True,
        "protocol_evidence_role": role,
        "satisfies_requirements": ["Replay the exact retained artifact."] if role == "requirement_satisfying" else [],
        "does_not_establish": ["Scientific acceptance."],
    }


def next_obligation() -> dict:
    return {
        "now": "Human authority may inspect this exact rooted entry.",
        "if_accept": "Replay the successor and recompute Targets.",
        "if_reject": "Replay without accepted Standing change.",
    }


def answer(scenario: str = materialize.SCENARIOS[0]) -> dict:
    proposed = f"vcl_{'d' * 64}"
    target = f"vcl_{'e' * 64}"
    quantum = scenario == materialize.SCENARIOS[1]
    before = [{"claim_id": target, "claim_root": root("b")}] if quantum else []
    accepted = [{"claim_id": proposed, "claim_root": root("e")}]
    transition = "supersede accepted Claim with corrected Claim" if quantum else "add accepted Claim"
    scope_target = target if quantum else proposed
    affected = [proposed, target] if quantum else [proposed]
    unchanged = 4 if quantum else 14
    result = {
        "schema": "vela.product-compression-answer.v9",
        "scenario": scenario,
        "frontier": {
            "frontier_id": "vfr_0123456789abcdef",
            "repository_root": root("1"),
            "configured_targets": 0,
        },
        "decision": {
            "proposal_id": "vpr_0123456789abcdef",
            "proposal_root": root("a"),
            "source_submission_id": "vsb_0123456789abcdef",
            "source_submission_root": root("3"),
            "proposed_claim_id": proposed,
            "proposed_claim_root": root("e"),
            "requested_change": (
                {"kind": "supersede_claim", "target_claim_id": target, "target_claim_root": root("b")}
                if quantum else {"kind": "add_claim"}
            ),
            "assertion": "This exact bounded computation establishes one scoped result.",
            "conditions": ["The retained exact inputs."],
            "limits": ["This does not establish a broader result."],
            "verifications": [verification()],
            "verification_set_root": root("f"),
            "inbox_entry_root": root("c"),
            "protocol_gate": "satisfied",
            "blockers": [],
            "human_decision_required": True,
            "verification_is_acceptance": False,
            "standing_delta": {
                "transition": transition,
                "scope": {
                    "kind": "proposal_affected_claims",
                    "target_claim_id": scope_target,
                    "affected_claim_ids": affected,
                },
                "before": {"repository_root": root("1"), "accepted": before},
                "if_accept": {"repository_root": root("4"), "accepted": accepted},
                "if_reject": {"repository_root": root("5"), "accepted": before},
                "counts": {
                    "unchanged_accepted_claims": unchanged,
                    "global_accepted_claims": {
                        "before": unchanged + len(before),
                        "if_accept": unchanged + len(accepted),
                        "if_reject": unchanged + len(before),
                    },
                },
            },
            "staleness": "current",
            "next_obligation": next_obligation(),
        },
    }
    if quantum:
        result["decision"]["verifications"].append(
            verification("vvr_fedcba9876543210", "complementary")
        )
    return result


def fixture(
    scenario: str = materialize.SCENARIOS[0], commit: str = "1" * 40, tree: str = "2" * 40,
) -> dict:
    return contract.seal({
        "schema": "vela.product-compression-fixture.v6",
        "fixture_root": "",
        "scenario": scenario,
        "vela": {"version": "vela test", "binary_sha256": root("7")},
        "frontier": {
            "frontier_id": "vfr_0123456789abcdef",
            "git_commit": commit,
            "git_tree": tree,
            "repository_root": root("1"),
            "inbox_projection_root": root("2"),
            "configured_targets": 0,
        },
        "anchor": {"kind": "test"},
    }, "fixture_root")


def answer_key(source: dict | None = None) -> dict:
    source = source or fixture()
    return contract.seal({
        "schema": "vela.product-compression-answer-key.v9",
        "answer_key_root": "",
        "fixture_root": source["fixture_root"],
        "scenario": source["scenario"],
        "expected": answer(source["scenario"]),
    }, "answer_key_root")


def plan(scenario: str = materialize.SCENARIOS[0]) -> dict:
    source = fixture(scenario)
    key = answer_key(source)
    return contract.seal({
        "schema": "vela.product-compression-plan.v11",
        "plan_root": "",
        "scenario": scenario,
        "fixture_root": source["fixture_root"],
        "answer_key_root": key["answer_key_root"],
        "task_roots": [],
        "harbor_job_root": root("8"),
        "comparison_rule": materialize.COMPARISON,
        "claim_limit": "Bounded first-party benchmark evidence only.",
    }, "plan_root")


def write_job(directory: Path, scenario: str, exact_trials: set[tuple[str, int]]) -> None:
    total_cost = 0.0
    rows = [(arm, attempt) for attempt in range(1, 3) for arm in materialize.ARMS]
    for index, (arm, attempt) in enumerate(rows, start=1):
        trial = directory / f"native-{index}"
        cost = 0.25 if arm == "vela-guided" else 0.5
        total_cost += cost
        contract.write_json(trial / "result.json", {
            "id": f"trial-{index}",
            "task_name": f"vela/product-compression-{scenario}-{arm}",
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
    def test_harbor_verifier_creates_its_output_directory(self) -> None:
        test_script = (Path(__file__).parent / "task" / "tests" / "test.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn("mkdir -p /logs/verifier", test_script)

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

    def test_contract_rejects_invented_target(self) -> None:
        value = answer()
        value["frontier"]["configured_targets"] = 1
        with self.assertRaisesRegex(contract.ContractError, "no invented Target"):
            contract.validate_answer(value)

    def test_quantum_supersession_is_exact(self) -> None:
        value = answer(materialize.SCENARIOS[1])
        contract.validate_answer(value)
        value["decision"]["standing_delta"]["if_accept"] = value["decision"]["standing_delta"]["before"]
        with self.assertRaisesRegex(contract.ContractError, "replace exactly"):
            contract.validate_answer(value)

    def test_quantum_target_must_be_accepted_predecessor(self) -> None:
        value = answer(materialize.SCENARIOS[1])
        value["decision"]["requested_change"]["target_claim_id"] = f"vcl_{'a' * 64}"
        with self.assertRaisesRegex(contract.ContractError, "invalid supersession scope"):
            contract.validate_answer(value)

    def test_quantum_count_drift_fails(self) -> None:
        value = answer(materialize.SCENARIOS[1])
        value["decision"]["standing_delta"]["counts"]["global_accepted_claims"]["if_accept"] += 1
        with self.assertRaisesRegex(contract.ContractError, "counts"):
            contract.validate_answer(value)

    def test_answer_key_is_content_bound(self) -> None:
        key = answer_key()
        contract.validate_answer_key(key)
        key["expected"]["decision"]["proposal_root"] = root("4")
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

    def test_harbor_verifier_normalizes_scientific_sets_only(self) -> None:
        verifier = load_verifier()
        source = fixture(materialize.SCENARIOS[1])
        key = answer_key(source)
        exact = verifier.outcome(answer(source["scenario"]), key, source)
        self.assertEqual((exact["eligible"], exact["exact"]), (True, True))
        reordered = answer(source["scenario"])
        reordered["decision"]["verifications"].reverse()
        reordered["decision"]["verifications"][0]["does_not_establish"].reverse()
        order_independent = verifier.outcome(reordered, key, source)
        self.assertEqual((order_independent["eligible"], order_independent["exact"]), (True, True))
        omitted = answer(source["scenario"])
        omitted["decision"]["verifications"].pop()
        mismatch = verifier.outcome(omitted, key, source)
        self.assertEqual((mismatch["eligible"], mismatch["exact"]), (True, False))
        source["scenario"] = materialize.SCENARIOS[0]
        invalid = verifier.outcome(answer(materialize.SCENARIOS[1]), key, source)
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
            commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=frontier, check=True, capture_output=True, text=True).stdout.strip()
            tree = subprocess.run(["git", "rev-parse", "HEAD^{tree}"], cwd=frontier, check=True, capture_output=True, text=True).stdout.strip()

            source = fixture(materialize.SCENARIOS[1], commit, tree)
            key = answer_key(source)
            binary = base / "vela-linux"
            binary.write_bytes(b"\x7fELFtest")
            output = base / "study"
            prepared = materialize.build_study(
                source, key, "Inspect the quantum supersession.", "Bounded claim.",
                frontier, binary, "test-model", "0.1.0",
                "vela-product-compression-test", output,
            )
            self.assertEqual(prepared["schema"], "vela.product-compression-plan.v11")
            self.assertEqual(prepared["scenario"], materialize.SCENARIOS[1])
            self.assertEqual(len(prepared["task_roots"]), 2)
            baseline = output / "tasks/git-files"
            guided = output / "tasks/vela-guided"
            self.assertNotIn("{{", (baseline / "instruction.md").read_text())
            self.assertNotIn("COPY vela", (baseline / "environment/Dockerfile").read_text())
            self.assertIn("COPY vela", (guided / "environment/Dockerfile").read_text())
            self.assertIn(".frontier.git_commit", (guided / "environment/Dockerfile").read_text())
            self.assertIn("quantum-certificate-supersession", (guided / "task.toml").read_text())
            self.assertIn("@openai/codex@0.1.0", (baseline / "environment/Dockerfile").read_text())
            self.assertNotIn("python3", (baseline / "environment/Dockerfile").read_text())
            self.assertFalse(any(baseline.rglob("__pycache__")))
            resolved = subprocess.run(
                ["harbor", "run", "--config", str(output / "harbor-job.json"), "--print-config"],
                check=False, capture_output=True, text=True,
            )
            self.assertEqual(resolved.returncode, 0, resolved.stderr)
            self.assertEqual(json.loads(resolved.stdout)["n_attempts"], 2)

    def test_native_harbor_summary_records_bounded_lift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            plan_path = base / "plan.json"
            job = base / "job"
            scenario = materialize.SCENARIOS[1]
            contract.write_json(plan_path, plan(scenario))
            write_job(job, scenario, {("vela-guided", 1), ("vela-guided", 2)})
            result = summarize.summarize(plan_path, job)
            self.assertEqual(result["schema"], "vela.product-compression-native-harbor-result.v6")
            self.assertEqual(result["scenario"], scenario)
            self.assertEqual(result["conclusion"]["outcome"], "pass_task_specific_exactness_advantage")
            self.assertEqual(result["result_root"], contract.record_root(result, "result_root"))

    def test_native_harbor_summary_preserves_negative_result(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            plan_path = base / "plan.json"
            job = base / "job"
            scenario = materialize.SCENARIOS[1]
            contract.write_json(plan_path, plan(scenario))
            write_job(job, scenario, set())
            result = summarize.summarize(plan_path, job)
            self.assertEqual(result["conclusion"]["outcome"], "failed_no_product_lift_credit")
            self.assertEqual(result["result_root"], contract.record_root(result, "result_root"))

    def test_native_harbor_summary_rejects_mixed_scenarios(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            plan_path = base / "plan.json"
            job = base / "job"
            scenario = materialize.SCENARIOS[1]
            contract.write_json(plan_path, plan(scenario))
            write_job(job, scenario, set())
            first = next(job.glob("*/result.json"))
            value = contract.read_json(first)
            value["task_name"] = f"vela/product-compression-{materialize.SCENARIOS[0]}-git-files"
            contract.write_json(first, value)
            with self.assertRaisesRegex(contract.ContractError, "unknown task identity"):
                summarize.summarize(plan_path, job)


if __name__ == "__main__":
    unittest.main()
