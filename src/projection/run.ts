import type { MissionRoots } from "../contracts/mission.js";
import {
  arrayAt,
  enumAt,
  exactKeys,
  gitObjectAt,
  integerAt,
  objectAt,
  relativePathAt,
  sha256At,
  stringAt,
} from "../contracts/validation.js";

export const RUN_RECORD_SCHEMA = "canopus.run.v1" as const;
export const LEGACY_RUN_RECORD_SCHEMA = "canopus.run.v0" as const;
export const RUN_PROJECTION_SCHEMA = "canopus.run-projection.v0" as const;

export interface RunRecord {
  schema: typeof RUN_RECORD_SCHEMA | typeof LEGACY_RUN_RECORD_SCHEMA;
  run_id: string;
  status: "completed";
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
  observations?: {
    worker_observations: string[];
    verifier_observations: string[];
    standing_caveats: string[];
  };
  verifier: {
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
  };
  landing: {
    operation_id: string;
    receipt_root: string;
    proposal_id: string;
    route: "permit" | "defer" | "exact_retry";
    original_route: "permit" | "defer" | null;
    accepted_event_delta: number | null;
    publication_state: string;
  };
  final_roots: MissionRoots;
  reproduction: {
    matched: boolean;
    roots: MissionRoots;
    verifier_status: "passed" | "failed" | "error";
    stdout_digest: string;
    stderr_digest: string;
  };
  budget: {
    research_elapsed_ms: number;
    research_processes: number;
    research_output_bytes: number;
    prompt_bytes: number;
    artifact_bytes: number;
    attempts: number;
    observed_tokens: number;
  };
}

export interface RunProjection {
  schema: typeof RUN_PROJECTION_SCHEMA;
  authority: "read_only_projection";
  run_id: string;
  mission_id: string;
  target: string;
  starting_roots: MissionRoots;
  final_roots: MissionRoots;
  candidate_digest: string;
  candidate_status: "success" | "null" | "failed";
  artifact_digests: string[];
  verifier_status: "passed" | "failed" | "error";
  receipt_root: string;
  proposal_id: string;
  route: "permit" | "defer" | "exact_retry";
  original_route: "permit" | "defer" | null;
  accepted_state_effect: "policy_admitted" | "unchanged_pending" | "exact_retry";
  publication_state: string;
  clean_clone_reproduced: boolean;
  deletion_test: "Vela replay and accepted state do not depend on this projection.";
}

function literal<const T extends string | boolean>(value: unknown, expected: T, at: string): T {
  if (value !== expected) throw new Error(`${at} must be ${String(expected)}`);
  return expected;
}

function rootsAt(value: unknown, at: string): MissionRoots {
  const object = objectAt(value, at);
  exactKeys(object, ["git_commit", "git_tree", "vela_repository"], [], at);
  return {
    git_commit: gitObjectAt(object.git_commit, `${at}.git_commit`),
    git_tree: gitObjectAt(object.git_tree, `${at}.git_tree`),
    vela_repository: sha256At(object.vela_repository, `${at}.vela_repository`),
  };
}

function nullableRoute(value: unknown, at: string): "permit" | "defer" | null {
  return value === null ? null : enumAt(value, at, ["permit", "defer"] as const);
}

function nullableCount(value: unknown, at: string): number | null {
  return value === null ? null : integerAt(value, at, 0, Number.MAX_SAFE_INTEGER);
}

