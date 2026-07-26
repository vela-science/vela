import path from "node:path";

import type { FrozenArtifact } from "../contracts/candidate.js";
import type {
  ExecutionBindingV1,
  Mission,
  MissionRoots,
  StrictBaseline,
} from "../contracts/mission.js";
import {
  GIT_OBJECT_RE,
  SHA256_RE,
  objectAt,
  relativePathAt,
  stringAt,
} from "../contracts/validation.js";
import {
  MAX_COMMAND_ARGV,
  isolatedEnvironment,
  runCommand,
  type CommandOptions,
  type CommandResult,
  type CommandRunner,
} from "../util/command.js";
import { canonicalJcs, sha256Bytes } from "../util/canonical.js";
import { MAX_EXECUTABLE_BYTES, readBoundedRegularFile, sha256RegularFile } from "../util/files.js";
import type { LandResult, VelaCommandResponse, VelaInspection } from "./types.js";

export type { CommandRunner } from "../util/command.js";

// Large frontiers can spend several minutes in Vela's recoverable transaction
// preparation, exact replay, and derived-view materialization. This ceiling is
// bounded independently from model, verifier, and mission budgets: it prevents
// Canopus from killing a healthy authority transaction while still stopping a
// stalled Vela subprocess.
export const DEFAULT_VELA_COMMAND_TIMEOUT_MS = 600_000;

export function retainedArtifactPath(
  repoRoot: string,
  frontier: string,
  digest: string,
): string {
  if (!/^sha256:[0-9a-f]{64}$/u.test(digest)) {
    throw new VelaClientError("malformed_output", "artifact digest is not sha256");
  }
  return path.join(
    repoRoot,
    frontier,
    "records",
    "artifacts",
    "sha256",
    digest.slice("sha256:".length),
  );
}

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

export interface AuthoredReceiptInput {
  claim: string;
  claimType: Mission["claim_type"];
  replayability: Mission["replayability"];
  artifacts: Array<{ path: string; kind: string }>;
  caveats: string[];
  predictedObservable?: string;
  notApplicable?: true;
  performedTest: string;
  result: string;
  evidence: string[];
  counterevidence: string[];
  executionBinding?: ExecutionBindingV1;
  work?: string;
}

export interface VelaLandCommandObservation {
  argv: string[];
  exit_code: number;
  stdout: string;
  stderr: string;
  stdout_digest: string;
  stderr_digest: string;
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

function usesCompactStatusProjection(version: string): boolean {
  const match = /^0\.(\d+)\.(\d+)(?:-[0-9A-Za-z.-]+)?$/u.exec(version);
  if (match === null) return false;
  const minor = Number(match[1]);
  return Number.isSafeInteger(minor) && minor >= 900 && minor <= 930;
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

function bool(value: unknown, at: string): boolean {
  if (typeof value !== "boolean") {
    throw new VelaClientError("malformed_output", `${at} must be a boolean`);
  }
  return value;
}

function nullableCount(value: unknown, at: string): number | null {
  if (value === null) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new VelaClientError("malformed_output", `${at} must be a nonnegative integer or null`);
  }
  return value;
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
  assertEqual(actual.vela_event_log, expected.vela_event_log, "Vela event log");
  assertEqual(actual.vela_snapshot, expected.vela_snapshot, "Vela snapshot");
}

function nonnegativeInteger(value: unknown, at: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new VelaClientError("malformed_output", `${at} must be a nonnegative integer`);
  }
  return value;
}

