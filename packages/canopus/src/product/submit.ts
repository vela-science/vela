import { constants } from "node:fs";
import { access, mkdtemp, realpath, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { protocolDigest, sha256Bytes } from "../util/canonical.js";
import { runCommand, type CommandRunner } from "../util/command.js";
import { readBoundedRegularFile } from "../util/files.js";
import { SUPPORTED_VELA_VERSION } from "./version.js";
import { findExecutable } from "./runtime.js";
import {
  verifySubmission,
  type SubmissionBundleManifest,
  type SubmissionV1,
} from "./submission.js";

async function command(
  runner: CommandRunner,
  argv: readonly string[],
  cwd: string,
  maxOutputBytes = 8 * 1024 * 1024,
): Promise<string> {
  const result = await runner({
    argv,
    cwd,
    env: process.env,
    timeoutMs: 120_000,
    maxOutputBytes,
  });
  if (result.exitCode !== 0) {
    let diagnostic = "no structured error";
    try {
      const parsed = JSON.parse(result.stdout.toString("utf8")) as unknown;
      if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
        const error = (parsed as Record<string, unknown>).error;
        if (typeof error === "object" && error !== null && !Array.isArray(error)) {
          const message = (error as Record<string, unknown>).message;
          if (typeof message === "string" && message.length > 0) {
            diagnostic = [...message]
              .slice(0, 512)
              .join("")
              .replace(/\b(?:sk|sess|key)-[A-Za-z0-9_-]{8,}\b/gu, "[secret-redacted]");
          }
        }
      }
    } catch {
      // Raw subprocess output is represented only by its digest below.
    }
    throw new Error(
      `${argv[0]} ${argv[1] ?? ""} exited ${result.exitCode}: ${diagnostic}; ` +
      `stdout_sha256=${sha256Bytes(result.stdout)}; stderr_sha256=${sha256Bytes(result.stderr)}`,
    );
  }
  return result.stdout.toString("utf8").trim();
}

