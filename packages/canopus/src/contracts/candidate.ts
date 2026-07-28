import {
  MISSION_ID_RE,
  arrayAt,
  enumAt,
  exactKeys,
  integerAt,
  objectAt,
  relativePathAt,
  sha256At,
  stringAt,
} from "./validation.js";

export const CANDIDATE_SCHEMA = "canopus.candidate.v0" as const;
export const CANDIDATE_STATUSES = ["success", "null", "failed"] as const;
export const MAX_CANDIDATE_ARTIFACTS = 10;
export const MAX_CANDIDATE_OBSERVATIONS = 16;
export const MAX_CANDIDATE_TESTS = 2;
export const MAX_CANDIDATE_CAVEATS = 10;

export type CandidateStatus = (typeof CANDIDATE_STATUSES)[number];

export interface FrozenArtifact {
  path: string;
  kind: string;
  digest: string;
  bytes: number;
}

export interface TestRecord {
  argv: string[];
  executable_digest: string;
  exit_code: number;
  stdout_digest: string;
  stderr_digest: string;
  duration_ms: number;
}

export interface CandidateCosts {
  wall_time_ms: number;
  attempt: number;
  input_tokens: number;
  output_tokens: number;
}

export interface Candidate {
  schema: typeof CANDIDATE_SCHEMA;
  mission_id: string;
  status: CandidateStatus;
  claim: string;
  artifacts: FrozenArtifact[];
  observations: string[];
  tests: TestRecord[];
  costs: CandidateCosts;
  caveats: string[];
  engine: {
    name: string;
    version: string;
    binary_sha256: string | null;
    model: string | null;
    configuration_sha256: string;
  };
  repair?: { parent_candidate: string; reason: string };
}

function parseArtifact(value: unknown, at: string): FrozenArtifact {
  const object = objectAt(value, at);
  exactKeys(object, ["path", "kind", "digest", "bytes"], [], at);
  return {
    path: relativePathAt(object.path, `${at}.path`),
    kind: stringAt(object.kind, `${at}.kind`, { min: 1, max: 128 }),
    digest: sha256At(object.digest, `${at}.digest`),
    bytes: integerAt(object.bytes, `${at}.bytes`, 0, 1_073_741_824),
  };
}

function parseTest(value: unknown, at: string): TestRecord {
  const object = objectAt(value, at);
  exactKeys(
    object,
    ["argv", "executable_digest", "exit_code", "stdout_digest", "stderr_digest", "duration_ms"],
    [],
    at,
  );
  return {
    argv: arrayAt(object.argv, `${at}.argv`, { min: 1, max: 64 }, (item, itemAt) =>
      stringAt(item, itemAt, { max: 4096 }),
    ),
    executable_digest: sha256At(object.executable_digest, `${at}.executable_digest`),
    exit_code: integerAt(object.exit_code, `${at}.exit_code`, -1, 255),
    stdout_digest: sha256At(object.stdout_digest, `${at}.stdout_digest`),
    stderr_digest: sha256At(object.stderr_digest, `${at}.stderr_digest`),
    duration_ms: integerAt(object.duration_ms, `${at}.duration_ms`, 0, 3_600_000),
  };
}

function parseCosts(value: unknown): CandidateCosts {
  const object = objectAt(value, "candidate.costs");
  exactKeys(
    object,
    ["wall_time_ms", "attempt", "input_tokens", "output_tokens"],
    [],
    "candidate.costs",
  );
  return {
    wall_time_ms: integerAt(object.wall_time_ms, "candidate.costs.wall_time_ms", 0, 3_600_000),
    attempt: integerAt(object.attempt, "candidate.costs.attempt", 1, 8),
    input_tokens: integerAt(object.input_tokens, "candidate.costs.input_tokens", 0, 1_000_000),
    output_tokens: integerAt(object.output_tokens, "candidate.costs.output_tokens", 0, 1_000_000),
  };
}

export function parseCandidate(value: unknown): Candidate {
  const object = objectAt(value, "candidate");
  exactKeys(
    object,
    [
      "schema",
      "mission_id",
      "status",
      "claim",
      "artifacts",
      "observations",
      "tests",
      "costs",
      "caveats",
      "engine",
    ],
    ["repair"],
    "candidate",
  );
  const engine = objectAt(object.engine, "candidate.engine");
  exactKeys(
    engine,
    ["name", "version", "binary_sha256", "model", "configuration_sha256"],
    [],
    "candidate.engine",
  );

  const base = {
    schema: enumAt(object.schema, "candidate.schema", [CANDIDATE_SCHEMA] as const),
    mission_id: stringAt(object.mission_id, "candidate.mission_id", {
      max: 134,
      pattern: MISSION_ID_RE,
    }),
    status: enumAt(object.status, "candidate.status", CANDIDATE_STATUSES),
    claim: stringAt(object.claim, "candidate.claim", { min: 1, max: 8192 }),
    artifacts: arrayAt(
      object.artifacts,
      "candidate.artifacts",
      { max: MAX_CANDIDATE_ARTIFACTS },
      parseArtifact,
    ),
    observations: arrayAt(
      object.observations,
      "candidate.observations",
      { max: MAX_CANDIDATE_OBSERVATIONS },
      (item, at) => stringAt(item, at, { min: 1, max: 4096 }),
    ),
    tests: arrayAt(object.tests, "candidate.tests", { max: MAX_CANDIDATE_TESTS }, parseTest),
    costs: parseCosts(object.costs),
    caveats: arrayAt(object.caveats, "candidate.caveats", { max: MAX_CANDIDATE_CAVEATS }, (item, at) =>
      stringAt(item, at, { min: 1, max: 4096 }),
    ),
    engine: {
      name: stringAt(engine.name, "candidate.engine.name", { min: 1, max: 128 }),
      version: stringAt(engine.version, "candidate.engine.version", { min: 1, max: 256 }),
      binary_sha256:
        engine.binary_sha256 === null
          ? null
          : sha256At(engine.binary_sha256, "candidate.engine.binary_sha256"),
      model:
        engine.model === null
          ? null
          : stringAt(engine.model, "candidate.engine.model", { min: 1, max: 256 }),
      configuration_sha256: sha256At(
        engine.configuration_sha256,
        "candidate.engine.configuration_sha256",
      ),
    },
  };

  if (object.repair === undefined) {
    return base;
  }
  const repair = objectAt(object.repair, "candidate.repair");
  exactKeys(repair, ["parent_candidate", "reason"], [], "candidate.repair");
  return {
    ...base,
    repair: {
      parent_candidate: sha256At(
        repair.parent_candidate,
        "candidate.repair.parent_candidate",
      ),
      reason: stringAt(repair.reason, "candidate.repair.reason", { min: 1, max: 4096 }),
    },
  };
}
