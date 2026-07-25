import assert from "node:assert/strict";
import test from "node:test";

import { runLiveShadow } from "../experiments/task-authority/run-live-shadow.mjs";

test("live shadow preserves clean work, blocks drift, and repeats across two workbenches", async () => {
  const report = await runLiveShadow();
  assert.equal(report.schema, "canopus.task-authority-live-shadow-report.v1");
  assert.equal(report.live_replay_root, "sha256:3c2b8721a86cc0e3d91c75db99e495799430a2b1d5f44a5996af1343d8604c47");
  assert.equal(report.accepted_event_delta, 0);
  assert.equal(report.workbenches.length, 2);
  assert.deepEqual(
    report.workbenches.map((item) => item.workbench),
    ["Codex CLI", "Claude Code"],
  );
  for (const workbench of report.workbenches) {
    assert.equal(workbench.clean_shadow, "permit");
    assert.equal(workbench.evidence_drift.verdict, "reauthorization_required");
    assert.deepEqual(workbench.evidence_drift.violations, [
      "reauthorization_required_evidence_changed",
    ]);
    assert.equal(workbench.hostile_cases.length, 8);
    assert.equal(workbench.hostile_detected, 8);
    assert.equal(
      workbench.hostile_cases.every((item) => item.verdict === "violation"),
      true,
    );
  }
  assert.equal(report.decision, "PIVOT_OPERATIONAL_ONLY");
  assert.equal(report.promotion, "none");
  assert.deepEqual(report.effects, {
    scientific: "none",
    authority: "none",
    standing: "none",
  });
  assert.equal(
    report.report_root,
    "sha256:e26c68f4b351844c7e35196a5c717e0ffcec28668ededc6a681dc94ca6a70970",
  );
});
