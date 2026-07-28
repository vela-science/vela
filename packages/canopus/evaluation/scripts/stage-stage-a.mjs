#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmod,
  lstat,
  mkdir,
  readFile,
  realpath,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  canonicalJson,
  parseEvaluationPlan,
  rootEvaluationPlan,
  verifyEvaluationPlanFiles,
} from "../lib/evaluation-plan.mjs";
import {
  ARTIFACT_PATH as ERDOS_ARTIFACT_PATH,
  SOURCE_PACKET_ROOT as ERDOS_SOURCE_ROOT,
  VERIFIER_BINARY_ROOT as ERDOS_VERIFIER_BINARY_ROOT,
  buildPacket as buildErdosPacket,
  packetBytes as erdosPacketBytes,
} from "../tasks/erdos-1056-10429401-10429600/task.mjs";
import {
  ARTIFACT_SCHEMA as SCIENTIFIC_ARTIFACT_SCHEMA,
  SOURCE_FILES as SCIENTIFIC_SOURCE_FILES,
  SOURCE_ARCHIVE_ROOT as SCIENTIFIC_SOURCE_ROOT,
  assertSafeArchiveEntries,
  buildPacket as buildScientificPacket,
  packetBytes as scientificPacketBytes,
} from "../tasks/core-bench-1108125/task.mjs";

const packageRoot = path.resolve(import.meta.dirname, "../..");
const repositoryRoot = path.resolve(packageRoot, "../..");
const VERIFIER_IMAGE_DIGEST =
  "sha256:503117b1e393779705fd34c2dbcabfb04fbd65d755887c13137566205418630a";

function sha256(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function options(argv) {
  const parsed = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (
      ![
        "--output",
        "--created-at",
        "--model",
        "--erdos-source",
        "--scientific-source",
        "--erdos-verifier",
        "--codex",
        "--vela",
        "--docker",
        "--max-tokens",
        "--assignment-prefix",
        "--amends-plan",
        "--amendment-reason",
      ].includes(key) ||
      value === undefined ||
      parsed.has(key)
    ) {
      throw new Error(`invalid Stage A staging option near ${key ?? "end"}`);
    }
    parsed.set(key, value);
  }
  for (const required of [
    "--output",
    "--created-at",
    "--model",
    "--erdos-source",
    "--scientific-source",
    "--erdos-verifier",
    "--codex",
    "--vela",
    "--docker",
  ]) {
    if (!parsed.has(required)) throw new Error(`missing ${required}`);
  }
  return parsed;
}

async function execute(argv, cwd = repositoryRoot, maxBytes = 8 * 1024 * 1024) {
  return await new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), {
      cwd,
      env: {
        PATH: process.env.PATH ?? "",
        HOME: process.env.HOME ?? "",
        LANG: "C",
        LC_ALL: "C",
        NO_COLOR: "1",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    let bytes = 0;
    const collect = (target, chunk) => {
      bytes += chunk.length;
      if (bytes > maxBytes) {
        child.kill("SIGKILL");
        reject(new Error(`${argv[0]} exceeded its output bound`));
        return;
      }
      target.push(chunk);
    };
    child.stdout.on("data", (chunk) => collect(stdout, chunk));
    child.stderr.on("data", (chunk) => collect(stderr, chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(
          `${argv[0]} exited ${String(code)}: ` +
          `${Buffer.concat(stderr).toString("utf8").trim()}`,
        ));
        return;
      }
      resolve(Buffer.concat(stdout));
    });
  });
}

async function copyExact(source, target, options = {}) {
  const resolved = await realpath(source);
  const metadata = await lstat(resolved);
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.nlink !== 1) {
    throw new Error(`${source} must be one regular singly linked file`);
  }
  const bytes = await readFile(resolved);
  if (bytes.length === 0 || bytes.length > (options.maxBytes ?? 512 * 1024 * 1024)) {
    throw new Error(`${source} violates its byte contract`);
  }
  if (options.expectedRoot !== undefined && sha256(bytes) !== options.expectedRoot) {
    throw new Error(`${source} root drifted`);
  }
  await mkdir(path.dirname(target), { recursive: true, mode: 0o700 });
  await writeFile(target, bytes, { flag: "wx", mode: options.executable ? 0o500 : 0o600 });
  if (options.executable) await chmod(target, 0o500);
  return { bytes, root: sha256(bytes) };
}

