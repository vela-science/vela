import type { CandidateStatus } from "../contracts/candidate.js";
import type { Mission } from "../contracts/mission.js";
import type { BudgetTracker } from "../budget/enforce.js";
import type { WorkspacePaths } from "../workspace/prepare.js";

export const ENGINE_OUTPUT_SCHEMA = "canopus.engine-output.v0" as const;
export const MAX_ENGINE_ARTIFACTS = 8;
export const MAX_ENGINE_OBSERVATIONS = 16;
export const MAX_ENGINE_CAVEATS = 8;

export interface DraftArtifact {
  path: string;
  kind: string;
  encoding: "utf8";
  content: string;
}

export interface CandidateDraft {
  schema: typeof ENGINE_OUTPUT_SCHEMA;
  status: CandidateStatus;
  claim: string;
  artifacts: DraftArtifact[];
  observations: string[];
  caveats: string[];
}

export interface EngineUsage {
  input_tokens: number;
  cached_input_tokens: number;
  output_tokens: number;
  reasoning_output_tokens: number;
}

export interface EngineIdentity {
  name: string;
  version: string;
  binary_sha256: string | null;
  model: string | null;
  configuration_sha256: string;
}

export interface EngineResult {
  draft: CandidateDraft;
  engine: EngineIdentity;
  usage: EngineUsage;
  wallTimeMs: number;
  eventTypes: string[];
  actionTypes: string[];
  eventsDigest: string;
  stderrDigest: string;
}

export interface EngineContext {
  mission: Mission;
  briefing: Record<string, unknown>;
  paths: WorkspacePaths;
  budget: BudgetTracker;
}

export interface Engine {
  readonly name: string;
  run(context: EngineContext): Promise<EngineResult>;
}
