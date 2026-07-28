import { createHash } from "node:crypto";

export const EVALUATION_PLAN_SCHEMA = "canopus.evaluation-plan.v1";
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
      "id", "class", "source", "source_root", "packet_path", "packet_root",
      "verifier_root", "license", "cpu_only", "network", "max_wall_time_ms",
      "max_observed_tokens",
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
  return {
    id: text(item.id, `${at}.id`, 256),
    class: item.class,
    source: text(item.source, `${at}.source`, 512),
    source_root: root(item.source_root, `${at}.source_root`),
    packet_path: relative(item.packet_path, `${at}.packet_path`),
    packet_root: root(item.packet_root, `${at}.packet_root`),
    verifier_root: root(item.verifier_root, `${at}.verifier_root`),
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
      100_000,
    ),
  };
}

function parseArm(value, at) {
  const item = object(value, at);
  exactKeys(
    item,
    ["id", "kind", "argv", "cwd", "dependency_lock_root", "environment_root"],
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
  const argv = array(item.argv, `${at}.argv`, 1, 64, (entry, entryAt) =>
    text(entry, entryAt, 4096));
  return {
    id: text(item.id, `${at}.id`, 128),
    kind: item.kind,
    argv,
    cwd: relative(item.cwd, `${at}.cwd`),
    dependency_lock_root: root(item.dependency_lock_root, `${at}.dependency_lock_root`),
    environment_root: root(item.environment_root, `${at}.environment_root`),
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
  const assignmentIds = new Set();
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
    3_600_000,
  );
  if (assignments.length > budgets.max_model_calls) {
    throw new Error("assignments exceed the registered model-call budget");
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
    (entry, at) => root(entry, at),
  );
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
    if (!scorers.includes(performanceRoot)) {
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
  array(
    identities.dependencies,
    "plan.identities.dependencies",
    0,
    32,
    parseIdentity,
  );
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

export function canonicalJson(value) {
  return `${canonical(value)}\n`;
}