export function parseRunRecord(value: unknown): RunRecord {
  const record = objectAt(value, "run");
  const schema = enumAt(
    record.schema,
    "run.schema",
    [RUN_RECORD_SCHEMA, LEGACY_RUN_RECORD_SCHEMA] as const,
  );
  exactKeys(
    record,
    [
      "schema", "run_id", "status", "authority", "external_gate_credit", "mission",
      "candidate", "verifier", "landing", "final_roots", "reproduction", "budget",
      ...(schema === RUN_RECORD_SCHEMA ? ["observations"] : []),
    ],
    schema === LEGACY_RUN_RECORD_SCHEMA ? ["observations"] : [],
    "run",
  );
  const mission = objectAt(record.mission, "run.mission");
  exactKeys(mission, ["id", "target", "digest", "starting_roots"], [], "run.mission");
  const candidate = objectAt(record.candidate, "run.candidate");
  exactKeys(candidate, ["digest", "status", "claim", "artifacts", "caveats"], [], "run.candidate");
  const verifier = objectAt(record.verifier, "run.verifier");
  exactKeys(verifier, ["status", "sandbox", "record"], [], "run.verifier");
  const testRecord = objectAt(verifier.record, "run.verifier.record");
  exactKeys(
    testRecord,
    ["argv", "executable_digest", "exit_code", "stdout_digest", "stderr_digest", "duration_ms"],
    [],
    "run.verifier.record",
  );
  const landing = objectAt(record.landing, "run.landing");
  exactKeys(
    landing,
    [
      "operation_id", "receipt_root", "proposal_id", "route", "original_route",
      "accepted_event_delta", "publication_state",
    ],
    [],
    "run.landing",
  );
  const reproduction = objectAt(record.reproduction, "run.reproduction");
  exactKeys(
    reproduction,
    ["matched", "roots", "verifier_status", "stdout_digest", "stderr_digest"],
    [],
    "run.reproduction",
  );
  const budget = objectAt(record.budget, "run.budget");
  exactKeys(
    budget,
    [
      "research_elapsed_ms", "research_processes", "research_output_bytes", "prompt_bytes",
      "artifact_bytes", "attempts", "observed_tokens",
    ],
    [],
    "run.budget",
  );
  const observations = record.observations === undefined ? undefined : (() => {
    const stages = objectAt(record.observations, "run.observations");
    exactKeys(
      stages,
      ["worker_observations", "verifier_observations", "standing_caveats"],
      [],
      "run.observations",
    );
    const strings = (value: unknown, at: string) =>
      arrayAt(value, at, { max: 16 }, (item, itemAt) =>
        stringAt(item, itemAt, { min: 1, max: 4096 }));
    return {
      worker_observations: strings(stages.worker_observations, "run.observations.worker_observations"),
      verifier_observations: strings(stages.verifier_observations, "run.observations.verifier_observations"),
      standing_caveats: strings(stages.standing_caveats, "run.observations.standing_caveats"),
    };
  })();
  return {
    schema,
    run_id: stringAt(record.run_id, "run.run_id", { min: 5, max: 128 }),
    status: literal(record.status, "completed", "run.status"),
    authority: literal(record.authority, "non_authoritative", "run.authority"),
    external_gate_credit: literal(
      record.external_gate_credit,
      false,
      "run.external_gate_credit",
    ),
    mission: {
      id: stringAt(mission.id, "run.mission.id", { min: 1, max: 134 }),
      target: stringAt(mission.target, "run.mission.target", { min: 1, max: 256 }),
      digest: sha256At(mission.digest, "run.mission.digest"),
      starting_roots: rootsAt(mission.starting_roots, "run.mission.starting_roots"),
    },
    candidate: {
      digest: sha256At(candidate.digest, "run.candidate.digest"),
      status: enumAt(candidate.status, "run.candidate.status", ["success", "null", "failed"] as const),
      claim: stringAt(candidate.claim, "run.candidate.claim", { min: 1, max: 8192 }),
      artifacts: arrayAt(candidate.artifacts, "run.candidate.artifacts", { max: 10 }, (item, at) => {
        const artifact = objectAt(item, at);
        exactKeys(artifact, ["path", "kind", "digest", "bytes"], [], at);
        return {
          path: relativePathAt(artifact.path, `${at}.path`),
          kind: stringAt(artifact.kind, `${at}.kind`, { min: 1, max: 128 }),
          digest: sha256At(artifact.digest, `${at}.digest`),
          bytes: integerAt(artifact.bytes, `${at}.bytes`, 0, 1_073_741_824),
        };
      }),
      caveats: arrayAt(candidate.caveats, "run.candidate.caveats", { max: 10 }, (item, at) =>
        stringAt(item, at, { min: 1, max: 4096 }),
      ),
    },
    ...(observations === undefined ? {} : { observations }),
    verifier: {
      status: enumAt(verifier.status, "run.verifier.status", ["passed", "failed", "error"] as const),
      sandbox: enumAt(
        verifier.sandbox,
        "run.verifier.sandbox",
        ["macos_sandbox", "container_network_denied"] as const,
      ),
      record: {
        argv: arrayAt(testRecord.argv, "run.verifier.record.argv", { min: 1, max: 64 }, (item, at) =>
          stringAt(item, at, { max: 4096 }),
        ),
        executable_digest: sha256At(testRecord.executable_digest, "run.verifier.record.executable_digest"),
        exit_code: integerAt(testRecord.exit_code, "run.verifier.record.exit_code", -1, 255),
        stdout_digest: sha256At(testRecord.stdout_digest, "run.verifier.record.stdout_digest"),
        stderr_digest: sha256At(testRecord.stderr_digest, "run.verifier.record.stderr_digest"),
        duration_ms: integerAt(testRecord.duration_ms, "run.verifier.record.duration_ms", 0, 3_600_000),
      },
    },
    landing: {
      operation_id: stringAt(landing.operation_id, "run.landing.operation_id", { min: 1, max: 256 }),
      receipt_root: sha256At(landing.receipt_root, "run.landing.receipt_root"),
      proposal_id: stringAt(landing.proposal_id, "run.landing.proposal_id", { min: 1, max: 256 }),
      route: enumAt(landing.route, "run.landing.route", ["permit", "defer", "exact_retry"] as const),
      original_route: nullableRoute(landing.original_route, "run.landing.original_route"),
      accepted_event_delta: nullableCount(landing.accepted_event_delta, "run.landing.accepted_event_delta"),
      publication_state: stringAt(landing.publication_state, "run.landing.publication_state", { min: 1, max: 64 }),
    },
    final_roots: rootsAt(record.final_roots, "run.final_roots"),
    reproduction: {
      matched: literal(reproduction.matched, true, "run.reproduction.matched"),
      roots: rootsAt(reproduction.roots, "run.reproduction.roots"),
      verifier_status: enumAt(
        reproduction.verifier_status,
        "run.reproduction.verifier_status",
        ["passed", "failed", "error"] as const,
      ),
      stdout_digest: sha256At(reproduction.stdout_digest, "run.reproduction.stdout_digest"),
      stderr_digest: sha256At(reproduction.stderr_digest, "run.reproduction.stderr_digest"),
    },
    budget: {
      research_elapsed_ms: integerAt(budget.research_elapsed_ms, "run.budget.research_elapsed_ms", 0, 3_600_000),
      research_processes: integerAt(budget.research_processes, "run.budget.research_processes", 0, 64),
      research_output_bytes: integerAt(budget.research_output_bytes, "run.budget.research_output_bytes", 0, 67_108_864),
      prompt_bytes: integerAt(budget.prompt_bytes, "run.budget.prompt_bytes", 0, 8_388_608),
      artifact_bytes: integerAt(budget.artifact_bytes, "run.budget.artifact_bytes", 0, 1_073_741_824),
      attempts: integerAt(budget.attempts, "run.budget.attempts", 0, 8),
      observed_tokens: integerAt(budget.observed_tokens, "run.budget.observed_tokens", 0, 1_000_000),
    },
  };
}

