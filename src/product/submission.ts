import {
  createPublicKey,
  generateKeyPairSync,
  sign,
  verify,
} from "node:crypto";
import { constants } from "node:fs";
import {
  chmod,
  copyFile,
  lstat,
  mkdir,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import path from "node:path";

import { parseMission, type MissionV1 } from "../contracts/mission.js";
import { parseCurrentRunRecord, projectCurrentRun } from "../projection/current-run.js";
import { canonicalJcs, canonicalJson, protocolDigest, sha256Bytes } from "../util/canonical.js";
import { readBoundedRegularFile } from "../util/files.js";

const ED25519_SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");

export interface IdentityBinding {
  schema: "vela.identity_binding.v0.1";
  binding_id: string;
  actor_id: string;
  actor_class: "agent";
  public_key_hex: string;
  created_at: string;
  signature: string;
}

export interface SubmissionV1 {
  schema: "vela.submission.v1";
  submission_id: string;
  claim: {
    assertion: string;
    type: "computational" | "theoretical" | "empirical" | "negative" | "contradiction";
    conditions: string[];
  };
  artifacts: Array<{ kind: string; path: string; digest: string }>;
  caveats: string[];
  replayability: "exact" | "bounded" | "approximate" | "unavailable" | "unknown";
  producer_checks: Array<{
    method: string;
    outcome: "pass" | "fail" | "error" | "skipped" | "unknown";
    authority: "producer_reported";
  }>;
  verification_requirements: string[];
  requested_change: { kind: "add_claim" };
  provenance: {
    producer: string;
    source_system: "canopus";
    source_run: string;
    emitted_at: string;
  };
  execution_binding: {
    schema: "vela.execution-binding.v1";
    packet_root: string;
    profile_root: string;
    verifier_capsule_root: string;
    result_contract_root: string;
  };
  authentication: {
    algorithm: "ed25519";
    identity_binding: IdentityBinding;
    signature: string;
  };
}

export interface SubmissionBundleManifest {
  schema: "canopus.submission-bundle.v1";
  run_id: string;
  run_root: string;
  source: {
    git_commit: string;
    git_tree: string;
    vela_version: string;
    vela_sha256: string;
  };
  producer: string;
  submission_id: string;
  submission_root: string;
  identity_binding_id: string;
  artifacts: Array<{
    source: string;
    frontier_path: string;
    digest: string;
    bytes: number;
  }>;
}

function rawPublicKey(publicKey: ReturnType<typeof createPublicKey>): Buffer {
  const der = publicKey.export({ type: "spki", format: "der" });
  if (!Buffer.isBuffer(der) || der.length !== ED25519_SPKI_PREFIX.length + 32) {
    throw new Error("generated Ed25519 public key has an unexpected encoding");
  }
  if (!der.subarray(0, ED25519_SPKI_PREFIX.length).equals(ED25519_SPKI_PREFIX)) {
    throw new Error("generated public key is not Ed25519");
  }
  return der.subarray(ED25519_SPKI_PREFIX.length);
}

function identityPreimage(binding: IdentityBinding): string {
  return canonicalJcs({ ...binding, binding_id: "", signature: "" });
}

function submissionPreimage(submission: SubmissionV1): string {
  return canonicalJcs({
    ...submission,
    submission_id: "",
    authentication: { ...submission.authentication, signature: "" },
  });
}

export function verifySubmission(submission: SubmissionV1): void {
  if (submission.schema !== "vela.submission.v1") {
    throw new Error("Submission schema must be vela.submission.v1");
  }
  const binding = submission.authentication.identity_binding;
  const publicBytes = Buffer.from(binding.public_key_hex, "hex");
  if (publicBytes.length !== 32) throw new Error("Submission public key is not Ed25519");
  const publicKey = createPublicKey({
    key: Buffer.concat([ED25519_SPKI_PREFIX, publicBytes]),
    format: "der",
    type: "spki",
  });
  const expectedBinding = `vib_${sha256Bytes(identityPreimage(binding)).slice(7, 23)}`;
  if (binding.binding_id !== expectedBinding) throw new Error("identity binding id mismatch");
  if (!verify(null, Buffer.from(identityPreimage(binding)), publicKey, Buffer.from(binding.signature, "hex"))) {
    throw new Error("identity binding signature does not verify");
  }
  if (
    binding.actor_class !== "agent" ||
    binding.actor_id !== submission.provenance.producer
  ) {
    throw new Error("Submission producer does not match its agent identity binding");
  }
  const expectedSubmission = `vsb_${sha256Bytes(submissionPreimage(submission)).slice(7, 23)}`;
  if (submission.submission_id !== expectedSubmission) throw new Error("Submission id mismatch");
  if (!verify(
    null,
    Buffer.from(submissionPreimage(submission)),
    publicKey,
    Buffer.from(submission.authentication.signature, "hex"),
  )) {
    throw new Error("Submission signature does not verify");
  }
}

async function assertFreshDirectory(directory: string): Promise<void> {
  try {
    await lstat(directory);
    throw new Error(`output already exists: ${directory}`);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
  await mkdir(directory, { recursive: true, mode: 0o700 });
}

function missionFileFor(runFile: string): string {
  const runRoot = path.dirname(runFile);
  return path.join(runRoot, "..", "mission", "mission.json");
}

export async function exportSubmission(options: {
  runFile: string;
  outputRoot: string;
  actor?: string;
  now?: Date;
}): Promise<{
  schema: "canopus.export-result.v1";
  ok: true;
  run_id: string;
  submission_id: string;
  submission_root: string;
  bundle: string;
  manifest: string;
}> {
  const runFile = await realpath(options.runFile);
  const runRoot = path.dirname(runFile);
  const record = parseCurrentRunRecord(JSON.parse(
    (await readBoundedRegularFile(runFile, 8 * 1024 * 1024)).toString("utf8"),
  ) as unknown);
  const mission = parseMission(JSON.parse(
    (await readBoundedRegularFile(missionFileFor(runFile), 8 * 1024 * 1024)).toString("utf8"),
  ) as unknown);
  if (mission.schema !== "canopus.mission.v1") {
    throw new Error("current export requires canopus.mission.v1");
  }
  if (mission.execution_binding === undefined) {
    throw new Error("run mission has no exact execution binding");
  }
  if (record.mission.digest !== sha256Bytes(canonicalJson(mission))) {
    throw new Error("run and mission roots disagree");
  }
  if (record.candidate.status !== "success" || record.verifier.status !== "passed") {
    throw new Error("only a successful, verifier-passing Run can export a Submission");
  }
  if (record.candidate.artifacts.length === 0 || record.candidate.caveats.length === 0) {
    throw new Error("Submission export requires at least one Artifact and one caveat");
  }
  const actor = options.actor ?? mission.actor;
  if (!actor.startsWith("agent:")) {
    throw new Error("Canopus Submission export requires an agent: producer");
  }
  const outputRoot = path.resolve(options.outputRoot);
  await assertFreshDirectory(outputRoot);
  try {
    const emittedAt = (options.now ?? new Date()).toISOString();
    const keys = generateKeyPairSync("ed25519");
    const publicKey = rawPublicKey(keys.publicKey);
    let binding: IdentityBinding = {
      schema: "vela.identity_binding.v0.1",
      binding_id: "",
      actor_id: actor,
      actor_class: "agent",
      public_key_hex: publicKey.toString("hex"),
      created_at: emittedAt,
      signature: "",
    };
    const bindingBytes = identityPreimage(binding);
    binding = {
      ...binding,
      binding_id: `vib_${sha256Bytes(bindingBytes).slice(7, 23)}`,
      signature: sign(null, Buffer.from(bindingBytes), keys.privateKey).toString("hex"),
    };
    const artifactDirectory = path.join(outputRoot, "artifacts", "sha256");
    await mkdir(artifactDirectory, { recursive: true, mode: 0o700 });
    const manifestArtifacts: SubmissionBundleManifest["artifacts"] = [];
    const submissionArtifacts: SubmissionV1["artifacts"] = [];
    for (const artifact of record.candidate.artifacts) {
      const hex = artifact.digest.slice("sha256:".length);
      const source = path.join(runRoot, "artifacts", hex);
      const observed = sha256Bytes(await readBoundedRegularFile(source, artifact.bytes + 1));
      if (observed !== artifact.digest) {
        throw new Error(`frozen Artifact ${artifact.digest} no longer matches the Run`);
      }
      const target = path.join(artifactDirectory, hex);
      await copyFile(source, target, constants.COPYFILE_EXCL);
      await chmod(target, 0o444);
      const frontierPath = `records/artifacts/sha256/${hex}`;
      manifestArtifacts.push({
        source: `artifacts/sha256/${hex}`,
        frontier_path: frontierPath,
        digest: artifact.digest,
        bytes: artifact.bytes,
      });
      submissionArtifacts.push({
        kind: artifact.kind,
        path: frontierPath,
        digest: artifact.digest,
      });
    }
    let submission: SubmissionV1 = {
      schema: "vela.submission.v1",
      submission_id: "",
      claim: {
        assertion: record.candidate.claim,
        type: mission.claim_type,
        conditions: [mission.completion_condition],
      },
      artifacts: submissionArtifacts,
      caveats: record.candidate.caveats,
      replayability: mission.replayability,
      producer_checks: [],
      verification_requirements: [
        `Replay Canopus Run ${record.run_id} with verifier capsule ${mission.execution_binding.verifier_capsule_root}.`,
      ],
      requested_change: { kind: "add_claim" },
      provenance: {
        producer: actor,
        source_system: "canopus",
        source_run: record.run_id,
        emitted_at: emittedAt,
      },
      execution_binding: mission.execution_binding,
      authentication: {
        algorithm: "ed25519",
        identity_binding: binding,
        signature: "",
      },
    };
    const submissionBytes = submissionPreimage(submission);
    submission = {
      ...submission,
      submission_id: `vsb_${sha256Bytes(submissionBytes).slice(7, 23)}`,
      authentication: {
        ...submission.authentication,
        signature: sign(null, Buffer.from(submissionBytes), keys.privateKey).toString("hex"),
      },
    };
    verifySubmission(submission);
    const submissionRoot = protocolDigest(submission);
    const submissionFile = path.join(outputRoot, "submission.json");
    await writeFile(submissionFile, `${canonicalJcs(submission)}\n`, { flag: "wx", mode: 0o444 });
    const manifest: SubmissionBundleManifest = {
      schema: "canopus.submission-bundle.v1",
      run_id: record.run_id,
      run_root: sha256Bytes(canonicalJson(record)),
      source: {
        git_commit: mission.roots.git_commit,
        git_tree: mission.roots.git_tree,
        vela_version: mission.vela_version,
        vela_sha256: mission.vela_sha256,
      },
      producer: actor,
      submission_id: submission.submission_id,
      submission_root: submissionRoot,
      identity_binding_id: binding.binding_id,
      artifacts: manifestArtifacts,
    };
    const manifestFile = path.join(outputRoot, "manifest.json");
    await writeFile(manifestFile, canonicalJson(manifest), { flag: "wx", mode: 0o444 });
    return {
      schema: "canopus.export-result.v1",
      ok: true,
      run_id: record.run_id,
      submission_id: submission.submission_id,
      submission_root: submissionRoot,
      bundle: outputRoot,
      manifest: manifestFile,
    };
  } catch (error) {
    await rm(outputRoot, { recursive: true, force: true });
    throw error;
  }
}

export function projectExportableRun(value: unknown): ReturnType<typeof projectCurrentRun> {
  return projectCurrentRun(parseCurrentRunRecord(value));
}
