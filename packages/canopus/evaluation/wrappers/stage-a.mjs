import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  realpath,
  unlink,
  writeFile,
} from "node:fs/promises";
import { writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

import { BudgetTracker } from "../../src/budget/enforce.ts";
import { CodexToolsNativeEngine } from "../../src/engines/codex-tools-native.ts";
import { parseCodexEvents } from "../../src/engines/codex-events.ts";
import { canonicalJson, sha256Bytes } from "../../src/util/canonical.ts";

const MODES = new Set(["native", "native_packet", "canopus"]);

function options(argv) {
  const parsed = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (
      ![
        "--mode",
        "--task-packet",
        "--output",
        "--assignment",
        "--seed",
        "--codex",
        "--codex-version",
        "--codex-root",
        "--model",
        "--permission-profile",
        "--output-schema",
        "--max-wall-ms",
        "--max-tokens",
        "--max-artifact-bytes",
      ].includes(key) ||
      value === undefined ||
      parsed.has(key)
    ) {
      throw new Error(`invalid Stage A wrapper option near ${key ?? "end"}`);
    }
    parsed.set(key, value);
  }
  for (const required of [
    "--mode",
    "--task-packet",
    "--output",
    "--assignment",
    "--seed",
    "--codex",
    "--codex-version",
    "--codex-root",
    "--model",
    "--permission-profile",
    "--output-schema",
    "--max-wall-ms",
    "--max-tokens",
    "--max-artifact-bytes",
  ]) {
    if (!parsed.has(required)) throw new Error(`missing ${required}`);
  }
  if (!MODES.has(parsed.get("--mode"))) throw new Error("unsupported Stage A arm mode");
  return parsed;
}

function positiveInteger(value, at, maximum) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 1 || parsed > maximum) {
    throw new Error(`${at} must be an integer in 1..${maximum}`);
  }
  return parsed;
}

function safeRelative(value, at) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.startsWith("/") ||
    value.includes("\\") ||
    value.split("/").some((part) => part === "" || part === "." || part === "..")
  ) {
    throw new Error(`${at} must be a safe relative path`);
  }
  return value;
}

function parsePacket(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("evaluation task packet must be an object");
  }
  if (
    value.schema !== "canopus.evaluation-task-packet.v1" ||
    typeof value.task_id !== "string" ||
    typeof value.objective !== "string" ||
    value.output === null ||
    typeof value.output !== "object" ||
    Array.isArray(value.output)
  ) {
    throw new Error("evaluation task packet is incomplete");
  }
  return {
    ...value,
    output: {
      ...value.output,
      path: safeRelative(value.output.path, "evaluation task output.path"),
    },
  };
}

function nativePrompt(mode, packet, assignment, seed) {
  const exactPacket = mode === "native_packet"
    ? ["Exact registered task packet:", canonicalJson(packet)]
    : [
        `Objective: ${packet.objective}`,
        `Required artifact path: ${packet.output.path}`,
        "The exact source material and output contract are available in task.json.",
      ];
  return [
    "Complete one bounded scientific producer task in the current fresh workspace.",
    "Shell and apply_patch are available. Network, browser, MCP, apps, delegation, signing, authority, and the separate verifier are unavailable.",
    "Do not inspect credentials, configuration, process state, unrelated paths, or anything outside the workspace.",
    "Write exactly one UTF-8 candidate artifact at the registered path. Do not claim verifier passage or scientific acceptance.",
    "Return only the supplied canopus.engine-output.v0 JSON. Use status success only when the complete artifact bytes exist; set that artifact's content to an empty string so the trusted wrapper reads the workspace bytes.",
    "A bounded negative result applies only to the exact registered range or source snapshot.",
    `Fresh assignment: ${assignment}; registered seed: ${seed}.`,
    ...exactPacket,
  ].join("\n");
}

