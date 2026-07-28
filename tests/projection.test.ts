import assert from "node:assert/strict";
import test from "node:test";

import { parseRunRecord, projectRun, type RunRecord } from "../src/projection/run.js";

const digest = `sha256:${"a".repeat(64)}`;
const roots = {
  git_commit: "b".repeat(40), git_tree: "c".repeat(40),
  vela_repository: digest,
};

function record(): RunRecord {
  return {
    schema: "canopus.run.v0", run_id: "run_12345678", status: "completed",
    authority: "non_authoritative", external_gate_credit: false,
    mission: { id: "mission_test", target: "target-1", digest, starting_roots: roots },
    candidate: {
      digest, status: "success", claim: "Bounded result.", caveats: ["Pending."],
      artifacts: [{ path: "result", kind: "witness", digest, bytes: 1 }],
    },
    verifier: {
      status: "passed", sandbox: "macos_sandbox",
      record: { argv: ["verify"], executable_digest: digest, exit_code: 0, stdout_digest: digest, stderr_digest: digest, duration_ms: 1 },
    },
    landing: {
      operation_id: "op", receipt_root: digest, proposal_id: "vpr_test",
      route: "defer", original_route: null, accepted_event_delta: 0, publication_state: "committed_local",
    },
    final_roots: roots,
    reproduction: { matched: true, roots, verifier_status: "passed", stdout_digest: digest, stderr_digest: digest },
    budget: {
      research_elapsed_ms: 1, research_processes: 2, research_output_bytes: 1,
      prompt_bytes: 1, artifact_bytes: 1, attempts: 1, observed_tokens: 0,
    },
  };
}

test("projection is explicitly read-only and rebuildable", () => {
  const first = projectRun(record());
  const second = projectRun(JSON.parse(JSON.stringify(record())) as RunRecord);
  assert.deepEqual(first, second);
  assert.equal(first.authority, "read_only_projection");
  assert.equal(first.accepted_state_effect, "unchanged_pending");
  assert.match(first.deletion_test, /do not depend/u);
});

test("run inspection rejects nested drift instead of casting it", () => {
  assert.deepEqual(parseRunRecord(record()), record());
  const drifted = structuredClone(record()) as unknown as Record<string, unknown>;
  (drifted.reproduction as Record<string, unknown>).matched = false;
  assert.throws(() => parseRunRecord(drifted), /run\.reproduction\.matched must be true/u);
});

test("run v1 requires stage-typed observations while v0 remains replayable", () => {
  const current = record();
  current.schema = "canopus.run.v1";
  current.observations = {
    worker_observations: ["Worker found one bounded candidate."],
    verifier_observations: ["Frozen verifier passed."],
    standing_caveats: ["The claim remains pending human review."],
  };
  assert.deepEqual(parseRunRecord(current), current);
  const missing = structuredClone(current) as unknown as Record<string, unknown>;
  delete missing.observations;
  assert.throws(() => parseRunRecord(missing), /run\.observations is required/u);
  const legacy = record();
  assert.deepEqual(parseRunRecord(legacy), legacy);
});
