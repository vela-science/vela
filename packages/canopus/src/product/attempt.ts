import { chmod, mkdir, mkdtemp, realpath, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";

import {
  arrayAt,
  exactKeys,
  gitObjectAt,
  integerAt,
  objectAt,
  relativePathAt,
  sha256At,
  stringAt,
} from "../contracts/validation.js";
import { CodexToolsNativeEngine } from "../engines/codex-tools-native.js";
import { prepareMission, parseVelaVersionOutput } from "../mission/prepare.js";
import { runCanopus, type CanopusCurrentRunResult } from "../run.js";
import { canonicalJson, contentDigest, protocolDigest, sha256Bytes } from "../util/canonical.js";
import { isolatedEnvironment, runCommand, type CommandRunner } from "../util/command.js";
import {
  MAX_EXECUTABLE_BYTES,
  sha256RegularFile,
} from "../util/files.js";
import { VelaClient } from "../vela/cli.js";
import { runNativeCustodyPreflight } from "./custody.js";
import {
  assertFreshOutput,
  assertToolUsingMissionPlatform,
  writeEvidenceManifest,
} from "./run.js";
import { runtimeIdentity } from "./runtime.js";

const REQUEST_SCHEMA = "vela.agent-run-request.internal.v1";
const BUNDLE_SCHEMA = "vela.agent-execution-bundle.v1";

interface RootedFile {
  path: string;
  size: number;
  sha256: string;
}

export interface AgentRunRequest {
  schema: typeof REQUEST_SCHEMA;
  authority: "none";
  effect: "none";
  request_root: string;
  frontier: {
    path: string;
    id: string;
    origin_id: string;
    repository_root: string;
  };
  attempt: {
    id: string;
    authorization_root: string;
    actor: string;
    created_at: string;
    expires_at: string;
    runner_build_root: string;
    allowed_operations: string[];
    allowed_artifact_classes: string[];
    budget: {
      max_submissions: number;
      max_verifications: number;
      max_artifacts: number;
      max_artifact_bytes: number;
      max_runs: number;
    };
    usage: {
      runs: number;
      submissions: number;
      verifications: number;
      artifacts: number;
      artifact_bytes: number;
      registered_submission_ids: string[];
      registered_verification_record_ids: string[];
    };
    consequence_ceiling: "evidence_only" | "pending_review";
    task_contract_root: string;
    controller_build: {
      program: string;
      version: string;
      binary_sha256: string;
    };
  };
  target: {
    id: string;
    binding_root: string;
    target_index_root: string;
    input_root: string;
    source: {
      git_object_format: "sha1" | "sha256";
      git_commit: string;
      git_tree: string;
    };
    claim_read_set: {
      git_object_format: "sha1" | "sha256";
      git_commit: string;
      git_tree: string;
    };
    packet: {
      schema: string;
      path: string;
      size: number;
      sha256: string;
    };
    packet_json: string;
  };
  execution_bundle: {
    reference: RootedFile & { schema: typeof BUNDLE_SCHEMA };
    value: Record<string, unknown>;
    mission: Record<string, unknown>;
  };
  runner_build: AgentHelperBuild;
  output_root: string | null;
}

export interface AgentHelperBuild {
  schema: "vela.agent-helper-build.v1";
  platform: string;
  runtime: {
    kind: "bun";
    version: "1.3.12";
    size: number;
    sha256: string;
  };
  bundle: {
    format: "esm";
    size: number;
    sha256: string;
  };
}

export interface AgentProductRunResult {
  run: CanopusCurrentRunResult;
  output_root: string;
  bundle_root: string;
  evidence_manifest: string;
  evidence_root: string;
  target: {
    id: string;
    binding_root: string;
    target_index_root: string;
    input_root: string;
    packet_root: string;
    source: AgentRunRequest["target"]["source"];
    claim_read_set: AgentRunRequest["target"]["claim_read_set"];
  };
  source_state: {
    state: "unchanged";
    git_object_format: "sha1" | "sha256";
    commit: string;
    tree: string;
  };
  submission: null;
  attempt_id: string;
  execution_bundle_root: string;
  request_root: string;
}

export interface AgentSupportAssets {
  engineOutputSchema: string;
  macosPermissionProfile: string;
  linuxPermissionProfile: string;
}

function rootedFile(value: unknown, at: string): RootedFile {
  const object = objectAt(value, at);
  exactKeys(object, ["path", "size", "sha256"], [], at);
  return {
    path: relativePathAt(object.path, `${at}.path`),
    size: integerAt(object.size, `${at}.size`, 1, 268_435_456),
    sha256: sha256At(object.sha256, `${at}.sha256`),
  };
}

function stringArray(value: unknown, at: string): string[] {
  return arrayAt(value, at, { min: 0, max: 256, unique: true }, (item, itemAt) =>
    stringAt(item, itemAt, { min: 1, max: 256 })
  );
}

function parseBudget(value: unknown): AgentRunRequest["attempt"]["budget"] {
  const object = objectAt(value, "Agent request attempt.budget");
  exactKeys(
    object,
    ["max_runs", "max_submissions", "max_verifications", "max_artifacts", "max_artifact_bytes"],
    [],
    "Agent request attempt.budget",
  );
  return {
    max_runs: integerAt(object.max_runs, "attempt.budget.max_runs", 1, 1),
    max_submissions: integerAt(object.max_submissions, "attempt.budget.max_submissions", 1, 16),
    max_verifications: integerAt(object.max_verifications, "attempt.budget.max_verifications", 1, 16),
    max_artifacts: integerAt(object.max_artifacts, "attempt.budget.max_artifacts", 1, 64),
    max_artifact_bytes: integerAt(
      object.max_artifact_bytes,
      "attempt.budget.max_artifact_bytes",
      1,
      64 * 1024 * 1024,
    ),
  };
}

function parseUsage(value: unknown): AgentRunRequest["attempt"]["usage"] {
  const object = objectAt(value, "Agent request attempt.usage");
  exactKeys(
    object,
    [
      "submissions",
      "verifications",
      "artifacts",
      "artifact_bytes",
      "registered_submission_ids",
      "registered_verification_record_ids",
      "runs",
    ],
    [],
    "Agent request attempt.usage",
  );
  return {
    runs: integerAt(object.runs, "attempt.usage.runs", 1, 1),
    submissions: integerAt(object.submissions, "attempt.usage.submissions", 0, 16),
    verifications: integerAt(object.verifications, "attempt.usage.verifications", 0, 16),
    artifacts: integerAt(object.artifacts, "attempt.usage.artifacts", 0, 64),
    artifact_bytes: integerAt(
      object.artifact_bytes,
      "attempt.usage.artifact_bytes",
      0,
      64 * 1024 * 1024,
    ),
    registered_submission_ids: stringArray(
      object.registered_submission_ids,
      "attempt.usage.registered_submission_ids",
    ),
    registered_verification_record_ids: stringArray(
      object.registered_verification_record_ids,
      "attempt.usage.registered_verification_record_ids",
    ),
  };
}

function parseSource(value: unknown, at: string): AgentRunRequest["target"]["source"] {
  const object = objectAt(value, at);
  exactKeys(object, ["git_object_format", "git_commit", "git_tree"], [], at);
  const format = stringAt(object.git_object_format, `${at}.git_object_format`, {
    min: 4,
    max: 6,
  });
  if (format !== "sha1" && format !== "sha256") {
    throw new Error(`${at}.git_object_format must be sha1 or sha256`);
  }
  return {
    git_object_format: format,
    git_commit: gitObjectAt(object.git_commit, `${at}.git_commit`),
    git_tree: gitObjectAt(object.git_tree, `${at}.git_tree`),
  };
}

function parseHelperBuild(value: unknown): AgentHelperBuild {
  const object = objectAt(value, "Agent helper build");
  exactKeys(object, ["schema", "platform", "runtime", "bundle"], [], "Agent helper build");
  if (object.schema !== "vela.agent-helper-build.v1") {
    throw new Error("Agent helper build schema is unsupported");
  }
  const runtime = objectAt(object.runtime, "Agent helper build.runtime");
  exactKeys(runtime, ["kind", "version", "size", "sha256"], [], "Agent helper build.runtime");
  if (runtime.kind !== "bun" || runtime.version !== "1.3.12") {
    throw new Error("Agent helper build requires Bun 1.3.12");
  }
  const bundle = objectAt(object.bundle, "Agent helper build.bundle");
  exactKeys(bundle, ["format", "size", "sha256"], [], "Agent helper build.bundle");
  if (bundle.format !== "esm") {
    throw new Error("Agent helper bundle format must be esm");
  }
  return {
    schema: "vela.agent-helper-build.v1",
    platform: stringAt(object.platform, "Agent helper build.platform", { min: 5, max: 64 }),
    runtime: {
      kind: "bun",
      version: "1.3.12",
      size: integerAt(runtime.size, "Agent helper build.runtime.size", 1, 536_870_912),
      sha256: sha256At(runtime.sha256, "Agent helper build.runtime.sha256"),
    },
    bundle: {
      format: "esm",
      size: integerAt(bundle.size, "Agent helper build.bundle.size", 1, 8 * 1024 * 1024),
      sha256: sha256At(bundle.sha256, "Agent helper build.bundle.sha256"),
    },
  };
}

function executionArtifactContract(
  bundle: Record<string, unknown>,
  mission: Record<string, unknown>,
): { path: string; kind: string } {
  const contract = objectAt(bundle.artifact_contract, "execution bundle artifact_contract");
  exactKeys(contract, ["path", "kind"], [], "execution bundle artifact_contract");
  const artifact = {
    path: relativePathAt(contract.path, "execution bundle artifact_contract.path"),
    kind: stringAt(contract.kind, "execution bundle artifact_contract.kind", {
      min: 1,
      max: 128,
    }),
  };
  const allowedPaths = arrayAt(
    mission.allowed_paths,
    "execution bundle mission.allowed_paths",
    { min: 1, max: 16, unique: true },
    (value, at) => relativePathAt(value, at),
  );
  if (allowedPaths.length !== 1 || allowedPaths[0] !== artifact.path) {
    throw new Error("execution bundle Artifact contract and mission allowed_paths disagree");
  }
  return artifact;
}

export function parseAgentRunRequest(value: unknown): AgentRunRequest {
  const object = objectAt(value, "Agent run request");
  exactKeys(
    object,
    [
      "schema",
      "authority",
      "effect",
      "request_root",
      "frontier",
      "attempt",
      "target",
      "execution_bundle",
      "runner_build",
      "output_root",
    ],
    [],
    "Agent run request",
  );
  if (object.schema !== REQUEST_SCHEMA || object.authority !== "none" || object.effect !== "none") {
    throw new Error("Agent run request must be authority-free and effect-free");
  }
  const requestRoot = sha256At(object.request_root, "Agent run request.request_root");
  const { request_root: _, ...preimage } = object;
  if (protocolDigest(preimage) !== requestRoot) {
    throw new Error("Agent run request root does not match its exact preimage");
  }

  const frontier = objectAt(object.frontier, "Agent run request.frontier");
  exactKeys(frontier, ["path", "id", "origin_id", "repository_root"], [], "frontier");
  const frontierPath = stringAt(frontier.path, "frontier.path", { min: 1, max: 4096 });
  if (!path.isAbsolute(frontierPath)) throw new Error("frontier.path must be absolute");

  const attempt = objectAt(object.attempt, "Agent run request.attempt");
  exactKeys(
    attempt,
    [
      "id",
      "authorization_root",
      "actor",
      "created_at",
      "expires_at",
      "runner_build_root",
      "allowed_operations",
      "allowed_artifact_classes",
      "budget",
      "usage",
      "consequence_ceiling",
      "task_contract_root",
      "controller_build",
    ],
    [],
    "attempt",
  );
  const consequence = stringAt(attempt.consequence_ceiling, "attempt.consequence_ceiling", {
    min: 1,
    max: 32,
  });
  if (consequence !== "evidence_only" && consequence !== "pending_review") {
    throw new Error("attempt.consequence_ceiling is unsupported");
  }

  const target = objectAt(object.target, "Agent run request.target");
  exactKeys(
    target,
    [
      "id",
      "binding_root",
      "target_index_root",
      "input_root",
      "source",
      "claim_read_set",
      "packet",
      "packet_json",
    ],
    [],
    "target",
  );
  const packet = objectAt(target.packet, "Agent run request.target.packet");
  exactKeys(packet, ["schema", "path", "size", "sha256"], [], "target.packet");

  const execution = objectAt(object.execution_bundle, "Agent run request.execution_bundle");
  exactKeys(execution, ["reference", "value", "mission"], [], "execution_bundle");
  const referenceObject = objectAt(execution.reference, "execution_bundle.reference");
  exactKeys(
    referenceObject,
    ["schema", "path", "size", "sha256"],
    [],
    "execution_bundle.reference",
  );
  if (referenceObject.schema !== BUNDLE_SCHEMA) {
    throw new Error(`execution_bundle.reference.schema must be ${BUNDLE_SCHEMA}`);
  }
  const reference = {
    schema: BUNDLE_SCHEMA,
    ...rootedFile(
      {
        path: referenceObject.path,
        size: referenceObject.size,
        sha256: referenceObject.sha256,
      },
      "execution_bundle.reference",
    ),
  } as const;
  const bundle = objectAt(execution.value, "execution_bundle.value");
  const mission = objectAt(execution.mission, "execution_bundle.mission");
  if (
    bundle.schema !== BUNDLE_SCHEMA ||
    bundle.authority !== "non_authoritative" ||
    bundle.effect !== "none"
  ) {
    throw new Error("execution bundle must be non-authoritative and effect-free");
  }
  const packetJson = stringAt(target.packet_json, "target.packet_json", {
    min: 2,
    max: 8 * 1024 * 1024,
  });
  if (
    Buffer.byteLength(packetJson, "utf8") !== packet.size ||
    sha256Bytes(packetJson) !== packet.sha256
  ) {
    throw new Error("inline Target packet bytes do not match their exact reference");
  }
  let packetValue: Record<string, unknown>;
  try {
    packetValue = objectAt(JSON.parse(packetJson) as unknown, "target.packet_json");
  } catch (error) {
    throw new Error(`inline Target packet is invalid JSON: ${String(error)}`);
  }
  const packetBundle = objectAt(packetValue.execution_bundle, "target.packet_value.execution_bundle");
  if (contentDigest(packetBundle) !== contentDigest(reference)) {
    throw new Error("Target packet does not select the exact execution bundle");
  }
  const bundleTarget = objectAt(bundle.target, "execution_bundle.value.target");
  const targetId = stringAt(target.id, "target.id", { min: 1, max: 256 });
  if (bundleTarget.id !== targetId || mission.target !== targetId) {
    throw new Error("execution bundle or mission substituted another Target");
  }
  if (
    mission.actor !== attempt.actor ||
    mission.frontier !== "." ||
    mission.role !== "producer"
  ) {
    throw new Error("mission does not match the exact producer Attempt");
  }
  executionArtifactContract(bundle, mission);
  const missionRef = rootedFile(bundle.mission, "execution_bundle.value.mission");
  if (
    Buffer.byteLength(canonicalJson(bundle), "utf8") !== reference.size ||
    contentDigest(bundle) !== reference.sha256
  ) {
    throw new Error("inline execution bundle bytes do not match their exact reference");
  }
  if (
    Buffer.byteLength(canonicalJson(mission), "utf8") !== missionRef.size ||
    contentDigest(mission) !== missionRef.sha256
  ) {
    throw new Error("inline mission bytes do not match the execution bundle");
  }
  const safeguards = objectAt(bundle.safeguards, "execution_bundle.value.safeguards");
  if (
    contentDigest(safeguards.worker_inputs) !== contentDigest(["mission", "target_packet"]) ||
    !Array.isArray(safeguards.prior_answer_inputs) ||
    safeguards.prior_answer_inputs.length !== 0 ||
    safeguards.duplicate_work !== "target_revalidation"
  ) {
    throw new Error("execution bundle does not enforce the closed worker input boundary");
  }
  const verifier = objectAt(bundle.verifier, "execution_bundle.value.verifier");
  const isolation = objectAt(verifier.isolation, "execution_bundle.value.verifier.isolation");
  if (isolation.network !== "deny" || isolation.writes !== "deny") {
    throw new Error("execution bundle verifier must deny network and writes");
  }

  const outputRoot = object.output_root === null
    ? null
    : stringAt(object.output_root, "output_root", { min: 1, max: 4096 });
  if (outputRoot !== null && !path.isAbsolute(outputRoot)) {
    throw new Error("output_root must be absolute");
  }

  const controller = objectAt(attempt.controller_build, "attempt.controller_build");
  exactKeys(
    controller,
    ["program", "version", "binary_sha256"],
    [],
    "attempt.controller_build",
  );
  const runnerBuild = parseHelperBuild(object.runner_build);
  const runnerBuildRoot = sha256At(attempt.runner_build_root, "attempt.runner_build_root");
  if (protocolDigest(runnerBuild) !== runnerBuildRoot) {
    throw new Error("Agent helper build does not match the Attempt runner root");
  }

  return {
    schema: REQUEST_SCHEMA,
    authority: "none",
    effect: "none",
    request_root: requestRoot,
    frontier: {
      path: frontierPath,
      id: stringAt(frontier.id, "frontier.id", { min: 1, max: 256 }),
      origin_id: stringAt(frontier.origin_id, "frontier.origin_id", { min: 1, max: 256 }),
      repository_root: sha256At(frontier.repository_root, "frontier.repository_root"),
    },
    attempt: {
      id: stringAt(attempt.id, "attempt.id", {
        min: 68,
        max: 68,
        pattern: /^vat_[0-9a-f]{64}$/u,
      }),
      authorization_root: sha256At(attempt.authorization_root, "attempt.authorization_root"),
      actor: stringAt(attempt.actor, "attempt.actor", { min: 1, max: 256 }),
      created_at: stringAt(attempt.created_at, "attempt.created_at", { min: 20, max: 64 }),
      expires_at: stringAt(attempt.expires_at, "attempt.expires_at", { min: 20, max: 64 }),
      controller_build: {
        program: stringAt(controller.program, "attempt.controller_build.program", {
          min: 1,
          max: 128,
        }),
        version: stringAt(controller.version, "attempt.controller_build.version", {
          min: 1,
          max: 128,
        }),
        binary_sha256: sha256At(
          controller.binary_sha256,
          "attempt.controller_build.binary_sha256",
        ),
      },
      runner_build_root: runnerBuildRoot,
      allowed_operations: stringArray(attempt.allowed_operations, "attempt.allowed_operations"),
      allowed_artifact_classes: stringArray(
        attempt.allowed_artifact_classes,
        "attempt.allowed_artifact_classes",
      ),
      budget: parseBudget(attempt.budget),
      usage: parseUsage(attempt.usage),
      consequence_ceiling: consequence,
      task_contract_root: sha256At(attempt.task_contract_root, "attempt.task_contract_root"),
    },
    target: {
      id: targetId,
      binding_root: sha256At(target.binding_root, "target.binding_root"),
      target_index_root: sha256At(target.target_index_root, "target.target_index_root"),
      input_root: sha256At(target.input_root, "target.input_root"),
      source: parseSource(target.source, "target.source"),
      claim_read_set: parseSource(target.claim_read_set, "target.claim_read_set"),
      packet: {
        schema: stringAt(packet.schema, "target.packet.schema", { min: 1, max: 256 }),
        path: relativePathAt(packet.path, "target.packet.path"),
        size: integerAt(packet.size, "target.packet.size", 1, 8 * 1024 * 1024),
        sha256: sha256At(packet.sha256, "target.packet.sha256"),
      },
      packet_json: packetJson,
    },
    execution_bundle: {
      reference,
      value: bundle,
      mission,
    },
    runner_build: runnerBuild,
    output_root: outputRoot,
  };
}

function defaultOutput(frontier: string): string {
  const stamp = new Date().toISOString().replaceAll(":", "-").replaceAll(".", "-");
  return path.join(os.homedir(), ".vela", "agent", "runs", path.basename(frontier), stamp);
}

function assertAuthorityEnvironmentIsAbsent(environment: NodeJS.ProcessEnv = process.env): void {
  if (environment.VELA_NO_KEY_ACCESS !== "1") {
    throw new Error("Vela Agent requires VELA_NO_KEY_ACCESS=1");
  }
  const forbidden = Object.keys(environment).filter(
    (name) =>
      name.startsWith("SSH_") ||
      name.startsWith("VELA_REPOSITORY_AUTHORITY") ||
      ["VELA_AGENT_KEY_HEX", "VELA_KEY_PATH", "VELA_AUTHORITY_KEY", "VELA_HUMAN_KEY"].includes(
        name,
      ),
  );
  if (forbidden.length !== 0) {
    throw new Error(`Vela Agent received forbidden authority environment: ${forbidden.sort().join(", ")}`);
  }
}

async function pinnedGitBlob(options: {
  request: AgentRunRequest;
  locator: RootedFile;
  runner: CommandRunner;
  runtimeHome: string;
  label: string;
}): Promise<Buffer> {
  const expression = `${options.request.target.source.git_commit}:${options.locator.path}`;
  const sizeResult = await options.runner({
    argv: ["git", "cat-file", "-s", expression],
    cwd: options.request.frontier.path,
    env: isolatedEnvironment(options.runtimeHome),
    timeoutMs: 30_000,
    maxOutputBytes: 4096,
  });
  const size = Number.parseInt(sizeResult.stdout.toString("utf8").trim(), 10);
  if (
    sizeResult.exitCode !== 0 ||
    sizeResult.stderr.length !== 0 ||
    !Number.isSafeInteger(size) ||
    size !== options.locator.size
  ) {
    throw new Error(`${options.label} Git blob size does not match the request`);
  }
  const result = await options.runner({
    argv: ["git", "cat-file", "blob", expression],
    cwd: options.request.frontier.path,
    env: isolatedEnvironment(options.runtimeHome),
    timeoutMs: 120_000,
    maxOutputBytes: options.locator.size,
  });
  if (result.exitCode !== 0 || result.stderr.length !== 0 || result.stdout.length !== size) {
    throw new Error(`${options.label} Git blob could not be read exactly`);
  }
  if (sha256Bytes(result.stdout) !== options.locator.sha256) {
    throw new Error(`${options.label} Git blob root does not match the request`);
  }
  return result.stdout;
}

function hostPlatform(): string {
  return `${process.platform}-${process.arch}`;
}

export async function observedHelperBuild(helperBinary: string): Promise<AgentHelperBuild> {
  const runtime = await realpath(process.execPath);
  const bundle = await realpath(helperBinary);
  const bunVersion = (process.versions as Record<string, string | undefined>).bun;
  if (bunVersion !== "1.3.12") {
    throw new Error(`Vela Agent requires Bun 1.3.12, observed ${bunVersion ?? "no Bun runtime"}`);
  }
  const [runtimeStat, bundleStat, runtimeRoot, bundleRoot] = await Promise.all([
    stat(runtime),
    stat(bundle),
    sha256RegularFile(runtime, MAX_EXECUTABLE_BYTES),
    sha256RegularFile(bundle, 8 * 1024 * 1024),
  ]);
  return {
    schema: "vela.agent-helper-build.v1",
    platform: hostPlatform(),
    runtime: {
      kind: "bun",
      version: "1.3.12",
      size: runtimeStat.size,
      sha256: runtimeRoot,
    },
    bundle: {
      format: "esm",
      size: bundleStat.size,
      sha256: bundleRoot,
    },
  };
}

export async function runAttemptProduct(
  raw: unknown,
  options: {
    helperBinary: string;
    supportAssets: AgentSupportAssets;
    runner?: CommandRunner;
  },
): Promise<AgentProductRunResult> {
  assertToolUsingMissionPlatform();
  assertAuthorityEnvironmentIsAbsent();
  const request = parseAgentRunRequest(raw);
  const observedBuild = await observedHelperBuild(options.helperBinary);
  if (
    protocolDigest(observedBuild) !== request.attempt.runner_build_root ||
    protocolDigest(observedBuild) !== protocolDigest(request.runner_build)
  ) {
    throw new Error("Agent helper runtime or bundle bytes do not match the Attempt");
  }
  if (request.runner_build.platform !== hostPlatform()) {
    throw new Error("Agent helper build targets another host platform");
  }
  if (!request.attempt.allowed_operations.includes("run_tool") ||
      !request.attempt.allowed_operations.includes("write_private_artifact")) {
    throw new Error("Attempt does not authorize Agent execution");
  }
  const expiresAt = Date.parse(request.attempt.expires_at);
  if (!Number.isFinite(expiresAt) || expiresAt <= Date.now()) {
    throw new Error("Attempt expired before Agent execution");
  }
  const frontier = await realpath(request.frontier.path);
  if (frontier !== request.frontier.path) {
    throw new Error("Agent request Frontier path is not canonical");
  }
  const runtimeHome = await mkdtemp(path.join(os.tmpdir(), "vela-agent-run-"));
  const runner = options.runner ?? runCommand;
  const outputRoot = path.resolve(request.output_root ?? defaultOutput(frontier));
  try {
    const bundle = request.execution_bundle.value;
    const verifier = objectAt(bundle.verifier, "execution bundle verifier");
    const runtime = objectAt(verifier.runtime, "execution bundle verifier.runtime");
    const supportedHosts = stringArray(runtime.supported_hosts, "verifier.runtime.supported_hosts");
    if (!supportedHosts.includes(hostPlatform())) {
      throw new Error(
        `execution bundle does not support ${hostPlatform()}; supported hosts: ${supportedHosts.join(", ")}`,
      );
    }
    const capsule = rootedFile(verifier.capsule, "execution bundle verifier.capsule");
    const source = rootedFile(verifier.source, "execution bundle verifier.source");
    const platform = stringAt(runtime.verifier_platform, "verifier.runtime.verifier_platform", {
      min: 1,
      max: 64,
    });
    if (platform !== "linux/amd64" && platform !== "linux/arm64") {
      throw new Error("verifier.runtime.verifier_platform is unsupported");
    }
    const [capsuleBytes] = await Promise.all([
      pinnedGitBlob({ request, locator: capsule, runner, runtimeHome, label: "verifier capsule" }),
      pinnedGitBlob({ request, locator: source, runner, runtimeHome, label: "verifier source" }),
    ]);
    const missionRef = rootedFile(bundle.mission, "execution bundle mission");
    const missionDirectory = path.posix.dirname(missionRef.path);
    const missionVerifier = objectAt(
      request.execution_bundle.mission.verifier,
      "execution bundle mission.verifier",
    );
    const capsuleRelative = relativePathAt(
      missionVerifier.capsule_path,
      "execution bundle mission.verifier.capsule_path",
    );
    if (path.posix.join(missionDirectory, capsuleRelative) !== capsule.path) {
      throw new Error("mission capsule path does not select the rooted verifier capsule");
    }
    const missionBudgets = objectAt(
      request.execution_bundle.mission.budgets,
      "execution bundle mission.budgets",
    );
    const artifactKind = executionArtifactContract(
      bundle,
      request.execution_bundle.mission,
    ).kind;
    if (!request.attempt.allowed_artifact_classes.includes(artifactKind)) {
      throw new Error(`execution bundle Artifact class ${artifactKind} is outside the Attempt`);
    }
    const missionArtifactBytes = integerAt(
      missionBudgets.max_artifact_bytes,
      "mission.budgets.max_artifact_bytes",
      1,
      1_073_741_824,
    );
    const remainingArtifactBytes =
      request.attempt.budget.max_artifact_bytes - request.attempt.usage.artifact_bytes;
    if (
      request.attempt.usage.artifacts >= request.attempt.budget.max_artifacts ||
      missionArtifactBytes > remainingArtifactBytes
    ) {
      throw new Error("mission exceeds the remaining Attempt Artifact budget");
    }
    const missionWallTime = integerAt(
      missionBudgets.max_research_wall_time_ms,
      "mission.budgets.max_research_wall_time_ms",
      100,
      3_600_000,
    );
    if (Date.now() + missionWallTime >= expiresAt) {
      throw new Error("Attempt expires before the bounded mission wall-time ceiling");
    }

    const [vela, codex, docker] = await Promise.all([
      runtimeIdentity({ name: "vela", cwd: frontier, home: runtimeHome, runner }),
      runtimeIdentity({ name: "codex", cwd: frontier, home: runtimeHome, runner }),
      runtimeIdentity({ name: "docker", cwd: frontier, home: runtimeHome, runner }),
    ]);
    if (
      request.attempt.controller_build.program !== "vela-cli" ||
      request.attempt.controller_build.version !== parseVelaVersionOutput(vela.version) ||
      request.attempt.controller_build.binary_sha256 !== vela.sha256
    ) {
      throw new Error("Vela controller bytes changed after the Attempt was authorized");
    }
    const supportRoot = path.join(runtimeHome, "support");
    await mkdir(supportRoot, { recursive: true, mode: 0o700 });
    const permissionProfile = path.join(supportRoot, "native-worker.toml");
    const outputSchema = path.join(supportRoot, "engine-output.v0.json");
    const selectedPermissionProfile = process.platform === "linux"
      ? options.supportAssets.linuxPermissionProfile
      : options.supportAssets.macosPermissionProfile;
    await Promise.all([
      writeFile(permissionProfile, selectedPermissionProfile, { flag: "wx", mode: 0o400 }),
      writeFile(outputSchema, options.supportAssets.engineOutputSchema, {
        flag: "wx",
        mode: 0o400,
      }),
    ]);
    const custody = await runNativeCustodyPreflight({
      binary: codex.binary,
      permissionProfile,
      runner,
    });
    if (custody.codex_sha256 !== codex.sha256 || custody.codex_version !== codex.version) {
      throw new Error("Agent custody preflight and Codex runtime identity disagree");
    }

    await assertFreshOutput(outputRoot, frontier);
    await mkdir(outputRoot, { recursive: true, mode: 0o700 });
    const draftRoot = path.join(runtimeHome, "draft");
    const stagedCapsule = path.join(draftRoot, capsuleRelative);
    await mkdir(path.dirname(stagedCapsule), { recursive: true, mode: 0o700 });
    await writeFile(stagedCapsule, capsuleBytes, { flag: "wx", mode: 0o500 });
    await chmod(stagedCapsule, 0o500);

    const bundleRoot = path.join(outputRoot, "mission");
    const prepared = await prepareMission({
      draft: request.execution_bundle.mission,
      draftRoot,
      sourceRepo: frontier,
      outputRoot: bundleRoot,
      velaBinary: vela.binary,
      codexBinary: codex.binary,
      dockerBinary: docker.binary,
      verifierImage: stringAt(verifier.image, "execution bundle verifier.image", {
        min: 1,
        max: 512,
      }),
      verifierPlatform: platform,
      outputSchema,
      permissionProfile,
      targetPacket: {
        target: request.target.id,
        schema: request.target.packet.schema,
      },
      landing: { expected_routes: ["defer"], max_accepted_delta: 0 },
      runner,
    });
    if (
      prepared.mission.roots.git_commit !== request.target.claim_read_set.git_commit ||
      prepared.mission.roots.git_tree !== request.target.claim_read_set.git_tree ||
      prepared.mission.roots.vela_repository !== request.frontier.repository_root ||
      prepared.mission.target_packet.path !== request.target.packet.path ||
      prepared.mission.target_packet.sha256 !== request.target.packet.sha256
    ) {
      throw new Error("Frontier read set changed before Agent execution");
    }
    const runRoot = path.join(outputRoot, "run");
    const velaClient = new VelaClient({
      binary: vela.binary,
      expectedVersion: parseVelaVersionOutput(vela.version),
      expectedSha256: vela.sha256,
      home: path.join(runRoot, "vela-home"),
      runner,
    });
    const engine = new CodexToolsNativeEngine({
      binary: codex.binary,
      authHome: path.resolve(process.env.CODEX_HOME ?? path.join(os.homedir(), ".codex")),
      outputSchema: path.join(bundleRoot, "contract", "engine-output.v0.json"),
      permissionProfile: path.join(bundleRoot, prepared.mission.worker.permission_profile_path),
      runner,
    });
    const run = await runCanopus({
      mission: prepared.mission,
      sourceRepo: frontier,
      runRoot,
      vela: velaClient,
      engine,
      bundleRoot,
      dockerBinary: docker.binary,
      verifierRunner: runner,
    });
    const evidence = await writeEvidenceManifest(run, contentDigest(prepared.mission), {
      attempt_id: request.attempt.id,
      attempt_authorization_root: request.attempt.authorization_root,
      task_contract_root: request.attempt.task_contract_root,
      target_binding_root: request.target.binding_root,
      target_packet_root: request.target.packet.sha256,
      execution_bundle_root: request.execution_bundle.reference.sha256,
      runner_build_root: request.attempt.runner_build_root,
      request_root: request.request_root,
    });
    return {
      run,
      output_root: outputRoot,
      bundle_root: bundleRoot,
      evidence_manifest: evidence.file,
      evidence_root: evidence.root,
      target: {
        id: request.target.id,
        binding_root: request.target.binding_root,
        target_index_root: request.target.target_index_root,
        input_root: request.target.input_root,
        packet_root: request.target.packet.sha256,
        source: request.target.source,
        claim_read_set: request.target.claim_read_set,
      },
      source_state: {
        state: "unchanged",
        git_object_format: request.target.claim_read_set.git_object_format,
        commit: prepared.mission.roots.git_commit,
        tree: prepared.mission.roots.git_tree,
      },
      submission: null,
      attempt_id: request.attempt.id,
      execution_bundle_root: request.execution_bundle.reference.sha256,
      request_root: request.request_root,
    };
  } finally {
    await rm(runtimeHome, { recursive: true, force: true });
  }
}
