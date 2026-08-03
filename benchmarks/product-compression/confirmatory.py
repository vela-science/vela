#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = [
#   "scipy==1.18.0",
# ]
# ///
"""Validate and analyze one preregistered paired confirmatory study.

Harbor owns execution and raw results. This script consumes only a rooted plan
and normalized trial rows exported from those results. It never invokes an
agent, retries a task, or changes a Frontier.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from collections import Counter, defaultdict
from pathlib import Path
from statistics import fmean, variance
from typing import Any, Sequence

from scipy import stats


ROOT_PREFIX = "sha256:"
PLAN_SCHEMA = "vela.product-compression-confirmatory-plan.v1"
RESULT_SCHEMA = "vela.product-compression-confirmatory-result.v1"
TRIAL_EXPORT_SCHEMA = "vela.product-compression-confirmatory-trials.v1"
ARMS = ("git-files", "vela-guided")
FAMILIES = ("target_continuation", "cross_frontier_inheritance")


class ContractError(ValueError):
    """Confirmatory evidence violates the preregistered contract."""


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def root(value: bytes) -> str:
    return ROOT_PREFIX + hashlib.sha256(value).hexdigest()


def record_root(value: dict[str, Any], field: str) -> str:
    return root(
        canonical_bytes({key: item for key, item in value.items() if key != field})
    )


def require_root(value: Any, location: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 71
        or not value.startswith(ROOT_PREFIX)
    ):
        raise ContractError(f"{location}: expected sha256 root")
    try:
        bytes.fromhex(value.removeprefix(ROOT_PREFIX))
    except ValueError as exc:
        raise ContractError(f"{location}: expected sha256 root") from exc
    return value


def validate_plan(plan: Any) -> dict[str, Any]:
    if not isinstance(plan, dict) or plan.get("schema") != PLAN_SCHEMA:
        raise ContractError("plan has the wrong schema")
    if plan.get("plan_root") != record_root(plan, "plan_root"):
        raise ContractError("plan root mismatch")
    execution = plan.get("execution")
    endpoint = plan.get("endpoint")
    analysis = plan.get("analysis")
    exactness = plan.get("exactness_gate")
    sample = plan.get("sample_size")
    randomization = plan.get("randomization")
    if not all(
        isinstance(item, dict)
        for item in (execution, endpoint, analysis, exactness, sample, randomization)
    ):
        raise ContractError("plan is missing its registered methods")
    require_root(plan.get("harbor_job_root"), "plan.harbor_job_root")
    if execution != {
        "harbor_version": "0.20.0",
        "agent": "codex",
        "agent_version": execution.get("agent_version"),
        "model": execution.get("model"),
        "attempts_per_task": 1,
        "deadline_ms": 900_000,
        "retry_policy": "pre_model_infrastructure_once",
        "canonical_checkout_mutable": False,
        "authority_credentials_available": False,
        "answer_key_available": False,
    } or not all(
        isinstance(execution.get(field), str) and execution[field]
        for field in ("agent_version", "model")
    ):
        raise ContractError("execution contract drift")
    if endpoint != {
        "id": "restricted-log-time-to-exact-v1",
        "restriction_ms": 900_000,
        "exact_completion_value": "agent_execution_elapsed_ms",
        "nonexact_value": 900_000,
        "transform": "natural_log",
        "contrast": "vela-guided-minus-git-files",
    }:
        raise ContractError("endpoint contract drift")
    if exactness != {
        "margin": -0.10,
        "confidence": 0.95,
        "method": "bonferroni-clopper-pearson-marginals",
        "all_trials_eligible": True,
        "maximum_guided_authority_errors": 0,
    }:
        raise ContractError("exactness gate drift")
    expected_analysis = {
        "alpha_two_sided": 0.05,
        "family_weights": "equal",
        "paired_by": "block_id",
        "minimum_useful_ratio": 0.80,
        "superiority_rule": "upper_95_ratio_below_1",
        "useful_effect_rule": "point_ratio_at_most_0.8",
        "strong_20_percent_rule": "upper_95_ratio_at_most_0.8",
        "consistency_rule": "each_family_point_ratio_below_1",
    }
    if analysis != expected_analysis:
        raise ContractError("analysis contract drift")
    if (
        sample.get("initial_blocks") != 40
        or sample.get("initial_blocks_per_family") != 20
    ):
        raise ContractError("confirmatory study requires 40 balanced blocks")
    if sample.get("minimum_blocks") != 40 or sample.get("maximum_blocks") != 120:
        raise ContractError("sample-size bounds drift")
    if (
        sample.get("blinded_reestimation") is not True
        or sample.get("may_decrease") is not False
    ):
        raise ContractError(
            "sample-size reestimation must remain blinded and nondecreasing"
        )
    if randomization.get("algorithm") != "sha256-within-block-order-v1":
        raise ContractError("randomization algorithm drift")
    if not isinstance(randomization.get("seed"), str) or not randomization["seed"]:
        raise ContractError("randomization seed is required")
    require_root(
        randomization.get("assignment_root"), "plan.randomization.assignment_root"
    )

    blocks = plan.get("blocks")
    if not isinstance(blocks, list) or not blocks:
        raise ContractError("plan has no frozen blocks")
    if len(blocks) != sample["initial_blocks"]:
        raise ContractError(
            "plan block count does not match the registered initial sample"
        )
    seen: dict[str, set[str]] = defaultdict(set)
    family_counts = Counter()
    first_arm_counts: dict[str, Counter[str]] = defaultdict(Counter)
    assignments = []
    for block in blocks:
        if not isinstance(block, dict):
            raise ContractError("invalid block")
        block_id = block.get("block_id")
        family = block.get("family")
        if (
            not isinstance(block_id, str)
            or not block_id
            or block_id in seen["block_id"]
        ):
            raise ContractError("duplicate or invalid block id")
        if family not in FAMILIES:
            raise ContractError("unsupported task family")
        seen["block_id"].add(block_id)
        family_counts[family] += 1
        for field in ("instance_root", "fixture_root", "answer_key_root"):
            value = require_root(block.get(field), f"block.{field}")
            if value in seen[field]:
                raise ContractError(f"duplicate {field}")
            seen[field].add(value)
        tasks = block.get("tasks")
        if not isinstance(tasks, dict) or set(tasks) != set(ARMS):
            raise ContractError("block must bind both arms")
        for arm in ARMS:
            task = tasks[arm]
            if not isinstance(task, dict) or not isinstance(task.get("task_name"), str):
                raise ContractError("invalid task binding")
            task_root = require_root(task.get("task_root"), "block.tasks.task_root")
            if task_root in seen["task_root"]:
                raise ContractError("duplicate task_root")
            seen["task_root"].add(task_root)
        order = block.get("arm_order")
        if not isinstance(order, list) or sorted(order) != sorted(ARMS):
            raise ContractError("invalid within-block arm order")
        first_arm_counts[family][order[0]] += 1
        assignments.append(
            {
                "block_id": block_id,
                "arm_order": order,
                "execution_wave": block.get("execution_wave"),
            }
        )
    if len(set(family_counts.values())) != 1:
        raise ContractError("confirmatory blocks must balance both families")
    if any(
        family_counts[family] != sample["initial_blocks_per_family"]
        for family in FAMILIES
    ):
        raise ContractError(
            "plan family counts do not match the registered initial sample"
        )
    if any(
        abs(counts[ARMS[0]] - counts[ARMS[1]]) > 1
        for counts in first_arm_counts.values()
    ):
        raise ContractError("arm order must be balanced within family")
    if randomization["assignment_root"] != root(canonical_bytes(assignments)):
        raise ContractError("randomization assignment root mismatch")
    return plan


def clopper_pearson_lower(successes: int, total: int, alpha: float = 0.025) -> float:
    if successes == 0:
        return 0.0
    return float(stats.beta.ppf(alpha, successes, total - successes + 1))


def clopper_pearson_upper(successes: int, total: int, alpha: float = 0.025) -> float:
    if successes == total:
        return 1.0
    return float(stats.beta.ppf(1 - alpha, successes + 1, total - successes))


def required_blocks_for_effect(sd: float, *, effect_ratio: float = 0.80) -> int:
    """Equal-arm two-sample plug-in size, rounded for two balanced families."""
    if not math.isfinite(sd) or sd <= 0:
        raise ContractError("blinded standard deviation must be positive")
    effect = abs(math.log(effect_ratio)) / sd
    for per_arm in range(2, 10_001):
        df = 2 * per_arm - 2
        critical = stats.t.ppf(0.975, df)
        noncentrality = effect * math.sqrt(per_arm / 2)
        power = stats.nct.cdf(-critical, df, noncentrality) + stats.nct.sf(
            critical, df, noncentrality
        )
        if power >= 0.80:
            return per_arm + (per_arm % 2)
    raise ContractError("required sample size exceeds calculation bound")


def reestimated_blocks(sd: float) -> tuple[int, str]:
    required = max(40, required_blocks_for_effect(sd))
    if required > 120:
        return required, "precision_infeasible"
    return required, "continue"


def paired_table(rows: list[dict[str, Any]]) -> dict[str, int]:
    by_block: dict[str, dict[str, bool]] = defaultdict(dict)
    for row in rows:
        by_block[row["block_id"]][row["arm"]] = row["exact"]
    result = Counter()
    for values in by_block.values():
        guided, baseline = values["vela-guided"], values["git-files"]
        if guided and baseline:
            key = "both_exact"
        elif guided:
            key = "vela_only"
        elif baseline:
            key = "git_only"
        else:
            key = "neither_exact"
        result[key] += 1
    return {
        key: result[key]
        for key in ("both_exact", "vela_only", "git_only", "neither_exact")
    }


def family_effect(differences: list[float]) -> dict[str, float | int | list[float]]:
    if len(differences) < 2:
        raise ContractError("each family requires at least two blocks")
    mean = fmean(differences)
    sample_variance = variance(differences)
    se = math.sqrt(sample_variance / len(differences))
    df = len(differences) - 1
    critical = float(stats.t.ppf(0.975, df))
    lower, upper = mean - critical * se, mean + critical * se
    return {
        "n": len(differences),
        "mean_log_difference": mean,
        "se": se,
        "df": df,
        "ratio": math.exp(mean),
        "ci95_ratio": [math.exp(lower), math.exp(upper)],
        "reduction_fraction": 1 - math.exp(mean),
    }


def validate_trial_export(
    plan: dict[str, Any], value: Any
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    if not isinstance(value, dict) or value.get("schema") != TRIAL_EXPORT_SCHEMA:
        raise ContractError("trial export has the wrong schema")
    if value.get("export_root") != record_root(value, "export_root"):
        raise ContractError("trial export root mismatch")
    if value.get("plan_root") != plan["plan_root"]:
        raise ContractError("trial export binds another plan")
    harbor_job = value.get("harbor_job")
    if not isinstance(harbor_job, dict) or not isinstance(harbor_job.get("id"), str):
        raise ContractError("trial export has no Harbor job identity")
    require_root(
        harbor_job.get("result_sha256"), "trial_export.harbor_job.result_sha256"
    )
    rows = value.get("trials")
    if not isinstance(rows, list):
        raise ContractError("trial export has no trial rows")
    return harbor_job, rows


def analyze(plan: Any, trial_export: Any) -> dict[str, Any]:
    plan = validate_plan(plan)
    harbor_job, trials = validate_trial_export(plan, trial_export)
    block_plan = {block["block_id"]: block for block in plan["blocks"]}
    if len(trials) != 2 * len(block_plan):
        raise ContractError("every block requires exactly two trial rows")
    seen = set()
    trial_ids = set()
    trial_roots = set()
    normalized = []
    for row in trials:
        if not isinstance(row, dict):
            raise ContractError("invalid trial row")
        key = (row.get("block_id"), row.get("arm"))
        if key in seen or key[0] not in block_plan or key[1] not in ARMS:
            raise ContractError("duplicate or unknown trial binding")
        seen.add(key)
        block = block_plan[key[0]]
        if row.get("family") != block["family"]:
            raise ContractError("trial family drift")
        for field in ("eligible", "exact", "authority_error"):
            if not isinstance(row.get(field), bool):
                raise ContractError(f"trial {field} must be boolean")
        elapsed = row.get("agent_execution_elapsed_ms")
        if not isinstance(elapsed, int) or elapsed < 0 or elapsed > 900_000:
            raise ContractError("invalid agent elapsed time")
        if row.get("retry_after_model_output") is not False:
            raise ContractError("post-output retry invalidates confirmation")
        trial_id = row.get("harbor_trial_id")
        if not isinstance(trial_id, str) or not trial_id or trial_id in trial_ids:
            raise ContractError("duplicate or missing Harbor trial id")
        trial_ids.add(trial_id)
        trial_root = require_root(
            row.get("trial_result_sha256"), "trial.trial_result_sha256"
        )
        if trial_root in trial_roots:
            raise ContractError("duplicate Harbor trial result")
        trial_roots.add(trial_root)
        answer_root = row.get("answer_root")
        if answer_root is not None:
            require_root(answer_root, "trial.answer_root")
        if row["exact"] and answer_root is None:
            raise ContractError("exact trial must bind its answer")
        cost = row.get("cost_usd")
        if not isinstance(cost, (int, float)) or not math.isfinite(cost) or cost < 0:
            raise ContractError("invalid trial cost")
        for field in ("input_tokens", "output_tokens", "tool_calls"):
            if not isinstance(row.get(field), int) or row[field] < 0:
                raise ContractError(f"invalid trial {field}")
        restricted = elapsed if row["eligible"] and row["exact"] else 900_000
        normalized.append({**row, "restricted_time_ms": restricted})
    if seen != {(block_id, arm) for block_id in block_plan for arm in ARMS}:
        raise ContractError("missing paired trial")

    guided = [row for row in normalized if row["arm"] == "vela-guided"]
    baseline = [row for row in normalized if row["arm"] == "git-files"]
    all_eligible = all(row["eligible"] for row in normalized)
    authority_errors = sum(row["authority_error"] for row in guided)
    guided_exact = sum(row["exact"] for row in guided)
    baseline_exact = sum(row["exact"] for row in baseline)
    total = len(guided)
    lower_difference = clopper_pearson_lower(
        guided_exact, total
    ) - clopper_pearson_upper(baseline_exact, total)
    exactness_passed = (
        all_eligible and authority_errors == 0 and lower_difference > -0.10
    )

    by_family: dict[str, list[float]] = defaultdict(list)
    for block_id, block in block_plan.items():
        pair = {row["arm"]: row for row in normalized if row["block_id"] == block_id}
        by_family[block["family"]].append(
            math.log(pair["vela-guided"]["restricted_time_ms"])
            - math.log(pair["git-files"]["restricted_time_ms"])
        )
    effects = {family: family_effect(by_family[family]) for family in FAMILIES}
    mean = fmean(float(effects[family]["mean_log_difference"]) for family in FAMILIES)
    variance_terms = [0.25 * (float(effects[family]["se"]) ** 2) for family in FAMILIES]
    pooled_variance = sum(variance_terms)
    pooled_se = math.sqrt(pooled_variance)
    denominator = sum(
        term**2 / (int(effects[family]["n"]) - 1)
        for term, family in zip(variance_terms, FAMILIES, strict=True)
    )
    pooled_df = pooled_variance**2 / denominator if denominator else math.inf
    critical = (
        float(stats.t.ppf(0.975, pooled_df))
        if math.isfinite(pooled_df)
        else 1.959963984540054
    )
    lower, upper = mean - critical * pooled_se, mean + critical * pooled_se
    ratio = math.exp(mean)
    ci = [math.exp(lower), math.exp(upper)]
    superiority = ci[1] < 1
    useful = ratio <= 0.80
    strong = ci[1] <= 0.80
    consistent = all(float(effects[family]["ratio"]) < 1 for family in FAMILIES)
    if not all_eligible or authority_errors:
        outcome = "failed_integrity"
    elif not exactness_passed:
        outcome = "failed_exactness_noninferiority"
    elif not consistent:
        outcome = "failed_family_inconsistency"
    elif not (superiority and useful):
        outcome = "no_confirmatory_efficiency"
    elif strong:
        outcome = "confirmatory_at_least_20_percent"
    else:
        outcome = "confirmatory_pooled_two_family_efficiency"
    result = {
        "schema": RESULT_SCHEMA,
        "result_root": "",
        "plan_root": plan["plan_root"],
        "harbor_job": harbor_job,
        "data_manifest_root": trial_export["export_root"],
        "enrollment": {
            "blocks": len(block_plan),
            "trials": len(normalized),
            "by_family": dict(
                Counter(block["family"] for block in block_plan.values())
            ),
        },
        "exactness": {
            "paired_table": paired_table(normalized),
            "per_arm": {
                "git-files": {"exact": baseline_exact, "attempted": total},
                "vela-guided": {"exact": guided_exact, "attempted": total},
            },
            "guided_authority_errors": authority_errors,
            "lower_difference_bound": lower_difference,
            "margin": -0.10,
            "passed": exactness_passed,
        },
        "primary": {
            "endpoint": "restricted-log-time-to-exact-v1",
            "restriction_ms": 900_000,
            "per_family": {family: effects[family] for family in FAMILIES},
            "pooled": {
                "n": len(block_plan),
                "mean_log_difference": mean,
                "se": pooled_se,
                "df": pooled_df,
                "ratio": ratio,
                "ci95_ratio": ci,
                "reduction_fraction": 1 - ratio,
            },
            "superiority_passed": superiority,
            "useful_point_effect_passed": useful,
            "strong_20_percent_passed": strong,
            "family_consistency_passed": consistent,
        },
        "secondary": {
            arm: {
                "total_cost_usd": sum(
                    float(row["cost_usd"]) for row in normalized if row["arm"] == arm
                ),
                "total_input_tokens": sum(
                    row["input_tokens"] for row in normalized if row["arm"] == arm
                ),
                "total_output_tokens": sum(
                    row["output_tokens"] for row in normalized if row["arm"] == arm
                ),
                "total_tool_calls": sum(
                    row["tool_calls"] for row in normalized if row["arm"] == arm
                ),
            }
            for arm in ARMS
        },
        "trials": normalized,
        "conclusion": {
            "outcome": outcome,
            "claim_credit": outcome
            in {
                "confirmatory_pooled_two_family_efficiency",
                "confirmatory_at_least_20_percent",
            },
            "claim_limit": plan.get("claim_limit"),
        },
    }
    result["result_root"] = record_root(result, "result_root")
    return result


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--trials", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        result = analyze(read_json(args.plan), read_json(args.trials))
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(canonical_bytes(result))
        sys.stdout.buffer.write(
            canonical_bytes(
                {
                    "ok": True,
                    "result_root": result["result_root"],
                    "outcome": result["conclusion"]["outcome"],
                }
            )
        )
        return 0
    except (ContractError, OSError, json.JSONDecodeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
