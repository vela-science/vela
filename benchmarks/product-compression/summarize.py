#!/usr/bin/env python3
"""Apply the registered Vela comparison to one native Harbor job."""

from __future__ import annotations

import argparse
import sys
from datetime import datetime
from pathlib import Path
from statistics import median
from typing import Any, Sequence

import contract
import materialize


def elapsed_ms(start: Any, finish: Any) -> int:
    if not isinstance(start, str) or not isinstance(finish, str):
        raise contract.ContractError("Harbor trial is missing agent timing")
    try:
        delta = datetime.fromisoformat(finish.replace("Z", "+00:00")) - datetime.fromisoformat(start.replace("Z", "+00:00"))
    except ValueError as exc:
        raise contract.ContractError("Harbor trial has invalid agent timing") from exc
    result = int(delta.total_seconds() * 1_000)
    if result < 0:
        raise contract.ContractError("Harbor trial has negative elapsed time")
    return result


def trial(result_path: Path) -> dict[str, Any]:
    result = contract.read_json(result_path)
    task_name = result.get("task_name")
    arms = [arm for arm in materialize.ARMS if task_name == f"vela/product-compression-{arm}"]
    if len(arms) != 1:
        raise contract.ContractError(f"Harbor trial has an unknown task identity: {task_name}")
    if result.get("finished_at") is None or result.get("exception_info") is not None:
        raise contract.ContractError(f"Harbor trial {result.get('id')} did not finish cleanly")
    rewards = (result.get("verifier_result") or {}).get("rewards")
    execution = result.get("agent_execution")
    agent = result.get("agent_result")
    if not isinstance(rewards, dict) or rewards.get("eligible") not in {0, 1} or rewards.get("exact") not in {0, 1}:
        raise contract.ContractError(f"Harbor trial {result.get('id')} has invalid native rewards")
    if not isinstance(execution, dict) or not isinstance(agent, dict) or not isinstance(agent.get("cost_usd"), (int, float)):
        raise contract.ContractError(f"Harbor trial {result.get('id')} is missing native metrics")
    return {
        "arm": arms[0],
        "trial_id": result.get("id"),
        "trial_result_sha256": contract.sha256_root(result_path.read_bytes()),
        "eligible": bool(rewards["eligible"]),
        "exact": bool(rewards["exact"]),
        "agent_elapsed_ms": elapsed_ms(execution.get("started_at"), execution.get("finished_at")),
        "cost_usd": agent["cost_usd"],
    }


def summarize(plan_path: Path, job: Path) -> dict[str, Any]:
    plan = contract.read_json(plan_path)
    if plan.get("schema") != "vela.product-compression-plan.v10" or plan.get("plan_root") != contract.record_root(plan, "plan_root"):
        raise contract.ContractError("invalid product-compression plan")
    if plan.get("comparison_rule") != materialize.COMPARISON:
        raise contract.ContractError("unsupported comparison rule")
    job_result_path = job / "result.json"
    job_result = contract.read_json(job_result_path)
    stats = job_result.get("stats") if isinstance(job_result, dict) else None
    if not isinstance(stats, dict) or job_result.get("finished_at") is None:
        raise contract.ContractError("Harbor job is not terminal")
    if job_result.get("n_total_trials") != 4 or any(stats.get(field) != 0 for field in (
        "n_running_trials", "n_pending_trials", "n_errored_trials", "n_cancelled_trials", "n_retries",
    )):
        raise contract.ContractError("Harbor job must contain four clean, terminal, unretried trials")
    trials = [trial(path) for path in sorted(job.glob("*/result.json"))]
    if len(trials) != 4:
        raise contract.ContractError("Harbor job must contain four trial results")
    arms = {}
    for arm in materialize.ARMS:
        selected = [item for item in trials if item["arm"] == arm]
        if len(selected) != 2:
            raise contract.ContractError(f"Harbor job must contain two native attempts for {arm}")
        arms[arm] = {
            "eligible": sum(item["eligible"] for item in selected),
            "exact": sum(item["eligible"] and item["exact"] for item in selected),
            "median_agent_elapsed_ms": median(item["agent_elapsed_ms"] for item in selected),
            "median_cost_usd": median(item["cost_usd"] for item in selected),
        }
    baseline, guided = arms["git-files"], arms["vela-guided"]
    improvement = round((baseline["median_agent_elapsed_ms"] - guided["median_agent_elapsed_ms"]) * 10_000 / baseline["median_agent_elapsed_ms"])
    all_eligible = all(item["eligible"] for item in trials)
    cost_ok = guided["median_cost_usd"] <= baseline["median_cost_usd"]
    if all_eligible and guided["exact"] == 2 and guided["exact"] > baseline["exact"] and cost_ok:
        outcome = "pass_task_specific_exactness_advantage"
    elif all_eligible and baseline["exact"] == guided["exact"] == 2 and cost_ok and improvement >= 2_000:
        outcome = "pass_efficiency_when_exactness_tied"
    else:
        outcome = "failed_no_product_lift_credit"
    return contract.seal({
        "schema": "vela.product-compression-native-harbor-result.v5",
        "result_root": "",
        "plan_root": plan["plan_root"],
        "harbor_job": {
            "id": job_result.get("id"),
            "result_sha256": contract.sha256_root(job_result_path.read_bytes()),
        },
        "trials": trials,
        "comparison": {"arms": arms, "elapsed_improvement_basis_points": improvement},
        "conclusion": {"outcome": outcome, "claim_limit": plan["claim_limit"]},
    }, "result_root")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--plan", type=Path, required=True)
    result.add_argument("--job", type=Path, required=True)
    result.add_argument("--output", type=Path, required=True)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        result = summarize(args.plan, args.job)
        contract.write_json(args.output, result)
        sys.stdout.buffer.write(contract.canonical_bytes({
            "ok": True,
            "result_root": result["result_root"],
            "outcome": result["conclusion"]["outcome"],
        }))
        return 0
    except contract.ContractError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
