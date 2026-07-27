export {
  LEGACY_RUN_RECORD_SCHEMA,
  RUN_PROJECTION_SCHEMA,
  RUN_RECORD_SCHEMA,
  parseRunRecord,
  projectRun,
} from "./projection/run.js";
export type { RunProjection, RunRecord } from "./projection/run.js";

export {
  FAILURE_PROJECTION_SCHEMA,
  FAILURE_RECORD_SCHEMA,
  parseFailureRecord,
  projectFailure,
} from "./projection/failure.js";
export type { FailureProjection, FailureRecord } from "./projection/failure.js";

export {
  parseDiagnosticRunRecord,
  projectDiagnosticRun,
} from "./projection/diagnostic.js";
