import { constants } from "node:fs";
import { access, realpath } from "node:fs/promises";
import path from "node:path";

import type { TestRecord } from "../contracts/candidate.js";
import type { Mission, MissionV1 } from "../contracts/mission.js";
import { relativePathAt } from "../contracts/validation.js";
import type { BudgetTracker } from "../budget/enforce.js";
import {
  verifyFrozenArtifact,
  type FrozenArtifactLocation,
} from "../artifact/freeze.js";
import {
  CommandFailure,
  isolatedEnvironment,
  runCommand,
  type CommandRunner,
} from "../util/command.js";
import { sha256Bytes } from "../util/canonical.js";
import { readBoundedRegularFile } from "../util/files.js";
import type { WorkspacePaths } from "../workspace/prepare.js";

export class VerifierError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = "VerifierError";
  }
}

export interface VerifierOutcome {
  status: "passed" | "failed" | "error";
  record: TestRecord;
  sandbox: "macos_sandbox" | "container_network_denied";
  error?: string;
}

function safeDockerMount(value: string, label: string): string {
  const resolved = path.resolve(value);
  if (resolved.includes(",") || resolved.includes("\n") || resolved.includes("\0")) {
    throw new VerifierError(`${label} cannot be represented as a Docker mount`);
  }
  return resolved;
}

function dockerBind(source: string, target: string): string {
  return `type=bind,src=${safeDockerMount(source, target)},dst=${target},readonly`;
}

function sbpl(value: string): string {
  return value.replaceAll("\\", "\\\\").replaceAll('"', '\\"');
}

function macSandboxProfile(readRoots: readonly string[], executable: string): string {
  const lines = [
    "(version 1)",
    "(deny default)",
    '(import "dyld-support.sb")',
    "(deny file-link file-clone)",
    "(allow process-info* (target same-sandbox))",
    "(allow signal (target same-sandbox))",
    '(allow sysctl-read (sysctl-name "hw.activecpu") (sysctl-name "hw.logicalcpu") (sysctl-name "hw.ncpu") (sysctl-name "hw.pagesize") (sysctl-name "kern.argmax") (sysctl-name "kern.osproductversion") (sysctl-name "kern.osrelease") (sysctl-name "kern.ostype"))',
    '(allow mach-lookup (global-name "com.apple.system.opendirectoryd.libinfo"))',
    `(allow process-exec (literal "${sbpl(executable)}"))`,
    `(allow file-map-executable (literal "${sbpl(executable)}"))`,
    '(allow file-read* (subpath "/Library/Apple") (subpath "/System") (subpath "/usr/lib") (subpath "/usr/share") (subpath "/private/var/db/timezone") (literal "/dev/null") (literal "/dev/urandom"))',
  ];
  for (const root of [...readRoots, executable]) {
    const operation = root === executable ? "literal" : "subpath";
    lines.push(`(allow file-read* (${operation} "${sbpl(root)}"))`);
    lines.push(`(allow file-read-metadata (literal "${sbpl(root)}"))`);
    for (let parent = path.dirname(root); ; parent = path.dirname(parent)) {
      lines.push(`(allow file-read-metadata (literal "${sbpl(parent)}"))`);
      if (parent === "/") break;
    }
  }
  lines.push(
    '(allow file-write* (literal "/dev/null"))',
    "(deny network*)",
  );
  return lines.join(" ");
}

function replaceArg(
  value: string,
  paths: WorkspacePaths,
  artifacts: readonly FrozenArtifactLocation[],
): string {
  if (value === "{input}") return paths.input;
  const match = /^\{artifact:(.+)\}$/u.exec(value);
  if (match === null) return value;
  const requested = relativePathAt(match[1], "verifier artifact placeholder");
  const found = artifacts.find((entry) => entry.artifact.path === requested);
  if (found === undefined) throw new VerifierError(`verifier requested undeclared artifact ${requested}`);
  return found.frozenPath;
}

function replaceContainerArg(
  value: string,
  artifacts: readonly FrozenArtifactLocation[],
): string {
  if (value === "{input}") return "/input";
  const match = /^\{artifact:(.+)\}$/u.exec(value);
  if (match === null) return value;
  const requested = relativePathAt(match[1], "verifier artifact placeholder");
  const index = artifacts.findIndex((entry) => entry.artifact.path === requested);
  if (index < 0) throw new VerifierError(`verifier requested undeclared artifact ${requested}`);
  return `/artifacts/${index}`;
}

