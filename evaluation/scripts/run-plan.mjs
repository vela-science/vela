#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  EVALUATION_RUN_SCHEMA,
  canonicalJson,
  digest,
  parseEvaluationPlan,
} from "../lib/evaluation-plan.mjs";

function options(argv) {
  const parsed = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!["--plan", "--stage", "--output"].includes(key) || value === undefined || parsed.has(key)) {
      throw new Error(`invalid evaluation option near ${key ?? "end"}`);
    }
    parsed.set(key, value);
  }
  return parsed;
}

function sha256(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

async function execute(argv, cwd, timeoutMs) {
  return await new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), {
      cwd,
      env: Object.fromEntries(
        ["PATH", "HOME", "CODEX_HOME", "TMPDIR", "LANG", "LC_ALL", "NO_COLOR"]
          .flatMap((key) => process.env[key] === undefined ? [] : [[key, process.env[key]]]),
      ),
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    let bytes = 0;
    const collect = (chunks, chunk) => {
      bytes += chunk.length;
      if (bytes > 64 * 1024 * 1024) {
        child.kill("SIGKILL");
        reject(new Error("evaluation command exceeded the 64 MiB output bound"));
        return;
      }
      chunks.push(chunk);
    };
    child.stdout.on("data", (chunk) => collect(stdout, chunk));
    child.stderr.on("data", (chunk) => collect(stderr, chunk));
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.on("error", reject);
    child.on("close", (code, signal) => {
      clearTimeout(timer);
      resolve({
        exit_code: code,
        signal,
        stdout: Buffer.concat(stdout),
        stderr: Buffer.concat(stderr),
      });
    });
  });
}

const values = options(process.argv.slice(2).filter((value) => value !== "--"));
const planFile = await realpath(values.get("--plan"));
const stage = values.get("--stage");
if (!["A", "B", "C"].includes(stage)) throw new Error("--stage must be A, B, or C");
const output = path.resolve(values.get("--output"));
const plan = parseEvaluationPlan(JSON.parse(await readFile(planFile, "utf8")));
if (plan.status !== "registered") throw new Error("evaluation execution requires a registered plan");
const assignments = plan.assignments.filter((assignment) => assignment.stage === stage);
if (assignments.length === 0) throw new Error(`registered plan has no Stage ${stage} assignments`);
await mkdir(output, { recursive: false, mode: 0o700 });
const planDirectory = path.dirname(planFile);
const results = [];
for (const assignment of assignments) {
  const task = plan.tasks.find((candidate) => candidate.id === assignment.task_id);
  const arm = plan.arms.find((candidate) => candidate.id === assignment.arm_id);
  const packet = await realpath(path.join(planDirectory, task.packet_path));
  if (sha256(await readFile(packet)) !== task.packet_root) {
    throw new Error(`task ${task.id} packet root drifted`);
  }
  const assignmentRoot = path.join(output, assignment.id);
  await mkdir(assignmentRoot, { mode: 0o700 });
  const argv = arm.argv.map((entry) =>
    entry
      .replaceAll("{task_packet}", packet)
      .replaceAll("{output}", assignmentRoot)
      .replaceAll("{seed}", String(assignment.seed)));
  const startedAt = new Date();
  const started = process.hrtime.bigint();
  const outcome = await execute(
    argv,
    path.resolve(planDirectory, arm.cwd),
    task.max_wall_time_ms,
  );
  const endedAt = new Date();
  const wallTimeMs = Number((process.hrtime.bigint() - started) / 1_000_000n);
  await writeFile(path.join(assignmentRoot, "stdout.bin"), outcome.stdout, { mode: 0o600 });
  await writeFile(path.join(assignmentRoot, "stderr.bin"), outcome.stderr, { mode: 0o600 });
  const record = {
    schema: EVALUATION_RUN_SCHEMA,
    run_id: `eval_${digest({
      plan_root: plan.plan_root,
      assignment_id: assignment.id,
      started_at: startedAt.toISOString(),
    }).slice(7, 23)}`,
    plan_root: plan.plan_root,
    assignment,
    task_root: digest(task),
    arm_root: digest(arm),
    started_at: startedAt.toISOString(),
    ended_at: endedAt.toISOString(),
    wall_time_ms: wallTimeMs,
    exit_code: outcome.exit_code,
    signal: outcome.signal,
    stdout_root: sha256(outcome.stdout),
    stderr_root: sha256(outcome.stderr),
    model_output_observed: outcome.stdout.length > 0,
    retry_of: null,
    authority_effect: "none",
  };
  await writeFile(path.join(assignmentRoot, "run.json"), canonicalJson(record), {
    mode: 0o600,
    flag: "wx",
  });
  results.push(record);
  if (outcome.exit_code !== 0) break;
}
await writeFile(
  path.join(output, "index.json"),
  canonicalJson({
    schema: "canopus.evaluation-run-set.v1",
    plan_root: plan.plan_root,
    stage,
    run_roots: results.map((result) => digest(result)),
  }),
  { mode: 0o600, flag: "wx" },
);
process.stdout.write(`${JSON.stringify({
  ok: results.every((result) => result.exit_code === 0),
  command: "eval:run",
  stage,
  output,
  runs: results.map((result) => result.run_id),
})}\n`);
