import assert from "node:assert/strict";
import test from "node:test";

import {
  parseEvaluationPlan,
  rootEvaluationPlan,
} from "../evaluation/lib/evaluation-plan.mjs";

const root = (character) => `sha256:${character.repeat(64)}`;
const identity = (name, character) => ({
  name,
  version: "1.0.0",
  sha256: root(character),
});

function plan() {
  const base = {
    schema: "canopus.evaluation-plan.v1",
    plan_id: "eval_fixture_stage_a",
    status: "registered",
    created_at: "2026-07-28T00:00:00Z",
    campaign: "Math-first framework-neutral fixture.",
    identities: {
      model: identity("model", "1"),
      codex: identity("codex", "2"),
      canopus: identity("canopus", "3"),
      vela: identity("vela", "4"),
      git: identity("git", "5"),
      environment: identity("environment", "6"),
      dependencies: [],
    },
    tasks: [{
      id: "math:fixture",
      class: "math",
      source: "fixture",
      source_root: root("7"),
      packet_path: "packets/math.json",
      packet_root: root("8"),
      verifier_root: root("9"),
      license: "MIT",
      cpu_only: true,
      network: "deny",
      max_wall_time_ms: 60_000,
      max_observed_tokens: 10_000,
    }],
    arms: [{
      id: "native",
      kind: "native_codex",
      argv: ["codex", "exec", "{task_packet}"],
      cwd: "workspace",
      dependency_lock_root: root("a"),
      environment_root: root("b"),
    }],
    assignments: [{
      id: "A-math-native-r1",
      stage: "A",
      task_id: "math:fixture",
      arm_id: "native",
      repetition: 1,
      seed: 1,
    }],
    budgets: {
      max_model_calls: 1,
      max_total_wall_time_ms: 60_000,
      max_total_observed_tokens: 10_000,
    },
    retry_policy: {
      max_pre_output_infrastructure_retries: 1,
      post_output_retries: 0,
    },
    stopping_rules: ["Stop on any credential exposure."],
    scorers: [root("c"), root("d"), root("e")],
    performance_functions: {
      execution_lift: root("c"),
      state_lift: root("d"),
      inheritance_lift: root("e"),
    },
    exclusions: [],
    custody: {
      human_keys: "forbidden",
      repository_authority: "forbidden",
      secrets_in_records: "forbidden",
      chain_of_thought_in_traces: "forbidden",
      canonical_mutation: "forbidden",
    },
    publication: {
      raw_failures: "required",
      exclusions: "required",
      roots: "required",
      independence_credit: "none_first_party",
    },
    amends_root: null,
    amendment_reason: null,
    plan_root: "",
  };
  return rootEvaluationPlan(base);
}

test("evaluation plan is closed, rooted, and registered before execution", () => {
  const registered = plan();
  assert.deepEqual(parseEvaluationPlan(registered), registered);
  assert.throws(
    () => parseEvaluationPlan({ ...registered, plan_root: root("f") }),
    /plan root mismatch/u,
  );
  assert.throws(
    () => parseEvaluationPlan({ ...registered, hidden_retry: true }),
    /hidden_retry is not allowed/u,
  );
});

test("evaluation plan rejects authority and post-output retry paths", () => {
  const registered = plan();
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...registered,
      retry_policy: {
        max_pre_output_infrastructure_retries: 1,
        post_output_retries: 1,
      },
    })),
    /one pre-output retry and no post-output retry/u,
  );
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...registered,
      custody: { ...registered.custody, repository_authority: "allowed" },
    })),
    /repository_authority must be forbidden/u,
  );
});

test("evaluation plan permits the required same-packet native control and caps calls at 36", () => {
  const registered = plan();
  const packetControl = {
    ...registered.arms[0],
    id: "native-packet",
    kind: "native_codex_packet",
  };
  assert.doesNotThrow(() => parseEvaluationPlan(rootEvaluationPlan({
    ...registered,
    arms: [...registered.arms, packetControl],
  })));
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...registered,
      assignments: Array.from({ length: 37 }, (_, index) => ({
        ...registered.assignments[0],
        id: `assignment-${index}`,
        repetition: (index % 16) + 1,
        seed: index,
      })),
      budgets: {
        ...registered.budgets,
        max_model_calls: 36,
      },
    })),
    /1\.\.36 entries/u,
  );
});

test("evaluation plan binds distinct execution, state, and inheritance scorers", () => {
  const registered = plan();
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...registered,
      performance_functions: {
        ...registered.performance_functions,
        inheritance_lift: registered.performance_functions.state_lift,
      },
    })),
    /three distinct scorer roots/u,
  );
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...registered,
      performance_functions: {
        ...registered.performance_functions,
        inheritance_lift: root("f"),
      },
    })),
    /absent from plan\.scorers/u,
  );
});
