#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  EVALUATION_RUN_SCHEMA,
  canonicalJson,
  digest,
  parseEvaluationArmResult,
  verifyEvaluationPlanFiles,
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
  return await new Promise((resolve) => {
    const child = spawn(argv[0], argv.slice(1), {
      cwd,
      env: Object.fromEntries(
        ["PATH", "HOME", "CODEX_HOME", "TMPDIR", "LANG", "LC_ALL", "NO_COLOR"]
          .flatMap((key) => process.env[key] === undefined ? [] : [[key, process.env[key]]]),
      ),
      // File descriptor 3 is a supervisor-only control channel. Trusted arm
      // wrappers write provider usage there after the model process exits and
      // must not inherit it into the model sandbox.
      stdio: ["ignore", "pipe", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    const control = [];
    let bytes = 0;
    let controlBytes = 0;
    let runnerError = null;
    let timedOut = false;
    let settled = false;
    const finish = (outcome) => {
      if (settled) return;
      settled = true;
      resolve(outcome);
    };
    const collect = (chunks, chunk) => {
      bytes += chunk.length;
      if (bytes > 64 * 1024 * 1024 && runnerError === null) {
        runnerError = "evaluation command exceeded the 64 MiB output bound";
        child.kill("SIGKILL");
        return;
      }
      if (runnerError === null) chunks.push(chunk);
    };
    child.stdout.on("data", (chunk) => collect(stdout, chunk));
    child.stderr.on("data", (chunk) => collect(stderr, chunk));
    child.stdio[3].on("data", (chunk) => {
      controlBytes += chunk.length;
      if (controlBytes > 64 * 1024 && runnerError === null) {
        runnerError = "evaluation control record exceeded the 64 KiB bound";
        child.kill("SIGKILL");
        return;
      }
      if (runnerError === null) control.push(chunk);
    });
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGKILL");
    }, timeoutMs);
    child.on("error", (error) => {
      clearTimeout(timer);
      finish({
        exit_code: null,
        signal: null,
        stdout: Buffer.concat(stdout),
        stderr: Buffer.concat(stderr),
        control: Buffer.concat(control),
        timed_out: timedOut,
        runner_error: error.message,
      });
    });
    child.on("close", (code, signal) => {
      clearTimeout(timer);
      finish({
        exit_code: code,
        signal,
        stdout: Buffer.concat(stdout),
        stderr: Buffer.concat(stderr),
        control: Buffer.concat(control),
        timed_out: timedOut,
        runner_error: runnerError,
      });
    });
  });
}