function parseManifest(value: unknown): SubmissionBundleManifest {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("Submission bundle manifest is not an object");
  }
  const object = value as Record<string, unknown>;
  const exactKeys = (record: Record<string, unknown>, expected: string[], label: string): void => {
    const observed = Object.keys(record).sort();
    const wanted = [...expected].sort();
    if (observed.length !== wanted.length || observed.some((key, index) => key !== wanted[index])) {
      throw new Error(`${label} has unsupported or missing fields`);
    }
  };
  const requiredString = (record: Record<string, unknown>, key: string, label: string): string => {
    const field = record[key];
    if (typeof field !== "string" || field.length === 0 || field !== field.trim()) {
      throw new Error(`${label}.${key} must be non-empty trimmed text`);
    }
    return field;
  };
  const requiredObject = (
    record: Record<string, unknown>,
    key: string,
    label: string,
  ): Record<string, unknown> => {
    const field = record[key];
    if (typeof field !== "object" || field === null || Array.isArray(field)) {
      throw new Error(`${label}.${key} must be an object`);
    }
    return field as Record<string, unknown>;
  };
  const safePath = (candidate: string, label: string): string => {
    if (
      path.isAbsolute(candidate) ||
      candidate.includes("\\") ||
      candidate.split("/").some((segment) => segment === "" || segment === "." || segment === "..")
    ) {
      throw new Error(`${label} must be a safe relative POSIX path`);
    }
    return candidate;
  };

  exactKeys(object, [
    "schema",
    "run_id",
    "run_root",
    "source",
    "producer",
    "submission_id",
    "submission_root",
    "identity_binding_id",
    "artifacts",
  ], "Submission bundle manifest");
  const source = requiredObject(object, "source", "manifest");
  exactKeys(source, ["git_commit", "git_tree", "vela_version", "vela_sha256"], "manifest.source");
  const rawArtifacts = object.artifacts;
  if (!Array.isArray(rawArtifacts) || rawArtifacts.length === 0) {
    throw new Error("Submission bundle contains no Artifacts");
  }
  const artifacts = rawArtifacts.map((entry, index) => {
    if (typeof entry !== "object" || entry === null || Array.isArray(entry)) {
      throw new Error(`manifest.artifacts[${index}] must be an object`);
    }
    const artifact = entry as Record<string, unknown>;
    exactKeys(artifact, ["source", "frontier_path", "digest", "bytes"], `manifest.artifacts[${index}]`);
    const bytes = artifact.bytes;
    if (!Number.isSafeInteger(bytes) || Number(bytes) <= 0) {
      throw new Error(`manifest.artifacts[${index}].bytes must be a positive safe integer`);
    }
    return {
      source: safePath(
        requiredString(artifact, "source", `manifest.artifacts[${index}]`),
        `manifest.artifacts[${index}].source`,
      ),
      frontier_path: safePath(
        requiredString(artifact, "frontier_path", `manifest.artifacts[${index}]`),
        `manifest.artifacts[${index}].frontier_path`,
      ),
      digest: requiredString(artifact, "digest", `manifest.artifacts[${index}]`),
      bytes: Number(bytes),
    };
  });
  const manifest: SubmissionBundleManifest = {
    schema: requiredString(object, "schema", "manifest") as "canopus.submission-bundle.v1",
    run_id: requiredString(object, "run_id", "manifest"),
    run_root: requiredString(object, "run_root", "manifest"),
    source: {
      git_commit: requiredString(source, "git_commit", "manifest.source"),
      git_tree: requiredString(source, "git_tree", "manifest.source"),
      vela_version: requiredString(source, "vela_version", "manifest.source"),
      vela_sha256: requiredString(source, "vela_sha256", "manifest.source"),
    },
    producer: requiredString(object, "producer", "manifest"),
    submission_id: requiredString(object, "submission_id", "manifest"),
    submission_root: requiredString(object, "submission_root", "manifest"),
    identity_binding_id: requiredString(object, "identity_binding_id", "manifest"),
    artifacts,
  };
  if (manifest.schema !== "canopus.submission-bundle.v1") {
    throw new Error("Submission bundle schema must be canopus.submission-bundle.v1");
  }
  if (
    new Set(manifest.artifacts.map((artifact) => artifact.source)).size !== manifest.artifacts.length ||
    new Set(manifest.artifacts.map((artifact) => artifact.frontier_path)).size !== manifest.artifacts.length
  ) {
    throw new Error("Submission bundle contains duplicate Artifact paths");
  }
  return manifest;
}

