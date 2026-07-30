#!/usr/bin/env bun

// Source-only evaluation command; excluded from the npm payload.
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  isLiveEvaluationRegistration,
  verifyEvaluationPlanFiles,
} from "../lib/evaluation-plan.mjs";

const supplied = process.argv.slice(2).filter((value) => value !== "--");
let files = supplied;
if (files.length === 0) {
  try {
    files = (await readdir(new URL("../registrations/", import.meta.url)))
      .filter((file) => file.endsWith(".json"))
      .sort()
      .map((file) => path.join("evaluation", "registrations", file));
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
    files = [];
  }
}
const plans = [];
for (const file of files) {
  const planFile = path.resolve(file);
  const { plan, verified_files: verifiedFiles } = await verifyEvaluationPlanFiles(
    JSON.parse(await readFile(planFile, "utf8")),
    planFile,
  );
  plans.push({
    file,
    plan_id: plan.plan_id,
    schema: plan.schema,
    status: plan.status,
    plan_root: plan.plan_root,
    assignments: plan.assignments.length,
    matrices: plan.matrices?.map((matrix) => ({
      stage: matrix.stage,
      purpose: matrix.purpose,
      tasks: matrix.task_ids.length,
      arms: matrix.arm_ids.length,
      repetitions: matrix.repetitions.length,
    })) ?? [],
    verified_files: verifiedFiles,
  });
}
process.stdout.write(`${JSON.stringify({
  ok: true,
  command: "eval:validate",
  registrations: plans,
  live_registration_ready: plans.some(isLiveEvaluationRegistration),
})}\n`);
