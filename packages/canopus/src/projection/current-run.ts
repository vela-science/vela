import type { CurrentRunRecord } from "../run.js";
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

function literal<const T extends string | boolean | null>(
  value: unknown,
  expected: T,
  at: string,
): T {
  if (value !== expected) throw new Error(`${at} must be ${String(expected)}`);
  return expected;
}

function rootsAt(value: unknown, at: string): MissionRoots {
  const roots = objectAt(value, at);
  exactKeys(roots, ["git_commit", "git_tree", "vela_repository"], [], at);
  return {
    git_commit: gitObjectAt(roots.git_commit, `${at}.git_commit`),
    git_tree: gitObjectAt(roots.git_tree, `${at}.git_tree`),
    vela_repository: sha256At(roots.vela_repository, `${at}.vela_repository`),
  };
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
  const verifierRecord = objectAt(verifier.record, "run.verifier.record");
  exactKeys(
    verifierRecord,
    ["argv", "executable_digest", "exit_code", "stdout_digest", "stderr_digest", "duration_ms"],
    [],
    "run.verifier.record",
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
      "research_elapsed_ms", "research_processes", "research_output_bytes",
      "prompt_bytes", "artifact_bytes", "attempts", "observed_tokens",
    ],
    [],
    "run.budget",
  );
  rootsAt(mission.starting_roots, "run.mission.starting_roots");
  rootsAt(reproduction.roots, "run.reproduction.roots");
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
    integerAt(artifact.bytes, `${at}.bytes`, 0, 1_073_741_824);
    return true;
  });
  arrayAt(candidate.caveats, "run.candidate.caveats", { min: 1, max: 10 }, (item, at) =>
    stringAt(item, at, { min: 1, max: 4096 }));
  literal(verifier.status, "passed", "run.verifier.status");
  enumAt(
    verifier.sandbox,
    "run.verifier.sandbox",
    ["macos_sandbox", "container_network_denied"] as const,
  );
  arrayAt(verifierRecord.argv, "run.verifier.record.argv", { min: 1, max: 64 }, (item, at) =>
    stringAt(item, at, { max: 4096 }));
  sha256At(verifierRecord.executable_digest, "run.verifier.record.executable_digest");
  integerAt(verifierRecord.exit_code, "run.verifier.record.exit_code", -1, 255);
  sha256At(verifierRecord.stdout_digest, "run.verifier.record.stdout_digest");
  sha256At(verifierRecord.stderr_digest, "run.verifier.record.stderr_digest");
  integerAt(verifierRecord.duration_ms, "run.verifier.record.duration_ms", 0, 3_600_000);
  literal(reproduction.matched, true, "run.reproduction.matched");
  enumAt(
    reproduction.verifier_status,
    "run.reproduction.verifier_status",
    ["passed", "failed", "error"] as const,
  );
  sha256At(reproduction.stdout_digest, "run.reproduction.stdout_digest");
  sha256At(reproduction.stderr_digest, "run.reproduction.stderr_digest");
  integerAt(budget.research_elapsed_ms, "run.budget.research_elapsed_ms", 0, 3_600_000);
  integerAt(budget.research_processes, "run.budget.research_processes", 0, 64);
  integerAt(budget.research_output_bytes, "run.budget.research_output_bytes", 0, 67_108_864);
  integerAt(budget.prompt_bytes, "run.budget.prompt_bytes", 0, 8_388_608);
  integerAt(budget.artifact_bytes, "run.budget.artifact_bytes", 0, 1_073_741_824);
  integerAt(budget.attempts, "run.budget.attempts", 0, 8);
  integerAt(budget.observed_tokens, "run.budget.observed_tokens", 0, 1_000_000);
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