async function version(binary) {
  return (await execute([binary, "--version"], repositoryRoot, 4096))
    .toString("utf8")
    .trim();
}

async function bundleBunEntrypoint(source, target) {
  await execute([
    process.execPath,
    "build",
    source,
    "--target",
    "bun",
    "--outfile",
    target,
  ], packageRoot);
  const bytes = await readFile(target);
  if (bytes.length === 0 || bytes.length > 4 * 1024 * 1024) {
    throw new Error(`${source} bundled verifier violates its byte contract`);
  }
  await chmod(target, 0o500);
  return { bytes, root: sha256(bytes) };
}

function identity(name, versionValue, root) {
  return { name, version: versionValue, sha256: root };
}

const values = options(process.argv.slice(2).filter((value) => value !== "--"));
const maxTokens = Number(values.get("--max-tokens") ?? "100000");
if (!Number.isSafeInteger(maxTokens) || maxTokens < 1 || maxTokens > 300_000) {
  throw new Error("--max-tokens must be an integer in 1..300000");
}
const assignmentPrefix = values.get("--assignment-prefix") ?? "A";
if (!/^[A-Z][A-Z0-9]{0,7}$/u.test(assignmentPrefix)) {
  throw new Error("--assignment-prefix must be 1..8 uppercase alphanumeric characters");
}
const amendsPlanPath = values.get("--amends-plan");
const amendmentReason = values.get("--amendment-reason");
if ((amendsPlanPath === undefined) !== (amendmentReason === undefined)) {
  throw new Error("--amends-plan and --amendment-reason must be provided together");
}
let amendsRoot = null;
if (amendsPlanPath !== undefined) {
  const previousPlanFile = await realpath(amendsPlanPath);
  const previousPlan = parseEvaluationPlan(
    JSON.parse(await readFile(previousPlanFile, "utf8")),
  );
  await verifyEvaluationPlanFiles(previousPlan, previousPlanFile);
  amendsRoot = previousPlan.plan_root;
}
const output = path.resolve(values.get("--output"));
await mkdir(path.dirname(output), { recursive: true, mode: 0o700 });
try {
  await lstat(output);
  throw new Error("Stage A output already exists");
} catch (error) {
  if (!(error instanceof Error) || !("code" in error) || error.code !== "ENOENT") {
    throw error;
  }
}
const status = (await execute(["/usr/bin/git", "status", "--porcelain=v1"], repositoryRoot))
  .toString("utf8");
if (status.length !== 0) {
  throw new Error("Stage A registration requires a clean Vela monorepo");
}

const [erdosSource, scientificSource, erdosVerifier, codex, vela, docker] =
  await Promise.all([
    realpath(values.get("--erdos-source")),
    realpath(values.get("--scientific-source")),
    realpath(values.get("--erdos-verifier")),
    realpath(values.get("--codex")),
    realpath(values.get("--vela")),
    realpath(values.get("--docker")),
  ]);
await mkdir(output, { recursive: false, mode: 0o700 });
for (const directory of [
  "sources",
  "packets",
  "verifiers",
  "wrappers",
  "resources",
  "locks",
  "environments",
  "scorers",
  "workspace",
]) {
  await mkdir(path.join(output, directory), { mode: 0o700 });
}

const erdosSourceCopy = await copyExact(
  erdosSource,
  path.join(output, "sources/erdos-1056.json"),
  { expectedRoot: ERDOS_SOURCE_ROOT, maxBytes: 1_048_576 },
);
const scientificSourceCopy = await copyExact(
  scientificSource,
  path.join(output, "sources/core-bench-1108125.tar.gz"),
  { expectedRoot: SCIENTIFIC_SOURCE_ROOT, maxBytes: 64 * 1024 * 1024 },
);
const scientificArchiveEntries = (await execute([
  "/usr/bin/tar",
  "-tzf",
  scientificSource,
], repositoryRoot, 2 * 1024 * 1024))
  .toString("utf8")
  .split("\n")
  .filter((entry) => entry.length > 0);
