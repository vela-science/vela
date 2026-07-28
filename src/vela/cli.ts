import path from "node:path";

import type {
  Mission,
  MissionRoots,
} from "../contracts/mission.js";
import {
  GIT_OBJECT_RE,
  SHA256_RE,
  objectAt,
  relativePathAt,
  stringAt,
} from "../contracts/validation.js";
import {
  isolatedEnvironment,
  runCommand,
  type CommandOptions,
  type CommandResult,
  type CommandRunner,
} from "../util/command.js";
import { sha256Bytes } from "../util/canonical.js";
import { MAX_EXECUTABLE_BYTES, sha256RegularFile } from "../util/files.js";
import type { VelaCommandResponse, VelaInspection } from "./types.js";

export type { CommandRunner } from "../util/command.js";

// Large frontiers can spend several minutes in Vela's recoverable transaction
// preparation, exact replay, and derived-view materialization. This ceiling is
// bounded independently from model, verifier, and mission budgets: it prevents
// Canopus from killing a healthy authority transaction while still stopping a
// stalled Vela subprocess.
export const DEFAULT_VELA_COMMAND_TIMEOUT_MS = 600_000;

export class VelaClientError extends Error {
  public readonly code:
    | "command_failed"
    | "malformed_output"
    | "version_mismatch"
    | "root_mismatch"
    | "unexpected_route";

  public constructor(
    code: VelaClientError["code"],
    message: string,
  ) {
    super(message);
    this.name = "VelaClientError";
    this.code = code;
  }
}

export interface VelaClientOptions {
  binary: string;
  expectedVersion: string;
  expectedSha256: string;
  home: string;
  repositoryAuthorityAgentSocket?: string;
  maxOutputBytes?: number;
  timeoutMs?: number;
  runner?: CommandRunner;
}

function parseJsonObject(stdout: Buffer, command: string): Record<string, unknown> {
  let value: unknown;
  try {
    value = JSON.parse(stdout.toString("utf8")) as unknown;
  } catch (error) {
    throw new VelaClientError(
      "malformed_output",
      `${command} did not return one JSON value: ${String(error)}`,
    );
  }
  try {
    return objectAt(value, command);
  } catch (error) {
    throw new VelaClientError("malformed_output", String(error));
  }
}

function safeFailureMessage(value: unknown): string | undefined {
  if (typeof value !== "string" || value.length === 0) return undefined;
  const normalized = value
    .replace(/[\u0000-\u001f\u007f]+/gu, " ")
    .replace(/\bBearer\s+[A-Za-z0-9._~+\/-]+=*/giu, "Bearer [redacted]")
    .replace(/\b(?:sk|sess|key)-[A-Za-z0-9_-]{8,}\b/gu, "[secret-redacted]")
    .replace(/\s+/gu, " ")
    .trim();
  return [...normalized].slice(0, 512).join("");
}

function commandFailureSummary(result: CommandResult): string {
  const diagnostics: string[] = [];
  for (const bytes of [result.stdout, result.stderr]) {
    try {
      const parsed = JSON.parse(bytes.toString("utf8")) as unknown;
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) continue;
      const object = parsed as Record<string, unknown>;
      const direct = safeFailureMessage(object.error) ?? safeFailureMessage(object.message);
      if (direct !== undefined && !diagnostics.includes(direct)) diagnostics.push(direct);
      const nested = object.error;
      if (typeof nested === "object" && nested !== null && !Array.isArray(nested)) {
        const message = safeFailureMessage((nested as Record<string, unknown>).message);
        if (message !== undefined && !diagnostics.includes(message)) diagnostics.push(message);
      }
      const integrity = object.state_integrity;
      if (typeof integrity === "object" && integrity !== null && !Array.isArray(integrity)) {
        const errors = (integrity as Record<string, unknown>).structural_errors;
        if (Array.isArray(errors)) {
          for (const error of errors) {
            if (typeof error !== "object" || error === null || Array.isArray(error)) continue;
            const message = safeFailureMessage((error as Record<string, unknown>).message);
            if (message !== undefined && !diagnostics.includes(message)) diagnostics.push(message);
            if (diagnostics.length === 2) break;
          }
        }
      }
    } catch {
      // Only documented structured error fields are eligible for display.
    }
  }
  return [
    ...(diagnostics.length === 0 ? ["no structured Vela failure message"] : diagnostics.slice(0, 2)),
    `stdout_sha256=${sha256Bytes(result.stdout)}`,
    `stderr_sha256=${sha256Bytes(result.stderr)}`,
  ].join("; ");
}

