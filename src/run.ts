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
import type { Mission, MissionRoots, StrictBaseline } from "./contracts/mission.js";
import type { FrozenArtifact } from "./contracts/candidate.js";
import type { Engine } from "./engines/engine.js";
import { engineManifest, verifierManifest } from "./evidence/manifests.js";
import { projectRun, RUN_RECORD_SCHEMA, type RunProjection, type RunRecord } from "./projection/run.js";
import {
  finalizeCandidate,
  installFrozenArtifacts,
  mapCandidateToReceipt,
} from "./receipt/map.js";
import { canonicalJson, contentDigest } from "./util/canonical.js";
import type { CommandRunner } from "./util/command.js";
import type {
  AuthoredReceiptInput,
  VelaClient,
  VelaLandCommandObservation,
} from "./vela/cli.js";
import { retainedArtifactPath } from "./vela/cli.js";
import type { LandResult, VelaCommandResponse, VelaInspection } from "./vela/types.js";
import { runVerifier } from "./verifier/run.js";
import { cleanupWorkspace, prepareWorkspace, type WorkspacePaths } from "./workspace/prepare.js";

export interface VelaPort {
  assertRoots(
    repoRoot: string,
    frontier: string,
    expected: MissionRoots,
    strictBaseline?: StrictBaseline,
  ): Promise<VelaInspection>;
  inspect(
    repoRoot: string,
    frontier: string,
    strictBaseline?: StrictBaseline,
  ): Promise<VelaInspection>;
  next(mission: Mission, repoRoot: string): Promise<VelaCommandResponse>;
  work(
    mission: Mission,
    repoRoot: string,
    target: string,
    expected: MissionRoots,
  ): Promise<VelaCommandResponse>;
  landAuthoredCommand(
    mission: Mission,
    repoRoot: string,
    input: AuthoredReceiptInput,
    expected: MissionRoots,
  ): Promise<VelaLandCommandObservation>;
  parseLandCommand(observation: VelaLandCommandObservation): Record<string, unknown>;
  validateLandResult(mission: Mission, raw: Record<string, unknown>): LandResult;
  verifyReceiptBinding(
    mission: Mission,
    repoRoot: string,
    landing: LandResult,
    input: AuthoredReceiptInput,
    artifacts: readonly FrozenArtifact[],
  ): Promise<void>;
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
  noLand?: false;
}

export interface CanopusNoLandOptions extends Omit<CanopusRunOptions, "noLand"> {
  noLand: true;
}

export interface CanopusRunResult {
  record: RunRecord;
  projection: RunProjection;
  paths: WorkspacePaths;
}

export interface DiagnosticRunRecord {
  schema: "canopus.diagnostic-run.v1";
  run_id: string;
  status: "completed";
  mode: "no_land";
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
  verifier: RunRecord["verifier"];
  landing: null;
  reproduction: RunRecord["reproduction"];
  budget: RunRecord["budget"];
}

export interface CanopusDiagnosticRunResult {
  record: DiagnosticRunRecord;
  projection: {
    schema: "canopus.diagnostic-projection.v1";
    authority: "read_only_projection";
    run_id: string;
    target: string;
    candidate_digest: string;
    verifier_status: "passed" | "failed" | "error";
    landed: false;
    clean_clone_reproduced: boolean;
  };
  paths: WorkspacePaths;
}

function sameRoots(left: MissionRoots, right: MissionRoots): boolean {
  return (
    left.git_commit === right.git_commit &&
    left.git_tree === right.git_tree &&
    left.vela_event_log === right.vela_event_log &&
    left.vela_snapshot === right.vela_snapshot
  );
}

function strictBaseline(mission: Mission): StrictBaseline | undefined {
  return mission.schema === "canopus.mission.v1" ? mission.strict_baseline : undefined;
}

function recordField(
  value: Record<string, unknown>,
  key: string,
  at: string,
): Record<string, unknown> {
  const field = value[key];
  if (typeof field !== "object" || field === null || Array.isArray(field)) {
    throw new Error(`${at}.${key} is not an object`);
  }
  return field as Record<string, unknown>;
}

