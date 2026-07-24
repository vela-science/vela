import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { contentRoot, runExperiment } from "../experiments/task-authority/run.mjs";

test("task-authority candidate catches every frozen hostile without gaining authority", async () => {
  const report = await runExperiment();
  assert.equal(report.hostile_case_count, 8);
  assert.equal(report.baseline_detected, 3);
  assert.equal(report.candidate_detected, 8);
  assert.deepEqual(report.baseline_false_passes, [
    "compromised_ci_valid_provenance",
    "correct_result_forbidden_source",
    "producer_or_workbench_swap",
    "stale_approval_principal",
    "evidence_drift_after_approval",
  ]);
  assert.deepEqual(report.effects, {
    scientific: "none",
    authority: "none",
    standing: "none",
  });
  assert.equal(report.promotion, "none");
  assert.equal(report.decision, "PIVOT_OPERATIONAL_ONLY");
  const { report_root: reportedRoot, ...reportWithoutRoot } = report;
  assert.equal(reportedRoot, contentRoot(reportWithoutRoot));
});

test("green provenance or verification never rescues a task-authority violation", async () => {
  const report = await runExperiment();
  const green = report.cases.filter((item) => item.green_provenance || item.green_verifier);
  assert.ok(green.length > 0);
  assert.ok(green.every((item) => item.candidate.verdict === "violation"));
});

test("task-authority experiment remains outside the published package", async () => {
  const packageJson = JSON.parse(
    await readFile(new URL("../package.json", import.meta.url), "utf8"),
  );
  assert.ok(
    packageJson.files.every(
      (path) =>
        path !== "experiments" &&
        !path.startsWith("experiments/") &&
        path !== "tests/task-authority-experiment.test.mjs",
    ),
  );
});
