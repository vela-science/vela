import { createHash } from "node:crypto";
import { constants } from "node:fs";
import { access, lstat, readFile, realpath } from "node:fs/promises";
import path from "node:path";

export const EVALUATION_PLAN_SCHEMA = "canopus.evaluation-plan.v1";
export const EVALUATION_ARM_RESULT_SCHEMA = "canopus.evaluation-arm-result.v1";
export const EVALUATION_RUN_SCHEMA = "canopus.evaluation-run.v1";
export const EVALUATION_REPORT_SCHEMA = "canopus.evaluation-report.v1";
export const SHA256 = /^sha256:[0-9a-f]{64}$/u;

function canonical(value) {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  return `{${Object.keys(value).sort().map((key) =>
    `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
}

export function digest(value) {
  return `sha256:${createHash("sha256").update(canonical(value)).digest("hex")}`;
}

function object(value, at) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${at} must be an object`);
  }
  return value;
}

function exactKeys(value, required, optional, at) {
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) throw new Error(`${at}.${key} is not allowed`);
  }
  for (const key of required) {
    if (!(key in value)) throw new Error(`${at}.${key} is required`);
  }
}

function text(value, at, max = 4096) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > max ||
    value !== value.trim()
  ) {
    throw new Error(`${at} must be nonempty trimmed text`);
  }
  return value;
}

function root(value, at) {
  if (typeof value !== "string" || !SHA256.test(value)) {
    throw new Error(`${at} must be a full lowercase sha256 root`);
  }
  return value;
}

function integer(value, at, min, max) {
  if (!Number.isSafeInteger(value) || value < min || value > max) {
    throw new Error(`${at} must be an integer in ${min}..${max}`);
  }
  return value;
}

function boolean(value, at) {
  if (typeof value !== "boolean") throw new Error(`${at} must be boolean`);
  return value;
}

function nullableText(value, at, max = 4096) {
  return value === null ? null : text(value, at, max);
}

function timestamp(value, at) {
  const candidate = text(value, at, 64);
  const parsed = new Date(candidate);
  if (Number.isNaN(parsed.valueOf())) {
    throw new Error(`${at} must be a canonical RFC3339 UTC timestamp`);
  }
  const normalized = parsed.toISOString();
  if (
    !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u.test(candidate) ||
    ![normalized, normalized.replace(".000Z", "Z")].includes(candidate)
  ) {
    throw new Error(`${at} must be a canonical RFC3339 UTC timestamp`);
  }
  return candidate;
}

function array(value, at, min, max, parse) {
  if (!Array.isArray(value) || value.length < min || value.length > max) {
    throw new Error(`${at} must contain ${min}..${max} entries`);
  }
  return value.map((entry, index) => parse(entry, `${at}[${index}]`));
}

function relative(value, at) {
  const candidate = text(value, at, 512);
  if (
    candidate.startsWith("/") ||
    candidate.includes("\\") ||
    candidate.split("/").some((segment) => segment === "" || segment === "." || segment === "..")
  ) {
    throw new Error(`${at} must be a safe relative POSIX path`);
  }
  return candidate;
}

function parseIdentity(value, at) {
  const item = object(value, at);
  exactKeys(item, ["name", "version", "sha256"], [], at);
  return {
    name: text(item.name, `${at}.name`, 128),
    version: text(item.version, `${at}.version`, 256),
    sha256: root(item.sha256, `${at}.sha256`),
  };
}

function parseTask(value, at) {
  const item = object(value, at);
  exactKeys(
    item,
    [
      "id", "class", "source", "source_path", "source_root", "packet_path", "packet_root",
      "verifier_path", "verifier_root", "verifier_runtime", "verifier_runtime_root",
      "verifier_args", "verifier_resources", "artifact_path",
      "max_artifact_bytes", "license", "cpu_only", "network",
      "max_wall_time_ms", "max_observed_tokens",
    ],
    [],
    at,
  );
  if (!["math", "scientific_computing"].includes(item.class)) {
    throw new Error(`${at}.class is unsupported`);
  }
  if (item.cpu_only !== true || item.network !== "deny") {
    throw new Error(`${at} must be CPU-only and network-denied`);
  }
  const verifierResources = array(
    item.verifier_resources,
    `${at}.verifier_resources`,
    0,
    16,
    (entry, entryAt) => {
      const resource = object(entry, entryAt);
      exactKeys(resource, ["name", "path", "root"], [], entryAt);
      const name = text(resource.name, `${entryAt}.name`, 64);
      if (!/^[a-z][a-z0-9_]*$/u.test(name)) {
        throw new Error(`${entryAt}.name must be a lowercase resource name`);
      }
      return {
        name,
        path: relative(resource.path, `${entryAt}.path`),
        root: root(resource.root, `${entryAt}.root`),
      };
    },
  );
  if (
    new Set(verifierResources.map((resource) => resource.name)).size !==
      verifierResources.length
  ) {
    throw new Error(`${at}.verifier_resources contains duplicate names`);
  }
  const verifierArgs = array(
    item.verifier_args,
    `${at}.verifier_args`,
    1,
    32,
    (entry, entryAt) => text(entry, entryAt, 4096),
  );
  if (verifierArgs.filter((entry) => entry === "{artifact}").length !== 1) {
    throw new Error(`${at}.verifier_args must contain exactly one {artifact}`);
  }
  if (verifierArgs.filter((entry) => entry === "{source}").length > 1) {
    throw new Error(`${at}.verifier_args may contain at most one {source}`);
  }
  for (const entry of verifierArgs) {
    for (const placeholder of entry.match(/\{[^{}]+\}/gu) ?? []) {
      if (["{artifact}", "{source}"].includes(placeholder)) continue;
      const match = placeholder.match(/^\{resource:([a-z][a-z0-9_]*)\}$/u);
      if (
        match?.[1] === undefined ||
        !verifierResources.some((resource) => resource.name === match[1])
      ) {
        throw new Error(
          `${at}.verifier_args contains unsupported placeholder ${placeholder}`,
        );
      }
    }
  }
  if (!["direct", "bun"].includes(item.verifier_runtime)) {
    throw new Error(`${at}.verifier_runtime is unsupported`);
  }
  const verifierRoot = root(item.verifier_root, `${at}.verifier_root`);
  const verifierRuntimeRoot = root(
    item.verifier_runtime_root,
    `${at}.verifier_runtime_root`,
  );
  if (item.verifier_runtime === "direct" && verifierRuntimeRoot !== verifierRoot) {
    throw new Error(`${at} direct verifier runtime must equal its verifier root`);
  }
  return {
    id: text(item.id, `${at}.id`, 256),
    class: item.class,
    source: text(item.source, `${at}.source`, 512),
    source_path: relative(item.source_path, `${at}.source_path`),
    source_root: root(item.source_root, `${at}.source_root`),
    packet_path: relative(item.packet_path, `${at}.packet_path`),
    packet_root: root(item.packet_root, `${at}.packet_root`),
    verifier_path: relative(item.verifier_path, `${at}.verifier_path`),
    verifier_root: verifierRoot,
    verifier_runtime: item.verifier_runtime,
    verifier_runtime_root: verifierRuntimeRoot,
    verifier_args: verifierArgs,
    verifier_resources: verifierResources,
    artifact_path: relative(item.artifact_path, `${at}.artifact_path`),
    max_artifact_bytes: integer(
      item.max_artifact_bytes,
      `${at}.max_artifact_bytes`,
      1,
      64 * 1024 * 1024,
    ),
    license: text(item.license, `${at}.license`, 128),
    cpu_only: true,
    network: "deny",
    max_wall_time_ms: integer(
      item.max_wall_time_ms,
      `${at}.max_wall_time_ms`,
      1_000,
      7_200_000,
    ),
    max_observed_tokens: integer(
      item.max_observed_tokens,
      `${at}.max_observed_tokens`,
      1,
      2_000_000,
    ),
  };
}

function parseArm(value, at) {
  const item = object(value, at);
  exactKeys(
    item,
    [
      "id", "kind", "argv", "cwd", "wrapper_path", "wrapper_root", "dependency_lock_path",
      "dependency_lock_root", "environment_path", "environment_root",
      "executable_root", "resources",
    ],
    [],
    at,
  );
  if (
    ![
      "native_codex",
      "native_codex_packet",
      "canopus",
      "plain_typescript",
      "langgraph",
      "openai_agents",
    ].includes(item.kind)
  ) {
    throw new Error(`${at}.kind is unsupported`);
  }
  const resources = array(item.resources, `${at}.resources`, 0, 16, (entry, entryAt) => {
    const resource = object(entry, entryAt);
    exactKeys(resource, ["name", "path", "root"], [], entryAt);
    const name = text(resource.name, `${entryAt}.name`, 64);
    if (!/^[a-z][a-z0-9_]*$/u.test(name)) {
      throw new Error(`${entryAt}.name must be a lowercase resource name`);
    }
    return {
      name,
      path: relative(resource.path, `${entryAt}.path`),
      root: root(resource.root, `${entryAt}.root`),
    };
  });
  if (new Set(resources.map((resource) => resource.name)).size !== resources.length) {
    throw new Error(`${at}.resources contains duplicate names`);
  }
  const argv = array(item.argv, `${at}.argv`, 1, 64, (entry, entryAt) =>
    text(entry, entryAt, 4096));
  if (argv.filter((entry) => entry === "{wrapper}").length !== 1) {
    throw new Error(`${at}.argv must contain exactly one {wrapper} control entrypoint`);
  }
  const standardPlaceholders = new Set([
    "{task_packet}",
    "{wrapper}",
    "{output}",
    "{assignment_id}",
    "{seed}",
    "{dependency_lock}",
    "{environment}",
  ]);
  for (const entry of argv) {
    for (const placeholder of entry.match(/\{[^{}]+\}/gu) ?? []) {
      if (standardPlaceholders.has(placeholder)) continue;
      const match = placeholder.match(/^\{resource:([a-z][a-z0-9_]*)\}$/u);
      if (
        match?.[1] === undefined ||
        !resources.some((resource) => resource.name === match[1])
      ) {
        throw new Error(`${at}.argv contains unsupported placeholder ${placeholder}`);
      }
    }
  }
  return {
    id: text(item.id, `${at}.id`, 128),
    kind: item.kind,
    argv,
    cwd: relative(item.cwd, `${at}.cwd`),
    wrapper_path: relative(item.wrapper_path, `${at}.wrapper_path`),
    wrapper_root: root(item.wrapper_root, `${at}.wrapper_root`),
    dependency_lock_path: relative(
      item.dependency_lock_path,
      `${at}.dependency_lock_path`,
    ),
    dependency_lock_root: root(item.dependency_lock_root, `${at}.dependency_lock_root`),
    environment_path: relative(item.environment_path, `${at}.environment_path`),
    environment_root: root(item.environment_root, `${at}.environment_root`),
    executable_root: root(item.executable_root, `${at}.executable_root`),
    resources,
  };
}

function parseScorer(value, at) {
  const item = object(value, at);
  exactKeys(item, ["id", "path", "root"], [], at);
  return {
    id: text(item.id, `${at}.id`, 128),
    path: relative(item.path, `${at}.path`),
    root: root(item.root, `${at}.root`),
  };
}

function parseAssignment(value, at) {
  const item = object(value, at);
  exactKeys(item, ["id", "stage", "task_id", "arm_id", "repetition", "seed"], [], at);
  if (!["A", "B", "C"].includes(item.stage)) throw new Error(`${at}.stage is unsupported`);
  return {
    id: text(item.id, `${at}.id`, 128),
    stage: item.stage,
    task_id: text(item.task_id, `${at}.task_id`, 256),
    arm_id: text(item.arm_id, `${at}.arm_id`, 128),
    repetition: integer(item.repetition, `${at}.repetition`, 1, 16),
    seed: integer(item.seed, `${at}.seed`, 0, 2_147_483_647),
  };
}

function parseUsage(value, at) {
  const usage = object(value, at);
  exactKeys(
    usage,
    [
      "input_tokens", "cached_input_tokens", "output_tokens",
      "reasoning_output_tokens",
    ],
    [],
    at,
  );
  return {
    input_tokens: integer(usage.input_tokens, `${at}.input_tokens`, 0, 1_000_000_000),
    cached_input_tokens: integer(
      usage.cached_input_tokens,
      `${at}.cached_input_tokens`,
      0,
      1_000_000_000,
    ),
    output_tokens: integer(
      usage.output_tokens,
      `${at}.output_tokens`,
      0,
      1_000_000_000,
    ),
    reasoning_output_tokens: integer(
      usage.reasoning_output_tokens,
      `${at}.reasoning_output_tokens`,
      0,
      1_000_000_000,
    ),
  };
}

export function parseEvaluationArmResult(value) {
  const result = object(value, "arm_result");
  exactKeys(
    result,
    ["schema", "assignment_id", "model_output_observed", "usage"],
    [],
    "arm_result",
  );
  if (result.schema !== EVALUATION_ARM_RESULT_SCHEMA) {
    throw new Error("arm_result.schema is unsupported");
  }
  return {
    schema: EVALUATION_ARM_RESULT_SCHEMA,
    assignment_id: text(result.assignment_id, "arm_result.assignment_id", 128),
    model_output_observed: boolean(
      result.model_output_observed,
      "arm_result.model_output_observed",
    ),
    usage: parseUsage(result.usage, "arm_result.usage"),
  };
}

export function parseEvaluationPlan(value) {
  const plan = object(value, "plan");
  exactKeys(
    plan,
    [
      "schema", "plan_id", "status", "created_at", "campaign", "identities",
      "tasks", "arms", "assignments", "budgets", "retry_policy",
      "stopping_rules", "scorers", "performance_functions", "exclusions",
      "custody", "publication",
      "amends_root", "amendment_reason", "plan_root",
    ],
    [],
    "plan",
  );
  if (plan.schema !== EVALUATION_PLAN_SCHEMA) throw new Error("plan.schema is unsupported");
  if (!["draft", "registered", "stopped", "complete"].includes(plan.status)) {
    throw new Error("plan.status is unsupported");
  }
  text(plan.plan_id, "plan.plan_id", 128);
  timestamp(plan.created_at, "plan.created_at");
  text(plan.campaign, "plan.campaign", 4096);
  const identities = object(plan.identities, "plan.identities");
  exactKeys(
    identities,
    ["model", "codex", "canopus", "vela", "git", "environment", "dependencies"],
    [],
    "plan.identities",
  );
  const tasks = array(plan.tasks, "plan.tasks", 1, 8, parseTask);
  const arms = array(plan.arms, "plan.arms", 1, 8, parseArm);
  const assignments = array(plan.assignments, "plan.assignments", 1, 36, parseAssignment);
  const taskIds = new Set(tasks.map((task) => task.id));
  const armIds = new Set(arms.map((arm) => arm.id));
  if (taskIds.size !== tasks.length) throw new Error("plan.tasks contains duplicate ids");
  if (armIds.size !== arms.length) throw new Error("plan.arms contains duplicate ids");
  const assignmentIds = new Set();
  const assignmentTuples = new Set();
  for (const assignment of assignments) {
    if (!taskIds.has(assignment.task_id)) {
      throw new Error(`assignment ${assignment.id} names an unknown task`);
    }
    if (!armIds.has(assignment.arm_id)) {
      throw new Error(`assignment ${assignment.id} names an unknown arm`);
    }
    if (assignmentIds.has(assignment.id)) {
      throw new Error(`duplicate assignment ${assignment.id}`);
    }
    assignmentIds.add(assignment.id);
    const tuple = [
      assignment.stage,
      assignment.task_id,
      assignment.arm_id,
      assignment.repetition,
    ].join("/");
    if (assignmentTuples.has(tuple)) {
      throw new Error(`duplicate assignment tuple ${tuple}`);
    }
    assignmentTuples.add(tuple);
  }
  const budgets = object(plan.budgets, "plan.budgets");
  exactKeys(
    budgets,
    ["max_model_calls", "max_total_wall_time_ms", "max_total_observed_tokens"],
    [],
    "plan.budgets",
  );
  integer(budgets.max_model_calls, "plan.budgets.max_model_calls", 1, 36);
  integer(
    budgets.max_total_wall_time_ms,
    "plan.budgets.max_total_wall_time_ms",
    1_000,
    86_400_000,
  );
  integer(
    budgets.max_total_observed_tokens,
    "plan.budgets.max_total_observed_tokens",
    1,
    24_000_000,
  );
  if (assignments.length > budgets.max_model_calls) {
    throw new Error("assignments exceed the registered model-call budget");
  }
  const taskById = new Map(tasks.map((task) => [task.id, task]));
  const assignedWallTime = assignments.reduce(
    (sum, assignment) => sum + taskById.get(assignment.task_id).max_wall_time_ms,
    0,
  );
  const assignedTokens = assignments.reduce(
    (sum, assignment) => sum + taskById.get(assignment.task_id).max_observed_tokens,
    0,
  );
  if (assignedWallTime > budgets.max_total_wall_time_ms) {
    throw new Error("assignment wall-time ceilings exceed the registered total budget");
  }
  if (assignedTokens > budgets.max_total_observed_tokens) {
    throw new Error("assignment token ceilings exceed the registered total budget");
  }
  if (plan.status === "registered") {
    const stageA = assignments.filter((assignment) => assignment.stage === "A");
    if (stageA.length > 0) {
      const stageATaskIds = new Set(stageA.map((assignment) => assignment.task_id));
      const stageATasks = tasks.filter((task) => stageATaskIds.has(task.id));
      const stageAArmIds = new Set(stageA.map((assignment) => assignment.arm_id));
      const stageAArms = arms.filter((arm) => stageAArmIds.has(arm.id));
      if (
        stageATasks.length !== 2 ||
        new Set(stageATasks.map((task) => task.class)).size !== 2 ||
        !stageATasks.some((task) => task.class === "math") ||
        !stageATasks.some((task) => task.class === "scientific_computing")
      ) {
        throw new Error(
          "registered Stage A requires one math and one scientific-computing task",
        );
      }
      const requiredKinds = new Set([
        "native_codex",
        "native_codex_packet",
        "canopus",
      ]);
      if (
        stageAArms.length !== requiredKinds.size ||
        stageAArms.some((arm) => !requiredKinds.has(arm.kind))
      ) {
        throw new Error(
          "registered Stage A requires native Codex, same-packet native Codex, and Canopus",
        );
      }
      for (const task of stageATasks) {
        for (const arm of stageAArms) {
          const repetitions = stageA
            .filter((assignment) =>
              assignment.task_id === task.id && assignment.arm_id === arm.id)
            .map((assignment) => assignment.repetition)
            .sort((left, right) => left - right);
          if (repetitions.length !== 2 || repetitions[0] !== 1 || repetitions[1] !== 2) {
            throw new Error(
              `registered Stage A requires repetitions 1 and 2 for ${task.id}/${arm.id}`,
            );
          }
        }
      }
      if (stageA.length !== 12) {
        throw new Error("registered Stage A requires exactly 12 assignments");
      }
    }
  }
  const retry = object(plan.retry_policy, "plan.retry_policy");
  exactKeys(
    retry,
    ["max_pre_output_infrastructure_retries", "post_output_retries"],
    [],
    "plan.retry_policy",
  );
  if (
    retry.max_pre_output_infrastructure_retries !== 1 ||
    retry.post_output_retries !== 0
  ) {
    throw new Error("retry policy must allow one pre-output retry and no post-output retry");
  }
  const custody = object(plan.custody, "plan.custody");
  exactKeys(
    custody,
    [
      "human_keys", "repository_authority", "secrets_in_records",
      "chain_of_thought_in_traces", "canonical_mutation",
    ],
    [],
    "plan.custody",
  );
  for (const key of Object.keys(custody)) {
    if (custody[key] !== "forbidden") {
      throw new Error(`plan.custody.${key} must be forbidden`);
    }
  }
  const publication = object(plan.publication, "plan.publication");
  exactKeys(publication, ["raw_failures", "exclusions", "roots", "independence_credit"], [], "plan.publication");
  if (
    publication.raw_failures !== "required" ||
    publication.exclusions !== "required" ||
    publication.roots !== "required" ||
    publication.independence_credit !== "none_first_party"
  ) {
    throw new Error("plan publication contract is not evidence-complete");
  }
  array(plan.stopping_rules, "plan.stopping_rules", 1, 32, (entry, at) => text(entry, at));
  const scorers = array(
    plan.scorers,
    "plan.scorers",
    3,
    16,
    parseScorer,
  );
  if (new Set(scorers.map((scorer) => scorer.id)).size !== scorers.length) {
    throw new Error("plan.scorers contains duplicate ids");
  }
  if (new Set(scorers.map((scorer) => scorer.root)).size !== scorers.length) {
    throw new Error("plan.scorers contains duplicate roots");
  }
  const performanceFunctions = object(
    plan.performance_functions,
    "plan.performance_functions",
  );
  exactKeys(
    performanceFunctions,
    ["execution_lift", "state_lift", "inheritance_lift"],
    [],
    "plan.performance_functions",
  );
  const performanceRoots = [
    root(
      performanceFunctions.execution_lift,
      "plan.performance_functions.execution_lift",
    ),
    root(
      performanceFunctions.state_lift,
      "plan.performance_functions.state_lift",
    ),
    root(
      performanceFunctions.inheritance_lift,
      "plan.performance_functions.inheritance_lift",
    ),
  ];
  if (new Set(performanceRoots).size !== performanceRoots.length) {
    throw new Error("plan performance functions must use three distinct scorer roots");
  }
  for (const performanceRoot of performanceRoots) {
    if (!scorers.some((scorer) => scorer.root === performanceRoot)) {
      throw new Error(
        `plan performance function ${performanceRoot} is absent from plan.scorers`,
      );
    }
  }
  array(plan.exclusions, "plan.exclusions", 0, 32, (entry, at) => text(entry, at));
  parseIdentity(identities.model, "plan.identities.model");
  for (const key of ["codex", "canopus", "vela", "git", "environment"]) {
    parseIdentity(identities[key], `plan.identities.${key}`);
  }
  const dependencies = array(
    identities.dependencies,
    "plan.identities.dependencies",
    0,
    32,
    parseIdentity,
  );
  if (new Set(dependencies.map((dependency) => dependency.name)).size !== dependencies.length) {
    throw new Error("plan.identities.dependencies contains duplicate names");
  }
  if (plan.amends_root === null) {
    if (plan.amendment_reason !== null) {
      throw new Error("an original plan cannot have an amendment reason");
    }
  } else {
    root(plan.amends_root, "plan.amends_root");
    text(plan.amendment_reason, "plan.amendment_reason");
  }
  const rooted = { ...plan, plan_root: "" };
  const expected = digest(rooted);
  if (plan.plan_root !== expected) {
    throw new Error(`plan root mismatch: expected ${expected}, observed ${String(plan.plan_root)}`);
  }
  return plan;
}

export function rootEvaluationPlan(value) {
  const rooted = { ...value, plan_root: "" };
  return { ...rooted, plan_root: digest(rooted) };
}

export function parseEvaluationRun(value) {
  const record = object(value, "run");
  exactKeys(
    record,
    [
      "schema", "run_id", "plan_root", "assignment", "task_root", "arm_root",
      "started_at", "ended_at", "wall_time_ms", "exit_code", "signal",
      "stdout_root", "stderr_root", "model_output_observed", "timed_out",
      "runner_error", "retry_of", "authority_effect", "arm_result_root",
      "usage", "observed_tokens", "artifact_root", "producer_wall_time_ms",
      "verifier",
    ],
    [],
    "run",
  );
  if (record.schema !== EVALUATION_RUN_SCHEMA) {
    throw new Error("run.schema is unsupported");
  }
  const startedAt = timestamp(record.started_at, "run.started_at");
  const endedAt = timestamp(record.ended_at, "run.ended_at");
  if (endedAt < startedAt) throw new Error("run ended before it started");
  if (
    record.exit_code !== null &&
    (!Number.isSafeInteger(record.exit_code) || record.exit_code < 0 || record.exit_code > 255)
  ) {
    throw new Error("run.exit_code must be null or an integer in 0..255");
  }
  if (record.authority_effect !== "none") {
    throw new Error("run.authority_effect must be none");
  }
  const hasUsage = record.usage !== null;
  if (
    hasUsage !== (record.arm_result_root !== null) ||
    hasUsage !== (record.observed_tokens !== null)
  ) {
    throw new Error(
      "run usage, observed_tokens, and arm_result_root must be present together",
    );
  }
  const usage = hasUsage ? parseUsage(record.usage, "run.usage") : null;
  const observedTokens = hasUsage
    ? integer(record.observed_tokens, "run.observed_tokens", 0, 1_000_000_000)
    : null;
  if (
    usage !== null &&
    observedTokens !== usage.input_tokens + usage.output_tokens
  ) {
    throw new Error(
      "run.observed_tokens must equal input_tokens plus output_tokens",
    );
  }
  let verifier = null;
  if (record.verifier !== null) {
    const item = object(record.verifier, "run.verifier");
    exactKeys(
      item,
      [
        "outcome", "exit_code", "signal", "wall_time_ms", "stdout_root",
        "stderr_root", "error",
      ],
      [],
      "run.verifier",
    );
    if (!["pass", "fail"].includes(item.outcome)) {
      throw new Error("run.verifier.outcome is unsupported");
    }
    if (
      item.exit_code !== null &&
      (!Number.isSafeInteger(item.exit_code) || item.exit_code < 0 || item.exit_code > 255)
    ) {
      throw new Error("run.verifier.exit_code must be null or an integer in 0..255");
    }
    const error = nullableText(item.error, "run.verifier.error");
    if (
      (item.outcome === "pass" && (item.exit_code !== 0 || error !== null)) ||
      (item.outcome === "fail" && item.exit_code === 0 && error === null)
    ) {
      throw new Error("run.verifier outcome contradicts its process result");
    }
    verifier = {
      outcome: item.outcome,
      exit_code: item.exit_code,
      signal: nullableText(item.signal, "run.verifier.signal", 64),
      wall_time_ms: integer(
        item.wall_time_ms,
        "run.verifier.wall_time_ms",
        0,
        86_400_000,
      ),
      stdout_root: root(item.stdout_root, "run.verifier.stdout_root"),
      stderr_root: root(item.stderr_root, "run.verifier.stderr_root"),
      error,
    };
  }
  if (verifier?.outcome === "pass" && record.artifact_root === null) {
    throw new Error("passing run verifier requires an artifact_root");
  }
  return {
    schema: EVALUATION_RUN_SCHEMA,
    run_id: text(record.run_id, "run.run_id", 128),
    plan_root: root(record.plan_root, "run.plan_root"),
    assignment: parseAssignment(record.assignment, "run.assignment"),
    task_root: root(record.task_root, "run.task_root"),
    arm_root: root(record.arm_root, "run.arm_root"),
    started_at: startedAt,
    ended_at: endedAt,
    wall_time_ms: integer(record.wall_time_ms, "run.wall_time_ms", 0, 86_400_000),
    exit_code: record.exit_code,
    signal: nullableText(record.signal, "run.signal", 64),
    stdout_root: root(record.stdout_root, "run.stdout_root"),
    stderr_root: root(record.stderr_root, "run.stderr_root"),
    model_output_observed: boolean(
      record.model_output_observed,
      "run.model_output_observed",
    ),
    timed_out: boolean(record.timed_out, "run.timed_out"),
    runner_error: nullableText(record.runner_error, "run.runner_error"),
    retry_of: nullableText(record.retry_of, "run.retry_of", 128),
    authority_effect: "none",
    arm_result_root: record.arm_result_root === null
      ? null
      : root(record.arm_result_root, "run.arm_result_root"),
    usage,
    observed_tokens: observedTokens,
    artifact_root: record.artifact_root === null
      ? null
      : root(record.artifact_root, "run.artifact_root"),
    producer_wall_time_ms: integer(
      record.producer_wall_time_ms,
      "run.producer_wall_time_ms",
      0,
      86_400_000,
    ),
    verifier,
  };
}

export function parseEvaluationRunSet(value) {
  const index = object(value, "run_set");
  exactKeys(
    index,
    [
      "schema", "plan_root", "stage", "status", "stop_reason",
      "registered_assignment_ids", "runs",
    ],
    [],
    "run_set",
  );
  if (index.schema !== "canopus.evaluation-run-set.v1") {
    throw new Error("run_set.schema is unsupported");
  }
  if (!["complete", "stopped"].includes(index.status)) {
    throw new Error("run_set.status is unsupported");
  }
  if (!["A", "B", "C"].includes(index.stage)) {
    throw new Error("run_set.stage is unsupported");
  }
  const registeredAssignmentIds = array(
    index.registered_assignment_ids,
    "run_set.registered_assignment_ids",
    1,
    36,
    (entry, at) => text(entry, at, 128),
  );
  if (new Set(registeredAssignmentIds).size !== registeredAssignmentIds.length) {
    throw new Error("run_set.registered_assignment_ids contains duplicates");
  }
  const runs = array(index.runs, "run_set.runs", 1, 36, (entry, at) => {
    const item = object(entry, at);
    exactKeys(item, ["assignment_id", "run_root"], [], at);
    return {
      assignment_id: text(item.assignment_id, `${at}.assignment_id`, 128),
      run_root: root(item.run_root, `${at}.run_root`),
    };
  });
  if (new Set(runs.map((entry) => entry.assignment_id)).size !== runs.length) {
    throw new Error("run_set.runs contains duplicate assignments");
  }
  if (runs.some((entry) => !registeredAssignmentIds.includes(entry.assignment_id))) {
    throw new Error("run_set.runs contains an unregistered assignment");
  }
  const missing = registeredAssignmentIds.filter(
    (assignmentId) => !runs.some((entry) => entry.assignment_id === assignmentId),
  );
  if (index.status === "complete" && missing.length > 0) {
    throw new Error("complete run_set is missing registered assignments");
  }
  if (
    (index.status === "complete" && index.stop_reason !== null) ||
    (index.status === "stopped" && index.stop_reason === null)
  ) {
    throw new Error("run_set stop reason does not match its status");
  }
  return {
    schema: "canopus.evaluation-run-set.v1",
    plan_root: root(index.plan_root, "run_set.plan_root"),
    stage: index.stage,
    status: index.status,
    stop_reason: nullableText(index.stop_reason, "run_set.stop_reason"),
    registered_assignment_ids: registeredAssignmentIds,
    runs,
  };
}

export function canonicalJson(value) {
  return `${canonical(value)}\n`;
}

function byteRoot(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function remainsBelow(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative !== "" && !relative.startsWith(`..${path.sep}`) && relative !== ".." &&
    !path.isAbsolute(relative);
}

async function verifyFile(planDirectory, relativePath, expectedRoot, at) {
  const candidate = path.resolve(planDirectory, relativePath);
  const resolved = await realpath(candidate);
  if (!remainsBelow(planDirectory, resolved)) {
    throw new Error(`${at} escapes the registered plan directory`);
  }
  const metadata = await lstat(resolved);
  if (!metadata.isFile()) throw new Error(`${at} is not a regular file`);
  const observed = byteRoot(await readFile(resolved));
  if (observed !== expectedRoot) {
    throw new Error(`${at} root drifted: expected ${expectedRoot}, observed ${observed}`);
  }
  return resolved;
}

async function verifyDirectory(planDirectory, relativePath, at) {
  const candidate = path.resolve(planDirectory, relativePath);
  const resolved = await realpath(candidate);
  if (!remainsBelow(planDirectory, resolved)) {
    throw new Error(`${at} escapes the registered plan directory`);
  }
  const metadata = await lstat(resolved);
  if (!metadata.isDirectory()) throw new Error(`${at} is not a directory`);
  return resolved;
}

async function resolveExecutable(command, cwd, expectedRoot, at) {
  const candidates = command.includes("/")
    ? [path.resolve(cwd, command)]
    : (process.env.PATH ?? "")
      .split(path.delimiter)
      .filter((entry) => entry.length > 0)
      .map((entry) => path.join(entry, command));
  for (const candidate of candidates) {
    try {
      await access(candidate, constants.X_OK);
      const resolved = await realpath(candidate);
      const metadata = await lstat(resolved);
      if (!metadata.isFile()) continue;
      const observed = byteRoot(await readFile(resolved));
      if (observed !== expectedRoot) {
        throw new Error(
          `${at} root drifted: expected ${expectedRoot}, observed ${observed}`,
        );
      }
      return resolved;
    } catch (error) {
      if (error instanceof Error && error.message.startsWith(`${at} root drifted:`)) {
        throw error;
      }
    }
  }
  throw new Error(`${at} is not an executable file on the registered PATH`);
}

export async function verifyEvaluationPlanFiles(plan, planFile) {
  const parsed = parseEvaluationPlan(plan);
  const resolvedPlan = await realpath(planFile);
  const planDirectory = path.dirname(resolvedPlan);
  let verifiedFiles = 0;
  const executablePaths = {};
  const wrapperPaths = {};
  const armDependencyLockPaths = {};
  const armEnvironmentPaths = {};
  const armResourcePaths = {};
  const taskSourcePaths = {};
  const taskPacketPaths = {};
  const taskVerifierPaths = {};
  const taskVerifierRuntimePaths = {};
  const taskVerifierResourcePaths = {};
  for (const task of parsed.tasks) {
    taskSourcePaths[task.id] = await verifyFile(
      planDirectory,
      task.source_path,
      task.source_root,
      `task ${task.id} source`,
    );
    taskPacketPaths[task.id] = await verifyFile(
      planDirectory,
      task.packet_path,
      task.packet_root,
      `task ${task.id} packet`,
    );
    taskVerifierPaths[task.id] = await verifyFile(
      planDirectory,
      task.verifier_path,
      task.verifier_root,
      `task ${task.id} verifier`,
    );
    if (task.verifier_runtime === "direct") {
      await access(taskVerifierPaths[task.id], constants.X_OK);
      taskVerifierRuntimePaths[task.id] = taskVerifierPaths[task.id];
    } else {
      taskVerifierRuntimePaths[task.id] = await resolveExecutable(
        "bun",
        planDirectory,
        task.verifier_runtime_root,
        `task ${task.id} verifier runtime`,
      );
    }
    taskVerifierResourcePaths[task.id] = {};
    for (const resource of task.verifier_resources) {
      taskVerifierResourcePaths[task.id][resource.name] = await verifyFile(
        planDirectory,
        resource.path,
        resource.root,
        `task ${task.id} verifier resource ${resource.name}`,
      );
    }
    verifiedFiles += 3 + task.verifier_resources.length;
  }
  for (const arm of parsed.arms) {
    const cwd = await verifyDirectory(planDirectory, arm.cwd, `arm ${arm.id} cwd`);
    executablePaths[arm.id] = await resolveExecutable(
      arm.argv[0],
      cwd,
      arm.executable_root,
      `arm ${arm.id} executable`,
    );
    wrapperPaths[arm.id] = await verifyFile(
      planDirectory,
      arm.wrapper_path,
      arm.wrapper_root,
      `arm ${arm.id} wrapper`,
    );
    armDependencyLockPaths[arm.id] = await verifyFile(
      planDirectory,
      arm.dependency_lock_path,
      arm.dependency_lock_root,
      `arm ${arm.id} dependency lock`,
    );
    armEnvironmentPaths[arm.id] = await verifyFile(
      planDirectory,
      arm.environment_path,
      arm.environment_root,
      `arm ${arm.id} environment`,
    );
    armResourcePaths[arm.id] = {};
    for (const resource of arm.resources) {
      armResourcePaths[arm.id][resource.name] = await verifyFile(
        planDirectory,
        resource.path,
        resource.root,
        `arm ${arm.id} resource ${resource.name}`,
      );
    }
    verifiedFiles += 4 + arm.resources.length;
  }
  for (const scorer of parsed.scorers) {
    await verifyFile(
      planDirectory,
      scorer.path,
      scorer.root,
      `scorer ${scorer.id}`,
    );
    verifiedFiles += 1;
  }
  return {
    plan: parsed,
    verified_files: verifiedFiles,
    executable_paths: executablePaths,
    wrapper_paths: wrapperPaths,
    arm_dependency_lock_paths: armDependencyLockPaths,
    arm_environment_paths: armEnvironmentPaths,
    arm_resource_paths: armResourcePaths,
    task_source_paths: taskSourcePaths,
    task_packet_paths: taskPacketPaths,
    task_verifier_paths: taskVerifierPaths,
    task_verifier_runtime_paths: taskVerifierRuntimePaths,
    task_verifier_resource_paths: taskVerifierResourcePaths,
  };
}