function normalizeSha256(value: unknown, at: string): string {
  if (typeof value !== "string") {
    throw new VelaClientError("malformed_output", `${at} must be a SHA-256 string`);
  }
  const normalized = value.startsWith("sha256:") ? value : `sha256:${value}`;
  if (!SHA256_RE.test(normalized)) {
    throw new VelaClientError("malformed_output", `${at} is not a full SHA-256 root`);
  }
  return normalized;
}

function frontierPath(value: string): string {
  return value === "." ? "." : relativePathAt(value, "frontier");
}

function fieldObject(parent: Record<string, unknown>, key: string, at: string): Record<string, unknown> {
  try {
    return objectAt(parent[key], `${at}.${key}`);
  } catch (error) {
    throw new VelaClientError("malformed_output", String(error));
  }
}

function assertEqual(actual: string, expected: string, label: string): void {
  if (actual !== expected) {
    throw new VelaClientError(
      "root_mismatch",
      `${label} mismatch: expected ${expected}, observed ${actual}`,
    );
  }
}

function compareRoots(actual: MissionRoots, expected: MissionRoots): void {
  assertEqual(actual.git_commit, expected.git_commit, "Git commit");
  assertEqual(actual.git_tree, expected.git_tree, "Git tree");
  assertEqual(actual.vela_repository, expected.vela_repository, "Vela repository");
}

function nonnegativeInteger(value: unknown, at: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new VelaClientError("malformed_output", `${at} must be a nonnegative integer`);
  }
  return value;
}

export class VelaClient {
  readonly #binary: string;
  readonly #expectedVersion: string;
  readonly #expectedSha256: string;
  readonly #home: string;
  readonly #maxOutputBytes: number;
  readonly #timeoutMs: number;
  readonly #env: NodeJS.ProcessEnv;
  readonly #runner: CommandRunner;