function strictSignalCheck(check: Record<string, unknown>): Record<string, unknown> {
  if (!Array.isArray(check.checks)) {
    throw new VelaClientError("malformed_output", "vela check.checks must be an array");
  }
  let signalCheck: Record<string, unknown> | undefined;
  for (const [index, value] of check.checks.entries()) {
    const entry = fieldObject({ entry: value }, "entry", `vela check.checks[${index}]`);
    const id = stringAt(entry.id, `vela check.checks[${index}].id`, { min: 1, max: 128 });
    const status = stringAt(entry.status, `vela check.checks[${index}].status`, {
      min: 1,
      max: 32,
    });
    if (id === "signals") {
      if (signalCheck !== undefined) {
        throw new VelaClientError("malformed_output", "vela check contains duplicate signals checks");
      }
      signalCheck = entry;
    } else if (status !== "pass") {
      throw new VelaClientError(
        "command_failed",
        `vela strict check failed outside the registered signals baseline: ${id}=${status}`,
      );
    }
  }
  if (signalCheck === undefined) {
    throw new VelaClientError("malformed_output", "vela check omitted the signals check");
  }
  return signalCheck;
}

export function strictBaselineFromCheck(check: Record<string, unknown>): StrictBaseline {
  const summary = fieldObject(check, "summary", "vela check");
  if (summary.strict !== true) {
    throw new VelaClientError("malformed_output", "vela check summary is not strict");
  }
  if (
    nonnegativeInteger(summary.errors, "vela check.summary.errors") !== 0 ||
    nonnegativeInteger(summary.invalid_findings, "vela check.summary.invalid_findings") !== 0
  ) {
    throw new VelaClientError(
      "command_failed",
      "vela strict check contains structural errors or invalid findings",
    );
  }
  const signalCheck = strictSignalCheck(check);
  if (!Array.isArray(signalCheck.blockers)) {
    throw new VelaClientError("malformed_output", "vela check signals.blockers must be an array");
  }
  const blockers = signalCheck.blockers.map((value, index) =>
    fieldObject({ blocker: value }, "blocker", `vela check signals.blockers[${index}]`));
  const blocker_count = nonnegativeInteger(
    signalCheck.failed,
    "vela check signals.failed",
  );
  if (blockers.length !== blocker_count) {
    throw new VelaClientError(
      "malformed_output",
      "vela check signals blocker array and failed count disagree",
    );
  }
  const counts = new Map<string, number>();
  for (const [index, blocker] of blockers.entries()) {
    const kind = stringAt(blocker.kind, `vela check signals.blockers[${index}].kind`, {
      min: 1,
      max: 128,
      pattern: /^[a-z][a-z0-9_]*$/u,
    });
    counts.set(kind, (counts.get(kind) ?? 0) + 1);
  }
  const canonicalBlockers = blockers
    .map((blocker) => canonicalJcs(blocker))
    .sort()
    .map((serialized) => JSON.parse(serialized) as unknown);
  const status = blocker_count === 0 ? "pass" : "fail";
  const signalStatus = stringAt(signalCheck.status, "vela check signals.status", {
    min: 1,
    max: 32,
  });
  if (signalStatus !== status || summary.status !== status || bool(check.ok, "vela check.ok") !== (status === "pass")) {
    throw new VelaClientError("malformed_output", "vela strict status fields disagree");
  }
  return {
    status,
    blocker_count,
    blockers_root: sha256Bytes(canonicalJcs(canonicalBlockers)),
    rule_counts: [...counts.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([rule, count]) => ({ rule, count })),
  };
}

function assertStrictBaseline(actual: StrictBaseline, expected?: StrictBaseline): void {
  if (expected === undefined) {
    if (actual.status !== "pass") {
      throw new VelaClientError(
        "command_failed",
        `vela strict check has ${actual.blocker_count} unregistered blockers`,
      );
    }
    return;
  }
  if (canonicalJcs(actual) !== canonicalJcs(expected)) {
    throw new VelaClientError(
      "root_mismatch",
      `strict baseline mismatch: expected ${canonicalJcs(expected)}, observed ${canonicalJcs(actual)}`,
    );
  }
}

