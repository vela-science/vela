#!/usr/bin/env bun

// Source-only evaluation command; excluded from the npm payload.
import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  EVALUATION_PLAN_SCHEMA,
  EVALUATION_REPORT_SCHEMA,
  canonicalJson,
  digest,
  parseEvaluationPlan,
  parseEvaluationRun,
  parseEvaluationRunSet,
} from "../lib/evaluation-plan.mjs";

const root = path.resolve(process.argv[2] ?? "");
if (process.argv[2] === undefined) throw new Error("usage: eval:report <run-directory>");
const index = parseEvaluationRunSet(
  JSON.parse(await readFile(path.join(root, "index.json"), "utf8")),
);
let plan = null;
try {
  plan = parseEvaluationPlan(
    JSON.parse(await readFile(path.join(root, "plan.json"), "utf8")),
  );
} catch (error) {
  if (error?.code !== "ENOENT") throw error;
}
if (plan !== null && plan.plan_root !== index.plan_root) {
  throw new Error("run set and retained evaluation plan roots disagree");
}
const matrix = plan?.schema === EVALUATION_PLAN_SCHEMA
  ? plan.matrices.find((candidate) => candidate.stage === index.stage)
  : undefined;
if (index.evaluation_use !== null) {
  if (matrix === undefined) {
    throw new Error(
      "run set declares evaluation use without its rooted v2 plan matrix",
    );
  }
  if (index.evaluation_use !== matrix.purpose) {
    throw new Error("run set evaluation use disagrees with its rooted plan matrix");
  }
}
if (matrix !== undefined && index.evaluation_use === null) {
  throw new Error("v2 run set omits its rooted plan matrix purpose");
}
const evaluationUse = matrix?.purpose ?? "historical_unspecified";
const confirmatoryEligible =
  plan?.schema === EVALUATION_PLAN_SCHEMA &&
  evaluationUse === "confirmatory_generation";
const records = [];
for (const entry of (await readdir(root, { withFileTypes: true }))
  .filter((item) => item.isDirectory())
  .sort((left, right) => left.name.localeCompare(right.name))) {
  const record = parseEvaluationRun(
    JSON.parse(await readFile(path.join(root, entry.name, "run.json"), "utf8")),
  );
  if (record.assignment.id !== entry.name) {
    throw new Error(`${entry.name}/run.json belongs to ${record.assignment.id}`);
  }
  if (record.plan_root !== index.plan_root) {
    throw new Error(`${entry.name}/run.json belongs to a different evaluation plan`);
  }
  records.push(record);
}
if (records.length === 0) throw new Error("evaluation output contains no Runs");
const observedRoots = new Map(records.map((record) => [
  record.assignment.id,
  digest(record),
]));
for (const expected of index.runs) {
  const observed = observedRoots.get(expected.assignment_id);
  if (observed === undefined) {
    throw new Error(`run set names missing assignment ${expected.assignment_id}`);
  }
  if (observed !== expected.run_root) {
    throw new Error(
      `run ${expected.assignment_id} drifted: expected ${expected.run_root}, observed ${observed}`,
    );
  }
}
if (observedRoots.size !== index.runs.length) {
  throw new Error("evaluation output contains a Run absent from its run set");
}
const missingAssignments = index.registered_assignment_ids.filter(
  (assignmentId) => !observedRoots.has(assignmentId),
);
const report = {
  schema: EVALUATION_REPORT_SCHEMA,
  plan_root: index.plan_root,
  stage: index.stage,
  evaluation_use: evaluationUse,
  confirmatory_eligible: confirmatoryEligible,
  run_set_status: index.status,
  stop_reason: index.stop_reason,
  registered: index.registered_assignment_ids.length,
  runs: records.length,
  completed: records.filter((record) =>
    record.exit_code === 0 && record.runner_error === null && !record.timed_out).length,
  failed: records.filter((record) =>
    record.exit_code !== 0 || record.runner_error !== null || record.timed_out).length,
  verifier_passed: records.filter((record) =>
    record.verifier?.outcome === "pass").length,
  verifier_failed: records.filter((record) =>
    record.verifier?.outcome === "fail").length,
  verifier_not_run: records.filter((record) => record.verifier === null).length,
  missing_assignment_ids: missingAssignments,
  wall_time_ms: records.reduce((sum, record) => sum + record.wall_time_ms, 0),
  observed_tokens: records.reduce(
    (sum, record) => sum + (record.observed_tokens ?? 0),
    0,
  ),
  unmeasured_token_runs: records
    .filter((record) => record.observed_tokens === null)
    .map((record) => record.assignment.id),
  run_roots: records.map((record) => digest(record)).sort(),
  interpretation:
    confirmatoryEligible
      ? "The plan declares held-out confirmatory generation. Verifier passage remains independent from process completion, scientific disposition, and expert-minute scoring."
      : "This run is reproduction, calibration, or retained historical evidence and cannot establish generation lift. Verifier passage remains independent from process completion and scientific disposition.",
};
const file = path.join(root, "report.json");
await writeFile(file, canonicalJson(report), { mode: 0o600, flag: "wx" });
process.stdout.write(`${JSON.stringify({ ok: true, command: "eval:report", file, report_root: digest(report) })}\n`);
