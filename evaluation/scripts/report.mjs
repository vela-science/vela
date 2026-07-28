#!/usr/bin/env bun

import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  EVALUATION_REPORT_SCHEMA,
  EVALUATION_RUN_SCHEMA,
  canonicalJson,
  digest,
} from "../lib/evaluation-plan.mjs";

const root = path.resolve(process.argv[2] ?? "");
if (process.argv[2] === undefined) throw new Error("usage: eval:report <run-directory>");
const records = [];
for (const entry of (await readdir(root, { withFileTypes: true }))
  .filter((item) => item.isDirectory())
  .sort((left, right) => left.name.localeCompare(right.name))) {
  const record = JSON.parse(await readFile(path.join(root, entry.name, "run.json"), "utf8"));
  if (record.schema !== EVALUATION_RUN_SCHEMA) {
    throw new Error(`${entry.name}/run.json is not an evaluation Run`);
  }
  records.push(record);
}
if (records.length === 0) throw new Error("evaluation output contains no Runs");
const report = {
  schema: EVALUATION_REPORT_SCHEMA,
  plan_root: records[0].plan_root,
  runs: records.length,
  completed: records.filter((record) => record.exit_code === 0).length,
  failed: records.filter((record) => record.exit_code !== 0).length,
  wall_time_ms: records.reduce((sum, record) => sum + record.wall_time_ms, 0),
  run_roots: records.map((record) => digest(record)).sort(),
  interpretation:
    "Process completion only. Verifier passage, scientific disposition, cost, and expert-minute scoring require the registered task scorer.",
};
const file = path.join(root, "report.json");
await writeFile(file, canonicalJson(report), { mode: 0o600, flag: "wx" });
process.stdout.write(`${JSON.stringify({ ok: true, command: "eval:report", file, report_root: digest(report) })}\n`);
