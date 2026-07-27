import type { MissionRoots } from "../contracts/mission.js";
import type { FrozenArtifact } from "../contracts/candidate.js";
import type { RunRecord } from "./run.js";
import {
  arrayAt,
  enumAt,
  exactKeys,
  objectAt,
  relativePathAt,
  sha256At,
  stringAt,
} from "../contracts/validation.js";

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
    artifacts: FrozenArtifact[];
    caveats: string[];
  };
  verifier: RunRecord["verifier"];
  landing: null;
  reproduction: RunRecord["reproduction"];
  budget: RunRecord["budget"];
}

function literal<const T extends string | boolean | null>(value: unknown, expected: T, at: string): T {
  if (value !== expected) throw new Error(`${at} must be ${String(expected)}`);
  return expected;
}

export function parseDiagnosticRunRecord(value: unknown): DiagnosticRunRecord {
  const record = objectAt(value, "diagnostic run");
  exactKeys(
    record,
    [
      "schema", "run_id", "status", "mode", "authority", "external_gate_credit",
      "mission", "candidate", "verifier", "landing", "reproduction", "budget",
    ],
    [],
    "diagnostic run",
  );
  const mission = objectAt(record.mission, "diagnostic run.mission");
  exactKeys(mission, ["id", "target", "digest", "starting_roots"], [], "diagnostic run.mission");
  const candidate = objectAt(record.candidate, "diagnostic run.candidate");
  exactKeys(candidate, ["digest", "status", "claim", "artifacts", "caveats"], [], "diagnostic run.candidate");
  const verifier = objectAt(record.verifier, "diagnostic run.verifier");
  exactKeys(verifier, ["status", "sandbox", "record"], [], "diagnostic run.verifier");
  const reproduction = objectAt(record.reproduction, "diagnostic run.reproduction");
  exactKeys(reproduction, ["matched", "roots", "verifier_status", "stdout_digest", "stderr_digest"], [], "diagnostic run.reproduction");
  objectAt(mission.starting_roots, "diagnostic run.mission.starting_roots");
  objectAt(reproduction.roots, "diagnostic run.reproduction.roots");
  objectAt(record.budget, "diagnostic run.budget");
  literal(record.schema, "canopus.diagnostic-run.v1", "diagnostic run.schema");
  literal(record.status, "completed", "diagnostic run.status");
  literal(record.mode, "no_land", "diagnostic run.mode");
  literal(record.authority, "non_authoritative", "diagnostic run.authority");
  literal(record.external_gate_credit, false, "diagnostic run.external_gate_credit");
  literal(record.landing, null, "diagnostic run.landing");
  stringAt(record.run_id, "diagnostic run.run_id", { min: 5, max: 128 });
  stringAt(mission.id, "diagnostic run.mission.id", { min: 1, max: 134 });
  stringAt(mission.target, "diagnostic run.mission.target", { min: 1, max: 256 });
  sha256At(mission.digest, "diagnostic run.mission.digest");
  sha256At(candidate.digest, "diagnostic run.candidate.digest");
  enumAt(candidate.status, "diagnostic run.candidate.status", ["success", "null", "failed"] as const);
  stringAt(candidate.claim, "diagnostic run.candidate.claim", { min: 1, max: 8192 });
  arrayAt(candidate.artifacts, "diagnostic run.candidate.artifacts", { max: 10 }, (item, at) => {
    const artifact = objectAt(item, at);
    exactKeys(artifact, ["path", "kind", "digest", "bytes"], [], at);
    relativePathAt(artifact.path, `${at}.path`);
    stringAt(artifact.kind, `${at}.kind`, { min: 1, max: 128 });
    sha256At(artifact.digest, `${at}.digest`);
    return true;
  });
  arrayAt(candidate.caveats, "diagnostic run.candidate.caveats", { max: 10 }, (item, at) =>
    stringAt(item, at, { min: 1, max: 4096 }));
  enumAt(verifier.status, "diagnostic run.verifier.status", ["passed", "failed", "error"] as const);
  literal(reproduction.matched, true, "diagnostic run.reproduction.matched");
  return value as DiagnosticRunRecord;
}

export function projectDiagnosticRun(record: DiagnosticRunRecord): {
  schema: "canopus.diagnostic-projection.v1";
  authority: "read_only_projection";
  run_id: string;
  target: string;
  candidate_digest: string;
  verifier_status: "passed" | "failed" | "error";
  landed: false;
  clean_clone_reproduced: true;
} {
  return {
    schema: "canopus.diagnostic-projection.v1",
    authority: "read_only_projection",
    run_id: record.run_id,
    target: record.mission.target,
    candidate_digest: record.candidate.digest,
    verifier_status: record.verifier.status,
    landed: false,
    clean_clone_reproduced: true,
  };
}