async function runContainerVerifier(options: {
  mission: MissionV1;
  paths: WorkspacePaths;
  artifacts: readonly FrozenArtifactLocation[];
  budget: BudgetTracker;
  bundleRoot: string;
  dockerBinary: string;
  runner: CommandRunner;
}): Promise<VerifierOutcome> {
  const declaredCapsule = relativePathAt(
    options.mission.verifier.capsule_path,
    "mission.verifier.capsule_path",
  );
  const bundleRoot = await realpath(options.bundleRoot);
  const capsule = await realpath(path.resolve(bundleRoot, declaredCapsule));
  if (!capsule.startsWith(`${bundleRoot}${path.sep}`)) {
    throw new VerifierError("verifier capsule escaped the mission bundle");
  }
  await access(capsule, constants.X_OK);
  const capsuleBytes = await readBoundedRegularFile(capsule, 268_435_456);
  const capsuleDigest = sha256Bytes(capsuleBytes);
  if (
    capsuleDigest !== options.mission.verifier.capsule_sha256 ||
    capsuleDigest !== options.mission.verifier.executable_sha256
  ) {
    throw new VerifierError("verifier capsule digest does not match the mission");
  }
  for (const artifact of options.artifacts) {
    await verifyFrozenArtifact(artifact, options.mission.budgets.max_artifact_bytes);
  }

  options.budget.beginProcess();
  const inspect = await options.runner({
    argv: [
      options.dockerBinary,
      "image",
      "inspect",
      "--format={{.Id}}",
      options.mission.verifier.image,
    ],
    cwd: options.paths.verifierHome,
    env: isolatedEnvironment(options.paths.verifierHome),
    timeoutMs: Math.min(30_000, options.budget.remainingTimeMs()),
    maxOutputBytes: 4096,
  });
  if (
    inspect.exitCode !== 0 ||
    inspect.stderr.length !== 0 ||
    inspect.stdout.toString("utf8").trim() !== options.mission.verifier.image
  ) {
    throw new VerifierError("pinned verifier image is unavailable or changed");
  }

  options.budget.beginProcess();
  const logicalArgv = options.mission.verifier.argv.map((item, index) =>
    index === 0 ? "/capsule/verifier" : replaceContainerArg(item, options.artifacts));
  const mounts = [
    "--mount", dockerBind(options.paths.input, "/input"),
    "--mount", dockerBind(capsule, "/capsule/verifier"),
    ...options.artifacts.flatMap((entry, index) => [
      "--mount",
      dockerBind(entry.frozenPath, `/artifacts/${index}`),
    ]),
  ];
  const argv = [
    options.dockerBinary,
    "run",
    ...(options.mission.verifier.platform === undefined
      ? []
      : ["--platform", options.mission.verifier.platform]),
    "--rm",
    "--init",
    "--read-only",
    "--network=none",
    "--cap-drop=ALL",
    "--security-opt=no-new-privileges",
    "--memory=1024m",
    "--cpus=1",
    "--pids-limit=64",
    "--env",
    "HOME=/nonexistent",
    "--env",
    "PYTHONDONTWRITEBYTECODE=1",
    "--workdir",
    `/input/${relativePathAt(options.mission.verifier.cwd, "mission.verifier.cwd")}`,
    ...mounts,
    options.mission.verifier.image,
    ...logicalArgv,
  ];
  const started = performance.now();
  try {
    const result = await options.runner({
      argv,
      cwd: options.paths.verifierHome,
      env: isolatedEnvironment(options.paths.verifierHome),
      timeoutMs: Math.min(
        options.mission.verifier.timeout_ms,
        options.budget.remainingTimeMs(),
      ),
      maxOutputBytes: Math.min(
        options.mission.verifier.max_output_bytes,
        options.budget.remainingOutputBytes(),
      ),
    });
    options.budget.addOutput(result.stdout.length + result.stderr.length);
    for (const artifact of options.artifacts) {
      await verifyFrozenArtifact(artifact, options.mission.budgets.max_artifact_bytes);
    }
    return {
      status: result.exitCode === 0 ? "passed" : "failed",
      record: {
        argv: logicalArgv,
        executable_digest: capsuleDigest,
        exit_code: result.exitCode,
        stdout_digest: sha256Bytes(result.stdout),
        stderr_digest: sha256Bytes(result.stderr),
        duration_ms: result.durationMs,
      },
      sandbox: "container_network_denied",
    };
  } catch (error) {
    for (const artifact of options.artifacts) {
      await verifyFrozenArtifact(artifact, options.mission.budgets.max_artifact_bytes);
    }
    const message = error instanceof Error ? error.message : String(error);
    return {
      status: "error",
      record: {
        argv: logicalArgv,
        executable_digest: capsuleDigest,
        exit_code: -1,
        stdout_digest: sha256Bytes(""),
        stderr_digest: sha256Bytes(message),
        duration_ms: Math.max(0, Math.round(performance.now() - started)),
      },
      sandbox: "container_network_denied",
      error: error instanceof CommandFailure ? `${error.kind}:${message}` : message,
    };
  }
}