export function projectRun(record: RunRecord): RunProjection {
  if (
    (record.landing.route === "exact_retry") !== (record.landing.original_route !== null)
  ) {
    throw new Error("exact retry must name exactly one durable original route");
  }
  const effectiveRoute =
    record.landing.route === "exact_retry"
      ? record.landing.original_route
      : record.landing.route;
  if (effectiveRoute === null) throw new Error("landing has no effective route");
  const expectedDelta = effectiveRoute === "permit" ? 1 : 0;
  if (
    record.landing.accepted_event_delta !== null &&
    record.landing.accepted_event_delta !== expectedDelta
  ) {
    throw new Error(
      `${record.landing.route} landing has invalid accepted-event delta ${String(record.landing.accepted_event_delta)}`,
    );
  }
  const acceptedStateEffect =
    record.landing.route === "permit"
      ? "policy_admitted"
      : record.landing.route === "defer"
        ? "unchanged_pending"
        : "exact_retry";
  return {
    schema: RUN_PROJECTION_SCHEMA,
    authority: "read_only_projection",
    run_id: record.run_id,
    mission_id: record.mission.id,
    target: record.mission.target,
    starting_roots: record.mission.starting_roots,
    final_roots: record.final_roots,
    candidate_digest: record.candidate.digest,
    candidate_status: record.candidate.status,
    artifact_digests: record.candidate.artifacts.map((artifact) => artifact.digest),
    verifier_status: record.verifier.status,
    receipt_root: record.landing.receipt_root,
    proposal_id: record.landing.proposal_id,
    route: record.landing.route,
    original_route: record.landing.original_route,
    accepted_state_effect: acceptedStateEffect,
    publication_state: record.landing.publication_state,
    clean_clone_reproduced: record.reproduction.matched,
    deletion_test: "Vela replay and accepted state do not depend on this projection.",
  };
}