function mission({ packet, packetRoot, assignment, mode, values, profileRoot, schemaRoot }) {
  const maxWallTime = positiveInteger(
    values.get("--max-wall-ms"),
    "--max-wall-ms",
    7_200_000,
  );
  const maxTokens = positiveInteger(
    values.get("--max-tokens"),
    "--max-tokens",
    100_000,
  );
  const maxArtifactBytes = positiveInteger(
    values.get("--max-artifact-bytes"),
    "--max-artifact-bytes",
    64 * 1024 * 1024,
  );
  const target = packet.task_id.startsWith("erdos:1056")
    ? "erdos:1056"
    : packet.task_id;
  const inertRoot = `sha256:${"0".repeat(64)}`;
  return {
    schema: "canopus.mission.v1",
    id: `mission_${assignment.replace(/[^A-Za-z0-9_-]/gu, "_")}`,
    target,
    vela_version: "evaluation-only",
    vela_sha256: inertRoot,
    frontier: ".",
    actor: `agent:evaluation-${mode.replace("_", "-")}`,
    role: "producer",
    claim_type: "computational",
    replayability: "exact",
    objective: packet.objective,
    completion_condition:
      "The separately registered task verifier exits zero on the exact candidate bytes.",
    roots: {
      git_commit: "0".repeat(40),
      git_tree: "0".repeat(40),
      vela_repository: inertRoot,
    },
    target_packet: {
      path: "task.json",
      sha256: packetRoot,
    },
    allowed_paths: [packet.output.path],
    budgets: {
      max_research_wall_time_ms: maxWallTime,
      max_research_processes: 8,
      max_research_output_bytes: 64 * 1024 * 1024,
      max_prompt_bytes: 8 * 1024 * 1024,
      max_artifact_bytes: maxArtifactBytes,
      max_attempts: 1,
      max_observed_tokens: maxTokens,
    },
    worker: {
      kind: "codex_tools_native",
      platform: process.platform,
      codex_version: values.get("--codex-version"),
      codex_sha256: values.get("--codex-root"),
      permission_profile_path: "contract/native-worker.config.toml",
      permission_profile_sha256: profileRoot,
      workspace: "target_packet_only",
      output_schema_sha256: schemaRoot,
      model: values.get("--model"),
      network: "provider_only",
      tools: ["shell", "apply_patch"],
    },
    verifier: {
      argv: ["registered-task-verifier", `{artifact:${packet.output.path}}`],
      executable_sha256: inertRoot,
      cwd: ".",
      timeout_ms: maxWallTime,
      max_output_bytes: 16 * 1024 * 1024,
      network: "deny",
      writes: "deny",
      capsule_path: "registered-task-verifier",
      capsule_sha256: inertRoot,
      image: inertRoot,
    },
    scientific_chain: {
      predicted_observable: "The registered task verifier exits zero.",
      performed_test: `registered-task-verifier ${packet.output.path}`,
    },
    landing: { expected_routes: ["defer"], max_accepted_delta: 0 },
  };
}

async function makePaths(output, packetBytes) {
  const root = path.join(output, "producer");
  const paths = Object.fromEntries([
    ["root", root],
    ["input", path.join(root, "input")],
    ["frontier", path.join(root, "frontier")],
    ["work", path.join(root, "work")],
    ["output", path.join(root, "output")],
    ["artifacts", path.join(root, "artifacts")],
    ["home", path.join(root, "home")],
    ["velaHome", path.join(root, "vela-home")],
    ["verifierHome", path.join(root, "verifier-home")],
  ]);
  await mkdir(root, { recursive: false, mode: 0o700 });
  await Promise.all(
    Object.entries(paths)
      .filter(([name]) => name !== "root")
      .map(([, directory]) => mkdir(directory, { recursive: true, mode: 0o700 })),
  );
  await mkdir(path.join(paths.frontier, ".git"), { mode: 0o700 });
  await writeFile(
    path.join(paths.frontier, ".git", "HEAD"),
    "ref: refs/heads/evaluation\n",
    { flag: "wx", mode: 0o600 },
  );
  await writeFile(path.join(paths.input, "task.json"), packetBytes, {
    flag: "wx",
    mode: 0o400,
  });
  return paths;
}

async function removePrivateEvents(root) {
  for (const file of ["worker-events.jsonl", "worker-stderr.bin"]) {
    try {
      await unlink(path.join(root, file));
    } catch (error) {
      if (!(error instanceof Error) || !("code" in error) || error.code !== "ENOENT") {
        throw error;
      }
    }
  }
}