export async function runVerifier(options: {
  mission: Mission;
  paths: WorkspacePaths;
  artifacts: readonly FrozenArtifactLocation[];
  budget: BudgetTracker;
  bundleRoot?: string;
  dockerBinary?: string;
  runner?: CommandRunner;
}): Promise<VerifierOutcome> {
  const runner = options.runner ?? runCommand;
  if (options.mission.schema === "canopus.mission.v1") {
    if (options.bundleRoot === undefined) {
      throw new VerifierError("mission v1 requires its portable bundle root");
    }
    return await runContainerVerifier({
      mission: options.mission,
      paths: options.paths,
      artifacts: options.artifacts,
      budget: options.budget,
      bundleRoot: options.bundleRoot,
      dockerBinary: options.dockerBinary ?? "docker",
      runner,
    });
  }
  if (process.platform !== "darwin") {
    throw new VerifierError("v0 requires the macOS sandbox backend; no permissive fallback exists");
  }
  const declaredExecutable = options.mission.verifier.argv[0];
  if (declaredExecutable === undefined) {
    throw new VerifierError("mission has no verifier executable");
  }
  let executable: string;
  try {
    const relativeExecutable = relativePathAt(
      declaredExecutable,
      "mission.verifier.argv[0]",
    );
    executable = await realpath(path.resolve(options.paths.input, relativeExecutable));
    const input = await realpath(options.paths.input);
    if (!executable.startsWith(`${input}${path.sep}`)) {
      throw new Error("verifier capsule escaped the exact input checkout");
    }
    await access(executable, constants.X_OK);
  } catch {
    throw new VerifierError("verifier executable is unavailable");
  }
  const executableBytes = await readBoundedRegularFile(executable, 268_435_456);
  const executableDigest = sha256Bytes(executableBytes);
  if (executableDigest !== options.mission.verifier.executable_sha256) {
    throw new VerifierError("verifier executable digest does not match the mission");
  }
  // macOS exposes its temporary directory through the lexical `/var` symlink
  // while `realpath` returns `/private/var`. Seatbelt matches both stages of
  // that traversal, so authorize the declared roots in both spellings. This is
  // still the same three bounded trees; it does not widen the readable set.
  const declaredReadRoots = [
    options.paths.input,
    options.paths.verifierHome,
    ...new Set(options.artifacts.map((artifact) => path.dirname(artifact.frozenPath))),
  ].map((entry) => path.resolve(entry));
  const readRoots = [
    ...new Set([
      ...declaredReadRoots,
      ...(await Promise.all(declaredReadRoots.map((entry) => realpath(entry)))),
    ]),
  ];
  const cwdRelative = relativePathAt(options.mission.verifier.cwd, "mission.verifier.cwd");
  const cwd = path.resolve(options.paths.input, cwdRelative);
  if (!cwd.startsWith(`${path.resolve(options.paths.input)}${path.sep}`)) {
    throw new VerifierError("verifier cwd escapes the exact input checkout");
  }

  for (const artifact of options.artifacts) {
    await verifyFrozenArtifact(artifact, options.mission.budgets.max_artifact_bytes);
  }
  options.budget.beginProcess();
  const argv = options.mission.verifier.argv.map((item, index) =>
    index === 0 ? executable :
    replaceArg(item, options.paths, options.artifacts),
  );
  const sandboxed = [
    "/usr/bin/sandbox-exec",
    "-p",
    macSandboxProfile(readRoots, executable),
    "--",
    ...argv,
  ];
  const started = performance.now();

  try {
    const result = await runner({
      argv: sandboxed,
      cwd,
      env: isolatedEnvironment(options.paths.verifierHome),
      timeoutMs: Math.min(
        options.mission.verifier.timeout_ms,
        options.budget.remainingTimeMs(),
      ),
      maxOutputBytes: Math.min(
        options.mission.verifier.max_output_bytes,
        options.budget.remainingOutputBytes(),
      ),
    });
    options.budget.addOutput(result.stdout.length + result.stderr.length);
    for (const artifact of options.artifacts) {
      await verifyFrozenArtifact(artifact, options.mission.budgets.max_artifact_bytes);
    }
    return {
      status: result.exitCode === 0 ? "passed" : "failed",
      record: {
        argv,
        executable_digest: executableDigest,
        exit_code: result.exitCode,
        stdout_digest: sha256Bytes(result.stdout),
        stderr_digest: sha256Bytes(result.stderr),
        duration_ms: result.durationMs,
      },
      sandbox: "macos_sandbox",
    };
  } catch (error) {
    for (const artifact of options.artifacts) {
      await verifyFrozenArtifact(artifact, options.mission.budgets.max_artifact_bytes);
    }
    const message = error instanceof Error ? error.message : String(error);
    return {
      status: "error",
      record: {
        argv,
        executable_digest: executableDigest,
        exit_code: -1,
        stdout_digest: sha256Bytes(""),
        stderr_digest: sha256Bytes(message),
        duration_ms: Math.max(0, Math.round(performance.now() - started)),
      },
      sandbox: "macos_sandbox",
      error: error instanceof CommandFailure ? `${error.kind}:${message}` : message,
    };
  }
}
