import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  parseEvaluationPlan,
  rootEvaluationPlan,
  verifyEvaluationPlanFiles,
} from "../evaluation/lib/evaluation-plan.mjs";

const root = (character) => `sha256:${character.repeat(64)}`;
const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const byteRoot = (value) =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;
const identity = (name, character) => ({
  name,
  version: "1.0.0",
  sha256: root(character),
});
const scorer = (id, character) => ({
  id,
  path: `scorers/${id}.json`,
  root: root(character),
});

function plan() {
  const base = {
    schema: "canopus.evaluation-plan.v1",
    plan_id: "eval_fixture",
    status: "draft",
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
      source_path: "sources/math.json",
      source_root: root("7"),
      packet_path: "packets/math.json",
      packet_root: root("8"),
      verifier_path: "verifiers/math",
      verifier_root: root("9"),
      verifier_runtime: "direct",
      verifier_runtime_root: root("9"),
      verifier_args: ["{artifact}"],
      verifier_resources: [],
      artifact_path: "artifacts/result.txt",
      max_artifact_bytes: 65_536,
      license: "MIT",
      cpu_only: true,
      network: "deny",
      max_wall_time_ms: 60_000,
      max_observed_tokens: 10_000,
    }],
    arms: [{
      id: "native",
      kind: "native_codex",
      argv: ["codex", "{wrapper}", "exec", "{task_packet}"],
      cwd: "workspace",
      wrapper_path: "wrappers/native.mjs",
      wrapper_root: root("0"),
      dependency_lock_path: "locks/native.lock",
      dependency_lock_root: root("a"),
      environment_path: "environments/native.json",
      environment_root: root("b"),
      executable_root: root("f"),
      resources: [],
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
    scorers: [
      scorer("execution", "c"),
      scorer("state", "d"),
      scorer("inheritance", "e"),
    ],
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

function registeredStageA() {
  const base = plan();
  const scientificTask = {
    ...base.tasks[0],
    id: "scientific:fixture",
    class: "scientific_computing",
    packet_path: "packets/scientific.json",
    packet_root: root("f"),
    source_path: "sources/scientific.json",
    source_root: root("1"),
    verifier_path: "verifiers/scientific",
    verifier_root: root("0"),
    verifier_runtime_root: root("0"),
  };
  const arms = [
    base.arms[0],
    {
      ...base.arms[0],
      id: "native-packet",
      kind: "native_codex_packet",
    },
    {
      ...base.arms[0],
      id: "canopus",
      kind: "canopus",
    },
  ];
  const tasks = [...base.tasks, scientificTask];
  const assignments = tasks.flatMap((task) =>
    arms.flatMap((arm) =>
      [1, 2].map((repetition) => ({
        id: `A-${task.id}-${arm.id}-r${repetition}`,
        stage: "A",
        task_id: task.id,
        arm_id: arm.id,
        repetition,
        seed: repetition,
      }))));
  return rootEvaluationPlan({
    ...base,
    plan_id: "eval_fixture_stage_a",
    status: "registered",
    tasks,
    arms,
    assignments,
    budgets: {
      max_model_calls: 12,
      max_total_wall_time_ms: 12 * 60_000,
      max_total_observed_tokens: 12 * 10_000,
    },
  });
}

test("evaluation plan is closed and rooted before execution", () => {
  const draft = plan();
  assert.deepEqual(parseEvaluationPlan(draft), draft);
  assert.throws(
    () => parseEvaluationPlan({ ...draft, plan_root: root("f") }),
    /plan root mismatch/u,
  );
  assert.throws(
    () => parseEvaluationPlan({ ...draft, hidden_retry: true }),
    /hidden_retry is not allowed/u,
  );
});

test("evaluation plan accepts a registered 2m per-assignment safety ceiling", () => {
  const draft = registeredStageA();
  const amended = rootEvaluationPlan({
    ...draft,
    tasks: draft.tasks.map((task) => ({
      ...task,
      max_observed_tokens: 2_000_000,
    })),
    budgets: {
      ...draft.budgets,
      max_total_observed_tokens: 24_000_000,
    },
    amends_root: draft.plan_root,
    amendment_reason:
      "Registered 100k, 200k, and 300k ceilings stopped the first cell before candidate ingestion; preserve them and run fresh assignments under a 2m safety ceiling.",
    plan_root: "",
  });
  assert.deepEqual(parseEvaluationPlan(amended), amended);
});

test("evaluation plan rejects authority and post-output retry paths", () => {
  const draft = plan();
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      retry_policy: {
        max_pre_output_infrastructure_retries: 1,
        post_output_retries: 1,
      },
    })),
    /one pre-output retry and no post-output retry/u,
  );
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      custody: { ...draft.custody, repository_authority: "allowed" },
    })),
    /repository_authority must be forbidden/u,
  );
});