function writeControl(assignment, usage, modelOutputObserved) {
  writeFileSync(
    3,
    canonicalJson({
      schema: "canopus.evaluation-arm-result.v1",
      assignment_id: assignment,
      model_output_observed: modelOutputObserved,
      usage,
    }),
  );
}

const values = options(process.argv.slice(2).filter((value) => value !== "--"));
const mode = values.get("--mode");
const assignment = values.get("--assignment");
const seed = positiveInteger(values.get("--seed"), "--seed", 2_147_483_647);
const output = await realpath(values.get("--output"));
const [packetFile, codex, permissionProfile, outputSchema] = await Promise.all([
  realpath(values.get("--task-packet")),
  realpath(values.get("--codex")),
  realpath(values.get("--permission-profile")),
  realpath(values.get("--output-schema")),
]);
const [packetBytes, profileBytes, schemaBytes] = await Promise.all([
  readFile(packetFile),
  readFile(permissionProfile),
  readFile(outputSchema),
]);
const packet = parsePacket(JSON.parse(packetBytes.toString("utf8")));
const packetRoot = sha256Bytes(packetBytes);
const profileRoot = sha256Bytes(profileBytes);
const schemaRoot = sha256Bytes(schemaBytes);
const paths = await makePaths(output, packetBytes);
const activeMission = mission({
  packet,
  packetRoot,
  assignment,
  mode,
  values,
  profileRoot,
  schemaRoot,
});
const authHome = process.env.CODEX_HOME;
if (authHome === undefined || !path.isAbsolute(authHome)) {
  throw new Error("Stage A wrapper requires an absolute CODEX_HOME");
}

let usage = null;
let modelOutputObserved = false;
let failed = false;
try {
  const engine = new CodexToolsNativeEngine({
    binary: codex,
    authHome,
    outputSchema,
    permissionProfile,
    ...(mode === "canopus"
      ? {}
      : {
          prompt: () => nativePrompt(mode, packet, assignment, seed),
        }),
  });
  const result = await engine.run({
    mission: activeMission,
    briefing: {},
    paths,
    budget: new BudgetTracker(activeMission.budgets),
  });
  usage = result.usage;
  modelOutputObserved = true;
  const artifact = result.draft.artifacts.find(
    (candidate) => candidate.path === packet.output.path,
  );
  if (
    result.draft.status !== "success" ||
    result.draft.artifacts.length !== 1 ||
    artifact === undefined ||
    artifact.content.length === 0
  ) {
    failed = true;
  } else {
    const artifactFile = path.join(output, packet.output.path);
    await mkdir(path.dirname(artifactFile), { recursive: true, mode: 0o700 });
    await writeFile(artifactFile, artifact.content, { flag: "wx", mode: 0o600 });
  }
  await writeFile(
    path.join(output, "engine-summary.json"),
    canonicalJson({
      schema: "canopus.evaluation-engine-summary.v1",
      assignment_id: assignment,
      mode,
      status: result.draft.status,
      engine: result.engine,
      usage: result.usage,
      wall_time_ms: result.wallTimeMs,
      event_types: result.eventTypes,
      action_types: result.actionTypes,
      events_root: result.eventsDigest,
      stderr_root: result.stderrDigest,
      artifact_path: artifact?.path ?? null,
    }),
    { flag: "wx", mode: 0o600 },
  );
} catch (error) {
  failed = true;
  try {
    const events = parseCodexEvents(
      await readFile(path.join(paths.root, "worker-events.jsonl"), "utf8"),
    );
    usage = events.usage;
    modelOutputObserved = true;
  } catch {
    usage = null;
  }
  const errorRoot = createHash("sha256")
    .update(error instanceof Error ? error.message : String(error))
    .digest("hex");
  process.stderr.write(`Stage A producer failed; error_sha256=${errorRoot}\n`);
} finally {
  await removePrivateEvents(paths.root);
}

if (usage !== null) writeControl(assignment, usage, modelOutputObserved);
process.exitCode = failed ? 1 : 0;