assertSafeArchiveEntries(scientificArchiveEntries);
const erdosVerifierBinaryCopy = await copyExact(
  erdosVerifier,
  path.join(output, "resources/erdos-1056-verifier"),
  {
    expectedRoot: ERDOS_VERIFIER_BINARY_ROOT,
    executable: true,
    maxBytes: 16 * 1024 * 1024,
  },
);
const erdosVerifierCopy = await bundleBunEntrypoint(
  "evaluation/tasks/erdos-1056-10429401-10429600/verify.mjs",
  path.join(output, "verifiers/erdos-1056.mjs"),
);
const scientificVerifierCopy = await bundleBunEntrypoint(
  "evaluation/tasks/core-bench-1108125/verify.mjs",
  path.join(output, "verifiers/core-bench-1108125.mjs"),
);
const codexCopy = await copyExact(codex, path.join(output, "resources/codex"), {
  executable: true,
  maxBytes: 512 * 1024 * 1024,
});
const dockerCopy = await copyExact(docker, path.join(output, "resources/docker"), {
  executable: true,
  maxBytes: 128 * 1024 * 1024,
});
const velaIdentity = await copyExact(vela, path.join(output, "resources/vela"), {
  executable: true,
  maxBytes: 128 * 1024 * 1024,
});
const permissionProfile = await copyExact(
  path.join(packageRoot, "runtime/native-worker/config.toml"),
  path.join(output, "resources/native-worker.config.toml"),
  { maxBytes: 1_048_576 },
);
const outputSchema = await copyExact(
  path.join(packageRoot, "schemas/engine-output.v0.json"),
  path.join(output, "resources/engine-output.v0.json"),
  { maxBytes: 1_048_576 },
);
const dependencyLock = await copyExact(
  path.join(repositoryRoot, "bun.lock"),
  path.join(output, "locks/bun.lock"),
  { maxBytes: 16 * 1024 * 1024 },
);

const erdosPacket = erdosPacketBytes(buildErdosPacket(erdosSourceCopy.bytes));
const scientificPacket = scientificPacketBytes(
  buildScientificPacket(new Map(
    (await Promise.all(
      SCIENTIFIC_SOURCE_FILES.map(
        async (relative) => {
          const bytes = await execute([
            "/usr/bin/tar",
            "-xOzf",
            scientificSource,
            `capsule-1108125/${relative}`,
          ]);
          return [relative, bytes];
        },
      ),
    )),
  )),
);
await writeFile(path.join(output, "packets/erdos-1056.json"), erdosPacket, {
  flag: "wx",
  mode: 0o600,
});
await writeFile(
  path.join(output, "packets/core-bench-1108125.json"),
  scientificPacket,
  { flag: "wx", mode: 0o600 },
);

const wrapper = path.join(output, "wrappers/stage-a.mjs");
await execute([
  process.execPath,
  path.join(packageRoot, "evaluation/wrappers/build-stage-a.mjs"),
  "--output",
  wrapper,
], packageRoot);
const wrapperRoot = sha256(await readFile(wrapper));

const [codexVersion, velaVersion, dockerVersion, gitVersion, gitCommit] =
  await Promise.all([
    version(codex),
    version(vela),
    version(docker),
    version("/usr/bin/git"),
    execute(["/usr/bin/git", "rev-parse", "HEAD"], repositoryRoot, 4096)
      .then((bytes) => bytes.toString("utf8").trim()),
  ]);
const bunRoot = sha256(await readFile(await realpath(process.execPath)));
const gitRoot = sha256(await readFile(await realpath("/usr/bin/git")));
const packageManifest = JSON.parse(
  await readFile(path.join(packageRoot, "package.json"), "utf8"),
);
const environment = {
  schema: "canopus.evaluation-environment.v1",
  platform: process.platform,
  architecture: process.arch,
  repository_commit: gitCommit,
  bun: { version: Bun.version, sha256: bunRoot },
  codex: { version: codexVersion, sha256: codexCopy.root },
  vela: { version: velaVersion, sha256: velaIdentity.root },
  docker: { version: dockerVersion, sha256: dockerCopy.root },
  permission_profile_root: permissionProfile.root,
  output_schema_root: outputSchema.root,
};
const environmentBytes = Buffer.from(canonicalJson(environment));
await writeFile(path.join(output, "environments/stage-a.json"), environmentBytes, {
  flag: "wx",
  mode: 0o600,
});