async function executeVerifier(argv, cwd, timeoutMs) {
  return await new Promise((resolve) => {
    const child = spawn(argv[0], argv.slice(1), {
      cwd,
      env: {
        PATH: "/usr/local/bin:/usr/bin:/bin",
        HOME: cwd,
        LANG: "C",
        LC_ALL: "C",
        NO_COLOR: "1",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    let bytes = 0;
    let runnerError = null;
    let timedOut = false;
    let settled = false;
    const finish = (outcome) => {
      if (settled) return;
      settled = true;
      resolve(outcome);
    };
    const collect = (chunks, chunk) => {
      bytes += chunk.length;
      if (bytes > 16 * 1024 * 1024 && runnerError === null) {
        runnerError = "evaluation verifier exceeded the 16 MiB output bound";
        child.kill("SIGKILL");
        return;
      }
      if (runnerError === null) chunks.push(chunk);
    };
    child.stdout.on("data", (chunk) => collect(stdout, chunk));
    child.stderr.on("data", (chunk) => collect(stderr, chunk));
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGKILL");
    }, timeoutMs);
    child.on("error", (error) => {
      clearTimeout(timer);
      finish({
        exit_code: null,
        signal: null,
        stdout: Buffer.concat(stdout),
        stderr: Buffer.concat(stderr),
        timed_out: timedOut,
        runner_error: error.message,
      });
    });
    child.on("close", (code, signal) => {
      clearTimeout(timer);
      finish({
        exit_code: code,
        signal,
        stdout: Buffer.concat(stdout),
        stderr: Buffer.concat(stderr),
        timed_out: timedOut,
        runner_error: runnerError,
      });
    });
  });
}

function remainsBelow(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative !== "" && relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

async function readArtifact(assignmentRoot, relativePath, maxBytes) {
  const resolvedRoot = await realpath(assignmentRoot);
  const candidate = path.resolve(resolvedRoot, relativePath);
  const resolved = await realpath(candidate);
  if (!remainsBelow(resolvedRoot, resolved)) {
    throw new Error("candidate artifact escapes its assigned output");
  }
  const metadata = await lstat(candidate);
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    metadata.nlink !== 1 ||
    metadata.size === 0 ||
    metadata.size > maxBytes
  ) {
    throw new Error("candidate artifact violates its regular-file byte contract");
  }
  return {
    path: resolved,
    bytes: await readFile(resolved),
  };
}

const values = options(process.argv.slice(2).filter((value) => value !== "--"));
for (const required of ["--plan", "--stage", "--output"]) {
  if (!values.has(required)) {
    throw new Error(`usage: eval:run --plan <plan> --stage <A|B|C> --output <new-directory>`);
  }
}
const planFile = await realpath(values.get("--plan"));
const stage = values.get("--stage");
if (!["A", "B", "C"].includes(stage)) throw new Error("--stage must be A, B, or C");
const output = path.resolve(values.get("--output"));
const {
  plan,
  executable_paths: executablePaths,
  wrapper_paths: wrapperPaths,
  task_source_paths: taskSourcePaths,
  task_packet_paths: taskPacketPaths,
  task_verifier_paths: taskVerifierPaths,
  task_verifier_runtime_paths: taskVerifierRuntimePaths,
} = await verifyEvaluationPlanFiles(
  JSON.parse(await readFile(planFile, "utf8")),
  planFile,
);
if (plan.status !== "registered") throw new Error("evaluation execution requires a registered plan");
const assignments = plan.assignments.filter((assignment) => assignment.stage === stage);
if (assignments.length === 0) throw new Error(`registered plan has no Stage ${stage} assignments`);
await mkdir(output, { recursive: false, mode: 0o700 });
const planDirectory = path.dirname(planFile);
const results = [];
let totalWallTimeMs = 0;
let totalObservedTokens = 0;
let stopReason = null;
for (const assignment of assignments) {
  const task = plan.tasks.find((candidate) => candidate.id === assignment.task_id);
  const arm = plan.arms.find((candidate) => candidate.id === assignment.arm_id);
  const packet = taskPacketPaths[task.id];
  if (sha256(await readFile(packet)) !== task.packet_root) {
    throw new Error(`task ${task.id} packet root drifted`);
  }
  const assignmentRoot = path.join(output, assignment.id);
  await mkdir(assignmentRoot, { mode: 0o700 });
  const argv = arm.argv.map((entry) =>
    entry
      .replaceAll("{task_packet}", packet)
      .replaceAll("{wrapper}", wrapperPaths[arm.id])
      .replaceAll("{output}", assignmentRoot)
      .replaceAll("{assignment_id}", assignment.id)
      .replaceAll("{seed}", String(assignment.seed)));
  const startedAt = new Date();
  const started = process.hrtime.bigint();
  const outcome = await execute(
    [executablePaths[arm.id], ...argv.slice(1)],
    path.resolve(planDirectory, arm.cwd),
    task.max_wall_time_ms,
  );
  const producerWallTimeMs = Number(
    (process.hrtime.bigint() - started) / 1_000_000n,
  );
  await writeFile(path.join(assignmentRoot, "stdout.bin"), outcome.stdout, { mode: 0o600 });
  await writeFile(path.join(assignmentRoot, "stderr.bin"), outcome.stderr, { mode: 0o600 });
  let armResult = null;
  let armResultRoot = null;
  let armResultError = null;
  try {
    if (outcome.control.length === 0) {
      throw new Error("trusted arm wrapper returned no fd 3 control record");
    }
    const candidateArmResult = parseEvaluationArmResult(
      JSON.parse(outcome.control.toString("utf8")),
    );
    if (candidateArmResult.assignment_id !== assignment.id) {
      throw new Error(
        `arm result belongs to ${candidateArmResult.assignment_id}, expected ${assignment.id}`,
      );
    }
    await writeFile(path.join(assignmentRoot, "arm-result.json"), outcome.control, {
      mode: 0o600,
      flag: "wx",
    });
    armResult = candidateArmResult;
    armResultRoot = sha256(outcome.control);
    totalObservedTokens +=
      candidateArmResult.usage.input_tokens + candidateArmResult.usage.output_tokens;
  } catch (error) {
    armResultError = error instanceof Error ? error.message : String(error);
  }
  const runnerError = outcome.runner_error ??
    (armResultError === null ? null : `invalid or missing trusted arm result: ${armResultError}`);
  const observedTokens = armResult === null
    ? null
    : armResult.usage.input_tokens + armResult.usage.output_tokens;
  let artifactRoot = null;
  let verifier = null;
  if (
    outcome.exit_code === 0 &&
    outcome.runner_error === null &&
    armResultError === null
  ) {
    const verifierStarted = process.hrtime.bigint();
    let verifierOutcome = {
      exit_code: null,
      signal: null,
      stdout: Buffer.alloc(0),
      stderr: Buffer.alloc(0),
      timed_out: false,
      runner_error: null,
    };
    try {
      const artifact = await readArtifact(
        assignmentRoot,
        task.artifact_path,
        task.max_artifact_bytes,
      );
      artifactRoot = sha256(artifact.bytes);
      const source = taskSourcePaths[task.id];
      if (sha256(await readFile(source)) !== task.source_root) {
        throw new Error(`task ${task.id} source root drifted`);
      }
      const verifierExecutable = taskVerifierPaths[task.id];
      if (sha256(await readFile(verifierExecutable)) !== task.verifier_root) {
        throw new Error(`task ${task.id} verifier root drifted`);
      }
      if (
        sha256(await readFile(taskVerifierRuntimePaths[task.id])) !==
          task.verifier_runtime_root
      ) {
        throw new Error(`task ${task.id} verifier runtime root drifted`);
      }
      const verifierArgv = task.verifier_args.map((entry) =>
        entry
          .replaceAll("{artifact}", artifact.path)
          .replaceAll("{source}", source));
      verifierOutcome = await executeVerifier(
        [
          taskVerifierRuntimePaths[task.id],
          ...(task.verifier_runtime === "bun" ? [verifierExecutable] : []),
          ...verifierArgv,
        ],
        assignmentRoot,
        task.max_wall_time_ms,
      );
    } catch (error) {
      verifierOutcome.runner_error = error instanceof Error ? error.message : String(error);
    }
    await writeFile(
      path.join(assignmentRoot, "verifier-stdout.bin"),
      verifierOutcome.stdout,
      { mode: 0o600, flag: "wx" },
    );
    await writeFile(
      path.join(assignmentRoot, "verifier-stderr.bin"),
      verifierOutcome.stderr,
      { mode: 0o600, flag: "wx" },
    );
    const verifierError = verifierOutcome.runner_error ??
      (verifierOutcome.timed_out ? "evaluation verifier timed out" : null);
    verifier = {
      outcome:
        verifierOutcome.exit_code === 0 && verifierError === null ? "pass" : "fail",
      exit_code: verifierOutcome.exit_code,
      signal: verifierOutcome.signal,
      wall_time_ms: Number(
        (process.hrtime.bigint() - verifierStarted) / 1_000_000n,
      ),
      stdout_root: sha256(verifierOutcome.stdout),
      stderr_root: sha256(verifierOutcome.stderr),
      error: verifierError,
    };
  }
  const endedAt = new Date();
  const wallTimeMs = Number((process.hrtime.bigint() - started) / 1_000_000n);
  totalWallTimeMs += wallTimeMs;
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
    model_output_observed: armResult?.model_output_observed ?? outcome.stdout.length > 0,
    timed_out: outcome.timed_out,
    runner_error: runnerError,
    retry_of: null,
    authority_effect: "none",
    arm_result_root: armResultRoot,
    usage: armResult?.usage ?? null,
    observed_tokens: observedTokens,
    artifact_root: artifactRoot,
    producer_wall_time_ms: producerWallTimeMs,
    verifier,
  };
  await writeFile(path.join(assignmentRoot, "run.json"), canonicalJson(record), {
    mode: 0o600,
    flag: "wx",
  });
  results.push(record);
  if (totalWallTimeMs > plan.budgets.max_total_wall_time_ms) {
    stopReason = "evaluation exceeded the registered total wall-time budget";
    break;
  }
  if (observedTokens !== null && observedTokens > task.max_observed_tokens) {
    stopReason = `task ${task.id} exceeded its registered observed-token budget`;
    break;
  }
  if (totalObservedTokens > plan.budgets.max_total_observed_tokens) {
    stopReason = "evaluation exceeded the registered total observed-token budget";
    break;
  }
  if (armResultError !== null) {
    stopReason = `assignment ${assignment.id} did not produce valid rooted token usage`;
    break;
  }
  if (outcome.runner_error === "evaluation command exceeded the 64 MiB output bound") {
    stopReason = outcome.runner_error;
    break;
  }
}
const registeredAssignmentIds = assignments.map((assignment) => assignment.id);
await writeFile(
  path.join(output, "index.json"),
  canonicalJson({
    schema: "canopus.evaluation-run-set.v1",
    plan_root: plan.plan_root,
    stage,
    status: stopReason === null ? "complete" : "stopped",
    stop_reason: stopReason,
    registered_assignment_ids: registeredAssignmentIds,
    runs: results.map((result) => ({
      assignment_id: result.assignment.id,
      run_root: digest(result),
    })),
  }),
  { mode: 0o600, flag: "wx" },
);
process.stdout.write(`${JSON.stringify({
  ok: stopReason === null && results.every((result) =>
    result.exit_code === 0 && result.runner_error === null && !result.timed_out),
  command: "eval:run",
  stage,
  output,
  status: stopReason === null ? "complete" : "stopped",
  stop_reason: stopReason,
  runs: results.map((result) => result.run_id),
})}\n`);