  public constructor(options: VelaClientOptions) {
    this.#binary = options.binary;
    this.#expectedVersion = options.expectedVersion;
    this.#expectedSha256 = options.expectedSha256;
    this.#home = options.home;
    this.#maxOutputBytes = options.maxOutputBytes ?? 16 * 1024 * 1024;
    this.#timeoutMs = options.timeoutMs ?? DEFAULT_VELA_COMMAND_TIMEOUT_MS;
    // The control lane accepts only the standard repository-authority agent
    // socket. It never accepts caller-supplied environment entries or key
    // bytes, and this environment is not reused by the Codex worker or
    // verifier.
    const baseEnvironment = isolatedEnvironment(options.home);
    if (options.repositoryAuthorityAgentSocket === undefined) {
      this.#env = baseEnvironment;
    } else {
      if (
        !path.isAbsolute(options.repositoryAuthorityAgentSocket) ||
        options.repositoryAuthorityAgentSocket.includes("\0")
      ) {
        throw new VelaClientError(
          "malformed_output",
          "repository authority agent socket must be one absolute path",
        );
      }
      this.#env = {
        ...baseEnvironment,
        SSH_AUTH_SOCK: options.repositoryAuthorityAgentSocket,
      };
    }
    this.#runner = options.runner ?? runCommand;
  }

  async #execute(argv: readonly string[], cwd: string): Promise<CommandResult> {
    const result = await this.#runner({
      argv,
      cwd,
      env: this.#env,
      timeoutMs: this.#timeoutMs,
      maxOutputBytes: this.#maxOutputBytes,
    });
    if (result.exitCode !== 0) {
      throw new VelaClientError(
        "command_failed",
        `${argv[0]} ${argv.slice(1).join(" ")} exited ${result.exitCode}: ${commandFailureSummary(result)}`,
      );
    }
    return result;
  }

  async #json(args: readonly string[], cwd: string, label: string): Promise<Record<string, unknown>> {
    const result = await this.#execute([this.#binary, ...args], cwd);
    return parseJsonObject(result.stdout, label);
  }

  public async assertVersion(cwd: string): Promise<string> {
    const binaryDigest = await sha256RegularFile(this.#binary, MAX_EXECUTABLE_BYTES);
    if (binaryDigest !== this.#expectedSha256) {
      throw new VelaClientError(
        "version_mismatch",
        `Vela binary digest mismatch: expected ${this.#expectedSha256}, observed ${binaryDigest}`,
      );
    }
    const result = await this.#execute([this.#binary, "--version"], cwd);
    if (result.stderr.length !== 0) {
      throw new VelaClientError("malformed_output", "vela --version wrote to stderr");
    }
    const observed = result.stdout.toString("utf8").trim();
    const expected = `vela ${this.#expectedVersion}`;
    if (observed !== expected) {
      throw new VelaClientError(
        "version_mismatch",
        `expected ${expected}, observed ${JSON.stringify(observed)}`,
      );
    }
    return this.#expectedVersion;
  }

  async #gitObject(repoRoot: string, expression: "HEAD^{commit}" | "HEAD^{tree}"): Promise<string> {
    const result = await this.#execute(["git", "rev-parse", "--verify", expression], repoRoot);
    const observed = result.stdout.toString("utf8").trim();
    if (!GIT_OBJECT_RE.test(observed)) {
      throw new VelaClientError(
        "malformed_output",
        `git rev-parse returned a non-full object ID for ${expression}`,
      );
    }
    return observed;
  }

  public async inspect(
    repoRoot: string,
    frontier: string,
  ): Promise<VelaInspection> {
    const safeFrontier = frontierPath(frontier);
    const version = await this.assertVersion(repoRoot);
    const [gitCommit, gitTree, status, repository] = await Promise.all([
      this.#gitObject(repoRoot, "HEAD^{commit}"),
      this.#gitObject(repoRoot, "HEAD^{tree}"),
      this.#json(["status", safeFrontier, "--json"], repoRoot, "vela status"),
      this.#json(
        ["repository", "verify", safeFrontier, "--json"],
        repoRoot,
        "vela repository verify",
      ),
    ]);
    if (
      status.schema !== "vela.status.v1" ||
      status.ok !== true ||
      status.command !== "status"
    ) {
      throw new VelaClientError("malformed_output", "vela status contract identity is invalid");
    }
    if (
      repository.schema !== "vela.repository-verification.v1" ||
      repository.ok !== true ||
      repository.command !== "repository verify"
    ) {
      throw new VelaClientError(
        "malformed_output",
        "vela repository verification contract identity is invalid",
      );
    }
    const statusGit = fieldObject(status, "git", "vela status");
    const integrity = fieldObject(status, "integrity", "vela status");
    const statusRoots = fieldObject(status, "roots", "vela status");
    if (
      integrity.replay !== "verified" ||
      integrity.strict !== "pass" ||
      nonnegativeInteger(integrity.blocker_count, "vela status.integrity.blocker_count") !== 0
    ) {
      throw new VelaClientError(
        "command_failed",
        "Vela repository must replay and pass strict verification before Canopus can run",
      );
    }
    const blockers = fieldObject(integrity, "blockers_by_code", "vela status.integrity");
    if (Object.keys(blockers).length !== 0) {
      throw new VelaClientError(
        "malformed_output",
        "vela status strict pass contains blocker classifications",
      );
    }
    const statusCommit = stringAt(statusGit.commit, "vela status.git.commit", {
      min: 40,
      max: 64,
      pattern: GIT_OBJECT_RE,
    });
    const statusTree = stringAt(statusGit.tree, "vela status.git.tree", {
      min: 40,
      max: 64,
      pattern: GIT_OBJECT_RE,
    });
    assertEqual(statusCommit, gitCommit, "Git/status commit");
    assertEqual(statusTree, gitTree, "Git/status tree");
    assertEqual(
      stringAt(repository.git_commit, "vela repository verify.git_commit", {
        min: 40,
        max: 64,
        pattern: GIT_OBJECT_RE,
      }),
      gitCommit,
      "Git/repository commit",
    );
    assertEqual(
      stringAt(repository.git_tree, "vela repository verify.git_tree", {
        min: 40,
        max: 64,
        pattern: GIT_OBJECT_RE,
      }),
      gitTree,
      "Git/repository tree",
    );
    const repositoryRoot = normalizeSha256(
      repository.repository_root,
      "vela repository verify.repository_root",
    );
    assertEqual(
      normalizeSha256(statusRoots.repository, "vela status.roots.repository"),
      repositoryRoot,
      "status/repository root",
    );
    for (const [statusKey, repositoryKey, label] of [
      ["epoch", "epoch_root", "epoch"],
      ["authority_keyset", "authority_keyset_root", "authority keyset"],
      ["authority_policy", "authority_policy_root", "authority policy"],
    ] as const) {
      assertEqual(
        normalizeSha256(statusRoots[statusKey], `vela status.roots.${statusKey}`),
        normalizeSha256(
          repository[repositoryKey],
          `vela repository verify.${repositoryKey}`,
        ),
        `status/repository ${label}`,
      );
    }
    const statusFrontier = fieldObject(status, "frontier", "vela status");
    assertEqual(
      stringAt(statusFrontier.id, "vela status.frontier.id", { min: 1, max: 64 }),
      stringAt(repository.frontier_id, "vela repository verify.frontier_id", {
        min: 1,
        max: 64,
      }),
      "status/repository frontier",
    );

    return {
      version,
      roots: {
        git_commit: gitCommit,
        git_tree: gitTree,
        vela_repository: repositoryRoot,
      },
      status,
      repository,
    };
  }

  public async assertRoots(
    repoRoot: string,
    frontier: string,
    expected: MissionRoots,
  ): Promise<VelaInspection> {
    const inspection = await this.inspect(repoRoot, frontier);
    compareRoots(inspection.roots, expected);
    return inspection;
  }

  public async next(mission: Mission, repoRoot: string): Promise<VelaCommandResponse> {
    if (mission.vela_version !== this.#expectedVersion) {
      throw new VelaClientError("version_mismatch", "mission and client Vela versions differ");
    }
    return await this.offer(
      repoRoot,
      mission.frontier,
      mission.roots,
    );
  }

  public async offer(
    repoRoot: string,
    frontier: string,
    roots: MissionRoots,
    limit = 128,
  ): Promise<VelaCommandResponse> {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 128) {
      throw new VelaClientError("malformed_output", "vela next limit must be 1..128");
    }
    const safeFrontier = frontierPath(frontier);
    await this.assertRoots(repoRoot, safeFrontier, roots);
    const value = await this.#json(
      ["next", safeFrontier, "--limit", String(limit), "--json"],
      repoRoot,
      "vela next",
    );
    if (value.ok === false) {
      throw new VelaClientError("command_failed", "vela next returned ok=false");
    }
    if (
      value.schema !== "vela.offer.v1" ||
      value.command !== "next" ||
      value.ok !== true
    ) {
      throw new VelaClientError("malformed_output", "vela next contract identity is invalid");
    }
    assertEqual(
      normalizeSha256(value.repository_root, "vela next.repository_root"),
      roots.vela_repository,
      "mission/offer repository",
    );
    return { ok: true, value };
  }
}
