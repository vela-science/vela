import {
  generateKeyPairSync,
  sign,
  type KeyObject,
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

import type {
  IdentityBinding,
  SubmissionV1,
} from "@vela-science/protocol";
import {
  identityBindingPreimage,
  submissionPreimage,
  verifySubmission,
} from "@vela-science/protocol";
export {
  verifySubmission,
  type IdentityBinding,
  type SubmissionV1,
} from "@vela-science/protocol";

import { finalizeWorkerCaveat } from "../candidate/finalize.js";
import { parseCurrentRunRecord, projectCurrentRun } from "../projection/current-run.js";
import { parseRetainedMission } from "../projection/retained-mission.js";
import { parseRetainedRunRecord } from "../projection/retained-run.js";
import { canonicalJcs, canonicalJson, protocolDigest, sha256Bytes } from "../util/canonical.js";
import { readBoundedRegularFile } from "../util/files.js";

const ED25519_SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const STALE_VERIFIER_LANGUAGE =
  /verif(?:y|ication|ier).*(?:pending|has not run|not (?:been )?(?:performed|executed|run))|pending.*verif(?:y|ication|ier)/iu;
const CONTROL_CHARACTER = /[\u0000-\u001f\u007f]/u;
const CORRECTION_NOTICE =
  "The Submission wording corrects a stale post-run Claim after verifier passage; the immutable Run remains unchanged.";
const REFINEMENT_NOTICE =
  "The Submission wording refines the retained Run Claim after verifier passage; the immutable Run remains unchanged.";

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

function rawPublicKey(publicKey: KeyObject): Buffer {
  const der = publicKey.export({ type: "spki", format: "der" });
  if (!Buffer.isBuffer(der) || der.length !== ED25519_SPKI_PREFIX.length + 32) {
    throw new Error("generated Ed25519 public key has an unexpected encoding");
  }
  if (!der.subarray(0, ED25519_SPKI_PREFIX.length).equals(ED25519_SPKI_PREFIX)) {
    throw new Error("generated public key is not Ed25519");
  }
  return der.subarray(ED25519_SPKI_PREFIX.length);
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
  attempt?: string;
  correctedClaim?: string;
  scopeLimit?: string;
  now?: Date;
}): Promise<{
  schema: "canopus.export-result.v1";
  ok: true;
  run_id: string;
  submission_id: string;
  submission_root: string;
  bundle: string;
  manifest: string;
  submission: string;
  producer: string;
  attempt: string | null;
}> {
  const runFile = await realpath(options.runFile);
  const runRoot = path.dirname(runFile);
  const retainedRun = parseRetainedRunRecord(JSON.parse(
    (await readBoundedRegularFile(runFile, 8 * 1024 * 1024)).toString("utf8"),
  ) as unknown);
  const retainedMission = parseRetainedMission(JSON.parse(
    (await readBoundedRegularFile(missionFileFor(runFile), 8 * 1024 * 1024)).toString("utf8"),
  ) as unknown);
  const record = retainedRun.record;
  const mission = retainedMission.mission;
  if (record.mission.digest !== retainedMission.exactRoot) {
    throw new Error("run and mission roots disagree");
  }
  if (retainedRun.exactStartingRoots !== retainedMission.exactRoots) {
    throw new Error("run and mission exact starting roots disagree");
  }
  if (record.candidate.status !== "success" || record.verifier.status !== "passed") {
    throw new Error("only a successful, verifier-passing Run can export a Submission");
  }
  if (record.candidate.artifacts.length === 0 || record.candidate.caveats.length === 0) {
    throw new Error("Submission export requires at least one Artifact and one caveat");
  }
  if ((options.correctedClaim === undefined) !== (options.scopeLimit === undefined)) {
    throw new Error("Submission correction requires both a corrected Claim and a scope limit");
  }
  const retainedClaimNeedsCorrection =
    STALE_VERIFIER_LANGUAGE.test(record.candidate.claim) ||
    CONTROL_CHARACTER.test(record.candidate.claim);
  if (retainedClaimNeedsCorrection && options.correctedClaim === undefined) {
    throw new Error(
      "Run Claim is stale after verifier passage or contains control bytes; export requires --claim and --scope-limit to author one corrected bounded Submission without changing the Run",
    );
  }
  const assertion = options.correctedClaim ?? record.candidate.claim;
  if (CONTROL_CHARACTER.test(assertion)) {
    throw new Error("Submission Claim contains a control character");
  }
  if (STALE_VERIFIER_LANGUAGE.test(assertion)) {
    throw new Error("Submission Claim contradicts the retained passing verifier outcome");
  }
  if (options.scopeLimit !== undefined && CONTROL_CHARACTER.test(options.scopeLimit)) {
    throw new Error("Submission scope limit contains a control character");
  }
  const caveats = [...new Set([
    ...record.candidate.caveats.map(finalizeWorkerCaveat),
    ...(options.scopeLimit === undefined
      ? []
      : [
          options.scopeLimit,
          retainedClaimNeedsCorrection ? CORRECTION_NOTICE : REFINEMENT_NOTICE,
        ]),
  ])];
  const actor = options.actor ?? mission.actor;
  if (!actor.startsWith("agent:")) {
    throw new Error("Vela Agent Submission export requires an agent: producer");
  }
  if (
    options.attempt !== undefined &&
    !/^vat_[0-9a-f]{64}$/u.test(options.attempt)
  ) {
    throw new Error("Vela Agent Submission export requires one full vat_ Attempt ID");
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
    const bindingBytes = identityBindingPreimage(binding);
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
        assertion,
        type: mission.claim_type,
        conditions: [mission.completion_condition],
      },
      artifacts: submissionArtifacts,
      caveats,
      replayability: mission.replayability,
      producer_checks: [],
      verification_requirements: [
        `Replay Vela Agent Run ${record.run_id} with verifier capsule ${mission.verifier.capsule_sha256}.`,
      ],
      requested_change: { kind: "add_claim" },
      provenance: {
        producer: actor,
        source_system: "canopus",
        ...(options.attempt === undefined ? {} : { source_attempt: options.attempt }),
        source_run: record.run_id,
        emitted_at: emittedAt,
      },
      ...(mission.execution_binding === undefined
        ? {}
        : { execution_binding: mission.execution_binding }),
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
      run_root: retainedRun.exactRoot,
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
      submission: submissionFile,
      producer: actor,
      attempt: options.attempt ?? null,
    };
  } catch (error) {
    await rm(outputRoot, { recursive: true, force: true });
    throw error;
  }
}

export function projectExportableRun(value: unknown): ReturnType<typeof projectCurrentRun> {
  return projectCurrentRun(parseCurrentRunRecord(value));
}