test("evaluation plan closes candidate artifact and verifier invocation", () => {
  const draft = plan();
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      tasks: [{
        ...draft.tasks[0],
        verifier_args: ["--candidate", "result.txt"],
      }],
    })),
    /exactly one \{artifact\}/u,
  );
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      tasks: [{
        ...draft.tasks[0],
        artifact_path: "../result.txt",
      }],
    })),
    /safe relative POSIX path/u,
  );
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      tasks: [{
        ...draft.tasks[0],
        verifier_args: ["{artifact}", "{oracle}"],
      }],
    })),
    /unsupported placeholder/u,
  );
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      arms: [{
        ...draft.arms[0],
        argv: ["codex", "{wrapper}", "{resource:missing}"],
      }],
    })),
    /unsupported placeholder \{resource:missing\}/u,
  );
});

test("registered Stage A requires both task classes and all matched controls", () => {
  const registered = registeredStageA();
  assert.deepEqual(parseEvaluationPlan(registered), registered);
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...registered,
      arms: registered.arms.filter((arm) => arm.kind !== "native_codex_packet"),
      assignments: registered.assignments.filter((assignment) =>
        assignment.arm_id !== "native-packet"),
      budgets: {
        max_model_calls: 8,
        max_total_wall_time_ms: 8 * 60_000,
        max_total_observed_tokens: 8 * 10_000,
      },
    })),
    /same-packet native Codex/u,
  );
});

test("evaluation plan rejects duplicate identities and underfunded totals", () => {
  const draft = plan();
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      tasks: [...draft.tasks, draft.tasks[0]],
    })),
    /duplicate ids/u,
  );
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      budgets: {
        ...draft.budgets,
        max_total_observed_tokens: draft.budgets.max_total_observed_tokens - 1,
      },
    })),
    /token ceilings exceed/u,
  );
});

test("evaluation plan caps calls at 36", () => {
  const draft = plan();
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      assignments: Array.from({ length: 37 }, (_, index) => ({
        ...draft.assignments[0],
        id: `assignment-${index}`,
        repetition: (index % 16) + 1,
        seed: index,
      })),
      budgets: {
        max_model_calls: 36,
        max_total_wall_time_ms: 3_600_000,
        max_total_observed_tokens: 1_000_000,
      },
    })),
    /1\.\.36 entries/u,
  );
});

test("evaluation plan binds distinct execution, state, and inheritance scorers", () => {
  const draft = plan();
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      performance_functions: {
        ...draft.performance_functions,
        inheritance_lift: draft.performance_functions.state_lift,
      },
    })),
    /three distinct scorer roots/u,
  );
  assert.throws(
    () => parseEvaluationPlan(rootEvaluationPlan({
      ...draft,
      performance_functions: {
        ...draft.performance_functions,
        inheritance_lift: root("f"),
      },
    })),
    /absent from plan\.scorers/u,
  );
});

