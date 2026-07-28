import {
  exactKeys,
  objectAt,
  relativePathAt,
  sha256At,
  stringAt,
} from "../contracts/validation.js";

export const FAILURE_RECORD_SCHEMA = "canopus.failure.v1" as const;
export const HISTORICAL_FAILURE_RECORD_SCHEMA = "canopus.failure.v0" as const;
export const FAILURE_PROJECTION_SCHEMA = "canopus.failure-projection.v1" as const;

export interface FailureRecord {
  schema: typeof FAILURE_RECORD_SCHEMA;
  run_id: string;
  error: string;
  phase: string;
  effect: "none";
  activity_tip: string;
  authority: "non_authoritative";
}

export interface HistoricalFailureRecord {
  schema: typeof HISTORICAL_FAILURE_RECORD_SCHEMA;
  run_id: string;
  error: string;
  phase: string;
  landing_observed: boolean;
  landing_recovery: string | null;
  activity_tip: string;
  authority: "non_authoritative";
}

export interface FailureProjection {
  schema: typeof FAILURE_PROJECTION_SCHEMA;
  authority: "read_only_projection";
  run_id: string;
  status: "failed";
  phase: string;
  error: string;
  effect: "none" | "historical_landing_observed";
  recovery: string | null;
  activity_tip: string;
  deletion_test: "Vela state does not depend on this projection; show retained recovery evidence for any historical landing effect.";
}

function literal<const T extends string | boolean>(value: unknown, expected: T, at: string): T {
  if (value !== expected) throw new Error(`${at} must be ${String(expected)}`);
  return expected;
}

function booleanAt(value: unknown, at: string): boolean {
  if (typeof value !== "boolean") throw new Error(`${at} must be a boolean`);
  return value;
}

export function parseFailureRecord(value: unknown): FailureRecord | HistoricalFailureRecord {
  const record = objectAt(value, "failure");
  if (record.schema === HISTORICAL_FAILURE_RECORD_SCHEMA) {
    exactKeys(
      record,
      [
        "schema",
        "run_id",
        "error",
        "phase",
        "landing_observed",
        "landing_recovery",
        "activity_tip",
        "authority",
      ],
      [],
      "failure",
    );
    return {
      schema: HISTORICAL_FAILURE_RECORD_SCHEMA,
      run_id: stringAt(record.run_id, "failure.run_id", {
        min: 5,
        max: 128,
        pattern: /^run_[A-Za-z0-9._-]+$/u,
      }),
      error: stringAt(record.error, "failure.error", { min: 1, max: 16_384 }),
      phase: stringAt(record.phase, "failure.phase", {
        min: 1,
        max: 128,
        pattern: /^[a-z][a-z0-9_]*$/u,
      }),
      landing_observed: booleanAt(record.landing_observed, "failure.landing_observed"),
      landing_recovery: record.landing_recovery === null
        ? null
        : relativePathAt(record.landing_recovery, "failure.landing_recovery"),
      activity_tip: sha256At(record.activity_tip, "failure.activity_tip"),
      authority: literal(record.authority, "non_authoritative", "failure.authority"),
    };
  }
  exactKeys(
    record,
    [
      "schema",
      "run_id",
      "error",
      "phase",
      "effect",
      "activity_tip",
      "authority",
    ],
    [],
    "failure",
  );
  return {
    schema: literal(record.schema, FAILURE_RECORD_SCHEMA, "failure.schema"),
    run_id: stringAt(record.run_id, "failure.run_id", {
      min: 5,
      max: 128,
      pattern: /^run_[A-Za-z0-9._-]+$/u,
    }),
    error: stringAt(record.error, "failure.error", { min: 1, max: 16_384 }),
    phase: stringAt(record.phase, "failure.phase", {
      min: 1,
      max: 128,
      pattern: /^[a-z][a-z0-9_]*$/u,
    }),
    effect: literal(record.effect, "none", "failure.effect"),
    activity_tip: sha256At(record.activity_tip, "failure.activity_tip"),
    authority: literal(record.authority, "non_authoritative", "failure.authority"),
  };
}

export function projectFailure(record: FailureRecord | HistoricalFailureRecord): FailureProjection {
  if (
    record.schema === HISTORICAL_FAILURE_RECORD_SCHEMA &&
    record.landing_observed !== (record.landing_recovery !== null)
  ) {
    throw new Error("failure landing observation and recovery evidence disagree");
  }
  const historicalObserved =
    record.schema === HISTORICAL_FAILURE_RECORD_SCHEMA && record.landing_observed;
  return {
    schema: FAILURE_PROJECTION_SCHEMA,
    authority: "read_only_projection",
    run_id: record.run_id,
    status: "failed",
    phase: record.phase,
    error: record.error,
    effect: historicalObserved ? "historical_landing_observed" : "none",
    recovery:
      record.schema === HISTORICAL_FAILURE_RECORD_SCHEMA ? record.landing_recovery : null,
    activity_tip: record.activity_tip,
    deletion_test:
      "Vela state does not depend on this projection; show retained recovery evidence for any historical landing effect.",
  };
}
