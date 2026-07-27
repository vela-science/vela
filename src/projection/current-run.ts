import type { CurrentRunRecord } from "../run.js";
import {
  arrayAt,
  enumAt,
  exactKeys,
  objectAt,
  relativePathAt,
  sha256At,
  stringAt,
} from "../contracts/validation.js";

function literal<const T extends string | boolean | null>(
  value: unknown,
  expected: T,
  at: string,
): T {
  if (value !== expected) throw new Error(`${at} must be ${String(expected)}`);
  return expected;
}

export function parseCurrentRunRecord(value: unknown): CurrentRunRecord {
  const record = objectAt(value, "run");
  exactKeys(
    record,
    [
      "schema", "run_id", "status", "effect", "authority", "external_gate_credit",
      "mission", "candidate", "verifier", "submission", "reproduction", "budget",
    ],
    [],
    "run",
  );
  const mission = objectAt(record.mission, "run.mission");
  exactKeys(mission, ["id", "target", "digest", "starting_roots"], [], "run.mission");
  const candidate = objectAt(record.candidate, "run.candidate");
  exactKeys(candidate, ["digest", "status", "claim", "artifacts", "caveats"], [], "run.candidate");
  const verifier = objectAt(record.verifier, "run.verifier");
  exactKeys(verifier, ["status", "sandbox", "record"], [], "run.verifier");
  const reproduction = objectAt(record.reproduction, "run.reproduction");
  exactKeys(
    reproduction,
    ["matched", "roots", "verifier_status", "stdout_digest", "stderr_digest"],
    [],
    "run.reproduction",
  );
  objectAt(mission.starting_roots, "run.mission.starting_roots");
  objectAt(reproduction.roots, "run.reproduction.roots");
  objectAt(record.budget, "run.budget");
  literal(record.schema, "canopus.run.v2", "run.schema");
  literal(record.status, "completed", "run.status");
  literal(record.effect, "none", "run.effect");
  literal(record.authority, "non_authoritative", "run.authority");
  literal(record.external_gate_credit, false, "run.external_gate_credit");
  literal(record.submission, null, "run.submission");
  stringAt(record.run_id, "run.run_id", { min: 5, max: 128 });
  stringAt(mission.id, "run.mission.id", { min: 1, max: 134 });
  stringAt(mission.target, "run.mission.target", { min: 1, max: 256 });
  sha256At(mission.digest, "run.mission.digest");
  sha256At(candidate.digest, "run.candidate.digest");
  enumAt(candidate.status, "run.candidate.status", ["success", "null", "failed"] as const);
  stringAt(candidate.claim, "run.candidate.claim", { min: 1, max: 8192 });
  arrayAt(candidate.artifacts, "run.candidate.artifacts", { min: 1, max: 10 }, (item, at) => {
    const artifact = objectAt(item, at);
    exactKeys(artifact, ["path", "kind", "digest", "bytes"], [], at);
    relativePathAt(artifact.path, `${at}.path`);
    stringAt(artifact.kind, `${at}.kind`, { min: 1, max: 128 });
    sha256At(artifact.digest, `${at}.digest`);
    return true;
  });
  arrayAt(candidate.caveats, "run.candidate.caveats", { min: 1, max: 10 }, (item, at) =>
    stringAt(item, at, { min: 1, max: 4096 }));
  literal(verifier.status, "passed", "run.verifier.status");
  literal(reproduction.matched, true, "run.reproduction.matched");
  return value as CurrentRunRecord;
}

export function projectCurrentRun(record: CurrentRunRecord): {
  schema: "canopus.run-projection.v2";
  authority: "read_only_projection";
  run_id: string;
  target: string;
  candidate_digest: string;
  verifier_status: "passed" | "failed" | "error";
  submitted: false;
  clean_clone_reproduced: true;
} {
  return {
    schema: "canopus.run-projection.v2",
    authority: "read_only_projection",
    run_id: record.run_id,
    target: record.mission.target,
    candidate_digest: record.candidate.digest,
    verifier_status: record.verifier.status,
    submitted: false,
    clean_clone_reproduced: true,
  };
}
