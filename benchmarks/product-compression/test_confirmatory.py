#!/usr/bin/env python3
"""Focused tests for the confirmatory product-compression analysis."""

from __future__ import annotations

import unittest

import confirmatory


def sha(char: str) -> str:
    return f"sha256:{char * 64}"


def plan() -> dict:
    blocks = []
    for index in range(40):
        family = confirmatory.FAMILIES[index // 20]
        block_id = f"pcb_{index:03d}"
        blocks.append(
            {
                "block_id": block_id,
                "family": family,
                "instance_root": f"sha256:{index + 1:064x}",
                "fixture_root": f"sha256:{index + 100:064x}",
                "answer_key_root": f"sha256:{index + 200:064x}",
                "tasks": {
                    arm: {
                        "task_name": f"vela/{block_id}-{arm}",
                        "task_root": f"sha256:{index * 2 + arm_index + 300:064x}",
                    }
                    for arm_index, arm in enumerate(confirmatory.ARMS)
                },
                "arm_order": list(
                    confirmatory.ARMS if index % 2 else reversed(confirmatory.ARMS)
                ),
                "execution_wave": index // 4,
            }
        )
    value = {
        "schema": confirmatory.PLAN_SCHEMA,
        "plan_root": "",
        "study_id": "vela-confirmatory-test",
        "claim_limit": "Model-specific first-party task evidence only.",
        "harbor_job_root": sha("f"),
        "execution": {
            "harbor_version": "0.20.0",
            "agent": "codex",
            "agent_version": "0.145.0",
            "model": "gpt-5.6-terra",
            "attempts_per_task": 1,
            "deadline_ms": 900_000,
            "retry_policy": "pre_model_infrastructure_once",
            "canonical_checkout_mutable": False,
            "authority_credentials_available": False,
            "answer_key_available": False,
        },
        "endpoint": {
            "id": "restricted-log-time-to-exact-v1",
            "restriction_ms": 900_000,
            "exact_completion_value": "agent_execution_elapsed_ms",
            "nonexact_value": 900_000,
            "transform": "natural_log",
            "contrast": "vela-guided-minus-git-files",
        },
        "exactness_gate": {
            "margin": -0.10,
            "confidence": 0.95,
            "method": "bonferroni-clopper-pearson-marginals",
            "all_trials_eligible": True,
            "maximum_guided_authority_errors": 0,
        },
        "analysis": {
            "alpha_two_sided": 0.05,
            "family_weights": "equal",
            "paired_by": "block_id",
            "minimum_useful_ratio": 0.80,
            "superiority_rule": "upper_95_ratio_below_1",
            "useful_effect_rule": "point_ratio_at_most_0.8",
            "strong_20_percent_rule": "upper_95_ratio_at_most_0.8",
            "consistency_rule": "each_family_point_ratio_below_1",
        },
        "sample_size": {
            "initial_blocks": 40,
            "initial_blocks_per_family": 20,
            "minimum_blocks": 40,
            "maximum_blocks": 120,
            "blinded_reestimation": True,
            "may_decrease": False,
        },
        "blocks": blocks,
    }
    value["randomization"] = {
        "algorithm": "sha256-within-block-order-v1",
        "seed": "test-seed",
        "assignment_root": confirmatory.root(
            confirmatory.canonical_bytes(
                [
                    {
                        "block_id": block["block_id"],
                        "arm_order": block["arm_order"],
                        "execution_wave": block["execution_wave"],
                    }
                    for block in blocks
                ]
            )
        ),
    }
    value["plan_root"] = confirmatory.record_root(value, "plan_root")
    return value


def trials(
    *, guided_failures: set[int] | None = None, authority_error: int | None = None
) -> dict:
    guided_failures = guided_failures or set()
    result = []
    for index in range(40):
        family = confirmatory.FAMILIES[index // 20]
        for arm_index, arm in enumerate(confirmatory.ARMS):
            guided = arm == "vela-guided"
            exact = not (guided and index in guided_failures)
            result.append(
                {
                    "block_id": f"pcb_{index:03d}",
                    "family": family,
                    "arm": arm,
                    "eligible": True,
                    "exact": exact,
                    "authority_error": guided and index == authority_error,
                    "agent_execution_elapsed_ms": 70_000 + index * 97
                    if guided
                    else 100_000 + index * 113,
                    "retry_after_model_output": False,
                    "harbor_trial_id": f"trial-{index:03d}-{arm}",
                    "trial_result_sha256": f"sha256:{10_000 + index * 2 + arm_index:064x}",
                    "answer_root": f"sha256:{20_000 + index * 2 + arm_index:064x}"
                    if exact
                    else None,
                    "cost_usd": 0.2 if guided else 0.4,
                    "input_tokens": 1_000,
                    "output_tokens": 200,
                    "tool_calls": 3,
                }
            )
    value = {
        "schema": confirmatory.TRIAL_EXPORT_SCHEMA,
        "export_root": "",
        "plan_root": plan()["plan_root"],
        "harbor_job": {"id": "harbor-confirmatory-test", "result_sha256": sha("e")},
        "trials": result,
    }
    value["export_root"] = confirmatory.record_root(value, "export_root")
    return value


class ConfirmatoryTests(unittest.TestCase):
    def test_pilot_variance_reproduces_registered_size(self) -> None:
        self.assertEqual(
            confirmatory.required_blocks_for_effect(0.3469300471612205), 40
        )

    def test_exactness_bound_matches_registered_values(self) -> None:
        lower_40 = confirmatory.clopper_pearson_lower(40, 40)
        self.assertAlmostEqual(lower_40 - 1, -0.0880973029, places=9)
        lower_39 = confirmatory.clopper_pearson_lower(39, 40)
        self.assertLess(lower_39 - 1, -0.10)

    def test_exact_two_family_study_can_confirm(self) -> None:
        result = confirmatory.analyze(plan(), trials())
        self.assertTrue(result["exactness"]["passed"])
        self.assertEqual(
            result["conclusion"]["outcome"], "confirmatory_at_least_20_percent"
        )
        self.assertTrue(result["conclusion"]["claim_credit"])
        self.assertEqual(
            result["result_root"], confirmatory.record_root(result, "result_root")
        )

    def test_fast_wrong_answer_receives_deadline_and_fails_exactness(self) -> None:
        rows = trials(guided_failures={0})
        row = next(
            item
            for item in rows["trials"]
            if item["block_id"] == "pcb_000" and item["arm"] == "vela-guided"
        )
        row["agent_execution_elapsed_ms"] = 1
        rows["export_root"] = confirmatory.record_root(rows, "export_root")
        result = confirmatory.analyze(plan(), rows)
        observed = next(
            item
            for item in result["trials"]
            if item["block_id"] == "pcb_000" and item["arm"] == "vela-guided"
        )
        self.assertEqual(observed["restricted_time_ms"], 900_000)
        self.assertEqual(
            result["conclusion"]["outcome"], "failed_exactness_noninferiority"
        )

    def test_authority_error_fails_before_efficiency(self) -> None:
        result = confirmatory.analyze(plan(), trials(authority_error=3))
        self.assertEqual(result["conclusion"]["outcome"], "failed_integrity")
        self.assertFalse(result["conclusion"]["claim_credit"])

    def test_duplicate_instance_fails(self) -> None:
        value = plan()
        value["blocks"][1]["instance_root"] = value["blocks"][0]["instance_root"]
        value["plan_root"] = confirmatory.record_root(value, "plan_root")
        with self.assertRaisesRegex(
            confirmatory.ContractError, "duplicate instance_root"
        ):
            confirmatory.validate_plan(value)

    def test_undersized_balanced_plan_cannot_earn_claim_credit(self) -> None:
        value = plan()
        value["blocks"] = value["blocks"][:10] + value["blocks"][20:30]
        value["randomization"]["assignment_root"] = confirmatory.root(
            confirmatory.canonical_bytes(
                [
                    {
                        "block_id": block["block_id"],
                        "arm_order": block["arm_order"],
                        "execution_wave": block["execution_wave"],
                    }
                    for block in value["blocks"]
                ]
            )
        )
        value["plan_root"] = confirmatory.record_root(value, "plan_root")
        with self.assertRaisesRegex(
            confirmatory.ContractError,
            "block count does not match the registered initial sample",
        ):
            confirmatory.validate_plan(value)

    def test_paired_table_keeps_neither_exact_distinct(self) -> None:
        rows = [
            {"block_id": "one", "arm": "git-files", "exact": False},
            {"block_id": "one", "arm": "vela-guided", "exact": False},
        ]
        self.assertEqual(confirmatory.paired_table(rows)["neither_exact"], 1)

    def test_reestimation_never_decreases_and_stops_when_infeasible(self) -> None:
        self.assertEqual(confirmatory.reestimated_blocks(0.2), (40, "continue"))
        required, action = confirmatory.reestimated_blocks(0.65)
        self.assertGreater(required, 120)
        self.assertEqual(action, "precision_infeasible")

    def test_family_reversal_blocks_pooled_credit(self) -> None:
        rows = trials()
        for row in rows["trials"]:
            if row["family"] == "cross_frontier_inheritance":
                row["agent_execution_elapsed_ms"] = (
                    150_000 if row["arm"] == "vela-guided" else 100_000
                )
        rows["export_root"] = confirmatory.record_root(rows, "export_root")
        result = confirmatory.analyze(plan(), rows)
        self.assertFalse(result["primary"]["family_consistency_passed"])
        self.assertEqual(result["conclusion"]["outcome"], "failed_family_inconsistency")


if __name__ == "__main__":
    unittest.main()
