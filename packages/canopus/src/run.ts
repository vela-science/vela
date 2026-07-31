import { randomUUID } from "node:crypto";
import { mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import { ActivityStore } from "./activity/store.js";
import {
  freezeArtifact,
  sealArtifactStore,
  type FrozenArtifactLocation,
} from "./artifact/freeze.js";
import { materializeDraftArtifacts } from "./artifact/materialize.js";
import { BudgetTracker } from "./budget/enforce.js";
import { finalizeCandidate } from "./candidate/finalize.js";
import type { Mission, MissionRoots } from "./contracts/mission.js";
import type { Engine } from "./engines/engine.js";
import {
  discardWorkerWorkspaceEvidence,
  promoteWorkerWorkspaceEvidence,
} from "./engines/workspace-evidence.js";
import { engineManifest, verifierManifest } from "./evidence/manifests.js";
import { canonicalJson, contentDigest } from "./util/canonical.js";
import type { CommandRunner } from "./util/command.js";
import type { VelaClient } from "./vela/cli.js";
import type { VelaCommandResponse, VelaInspection } from "./vela/types.js";
import { runVerifier } from "./verifier/run.js";
import { cleanupWorkspace, prepareWorkspace, type WorkspacePaths } from "./workspace/prepare.js";

export interface VelaPort {
  assertRoots(
    repoRoot: string,
    frontier: string,
    expected: MissionRoots,
  ): Promise<VelaInspection>;
  next(mission: Mission, repoRoot: string): Promise<VelaCommandResponse>;
}

export interface CanopusRunOptions {
  mission: Mission;
  sourceRepo: string;
  runRoot: string;
  vela: VelaPort | VelaClient;
  engine: Engine;
  bundleRoot?: string;
  dockerBinary?: string;
  verifierRunner?: CommandRunner;
  repairInput?: {
    path: string;
    digest: string;
    bytes: Buffer;
  };
}

export interface VerifierRun {
  status: "passed" | "failed" | "error";
  sandbox: "macos_sandbox" | "container_network_denied";
  record: {
    argv: string[];
    executable_digest: string;
    exit_code: number;
    stdout_digest: string;
    stderr_digest: string;
    duration_ms: number;
  };
}

export interface ReproductionResult {
  matched: boolean;
  roots: MissionRoots;
  verifier_status: "passed" | "failed" | "error";
  stdout_digest: string;
  stderr_digest: string;
}

export interface RunBudget {
  research_elapsed_ms: number;
  research_processes: number;
  research_output_bytes: number;
  prompt_bytes: number;
  artifact_bytes: number;
  attempts: number;
  observed_tokens: number;
}

export interface CurrentRunRecord {
  schema: "canopus.run.v2";
  run_id: string;
  status: "completed";
  effect: "none";
  authority: "non_authoritative";
  external_gate_credit: false;
  mission: {
    id: string;
    target: string;
    digest: string;
    starting_roots: MissionRoots;
  };
  candidate: {
    digest: string;
    status: "success" | "null" | "failed";
    claim: string;
    artifacts: Array<{ path: string; kind: string; digest: string; bytes: number }>;
    caveats: string[];
  };
  verifier: VerifierRun;
  submission: null;
  reproduction: ReproductionResult;
  budget: RunBudget;
}

export interface CanopusCurrentRunResult {
  record: CurrentRunRecord;
  projection: {
    schema: "canopus.run-projection.v2";
    authority: "read_only_projection";
    run_id: string;
    target: string;
    candidate_digest: string;
    verifier_status: "passed" | "failed" | "error";
    submitted: false;
    clean_clone_reproduced: boolean;
  };
  paths: WorkspacePaths;
}

function exactText(value: unknown, expected: string, at: string): void {
  if (value !== expected) {
    throw new Error(`${at} mismatch: expected ${expected}, observed ${String(value)}`);
  }
}

export function validateTargetOffer(
  target: string,
  response: VelaCommandResponse,
): { index: number; id: string } {
  exactText(response.value.command, "next", "vela next.command");
  const targets = response.value.targets;
  if (!Array.isArray(targets)) throw new Error("vela next.targets is not an array");
  const matches: Array<{ index: number; id: string }> = [];
  for (const [index, entry] of targets.entries()) {
    if (typeof entry !== "object" || entry === null || Array.isArray(entry)) {
      throw new Error(`vela next.targets[${index}] is not an object`);
    }
    const object = entry as Record<string, unknown>;
    const id = object.target_id ?? object.id;
    if (typeof id !== "string" || id.length === 0) {
      throw new Error(`vela next.targets[${index}].id is not a nonempty string`);
    }
    if (id === target) matches.push({ index, id });
  }
  if (matches.length !== 1) {
    throw new Error(
      `registered mission target must appear exactly once in vela next; observed ${matches.length}`,
    );
  }
  return matches[0] as { index: number; id: string };
}

async function writeExclusive(file: string, value: unknown): Promise<void> {
  await writeFile(file, canonicalJson(value), { flag: "wx", mode: 0o600 });
}

export async function runCanopus(
  options: CanopusRunOptions,
): Promise<CanopusCurrentRunResult> {
  const runId = `run_${randomUUID()}`;
  const paths = await prepareWorkspace({
    sourceRepo: options.sourceRepo,
    runRoot: options.runRoot,
    gitCommit: options.mission.roots.git_commit,
    gitTree: options.mission.roots.git_tree,
  });
  const activity = await ActivityStore.open(path.join(paths.root, "activity.jsonl"), runId);
  const budget = new BudgetTracker(options.mission.budgets);
  let phase = "initializing";

  try {
    await activity.append("run.started", {
      mission_id: options.mission.id,
      mission_digest: contentDigest(options.mission),
    });
    await activity.append("workspace.prepared", {
      input: "input",
      frontier: "frontier",
      output: "output",
      artifacts: "artifacts",
    });

    await Promise.all([
      options.vela.assertRoots(
        paths.input,
        options.mission.frontier,
        options.mission.roots,
      ),
      options.vela.assertRoots(
        paths.frontier,
        options.mission.frontier,
        options.mission.roots,
      ),
    ]);
    await activity.append("roots.verified", { roots: options.mission.roots });

    // Vela's task offer performs recovery-barrier bookkeeping even though it
    // does not change scientific state. Run it in the exact-root control clone;
    // the separate worker input clone stays sealed and read-only.
    const offer = await options.vela.next(options.mission, paths.frontier);
    const selected = validateTargetOffer(options.mission.target, offer);
    await activity.append("target.offered", {
      target: selected.id,
      rank: selected.index,
      offer_digest: contentDigest(offer.value),
    });

    // A Run is nonmutating. The rooted offer is the complete bounded briefing;
    // a durable Vela Attempt begins only through an explicit Vela workflow.
    const workBriefing = offer.value;
    await activity.append("work.skipped", {
      target: options.mission.target,
      mode: "no_submit",
      reason: "Vela Agent run is nonmutating by contract",
    });
    if (options.repairInput !== undefined) {
      await activity.append("repair.input_bound", {
        path: options.repairInput.path,
        digest: options.repairInput.digest,
        bytes: options.repairInput.bytes.length,
      });
    }

    await activity.append("engine.started", {
      engine: options.engine.name,
      role: options.mission.role,
    });
    const engine = await options.engine.run({
      mission: options.mission,
      briefing: workBriefing,
      paths,
      budget,
      ...(options.repairInput === undefined ? {} : { repairInput: options.repairInput }),
    });
    await activity.append("engine.completed", {
      status: engine.draft.status,
      claim: engine.draft.claim,
      observations: engine.draft.observations,
      caveats: engine.draft.caveats,
      declared_artifacts: engine.draft.artifacts.map((artifact) => ({
        path: artifact.path,
        kind: artifact.kind,
        bytes: Buffer.byteLength(artifact.content),
      })),
      engine: engine.engine,
      usage: engine.usage,
      events_digest: engine.eventsDigest,
      action_types: engine.actionTypes,
    });
    await writeExclusive(path.join(paths.root, "engine-result.json"), {
      schema: "canopus.engine-result.v0",
      authority: "non_authoritative",
      draft: engine.draft,
      engine: engine.engine,
      usage: engine.usage,
      wall_time_ms: engine.wallTimeMs,
      event_types: engine.eventTypes,
      action_types: engine.actionTypes,
      events_digest: engine.eventsDigest,
      stderr_digest: engine.stderrDigest,
    });
    if (engine.draft.status !== "success") {
      phase = "engine_non_success";
      throw new Error(
        `worker returned ${engine.draft.status}; verifier and export were not run`,
      );
    }

    await materializeDraftArtifacts({
      draft: engine.draft,
      outputRoot: paths.output,
      maxTotalBytes: options.mission.budgets.max_artifact_bytes,
    });
    const frozen: FrozenArtifactLocation[] = [];
    for (const artifact of engine.draft.artifacts) {
      const entry = await freezeArtifact({
        sourceRoot: paths.output,
        artifactRoot: paths.artifacts,
        path: artifact.path,
        kind: artifact.kind,
        maxBytes: budget.remainingArtifactBytes(),
      });
      budget.addArtifact(entry.artifact.bytes);
      frozen.push(entry);
      await activity.append("artifact.frozen", { artifact: entry.artifact });
    }
    const supporting = [engineManifest(engine), verifierManifest(options.mission)];
    for (const manifest of supporting) {
      const source = path.join(paths.output, manifest.path);
      await mkdir(path.dirname(source), { recursive: true, mode: 0o700 });
      await writeExclusive(source, manifest.value);
      const entry = await freezeArtifact({
        sourceRoot: paths.output,
        artifactRoot: paths.artifacts,
        path: manifest.path,
        kind: manifest.kind,
        maxBytes: budget.remainingArtifactBytes(),
      });
      budget.addArtifact(entry.artifact.bytes);
      frozen.push(entry);
      await activity.append("artifact.frozen", { artifact: entry.artifact });
    }
    await sealArtifactStore(paths.artifacts);

    const verifier = await runVerifier({
      mission: options.mission,
      paths,
      artifacts: frozen,
      budget,
      ...(options.bundleRoot === undefined ? {} : { bundleRoot: options.bundleRoot }),
      ...(options.dockerBinary === undefined ? {} : { dockerBinary: options.dockerBinary }),
      ...(options.verifierRunner === undefined ? {} : { runner: options.verifierRunner }),
    });
    await activity.append("verifier.completed", {
      status: verifier.status,
      record: verifier.record,
      sandbox: verifier.sandbox,
      ...(verifier.error === undefined ? {} : { error: verifier.error }),
    });
    if (verifier.status !== "passed") {
      phase = "verifier_non_success";
      throw new Error(`verifier returned ${verifier.status}; no Submission can be exported`);
    }

    const candidate = finalizeCandidate({
      mission: options.mission,
      engine,
      frozen,
      verifier,
      budget: budget.snapshot(),
      supportingArtifacts: supporting,
    });
    const candidateDigest = contentDigest(candidate);
    await activity.append("candidate.finalized", {
      candidate_digest: candidateDigest,
      status: candidate.status,
    });

    {
      phase = "clean_clone_reproduction";
      const reproductionRoot = `${paths.root}-reproduce`;
      const reproductionPaths = await prepareWorkspace({
        sourceRepo: options.sourceRepo,
        runRoot: reproductionRoot,
        gitCommit: options.mission.roots.git_commit,
        gitTree: options.mission.roots.git_tree,
      });
      let reproductionVerifier;
      try {
        await options.vela.assertRoots(
          reproductionPaths.input,
          options.mission.frontier,
          options.mission.roots,
        );
        reproductionVerifier = await runVerifier({
          mission: options.mission,
          paths: reproductionPaths,
          artifacts: frozen,
          budget,
          ...(options.bundleRoot === undefined ? {} : { bundleRoot: options.bundleRoot }),
          ...(options.dockerBinary === undefined ? {} : { dockerBinary: options.dockerBinary }),
          ...(options.verifierRunner === undefined ? {} : { runner: options.verifierRunner }),
        });
      } finally {
        await cleanupWorkspace(reproductionPaths);
      }
      const reproduced =
        reproductionVerifier.status === verifier.status &&
        reproductionVerifier.record.stdout_digest === verifier.record.stdout_digest &&
        reproductionVerifier.record.stderr_digest === verifier.record.stderr_digest;
      if (!reproduced) {
        throw new Error(
          `clean-clone verifier replay did not match: initial=${verifier.status}/` +
          `${verifier.record.exit_code}/${verifier.record.stdout_digest}/${verifier.record.stderr_digest}, reproduced=` +
          `${reproductionVerifier.status}/${reproductionVerifier.record.exit_code}/` +
          `${reproductionVerifier.record.stdout_digest}/` +
          `${reproductionVerifier.record.stderr_digest}`,
        );
      }
      const record: CurrentRunRecord = {
        schema: "canopus.run.v2",
        run_id: runId,
        status: "completed",
        effect: "none",
        authority: "non_authoritative",
        external_gate_credit: false,
        mission: {
          id: options.mission.id,
          target: options.mission.target,
          digest: contentDigest(options.mission),
          starting_roots: options.mission.roots,
        },
        candidate: {
          digest: candidateDigest,
          status: candidate.status,
          claim: candidate.claim,
          artifacts: candidate.artifacts,
          caveats: candidate.caveats,
        },
        verifier: {
          status: verifier.status,
          sandbox: verifier.sandbox,
          record: verifier.record,
        },
        submission: null,
        reproduction: {
          matched: true,
          roots: options.mission.roots,
          verifier_status: reproductionVerifier.status,
          stdout_digest: reproductionVerifier.record.stdout_digest,
          stderr_digest: reproductionVerifier.record.stderr_digest,
        },
        budget: budget.snapshot(),
      };
      const projection = {
        schema: "canopus.run-projection.v2" as const,
        authority: "read_only_projection" as const,
        run_id: runId,
        target: options.mission.target,
        candidate_digest: candidateDigest,
        verifier_status: verifier.status,
        submitted: false as const,
        clean_clone_reproduced: true,
      };
      await rm(paths.velaHome, { recursive: true, force: true });
      await writeExclusive(path.join(paths.root, "candidate.json"), candidate);
      await writeExclusive(path.join(paths.root, "projection.json"), projection);
      await writeExclusive(path.join(paths.root, "run.json"), record);
      await discardWorkerWorkspaceEvidence(paths.root);
      await activity.append("run.completed", {
        effect: "none",
        candidate_digest: candidateDigest,
      });
      return { record, projection, paths };
    }

  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const failureEvidence = await promoteWorkerWorkspaceEvidence(paths.root)
      .catch(async () => {
        await discardWorkerWorkspaceEvidence(paths.root).catch(() => undefined);
        return null;
      });
    await activity.append("run.failed", {
      error: message,
      phase,
      effect: "none",
      ...(failureEvidence === null ? {} : { failure_evidence: failureEvidence }),
    }).catch(() => undefined);
    await writeExclusive(path.join(paths.root, "failure.json"), {
      schema: "canopus.failure.v1",
      run_id: runId,
      error: message,
      phase,
      effect: "none",
      activity_tip: activity.tip,
      authority: "non_authoritative",
    }).catch(() => undefined);
    throw error;
  }
}
