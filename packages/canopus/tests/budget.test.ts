import assert from "node:assert/strict";
import test from "node:test";

import type { MissionBudgets } from "../src/contracts/mission.js";
import { BudgetExceeded, BudgetTracker } from "../src/budget/enforce.js";

const limits: MissionBudgets = {
  max_research_wall_time_ms: 100,
  max_research_processes: 2,
  max_research_output_bytes: 10,
  max_prompt_bytes: 12,
  max_artifact_bytes: 20,
  max_attempts: 2,
  max_observed_tokens: 30,
};

test("budget usage is monotone and reported", () => {
  let now = 0;
  const budget = new BudgetTracker(limits, () => now);
  assert.equal(budget.beginAttempt(), 1);
  assert.equal(budget.beginProcess(), 1);
  assert.equal(budget.addOutput(4), 4);
  assert.equal(budget.addPrompt(7), 7);
  assert.equal(budget.addArtifact(5), 5);
  assert.equal(budget.addTokens(6), 6);
  assert.equal(budget.remainingOutputBytes(), 6);
  assert.equal(budget.remainingArtifactBytes(), 15);
  now = 10;
  assert.deepEqual(budget.snapshot(), {
    research_elapsed_ms: 10,
    research_processes: 1,
    research_output_bytes: 4,
    prompt_bytes: 7,
    artifact_bytes: 5,
    attempts: 1,
    observed_tokens: 6,
  });
});

test("each budget boundary fails before state advances", () => {
  let now = 0;
  const budget = new BudgetTracker(limits, () => now);
  budget.beginAttempt();
  budget.beginAttempt();
  assert.throws(() => budget.beginAttempt(), (error: unknown) =>
    error instanceof BudgetExceeded && error.dimension === "max_attempts");
  budget.addOutput(10);
  assert.throws(() => budget.addOutput(1), /max_research_output_bytes budget exceeded/u);
  budget.addPrompt(12);
  assert.throws(() => budget.addPrompt(1), /max_prompt_bytes budget exceeded/u);
  budget.addArtifact(20);
  assert.throws(() => budget.addArtifact(1), /max_artifact_bytes budget exceeded/u);
  budget.addTokens(30);
  assert.throws(() => budget.addTokens(1), /max_observed_tokens budget exceeded/u);
  budget.beginProcess();
  budget.beginProcess();
  assert.throws(() => budget.beginProcess(), /max_research_processes budget exceeded/u);
  now = 101;
  assert.throws(() => budget.assertTime(), /max_research_wall_time_ms budget exceeded/u);
});
