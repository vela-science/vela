export const ACTIVITY_SCHEMA = "canopus.activity.v0" as const;

export const WRITABLE_ACTIVITY_TYPES = [
  "run.started",
  "workspace.prepared",
  "roots.verified",
  "target.offered",
  "work.skipped",
  "repair.input_bound",
  "work.claimed",
  "engine.started",
  "engine.completed",
  "artifact.frozen",
  "artifacts.published",
  "verifier.completed",
  "candidate.finalized",
  "receipt.mapped",
  "landing.observed",
  "landing.bound",
  "landing.completed",
  "reproduction.completed",
  "projection.written",
  "run.completed",
  "run.failed",
] as const;

export const HISTORICAL_ACTIVITY_TYPES = [
  "withdrawal_capability.retained",
] as const;

export const ACTIVITY_TYPES = [
  ...WRITABLE_ACTIVITY_TYPES,
  ...HISTORICAL_ACTIVITY_TYPES,
] as const;

export type ActivityType = (typeof ACTIVITY_TYPES)[number];
export type WritableActivityType = (typeof WRITABLE_ACTIVITY_TYPES)[number];

export interface ActivityEventBody {
  schema: typeof ACTIVITY_SCHEMA;
  run_id: string;
  sequence: number;
  at: string;
  type: ActivityType;
  previous: string | null;
  payload: Record<string, unknown>;
}

export interface ActivityEvent extends ActivityEventBody {
  event_digest: string;
}