export async function submitBundle(options: {
  bundle: string;
  frontier: string;
  velaBinary?: string;
  attempt?: string;
  runner?: CommandRunner;
}): Promise<{
  schema: "canopus.submit-result.v1";
  ok: true;
  submission_id: string;
  submission_root: string;
  registration_record_id: string;
  registration_record_root: string;
  proposal_id: string;
  claim_id: string;
  route: "pending_review";
  accepted_event_delta: 0;
  source_commit_before: string;
  source_commit_after: string;
  registration_binary_version: string;
  registration_binary_sha256: string;
}> {
  const runner = options.runner ?? runCommand;
  const bundle = await realpath(options.bundle);
  const frontier = await realpath(options.frontier);
  const manifest = parseManifest(JSON.parse(
    (await readBoundedRegularFile(path.join(bundle, "manifest.json"), 8 * 1024 * 1024)).toString("utf8"),
  ) as unknown);
  const submission = JSON.parse(
    (await readBoundedRegularFile(path.join(bundle, "submission.json"), 8 * 1024 * 1024)).toString("utf8"),
  ) as SubmissionV1;
  verifySubmission(submission);
  if (
    options.attempt !== undefined &&
    !/^vat_[0-9a-f]{64}$/u.test(options.attempt)
  ) {
    throw new Error("Canopus submit requires one full vat_ Attempt ID");
  }
  if (submission.provenance.source_attempt !== options.attempt) {
    throw new Error(
      "Submission provenance.source_attempt must exactly match --attempt, including absence",
    );
  }
  if (submission.submission_id !== manifest.submission_id) {
    throw new Error("Submission bundle identity mismatch");
  }
  if (
    submission.provenance.producer !== manifest.producer ||
    submission.authentication.identity_binding.binding_id !== manifest.identity_binding_id
  ) {
    throw new Error("Submission bundle producer binding mismatch");
  }
  if (protocolDigest(submission) !== manifest.submission_root) {
    throw new Error("Submission bundle root mismatch");
  }
  const declaredArtifacts = submission.artifacts
    .map((artifact) => `${artifact.path}\0${artifact.digest}`)
    .sort();
  const bundledArtifacts = manifest.artifacts
    .map((artifact) => `${artifact.frontier_path}\0${artifact.digest}`)
    .sort();
  if (
    declaredArtifacts.length !== bundledArtifacts.length ||
    declaredArtifacts.some((entry, index) => entry !== bundledArtifacts[index])
  ) {
    throw new Error("Submission and bundle Artifact bindings disagree");
  }
  const status = await command(
    runner,
    ["git", "status", "--porcelain=v1", "--untracked-files=all"],
    frontier,
  );
  if (status !== "") throw new Error("source frontier must be clean before submit");
  const before = await command(runner, ["git", "rev-parse", "--verify", "HEAD^{commit}"], frontier);
  const sourceTree = await command(
    runner,
    ["git", "rev-parse", "--verify", `${manifest.source.git_commit}^{tree}`],
    frontier,
  );
  const sourceOrigin = await command(runner, ["git", "remote", "get-url", "origin"], frontier);
  if (sourceTree !== manifest.source.git_tree) {
    throw new Error("retained Run source commit no longer matches its exact Git tree");
  }
  if (before !== manifest.source.git_commit) {
    const ancestry = await runner({
      argv: ["git", "merge-base", "--is-ancestor", manifest.source.git_commit, before],
      cwd: frontier,
      env: process.env,
      timeoutMs: 120_000,
      maxOutputBytes: 64 * 1024,
    });
    if (ancestry.exitCode !== 0) {
      throw new Error(
        "current frontier is not a descendant of the retained Run source commit",
      );
    }
  }
  const vela = await findExecutable(options.velaBinary ?? process.env.VELA_BIN ?? "vela");
  const observedVela = sha256Bytes(await readBoundedRegularFile(vela, 512 * 1024 * 1024));
  const version = await command(runner, [vela, "--version"], frontier, 64 * 1024);
  if (version !== `vela ${SUPPORTED_VELA_VERSION}`) {
    throw new Error(
      `Canopus requires registration through vela ${SUPPORTED_VELA_VERSION}, observed ${version}; ` +
      `the retained Run source binary remains recorded separately as ${manifest.source.vela_version} ` +
      `(${manifest.source.vela_sha256})`,
    );
  }

  for (const artifact of manifest.artifacts) {
    const source = path.join(bundle, artifact.source);
    await access(source, constants.R_OK);
    const bytes = await readBoundedRegularFile(source, artifact.bytes + 1);
    if (bytes.length !== artifact.bytes || sha256Bytes(bytes) !== artifact.digest) {
      throw new Error(`bundle Artifact ${artifact.digest} is missing or corrupt`);
    }
  }

  if (options.attempt !== undefined) {
    const resultText = await command(
      runner,
      [
        vela,
        "submit",
        path.join(bundle, "submission.json"),
        "--frontier",
        frontier,
        "--attempt",
        options.attempt,
        "--as",
        manifest.producer,
        "--json",
      ],
      frontier,
    );
    const result = JSON.parse(resultText) as Record<string, unknown>;
    if (
      result.ok !== true ||
      result.schema !== "vela.submit-result.v1" ||
      result.submission_id !== manifest.submission_id ||
      result.submission_root !== manifest.submission_root ||
      result.route !== "pending_review" ||
      result.accepted_event_delta !== 0
    ) {
      throw new Error("Vela returned an invalid or authority-changing submit result");
    }
    const after = await command(
      runner,
      ["git", "rev-parse", "--verify", "HEAD^{commit}"],
      frontier,
    );
    if (after === before) throw new Error("Vela submit produced no registration commit");
    const finalStatus = await command(
      runner,
      ["git", "status", "--porcelain=v1", "--untracked-files=all"],
      frontier,
    );
    if (finalStatus !== "") throw new Error("source frontier is dirty after submit");
    return {
      schema: "canopus.submit-result.v1",
      ok: true,
      submission_id: manifest.submission_id,
      submission_root: manifest.submission_root,
      registration_record_id: String(result.registration_record_id),
      registration_record_root: String(result.registration_record_root),
      proposal_id: String(result.proposal_id),
      claim_id: String(result.claim_id),
      route: "pending_review",
      accepted_event_delta: 0,
      source_commit_before: before,
      source_commit_after: after,
      registration_binary_version: version,
      registration_binary_sha256: observedVela,
    };
  }

  const temporary = await mkdtemp(path.join(os.tmpdir(), "canopus-submit-"));
  const clone = path.join(temporary, "frontier");
  try {
    await command(runner, ["git", "clone", "--no-hardlinks", "--", frontier, clone], temporary);
    const cloneHead = await command(runner, ["git", "rev-parse", "--verify", "HEAD^{commit}"], clone);
    if (cloneHead !== before) throw new Error("disposable submit clone does not match source HEAD");
    await command(runner, ["git", "remote", "set-url", "origin", sourceOrigin], clone);
    const resultText = await command(
      runner,
      [
        vela,
        "submit",
        path.join(bundle, "submission.json"),
        "--frontier",
        clone,
        "--as",
        manifest.producer,
        "--json",
      ],
      clone,
    );
    const result = JSON.parse(resultText) as Record<string, unknown>;
    if (
      result.ok !== true ||
      result.schema !== "vela.submit-result.v1" ||
      result.submission_id !== manifest.submission_id ||
      result.submission_root !== manifest.submission_root ||
      result.route !== "pending_review" ||
      result.accepted_event_delta !== 0
    ) {
      throw new Error("Vela returned an invalid or authority-changing submit result");
    }
    const after = await command(runner, ["git", "rev-parse", "--verify", "HEAD^{commit}"], clone);
    if (after === before) throw new Error("Vela submit produced no registration commit");
    const sourceStill = await command(runner, ["git", "rev-parse", "--verify", "HEAD^{commit}"], frontier);
    const sourceStatus = await command(
      runner,
      ["git", "status", "--porcelain=v1", "--untracked-files=all"],
      frontier,
    );
    if (sourceStill !== before || sourceStatus !== "") {
      throw new Error("source frontier changed before the verified registration was ready");
    }
    await command(runner, ["git", "fetch", "--no-tags", "--", clone, after], frontier);
    await command(runner, ["git", "merge", "--ff-only", "FETCH_HEAD"], frontier);
    const installed = await command(runner, ["git", "rev-parse", "--verify", "HEAD^{commit}"], frontier);
    if (installed !== after) throw new Error("source frontier did not fast-forward to the registration");
    const finalStatus = await command(
      runner,
      ["git", "status", "--porcelain=v1", "--untracked-files=all"],
      frontier,
    );
    if (finalStatus !== "") throw new Error("source frontier is dirty after submit");
    return {
      schema: "canopus.submit-result.v1",
      ok: true,
      submission_id: manifest.submission_id,
      submission_root: manifest.submission_root,
      registration_record_id: String(result.registration_record_id),
      registration_record_root: String(result.registration_record_root),
      proposal_id: String(result.proposal_id),
      claim_id: String(result.claim_id),
      route: "pending_review",
      accepted_event_delta: 0,
      source_commit_before: before,
      source_commit_after: after,
      registration_binary_version: version,
      registration_binary_sha256: observedVela,
    };
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}