test("evaluation validation rehashes every bound input file", async () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "canopus-evaluation-"));
  try {
    const files = {
      "sources/math.json": "{\"source\":\"math\"}\n",
      "packets/math.json": "{}\n",
      "verifiers/math": "#!/bin/sh\nexit 0\n",
      "wrappers/native.mjs": "#!/usr/bin/env node\nprocess.exit(0);\n",
      "locks/native.lock": "lock\n",
      "environments/native.json": "{}\n",
      "scorers/execution.json": "{\"metric\":\"execution\"}\n",
      "scorers/state.json": "{\"metric\":\"state\"}\n",
      "scorers/inheritance.json": "{\"metric\":\"inheritance\"}\n",
    };
    for (const [name, content] of Object.entries(files)) {
      const target = path.join(directory, name);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    chmodSync(path.join(directory, "verifiers/math"), 0o700);
    mkdirSync(path.join(directory, "workspace"));
    const draft = plan();
    const rooted = rootEvaluationPlan({
      ...draft,
      tasks: [{
        ...draft.tasks[0],
        source_root: byteRoot(files["sources/math.json"]),
        packet_root: byteRoot(files["packets/math.json"]),
        verifier_root: byteRoot(files["verifiers/math"]),
        verifier_runtime_root: byteRoot(files["verifiers/math"]),
      }],
      arms: [{
        ...draft.arms[0],
        argv: [process.execPath, "{wrapper}"],
        wrapper_root: byteRoot(files["wrappers/native.mjs"]),
        dependency_lock_root: byteRoot(files["locks/native.lock"]),
        environment_root: byteRoot(files["environments/native.json"]),
        executable_root: byteRoot(readFileSync(process.execPath)),
      }],
      scorers: draft.scorers.map((entry) => ({
        ...entry,
        root: byteRoot(files[entry.path]),
      })),
      performance_functions: {
        execution_lift: byteRoot(files["scorers/execution.json"]),
        state_lift: byteRoot(files["scorers/state.json"]),
        inheritance_lift: byteRoot(files["scorers/inheritance.json"]),
      },
    });
    const planFile = path.join(directory, "plan.json");
    writeFileSync(planFile, `${JSON.stringify(rooted)}\n`);
    const verified = await verifyEvaluationPlanFiles(rooted, planFile);
    assert.equal(verified.verified_files, 10);
    writeFileSync(path.join(directory, "verifiers/math"), "drift\n");
    await assert.rejects(
      verifyEvaluationPlanFiles(rooted, planFile),
      /verifier root drifted/u,
    );
    writeFileSync(path.join(directory, "verifiers/math"), files["verifiers/math"]);
    writeFileSync(path.join(directory, "wrappers/native.mjs"), "drift\n");
    await assert.rejects(
      verifyEvaluationPlanFiles(rooted, planFile),
      /wrapper root drifted/u,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("evaluation runner preserves every registered result after process failures", () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "canopus-evaluation-run-"));
  try {
    const files = {
      "sources/math.json": "{\"source\":\"math\"}\n",
      "sources/scientific.json": "{\"source\":\"scientific\"}\n",
      "packets/math.json": "{\"task\":\"math\"}\n",
      "packets/scientific.json": "{\"task\":\"scientific\"}\n",
      "verifiers/math": "#!/bin/sh\nexit 0\n",
      "verifiers/scientific": "#!/bin/sh\nexit 0\n",
      "wrappers/native.mjs": "#!/usr/bin/env node\nprocess.exit(0);\n",
      "locks/native.lock": "lock\n",
      "environments/native.json": "{}\n",
      "scorers/execution.json": "{\"metric\":\"execution\"}\n",
      "scorers/state.json": "{\"metric\":\"state\"}\n",
      "scorers/inheritance.json": "{\"metric\":\"inheritance\"}\n",
      "arm-wrapper.mjs": [
        "import { mkdirSync, writeFileSync } from 'node:fs';",
        "const [, , output, assignment, exitCode] = process.argv;",
        "mkdirSync(`${output}/artifacts`,{recursive:true});",
        "writeFileSync(`${output}/artifacts/result.txt`,'candidate\\n');",
        "writeFileSync(3,",
        "JSON.stringify({schema:'canopus.evaluation-arm-result.v1',",
        "assignment_id:assignment,model_output_observed:true,",
        "usage:{input_tokens:6,cached_input_tokens:2,output_tokens:4,",
        "reasoning_output_tokens:1}})+'\\n');",
        "if (exitCode === '0') process.stdout.write('ok\\n');",
        "process.exit(Number(exitCode));",
      ].join(""),
    };
    for (const [name, content] of Object.entries(files)) {
      const target = path.join(directory, name);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    chmodSync(path.join(directory, "verifiers/math"), 0o700);
    chmodSync(path.join(directory, "verifiers/scientific"), 0o700);
    mkdirSync(path.join(directory, "workspace"));
    const executableRoot = byteRoot(readFileSync(process.execPath));
    const base = registeredStageA();
    const rooted = rootEvaluationPlan({
      ...base,
      tasks: base.tasks.map((task) => ({
        ...task,
        source_root: byteRoot(files[task.source_path]),
        packet_root: byteRoot(files[task.packet_path]),
        verifier_root: byteRoot(files[task.verifier_path]),
        verifier_runtime_root: byteRoot(files[task.verifier_path]),
      })),
      arms: base.arms.map((arm) => ({
        ...arm,
        argv: [
          process.execPath,
          "{wrapper}",
          "{output}",
          "{assignment_id}",
          arm.kind === "native_codex" ? "7" : "0",
        ],
        wrapper_path: "arm-wrapper.mjs",
        wrapper_root: byteRoot(files["arm-wrapper.mjs"]),
        executable_root: executableRoot,
        dependency_lock_root: byteRoot(files[arm.dependency_lock_path]),
        environment_root: byteRoot(files[arm.environment_path]),
      })),
      scorers: base.scorers.map((entry) => ({
        ...entry,
        root: byteRoot(files[entry.path]),
      })),
      performance_functions: {
        execution_lift: byteRoot(files["scorers/execution.json"]),
        state_lift: byteRoot(files["scorers/state.json"]),
        inheritance_lift: byteRoot(files["scorers/inheritance.json"]),
      },
    });
    const planFile = path.join(directory, "plan.json");
    const output = path.join(directory, "runs");
    writeFileSync(planFile, `${JSON.stringify(rooted)}\n`);
    const run = spawnSync(
      process.execPath,
      [
        path.join(packageRoot, "evaluation/scripts/run-plan.mjs"),
        "--plan",
        planFile,
        "--stage",
        "A",
        "--output",
        output,
      ],
      {
        cwd: packageRoot,
        encoding: "utf8",
      },
    );
    assert.equal(run.status, 0, run.stderr);
    const index = JSON.parse(readFileSync(path.join(output, "index.json"), "utf8"));
    const firstRun = JSON.parse(readFileSync(
      path.join(output, index.registered_assignment_ids[0], "run.json"),
      "utf8",
    ));
    assert.equal(
      index.status,
      "complete",
      JSON.stringify({ index, first_run: firstRun }),
    );
    assert.equal(index.runs.length, 12);
    const report = spawnSync(
      process.execPath,
      [path.join(packageRoot, "evaluation/scripts/report.mjs"), output],
      {
        cwd: packageRoot,
        encoding: "utf8",
      },
    );
    assert.equal(report.status, 0, report.stderr);
    const summary = JSON.parse(readFileSync(path.join(output, "report.json"), "utf8"));
    assert.equal(summary.registered, 12);
    assert.equal(summary.runs, 12);
    assert.equal(summary.completed, 8);
    assert.equal(summary.failed, 4);
    assert.equal(summary.verifier_passed, 8);
    assert.equal(summary.verifier_failed, 0);
    assert.equal(summary.verifier_not_run, 4);
    assert.equal(summary.observed_tokens, 120);
    assert.deepEqual(summary.unmeasured_token_runs, []);
    assert.deepEqual(summary.missing_assignment_ids, []);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("evaluation runner rejects worker-created arm result files", () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "canopus-evaluation-hostile-"));
  try {
    const files = {
      "sources/math.json": "{\"source\":\"math\"}\n",
      "sources/scientific.json": "{\"source\":\"scientific\"}\n",
      "packets/math.json": "{\"task\":\"math\"}\n",
      "packets/scientific.json": "{\"task\":\"scientific\"}\n",
      "verifiers/math": "#!/bin/sh\nexit 0\n",
      "verifiers/scientific": "#!/bin/sh\nexit 0\n",
      "locks/native.lock": "lock\n",
      "environments/native.json": "{}\n",
      "scorers/execution.json": "{\"metric\":\"execution\"}\n",
      "scorers/state.json": "{\"metric\":\"state\"}\n",
      "scorers/inheritance.json": "{\"metric\":\"inheritance\"}\n",
      "hostile-wrapper.mjs": [
        "import { writeFileSync } from 'node:fs';",
        "const [, , output, assignment] = process.argv;",
        "const result=JSON.stringify({schema:'canopus.evaluation-arm-result.v1',",
        "assignment_id:assignment,model_output_observed:true,",
        "usage:{input_tokens:6,cached_input_tokens:2,output_tokens:4,",
        "reasoning_output_tokens:1}})+'\\n';",
        "writeFileSync(`${output}/arm-result.json`,result);",
        "writeFileSync(3,result);",
      ].join(""),
    };
    for (const [name, content] of Object.entries(files)) {
      const target = path.join(directory, name);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    }
    chmodSync(path.join(directory, "verifiers/math"), 0o700);
    chmodSync(path.join(directory, "verifiers/scientific"), 0o700);
    mkdirSync(path.join(directory, "workspace"));
    const executableRoot = byteRoot(readFileSync(process.execPath));
    const base = registeredStageA();
    const rooted = rootEvaluationPlan({
      ...base,
      tasks: base.tasks.map((task) => ({
        ...task,
        source_root: byteRoot(files[task.source_path]),
        packet_root: byteRoot(files[task.packet_path]),
        verifier_root: byteRoot(files[task.verifier_path]),
        verifier_runtime_root: byteRoot(files[task.verifier_path]),
      })),
      arms: base.arms.map((arm) => ({
        ...arm,
        argv: [
          process.execPath,
          "{wrapper}",
          "{output}",
          "{assignment_id}",
        ],
        wrapper_path: "hostile-wrapper.mjs",
        wrapper_root: byteRoot(files["hostile-wrapper.mjs"]),
        executable_root: executableRoot,
        dependency_lock_root: byteRoot(files[arm.dependency_lock_path]),
        environment_root: byteRoot(files[arm.environment_path]),
      })),
      scorers: base.scorers.map((entry) => ({
        ...entry,
        root: byteRoot(files[entry.path]),
      })),
      performance_functions: {
        execution_lift: byteRoot(files["scorers/execution.json"]),
        state_lift: byteRoot(files["scorers/state.json"]),
        inheritance_lift: byteRoot(files["scorers/inheritance.json"]),
      },
    });
    const planFile = path.join(directory, "plan.json");
    const output = path.join(directory, "runs");
    writeFileSync(planFile, `${JSON.stringify(rooted)}\n`);
    const run = spawnSync(
      process.execPath,
      [
        path.join(packageRoot, "evaluation/scripts/run-plan.mjs"),
        "--plan",
        planFile,
        "--stage",
        "A",
        "--output",
        output,
      ],
      {
        cwd: packageRoot,
        encoding: "utf8",
      },
    );
    assert.equal(run.status, 0, run.stderr);
    const index = JSON.parse(readFileSync(path.join(output, "index.json"), "utf8"));
    assert.equal(index.status, "stopped");
    assert.match(index.stop_reason, /did not produce valid rooted token usage/u);
    assert.equal(index.runs.length, 1);
    assert.equal(index.registered_assignment_ids.length, 12);
    const firstRun = JSON.parse(readFileSync(
      path.join(output, index.registered_assignment_ids[0], "run.json"),
      "utf8",
    ));
    assert.equal(firstRun.arm_result_root, null);
    assert.equal(firstRun.usage, null);
    assert.match(firstRun.runner_error, /EEXIST|file already exists/u);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