function exactText(value: unknown, expected: string, at: string): void {
  if (value !== expected) {
    throw new Error(`${at} mismatch: expected ${expected}, observed ${String(value)}`);
  }
}

function assertWorkBinding(
  mission: Mission,
  response: VelaCommandResponse,
  postWork: VelaInspection,
): void {
  if (response.value.schema === "vela.work.v1") {
    exactText(response.value.command, "work", "vela work.command");
    exactText(response.value.target_id, mission.target, "vela work.target_id");
    const roots = recordField(response.value, "starting_roots", "vela work");
    exactText(roots.event_log, mission.roots.vela_event_log, "vela work.starting_roots.event_log");
    exactText(roots.git_commit, mission.roots.git_commit, "vela work.starting_roots.git_commit");
    const session = recordField(response.value, "session", "vela work");
    if (typeof session.id !== "string" || session.id.length === 0) {
      throw new Error("vela work.session.id must be nonempty");
    }
    const publication = recordField(response.value, "publication", "vela work");
    const publicationState = publication.state;
    if (
      publicationState !== "committed_local" &&
      publicationState !== "pushed" &&
      publicationState !== "unchanged"
    ) {
      throw new Error(`vela work lease was not published: ${String(publicationState)}`);
    }
    exactText(publication.commit, postWork.roots.git_commit, "vela work publication commit");
    if (
      postWork.roots.git_commit === mission.roots.git_commit ||
      postWork.roots.git_tree === mission.roots.git_tree
    ) {
      throw new Error("vela work did not publish an exact lease delta");
    }
    return;
  }
  const claim = recordField(response.value, "claim", "vela work");
  const session = recordField(response.value, "session", "vela work");
  const publication = recordField(claim, "publication", "vela work.claim");
  exactText(response.value.command, "work", "vela work.command");
  exactText(claim.state_root_before, mission.roots.vela_event_log, "vela work.claim.state_root_before");
  exactText(session.base_event_log_root, mission.roots.vela_event_log, "vela work.session.base_event_log_root");
  exactText(session.source_git_commit_oid, mission.roots.git_commit, "vela work.session.source_git_commit_oid");
  exactText(claim.state_root_after, postWork.roots.vela_event_log, "vela work.claim.state_root_after");
  const publicationState = publication.state;
  if (
    publicationState !== "committed_local" &&
    publicationState !== "pushed" &&
    publicationState !== "unchanged"
  ) {
    throw new Error(`vela work lease was not published: ${String(publicationState)}`);
  }
  exactText(publication.commit, postWork.roots.git_commit, "vela work publication commit");
  if (
    postWork.roots.git_commit === mission.roots.git_commit ||
    postWork.roots.git_tree === mission.roots.git_tree
  ) {
    throw new Error("vela work did not publish an exact lease delta");
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

function publicationState(land: LandResult): string {
  const value = land.publication.state;
  return typeof value === "string" ? value : "unknown";
}

export function isPrivateWorkSessionStatus(entry: string): boolean {
  if (!entry.startsWith("?? ")) return false;
  const relative = entry.slice(3);
  const parts = relative.split("/");
  return (
    parts.length === 4 &&
    parts[0] === ".vela" &&
    parts[1] === "work" &&
    parts[2] !== undefined &&
    /^[a-zA-Z0-9._-]+$/u.test(parts[2]) &&
    parts[3] === "session.json"
  );
}

async function writeExclusive(file: string, value: unknown): Promise<void> {
  await writeFile(file, canonicalJson(value), { flag: "wx", mode: 0o600 });
}

export function runCanopus(options: CanopusNoLandOptions): Promise<CanopusDiagnosticRunResult>;
export function runCanopus(options: CanopusRunOptions): Promise<CanopusRunResult>;
export async function runCanopus(
  options: CanopusRunOptions | CanopusNoLandOptions,
): Promise<CanopusRunResult | CanopusDiagnosticRunResult> {
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
  let landingCommand: VelaLandCommandObservation | undefined;
  let rawLanding: Record<string, unknown> | undefined;
  let parsedLanding: LandResult | undefined;

  try {
    await activity.append("run.started", {
      mission_id: options.mission.id,
      mission_digest: contentDigest(options.mission),
    });
    await activity.append("workspace.prepared", {
      input: "input",
      landing: "landing",
      output: "output",
      artifacts: "artifacts",
    });

    await Promise.all([
      options.vela.assertRoots(
        paths.input,
        options.mission.frontier,
        options.mission.roots,
        strictBaseline(options.mission),
      ),
      options.vela.assertRoots(
        paths.landing,
        options.mission.frontier,
        options.mission.roots,
        strictBaseline(options.mission),
      ),
    ]);
    await activity.append("roots.verified", { roots: options.mission.roots });

    // Vela's task offer performs recovery-barrier bookkeeping even though it
    // does not change scientific state. Run it in the exact-root control clone;
    // the separate worker input clone stays sealed and read-only.
    const offer = await options.vela.next(options.mission, paths.landing);
    const selected = validateTargetOffer(options.mission.target, offer);
    await activity.append("target.offered", {
      target: selected.id,
      rank: selected.index,
      offer_digest: contentDigest(offer.value),
    });

    let workBriefing: Record<string, unknown>;
    let postWork: VelaInspection | undefined;
    if (options.noLand === true) {
      // Diagnostic mode has no landing edge and therefore needs no durable
      // lease. Avoiding `vela work` keeps the disposable clone at the exact
      // registered roots and prevents an operational attempt event from
      // needlessly invalidating a previously exported proof packet.
      workBriefing = offer.value;
      await activity.append("work.skipped", {
        target: options.mission.target,
        mode: "no_land",
        reason: "diagnostic run has no landing edge",
      });
    } else {
      const work = await options.vela.work(
        options.mission,
        paths.landing,
        options.mission.target,
        options.mission.roots,
      );
      postWork = await options.vela.inspect(
        paths.landing,
        options.mission.frontier,
        strictBaseline(options.mission),
      );
      assertWorkBinding(options.mission, work, postWork);
      workBriefing = work.value;
      await activity.append("work.claimed", {
        target: options.mission.target,
        roots: postWork.roots,
        publication: work.value.publication ?? null,
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
        `worker returned ${engine.draft.status}; verifier and landing were not run`,
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
      throw new Error(`verifier returned ${verifier.status}; candidate and landing were not advanced`);
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

    if (options.noLand === true) {
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
          strictBaseline(options.mission),
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
      if (!reproduced) throw new Error("clean-clone diagnostic verifier replay did not match");
      const record: DiagnosticRunRecord = {
        schema: "canopus.diagnostic-run.v1",
        run_id: runId,
        status: "completed",
        mode: "no_land",
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
        landing: null,
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
        schema: "canopus.diagnostic-projection.v1" as const,
        authority: "read_only_projection" as const,
        run_id: runId,
        target: options.mission.target,
        candidate_digest: candidateDigest,
        verifier_status: verifier.status,
        landed: false as const,
        clean_clone_reproduced: true,
      };
      await rm(paths.velaHome, { recursive: true, force: true });
      await writeExclusive(path.join(paths.root, "candidate.json"), candidate);
      await writeExclusive(path.join(paths.root, "projection.json"), projection);
      await writeExclusive(path.join(paths.root, "run.json"), record);
      await activity.append("run.completed", {
        mode: "no_land",
        candidate_digest: candidateDigest,
      });
      return { record, projection, paths };
    }

    if (postWork === undefined) {
      throw new Error("landing run omitted the required Vela work lease");
    }
    const preLand = await options.vela.assertRoots(
      paths.landing,
      options.mission.frontier,
      postWork.roots,
      strictBaseline(options.mission),
    );
    // Verify the complete Vela state before placing candidate artifacts. The
    // artifact files intentionally make vela.lock stale until `vela land`
    // admits them transactionally, so a strict check between installation and
    // the porcelain command would reject the ordinary authored-receipt flow.
    await installFrozenArtifacts({
      landingRepo: paths.landing,
      frontier: options.mission.frontier,
      frozen,
      maxBytes: options.mission.budgets.max_artifact_bytes,
    });
    const receipt = mapCandidateToReceipt(
      options.mission,
      candidate,
      verifier,
      options.mission.target,
    );
    await activity.append("receipt.mapped", {
      candidate_digest: candidateDigest,
      evidence: receipt.evidence,
      counterevidence: receipt.counterevidence,
    });
    phase = "landing_command";
    landingCommand = await options.vela.landAuthoredCommand(
      options.mission,
      paths.landing,
      receipt,
      preLand.roots,
    );
    await writeExclusive(path.join(paths.root, "landing-command.json"), {
      schema: "canopus.landing-command.v0",
      authority: "observed_process_effect",
      ...landingCommand,
    });
    rawLanding = options.vela.parseLandCommand(landingCommand);
    const rawLandingDigest = contentDigest(rawLanding);
    await writeExclusive(path.join(paths.root, "landing-observation.json"), {
      schema: "canopus.landing-observation.v0",
      authority: "observed_vela_effect",
      raw_digest: rawLandingDigest,
      raw: rawLanding,
    });
    await activity.append("landing.observed", {
      raw_digest: rawLandingDigest,
      observation: "landing-observation.json",
    });
    phase = "landing_validation";
    const landing = options.vela.validateLandResult(options.mission, rawLanding);
    parsedLanding = landing;
    phase = "receipt_binding";
    await options.vela.verifyReceiptBinding(
      options.mission,
      paths.landing,
      landing,
      receipt,
      candidate.artifacts,
    );
    await activity.append("landing.bound", {
      receipt_root: landing.receiptRoot,
      raw_digest: rawLandingDigest,
    });
    await activity.append("landing.completed", {
      receipt_root: landing.receiptRoot,
      proposal_id: landing.proposalId,
      route: landing.route,
      original_route: landing.originalRoute,
      accepted_event_delta: landing.acceptedEventDelta,
      publication_state: publicationState(landing),
    });

    const final = await options.vela.inspect(
      paths.landing,
      options.mission.frontier,
      strictBaseline(options.mission),
    );
    phase = "clean_clone_reproduction";
    const reproductionRoot = `${paths.root}-reproduce`;
    const reproductionPaths = await prepareWorkspace({
      sourceRepo: paths.landing,
      runRoot: reproductionRoot,
      gitCommit: final.roots.git_commit,
      gitTree: final.roots.git_tree,
    });
    let reproductionVerifier;
    let reproductionInspection;
    try {
      reproductionInspection = await options.vela.assertRoots(
        reproductionPaths.input,
        options.mission.frontier,
        final.roots,
        strictBaseline(options.mission),
      );
      await options.vela.verifyReceiptBinding(
        options.mission,
        reproductionPaths.input,
        landing,
        receipt,
        candidate.artifacts,
      );
      const reproducedArtifacts = candidate.artifacts.map((artifact) => ({
        artifact,
        frozenPath: retainedArtifactPath(
          reproductionPaths.input,
          options.mission.frontier,
          artifact.digest,
        ),
      }));
      reproductionVerifier = await runVerifier({
        mission: options.mission,
        paths: reproductionPaths,
        artifacts: reproducedArtifacts,
        budget,
        ...(options.bundleRoot === undefined ? {} : { bundleRoot: options.bundleRoot }),
        ...(options.dockerBinary === undefined ? {} : { dockerBinary: options.dockerBinary }),
        ...(options.verifierRunner === undefined ? {} : { runner: options.verifierRunner }),
      });
    } finally {
      await cleanupWorkspace(reproductionPaths);
    }
    const reproduced =
      sameRoots(reproductionInspection.roots, final.roots) &&
      reproductionVerifier.status === verifier.status &&
      reproductionVerifier.record.stdout_digest === verifier.record.stdout_digest &&
      reproductionVerifier.record.stderr_digest === verifier.record.stderr_digest;
    if (!reproduced) {
      throw new Error("clean-clone reproduction did not match the landed run");
    }
    await activity.append("reproduction.completed", {
      matched: true,
      roots: reproductionInspection.roots,
      verifier_status: reproductionVerifier.status,
    });
    await rm(paths.velaHome, { recursive: true, force: true });
    const record: RunRecord = {
      schema: RUN_RECORD_SCHEMA,
      run_id: runId,
      status: "completed",
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
      observations: {
        worker_observations: candidate.observations,
        verifier_observations: [
          `Frozen verifier passed in ${verifier.sandbox}.`,
          `Verifier stdout ${verifier.record.stdout_digest}; stderr ${verifier.record.stderr_digest}.`,
        ],
        standing_caveats: candidate.caveats,
      },
      verifier: {
        status: verifier.status,
        sandbox: verifier.sandbox,
        record: verifier.record,
      },
      landing: {
        operation_id: landing.operationId,
        receipt_root: landing.receiptRoot,
        proposal_id: landing.proposalId,
        route: landing.route,
        original_route: landing.originalRoute,
        accepted_event_delta: landing.acceptedEventDelta,
        publication_state: publicationState(landing),
      },
      final_roots: final.roots,
      reproduction: {
        matched: reproduced,
        roots: reproductionInspection.roots,
        verifier_status: reproductionVerifier.status,
        stdout_digest: reproductionVerifier.record.stdout_digest,
        stderr_digest: reproductionVerifier.record.stderr_digest,
      },
      budget: budget.snapshot(),
    };
    const projection = projectRun(record);
    phase = "record_finalization";
    await writeExclusive(path.join(paths.root, "candidate.json"), candidate);
    await writeExclusive(path.join(paths.root, "projection.json"), projection);
    await writeExclusive(path.join(paths.root, "run.json"), record);
    await activity.append("projection.written", {
      candidate_digest: candidateDigest,
      run_digest: contentDigest(record),
      projection_digest: contentDigest(projection),
    });
    await activity.append("run.completed", {
      receipt_root: landing.receiptRoot,
      candidate_digest: candidateDigest,
    });
    return { record, projection, paths };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    let recovery: Record<string, unknown> | undefined;
    if (
      phase.startsWith("landing_") ||
      phase === "receipt_binding" ||
      phase === "clean_clone_reproduction" ||
      phase === "record_finalization"
    ) {
      let observedRoots: MissionRoots | null = null;
      let inspectionError: string | null = null;
      try {
        observedRoots = (
          await options.vela.inspect(
            paths.landing,
            options.mission.frontier,
            strictBaseline(options.mission),
          )
        ).roots;
      } catch (inspectionFailure) {
        inspectionError =
          inspectionFailure instanceof Error ? inspectionFailure.message : String(inspectionFailure);
      }
      recovery = {
        schema: "canopus.landing-recovery.v0",
        authority: "observed_vela_effect",
        phase,
        command: landingCommand ?? null,
        raw_digest: rawLanding === undefined ? null : contentDigest(rawLanding),
        raw: rawLanding ?? null,
        parsed:
          parsedLanding === undefined
            ? null
            : {
                operation_id: parsedLanding.operationId,
                receipt_root: parsedLanding.receiptRoot,
                proposal_id: parsedLanding.proposalId,
                route: parsedLanding.route,
                original_route: parsedLanding.originalRoute,
                accepted_event_delta: parsedLanding.acceptedEventDelta,
                publication: parsedLanding.publication,
              },
        observed_roots: observedRoots,
        inspection_error: inspectionError,
      };
      await writeExclusive(path.join(paths.root, "landing-recovery.json"), recovery).catch(
        () => undefined,
      );
    }
    await activity.append("run.failed", {
      error: message,
      phase,
      landing_observed: landingCommand !== undefined,
    }).catch(() => undefined);
    await writeExclusive(path.join(paths.root, "failure.json"), {
      schema: "canopus.failure.v0",
      run_id: runId,
      error: message,
      phase,
      landing_observed: landingCommand !== undefined,
      landing_recovery: recovery === undefined ? null : "landing-recovery.json",
      activity_tip: activity.tip,
      authority: "non_authoritative",
    }).catch(() => undefined);
    throw error;
  }
}
