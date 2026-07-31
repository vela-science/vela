import type { FrozenArtifactLocation } from "../artifact/freeze.js";
import type { BudgetSnapshot } from "../budget/enforce.js";
import { parseCandidate, type Candidate } from "../contracts/candidate.js";
import type { Mission } from "../contracts/mission.js";
import type { EngineResult } from "../engines/engine.js";
import type { VerifierOutcome } from "../verifier/run.js";

function unique(values: readonly string[]): string[] {
  return [...new Set(values)];
}

export function finalizeWorkerCaveat(value: string): string {
  if (
    /verif(?:y|ication|ier).*(?:pending|has not run|not (?:been )?(?:performed|executed|run))|pending.*verif(?:y|ication|ier)/iu.test(
      value,
    )
  ) {
    return "The worker handed off without verifier authority; Vela Agent subsequently recorded the separate verifier outcome.";
  }
  return value;
}

export function finalizeCandidate(options: {
  mission: Mission;
  engine: EngineResult;
  frozen: readonly FrozenArtifactLocation[];
  verifier: VerifierOutcome;
  budget: BudgetSnapshot;
  supportingArtifacts?: ReadonlyArray<{ path: string; kind: string }>;
}): Candidate {
  const draftPaths = options.engine.draft.artifacts
    .map((artifact) => `${artifact.path}:${artifact.kind}`)
    .sort();
  const supporting = new Map(
    (options.supportingArtifacts ?? []).map((entry) => [entry.path, entry.kind]),
  );
  const frozenPaths = options.frozen
    .filter((entry) => !supporting.has(entry.artifact.path))
    .map((entry) => `${entry.artifact.path}:${entry.artifact.kind}`)
    .sort();
  if (JSON.stringify(draftPaths) !== JSON.stringify(frozenPaths)) {
    throw new Error("frozen artifacts do not exactly match the engine declaration");
  }
  for (const [path, kind] of supporting) {
    const entry = options.frozen.find((candidate) => candidate.artifact.path === path);
    if (entry === undefined || entry.artifact.kind !== kind) {
      throw new Error(`supporting artifact ${path} is absent or misclassified`);
    }
  }
  const verifierFailed =
    options.engine.draft.status === "success" && options.verifier.status !== "passed";
  const status = verifierFailed ? "failed" : options.engine.draft.status;
  const exactPositiveClaim =
    options.mission.schema === "canopus.mission.v1" &&
    options.mission.result_contract !== undefined &&
    options.engine.draft.status === "success" &&
    options.verifier.status === "passed"
      ? options.mission.result_contract.claim_exact
      : undefined;
  const claim = verifierFailed
    ? `The engine proposed a candidate, but the declared verifier did not pass: ${options.engine.draft.claim}`
    : exactPositiveClaim ?? options.engine.draft.claim;
  const caveats = unique([
    ...options.engine.draft.caveats.map(finalizeWorkerCaveat),
    `Declared verifier outcome: ${options.verifier.status}.`,
    "Vela Agent produced this record; it is not a Verification Record or Decision.",
  ]);
  const base = {
    schema: "canopus.candidate.v0" as const,
    mission_id: options.mission.id,
    status,
    claim,
    artifacts: options.frozen.map((entry) => entry.artifact),
    observations: options.engine.draft.observations,
    tests: [options.verifier.record],
    costs: {
      wall_time_ms: options.budget.research_elapsed_ms,
      attempt: options.budget.attempts,
      input_tokens: options.engine.usage.input_tokens,
      output_tokens: options.engine.usage.output_tokens,
    },
    caveats,
    engine: options.engine.engine,
  };
  return parseCandidate(
    options.mission.parent_candidate === undefined
      ? base
      : {
          ...base,
          repair: {
            parent_candidate: options.mission.parent_candidate,
            reason: options.mission.repair_reason ?? "Bounded repair of the named candidate.",
          },
        },
  );
}