const scorerValues = [
  {
    id: "execution",
    metric:
      "Verifier-passing bounded artifacts per observed token, producer plus verifier wall time, and recorded expert minute.",
    hard_failures: [
      "credential exposure",
      "workspace escape",
      "authority access",
      "unregistered retry",
    ],
  },
  {
    id: "state",
    metric:
      "Claim scope validity, verifier outcome separation, replay identity, and scientific-state correctness.",
    hard_failures: [
      "verifier success presented as acceptance",
      "bounded negative presented as universal",
    ],
  },
  {
    id: "inheritance",
    metric:
      "Expert minutes to locate decisive evidence, reproduce the artifact, and continue from retained state.",
    hard_failures: ["missing decisive evidence", "unreplayable retained state"],
  },
];
const scorers = [];
for (const scorer of scorerValues) {
  const bytes = Buffer.from(canonicalJson({
    schema: "canopus.evaluation-scorer.v1",
    ...scorer,
  }));
  const relative = `scorers/${scorer.id}.json`;
  await writeFile(path.join(output, relative), bytes, { flag: "wx", mode: 0o600 });
  scorers.push({ id: scorer.id, path: relative, root: sha256(bytes) });
}

const commonResources = [
  { name: "codex", path: "resources/codex", root: codexCopy.root },
  {
    name: "permission_profile",
    path: "resources/native-worker.config.toml",
    root: permissionProfile.root,
  },
  {
    name: "output_schema",
    path: "resources/engine-output.v0.json",
    root: outputSchema.root,
  },
];
const arms = [
  ["native", "native_codex", "native"],
  ["native-packet", "native_codex_packet", "native_packet"],
  ["canopus", "canopus", "canopus"],
].map(([id, kind, mode]) => ({
  id,
  kind,
  argv: [
    "bun",
    "{wrapper}",
    "--mode",
    mode,
    "--task-packet",
    "{task_packet}",
    "--output",
    "{output}",
    "--assignment",
    "{assignment_id}",
    "--seed",
    "{seed}",
    "--codex",
    "{resource:codex}",
    "--codex-version",
    codexVersion,
    "--codex-root",
    codexCopy.root,
    "--model",
    values.get("--model"),
    "--permission-profile",
    "{resource:permission_profile}",
    "--output-schema",
    "{resource:output_schema}",
    "--max-wall-ms",
    "3600000",
    "--max-tokens",
    String(maxTokens),
    "--max-artifact-bytes",
    "65536",
  ],
  cwd: "workspace",
  wrapper_path: "wrappers/stage-a.mjs",
  wrapper_root: wrapperRoot,
  dependency_lock_path: "locks/bun.lock",
  dependency_lock_root: dependencyLock.root,
  environment_path: "environments/stage-a.json",
  environment_root: sha256(environmentBytes),
  executable_root: bunRoot,
  resources: commonResources,
}));
const tasks = [
  {
    id: "erdos:1056:10429401-10429600",
    class: "math",
    source: "Erdős Frontier target erdos:1056 at exact current packet root.",
    source_path: "sources/erdos-1056.json",
    source_root: erdosSourceCopy.root,
    packet_path: "packets/erdos-1056.json",
    packet_root: sha256(erdosPacket),
    verifier_path: "verifiers/erdos-1056.mjs",
    verifier_root: erdosVerifierCopy.root,
    verifier_runtime: "bun",
    verifier_runtime_root: bunRoot,
    verifier_args: [
      "--artifact",
      "{artifact}",
      "--binary",
      "{resource:erdos_binary}",
      "--docker",
      "{resource:docker}",
    ],
    verifier_resources: [
      {
        name: "erdos_binary",
        path: "resources/erdos-1056-verifier",
        root: erdosVerifierBinaryCopy.root,
      },
      { name: "docker", path: "resources/docker", root: dockerCopy.root },
    ],
    artifact_path: ERDOS_ARTIFACT_PATH,
    max_artifact_bytes: 65_536,
    license: "Apache-2.0 OR MIT",
    cpu_only: true,
    network: "deny",
    max_wall_time_ms: 3_600_000,
    max_observed_tokens: maxTokens,
  },
  {
    id: "core-bench:capsule-1108125",
    class: "scientific_computing",
    source: "CORE-Bench capsule-1108125, MIT code and CC0 data.",
    source_path: "sources/core-bench-1108125.tar.gz",
    source_root: scientificSourceCopy.root,
    packet_path: "packets/core-bench-1108125.json",
    packet_root: sha256(scientificPacket),
    verifier_path: "verifiers/core-bench-1108125.mjs",
    verifier_root: scientificVerifierCopy.root,
    verifier_runtime: "bun",
    verifier_runtime_root: bunRoot,
    verifier_args: [
      "--archive",
      "{source}",
      "--artifact",
      "{artifact}",
      "--docker",
      "{resource:docker}",
    ],
    verifier_resources: [
      { name: "docker", path: "resources/docker", root: dockerCopy.root },
    ],
    artifact_path: "artifacts/result.json",
    max_artifact_bytes: 65_536,
    license: "MIT AND CC0-1.0",
    cpu_only: true,
    network: "deny",
    max_wall_time_ms: 3_600_000,
    max_observed_tokens: maxTokens,
  },
];
const assignments = tasks.flatMap((task) =>
  arms.flatMap((arm) =>
    [1, 2].map((repetition) => ({
      id: `${assignmentPrefix}-${task.id}-${arm.id}-r${repetition}`,
      stage: "A",
      task_id: task.id,
      arm_id: arm.id,
      repetition,
      seed: repetition,
    }))));