export function validateLandResult(
  mission: Mission,
  raw: Record<string, unknown>,
): LandResult {
  if (raw.ok !== true || raw.command !== "land") {
    throw new VelaClientError("malformed_output", "vela land did not return its success envelope");
  }
  const rawRoute = stringAt(raw.route, "vela land.route", { min: 1, max: 64 });
  const route =
    rawRoute === "policy_admitted"
      ? "permit"
      : rawRoute === "deferred"
        ? "defer"
        : rawRoute === "exact_retry"
            ? "exact_retry"
            : undefined;
  if (route === undefined) {
    throw new VelaClientError("malformed_output", `unknown landing route ${rawRoute}`);
  }
  const detail = stringAt(raw.detail, "vela land.detail", { max: 8192 });
  const rawOriginalRoute = raw.original_route;
  const originalRoute =
    route === "exact_retry"
      ? rawOriginalRoute === "policy_admitted"
        ? "permit"
        : rawOriginalRoute === "deferred"
          ? "defer"
          : undefined
      : rawOriginalRoute === null
        ? null
        : undefined;
  if (originalRoute === undefined) {
    throw new VelaClientError(
      "malformed_output",
      "landing returned an invalid structured original_route",
    );
  }
  const effectiveRoute = route === "exact_retry" ? originalRoute : route;
  if (effectiveRoute === null) {
    throw new VelaClientError("malformed_output", "landing has no effective route");
  }
  if (!mission.landing.expected_routes.includes(effectiveRoute)) {
    throw new VelaClientError(
      "unexpected_route",
      `landing route ${effectiveRoute} was not registered by the mission`,
    );
  }

  const before = nullableCount(raw.accepted_event_count_before, "vela land.accepted_event_count_before");
  const after = nullableCount(raw.accepted_event_count_after, "vela land.accepted_event_count_after");
  const delta = nullableCount(raw.accepted_event_delta, "vela land.accepted_event_delta");
  if ((before === null) !== (after === null) || (before === null) !== (delta === null)) {
    throw new VelaClientError("malformed_output", "landing returned a partial accepted-event count tuple");
  }
  if (before !== null && after !== null && delta !== null && (after < before || delta !== after - before)) {
    throw new VelaClientError("malformed_output", "landing accepted-event delta is inconsistent");
  }
  if (delta !== null && delta > mission.landing.max_accepted_delta) {
    throw new VelaClientError(
      "unexpected_route",
      `accepted-event delta ${delta} exceeds registered maximum`,
    );
  }
  const expectedDelta = effectiveRoute === "permit" ? 1 : 0;
  if (delta !== null && delta !== expectedDelta) {
    throw new VelaClientError(
      "malformed_output",
      `${effectiveRoute} durable landing requires accepted-event delta ${expectedDelta}`,
    );
  }
  if (route !== "exact_retry" && delta === null) {
    throw new VelaClientError("malformed_output", "new landing omitted accepted-event counts");
  }
  const publication = fieldObject(raw, "publication", "vela land");
  const publicationState = stringAt(publication.state, "vela land.publication.state", {
    min: 1,
    max: 64,
  });
  if (
    publicationState !== "committed_local" &&
    publicationState !== "pushed" &&
    publicationState !== "unchanged"
  ) {
    throw new VelaClientError(
      "command_failed",
      `vela land was durable but not Git-published: ${publicationState}`,
    );
  }

  return {
    operationId: stringAt(raw.operation_id, "vela land.operation_id", { min: 1, max: 256 }),
    receiptRoot: normalizeSha256(raw.receipt_root, "vela land.receipt_root"),
    recordId: stringAt(raw.record_id, "vela land.record_id", { min: 1, max: 256 }),
    proposalId: stringAt(raw.proposal_id, "vela land.proposal_id", { min: 1, max: 256 }),
    findingId: stringAt(raw.finding_id, "vela land.finding_id", { min: 1, max: 256 }),
    route,
    originalRoute,
    rawRoute,
    detail,
    acceptedEventCountBefore: before,
    acceptedEventCountAfter: after,
    acceptedEventDelta: delta,
    publication,
    raw,
  };
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

  async #strictCheck(
    frontier: string,
    cwd: string,
    baseline?: StrictBaseline,
  ): Promise<Record<string, unknown>> {
    const argv = [this.#binary, "check", frontier, "--strict", "--json"];
    const result = await this.#runner({
      argv,
      cwd,
      env: this.#env,
      timeoutMs: this.#timeoutMs,
      maxOutputBytes: this.#maxOutputBytes,
    });
    if (result.exitCode !== 0 && result.exitCode !== 1) {
      throw new VelaClientError(
        "command_failed",
        `${argv[0]} ${argv.slice(1).join(" ")} exited ${result.exitCode}: ${commandFailureSummary(result)}`,
      );
    }
    const check = parseJsonObject(result.stdout, "vela check");
    const actual = strictBaselineFromCheck(check);
    if ((result.exitCode === 0) !== (actual.status === "pass")) {
      throw new VelaClientError("malformed_output", "vela strict exit code and status disagree");
    }
    assertStrictBaseline(actual, baseline);
    return check;
  }

  public async observeStrictBaseline(
    repoRoot: string,
    frontier: string,
  ): Promise<StrictBaseline> {
    const safeFrontier = frontierPath(frontier);
    const argv = [this.#binary, "check", safeFrontier, "--strict", "--json"];
    const result = await this.#runner({
      argv,
      cwd: repoRoot,
      env: this.#env,
      timeoutMs: this.#timeoutMs,
      maxOutputBytes: this.#maxOutputBytes,
    });
    if (result.exitCode !== 0 && result.exitCode !== 1) {
      throw new VelaClientError(
        "command_failed",
        `${argv[0]} ${argv.slice(1).join(" ")} exited ${result.exitCode}: ${commandFailureSummary(result)}`,
      );
    }
    const check = parseJsonObject(result.stdout, "vela check");
    const baseline = strictBaselineFromCheck(check);
    if ((result.exitCode === 0) !== (baseline.status === "pass")) {
      throw new VelaClientError("malformed_output", "vela strict exit code and status disagree");
    }
    return baseline;
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
    strictBaseline?: StrictBaseline,
  ): Promise<VelaInspection> {
    const safeFrontier = frontierPath(frontier);
    const version = await this.assertVersion(repoRoot);
    const [gitCommit, gitTree, check] = await Promise.all([
      this.#gitObject(repoRoot, "HEAD^{commit}"),
      this.#gitObject(repoRoot, "HEAD^{tree}"),
      this.#strictCheck(safeFrontier, repoRoot, strictBaseline),
    ]);
    // Vela 0.9 makes the compact status projection the ordinary root reader.
    // Historical binaries retain proof verification for exact Mission v0/v1
    // replay, but a minimal 0.9 frontier intentionally has no proof bundle.
    const replay =
      typeof check.replay === "object" && check.replay !== null
        ? fieldObject(check, "replay", "vela check")
        : fieldObject(fieldObject(check, "state_integrity", "vela check"), "replay", "vela check.state_integrity");
    const checkEvent = normalizeSha256(replay.event_log_hash, "vela check.replay.event_log_hash");
    const checkCurrent = normalizeSha256(replay.current_hash, "vela check.replay.current_hash");
    if (replay.replayed_hash !== undefined) {
      assertEqual(
        normalizeSha256(replay.replayed_hash, "vela check.replay.replayed_hash"),
        checkCurrent,
        "Vela replayed snapshot",
      );
    }
    if (replay.source_hash !== undefined) {
      assertEqual(
        normalizeSha256(replay.source_hash, "vela check.replay.source_hash"),
        checkCurrent,
        "Vela source snapshot",
      );
    }

    const proof = usesCompactStatusProjection(version)
      ? {
          ok: true,
          command: "status_root_projection",
          event_log_hash: checkEvent,
          snapshot_hash: checkCurrent,
        }
      : await this.#json(["proof", "verify", safeFrontier, "--json"], repoRoot, "vela proof verify");

    if (!bool(proof.ok, "vela root projection.ok")) {
      throw new VelaClientError("command_failed", "vela root projection returned ok=false");
    }

    const proofEvent = normalizeSha256(
      proof.event_log_hash,
      "vela root projection.event_log_hash",
    );
    const proofSnapshot = normalizeSha256(
      proof.snapshot_hash,
      "vela root projection.snapshot_hash",
    );
    assertEqual(proofEvent, checkEvent, "check/proof event log");
    assertEqual(proofSnapshot, checkCurrent, "check/proof snapshot");

    if (proof.proof !== undefined) {
      const proofBody = fieldObject(proof, "proof", "vela proof verify");
      assertEqual(
        normalizeSha256(proofBody.event_log_hash, "vela proof verify.proof.event_log_hash"),
        checkEvent,
        "proof body event log",
      );
      assertEqual(
        normalizeSha256(proofBody.frontier_hash, "vela proof verify.proof.frontier_hash"),
        checkCurrent,
        "proof body snapshot",
      );
    }

    return {
      version,
      roots: {
        git_commit: gitCommit,
        git_tree: gitTree,
        vela_event_log: checkEvent,
        vela_snapshot: checkCurrent,
      },
      check,
      proof,
    };
  }

  public async assertRoots(
    repoRoot: string,
    frontier: string,
    expected: MissionRoots,
    strictBaseline?: StrictBaseline,
  ): Promise<VelaInspection> {
    const inspection = await this.inspect(repoRoot, frontier, strictBaseline);
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
      mission.schema === "canopus.mission.v1" ? mission.strict_baseline : undefined,
    );
  }

  public async offer(
    repoRoot: string,
    frontier: string,
    roots: MissionRoots,
    strictBaseline?: StrictBaseline,
    limit = 128,
  ): Promise<VelaCommandResponse> {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 128) {
      throw new VelaClientError("malformed_output", "vela next limit must be 1..128");
    }
    const safeFrontier = frontierPath(frontier);
    await this.assertRoots(repoRoot, safeFrontier, roots, strictBaseline);
    const value = await this.#json(
      ["next", safeFrontier, "--limit", String(limit), "--json"],
      repoRoot,
      "vela next",
    );
    if (value.ok === false) {
      throw new VelaClientError("command_failed", "vela next returned ok=false");
    }
    return { ok: true, value };
  }

  public async work(
    mission: Mission,
    repoRoot: string,
    target: string,
    expected: MissionRoots,
  ): Promise<VelaCommandResponse> {
    stringAt(target, "target", { min: 1, max: 256 });
    await this.assertRoots(
      repoRoot,
      mission.frontier,
      expected,
      mission.schema === "canopus.mission.v1" ? mission.strict_baseline : undefined,
    );
    const value = await this.#json(
      ["work", target, "--frontier", mission.frontier, "--as", mission.actor, "--json"],
      repoRoot,
      "vela work",
    );
    if (value.ok === false) {
      throw new VelaClientError("command_failed", "vela work returned ok=false");
    }
    return { ok: true, value };
  }

  public async landAuthoredCommand(
    mission: Mission,
    repoRoot: string,
    input: AuthoredReceiptInput,
    _expected: MissionRoots,
  ): Promise<VelaLandCommandObservation> {
    if ((input.predictedObservable === undefined) === (input.notApplicable === undefined)) {
      throw new VelaClientError(
        "malformed_output",
        "authored receipt requires exactly one prediction mode",
      );
    }
    // The caller performs the strict exact-root check immediately before it
    // installs the candidate artifacts. Those files legitimately stale the
    // derived artifacts hash until `vela land` records them atomically, so a
    // second strict check here would make the normal porcelain path unusable.
    const args = [
      "land",
      "--frontier",
      mission.frontier,
      "--claim",
      input.claim,
      "--type",
      input.claimType,
      "--replayability",
      input.replayability,
    ];
    for (const artifact of input.artifacts) {
      args.push("--artifact", `${relativePathAt(artifact.path, "artifact.path")}:${artifact.kind}`);
    }
    for (const caveat of input.caveats) args.push("--caveat", caveat);
    if (input.predictedObservable !== undefined) {
      args.push("--predicted-observable", input.predictedObservable);
    } else {
      args.push("--not-applicable");
    }
    args.push("--performed-test", input.performedTest, "--result", input.result);
    for (const evidence of input.evidence) args.push("--evidence", evidence);
    for (const counterevidence of input.counterevidence) {
      args.push("--counterevidence", counterevidence);
    }
    if (input.executionBinding !== undefined) {
      args.push(
        "--packet-root",
        input.executionBinding.packet_root,
        "--profile-root",
        input.executionBinding.profile_root,
        "--verifier-capsule-root",
        input.executionBinding.verifier_capsule_root,
        "--result-contract-root",
        input.executionBinding.result_contract_root,
      );
    }
    if (input.work !== undefined) args.push("--work", input.work);
    args.push("--as", mission.actor, "--json");
    const argv = [this.#binary, ...args];
    if (argv.length > MAX_COMMAND_ARGV) {
      throw new VelaClientError(
        "malformed_output",
        `authored Receipt requires ${argv.length} argv entries; maximum is ${MAX_COMMAND_ARGV}`,
      );
    }
    const result = await this.#runner({
      argv,
      cwd: repoRoot,
      env: this.#env,
      timeoutMs: this.#timeoutMs,
      maxOutputBytes: this.#maxOutputBytes,
    });
    return {
      argv,
      exit_code: result.exitCode,
      stdout: result.stdout.toString("utf8"),
      stderr: result.stderr.toString("utf8"),
      stdout_digest: sha256Bytes(result.stdout),
      stderr_digest: sha256Bytes(result.stderr),
    };
  }

  public parseLandCommand(observation: VelaLandCommandObservation): Record<string, unknown> {
    if (observation.exit_code !== 0) {
      throw new VelaClientError(
        "command_failed",
        `vela land exited ${observation.exit_code}: ${observation.stderr}`,
      );
    }
    return parseJsonObject(Buffer.from(observation.stdout), "vela land");
  }

  public validateLandResult(
    mission: Mission,
    raw: Record<string, unknown>,
  ): LandResult {
    return validateLandResult(mission, raw);
  }

  public async verifyReceiptBinding(
    mission: Mission,
    repoRoot: string,
    landing: LandResult,
    input: AuthoredReceiptInput,
    artifacts: readonly FrozenArtifact[],
  ): Promise<void> {
    const hex = landing.receiptRoot.slice("sha256:".length);
    const receiptPath = path.join(
      repoRoot,
      mission.frontier,
      "records",
      "receipts",
      "sha256",
      `${hex}.json`,
    );
    const bytes = await readBoundedRegularFile(receiptPath, 8 * 1024 * 1024);
    let parsed: unknown;
    try {
      parsed = JSON.parse(bytes.toString("utf8")) as unknown;
    } catch (error) {
      throw new VelaClientError("malformed_output", `retained receipt is not JSON: ${String(error)}`);
    }
    const receipt = objectAt(parsed, "retained receipt");
    if (sha256Bytes(canonicalJcs(receipt)) !== landing.receiptRoot) {
      throw new VelaClientError("root_mismatch", "retained Receipt v1 root does not match vela land");
    }
    const recordPath = path.join(
      repoRoot,
      mission.frontier,
      "records",
      `${landing.recordId}.json`,
    );
    let recordValue: unknown;
    try {
      recordValue = JSON.parse(
        (await readBoundedRegularFile(recordPath, 8 * 1024 * 1024)).toString("utf8"),
      ) as unknown;
    } catch (error) {
      throw new VelaClientError(
        "malformed_output",
        `retained activity record is unavailable or malformed: ${String(error)}`,
      );
    }
    const record = objectAt(recordValue, "retained activity record");
    const expectedReceiptPath = `records/receipts/sha256/${hex}.json`;
    if (
      record.id !== landing.recordId ||
      record.receipt_digest !== landing.receiptRoot ||
      record.receipt_path !== expectedReceiptPath
    ) {
      throw new VelaClientError(
        "root_mismatch",
        "retained activity record does not bind the landed Receipt v1",
      );
    }
    for (const [actual, expected, label] of [
      [receipt.claim, input.claim, "claim"],
      [receipt.type, input.claimType, "claim type"],
      [receipt.replayability, input.replayability, "replayability"],
    ]) {
      if (actual !== expected) {
        throw new VelaClientError("malformed_output", `retained receipt ${label} drifted`);
      }
    }
    if (JSON.stringify(receipt.caveats) !== JSON.stringify(input.caveats)) {
      throw new VelaClientError("malformed_output", "retained receipt caveats drifted");
    }
    const rawArtifacts = receipt.artifacts;
    if (!Array.isArray(rawArtifacts) || rawArtifacts.length !== artifacts.length) {
      throw new VelaClientError("malformed_output", "retained receipt artifact set drifted");
    }
    const recordArtifacts = record.artifacts;
    if (!Array.isArray(recordArtifacts) || recordArtifacts.length !== artifacts.length) {
      throw new VelaClientError("malformed_output", "activity record artifact set drifted");
    }
    for (const [index, expected] of artifacts.entries()) {
      const rawArtifact = objectAt(rawArtifacts[index], `retained receipt.artifacts[${index}]`);
      if (
        rawArtifact.path !== expected.path ||
        rawArtifact.kind !== expected.kind ||
        rawArtifact.sha256 !== expected.digest.slice("sha256:".length)
      ) {
        throw new VelaClientError("malformed_output", `retained receipt artifact ${index} drifted`);
      }
      const recordArtifact = objectAt(
        recordArtifacts[index],
        `retained activity record.artifacts[${index}]`,
      );
      const hexDigest = expected.digest.slice("sha256:".length);
      if (
        recordArtifact.kind !== expected.kind ||
        recordArtifact.sha256 !== hexDigest ||
        recordArtifact.locator !== `records/artifacts/sha256/${hexDigest}` ||
        recordArtifact.size_bytes !== expected.bytes ||
        recordArtifact.locator_integrity !== "immutable" ||
        recordArtifact.availability !== "available"
      ) {
        throw new VelaClientError(
          "malformed_output",
          `activity record artifact ${index} is not an immutable available blob binding`,
        );
      }
      const installed = await readBoundedRegularFile(
        retainedArtifactPath(repoRoot, mission.frontier, expected.digest),
        mission.budgets.max_artifact_bytes,
      );
      if (installed.length !== expected.bytes || sha256Bytes(installed) !== expected.digest) {
        throw new VelaClientError("root_mismatch", `retained artifact ${expected.path} drifted`);
      }
    }
    const environment = fieldObject(receipt, "environment", "retained receipt");
    if (input.executionBinding === undefined) {
      if (environment["vela:execution_binding"] !== undefined) {
        throw new VelaClientError(
          "malformed_output",
          "retained receipt invented an exact execution binding",
        );
      }
    } else {
      const binding = fieldObject(
        environment,
        "vela:execution_binding",
        "retained receipt.environment",
      );
      if (canonicalJcs(binding) !== canonicalJcs(input.executionBinding)) {
        throw new VelaClientError(
          "root_mismatch",
          "retained exact execution binding drifted",
        );
      }
    }
    const chain = fieldObject(environment, "vela:scientific_chain", "retained receipt.environment");
    const expectedChain = {
      schema: "vela.scientific-chain.producer.v1",
      authority: "producer",
      ...(input.predictedObservable === undefined
        ? { not_applicable: true }
        : { predicted_observable: input.predictedObservable, not_applicable: false }),
      performed_test: input.performedTest,
      result: input.result,
      evidence: input.evidence,
      counterevidence: input.counterevidence,
    };
    if (canonicalJcs(chain) !== canonicalJcs(expectedChain)) {
      throw new VelaClientError("malformed_output", "retained scientific-chain assertion drifted");
    }
  }
}