const model = values.get("--model");
const plan = rootEvaluationPlan({
  schema: "canopus.evaluation-plan.v1",
  plan_id: amendsRoot === null
    ? "vela-math-first-stage-a-2026-07-28"
    : "vela-math-first-stage-a-amendment-2026-07-28",
  status: "registered",
  created_at: values.get("--created-at"),
  campaign: "Math-first framework-neutral Stage A evaluation.",
  identities: {
    model: identity("OpenAI model", model, sha256(Buffer.from(`${model}\n`))),
    codex: identity("Codex CLI", codexVersion, codexCopy.root),
    canopus: identity(
      "@vela-science/canopus",
      packageManifest.version,
      sha256(await readFile(path.join(packageRoot, "package.json"))),
    ),
    vela: identity("Vela", velaVersion, velaIdentity.root),
    git: identity("Git", gitVersion, gitRoot),
    environment: identity("Bun", Bun.version, bunRoot),
    dependencies: [
      identity("Docker CLI", dockerVersion, dockerCopy.root),
      identity("CORE-Bench verifier image", "immutable digest", VERIFIER_IMAGE_DIGEST),
    ],
  },
  tasks,
  arms,
  assignments,
  budgets: {
    max_model_calls: 12,
    max_total_wall_time_ms: 43_200_000,
    max_total_observed_tokens: maxTokens * assignments.length,
  },
  retry_policy: {
    max_pre_output_infrastructure_retries: 1,
    post_output_retries: 0,
  },
  stopping_rules: [
    "Stop on credential, repository-authority, or human-key exposure.",
    "Stop on workspace escape, verifier network access, or unregistered mutation.",
    "Stop on plan drift, hidden failed runs, answer leakage, or post-output retries.",
  ],
  scorers,
  performance_functions: {
    execution_lift: scorers.find((scorer) => scorer.id === "execution").root,
    state_lift: scorers.find((scorer) => scorer.id === "state").root,
    inheritance_lift: scorers.find((scorer) => scorer.id === "inheritance").root,
  },
  exclusions: [
    "First-party repetitions receive no independent-participant credit.",
    "Task verifiers are withheld from every model workspace.",
    "Scientific acceptance and repository authority are outside the evaluation.",
  ],
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
  amends_root: amendsRoot,
  amendment_reason: amendmentReason ?? null,
  plan_root: "",
});
const planFile = path.join(output, "plan.json");
await writeFile(planFile, canonicalJson(plan), { flag: "wx", mode: 0o600 });
const verified = await verifyEvaluationPlanFiles(plan, planFile);
process.stdout.write(`${JSON.stringify({
  ok: true,
  command: "eval:stage-a",
  output,
  plan: planFile,
  plan_root: plan.plan_root,
  verified_files: verified.verified_files,
  assignments: assignments.length,
})}\n`);
