import assert from "node:assert/strict";
import test from "node:test";

import {
  parseCurrentRunRecord,
  projectCurrentRun,
} from "../src/projection/current-run.js";
import type { CurrentRunRecord } from "../src/run.js";

const digest = `sha256:${"a".repeat(64)}`;
const roots = {
  git_commit: "b".repeat(40),
  git_tree: "c".repeat(40),
  vela_repository: digest,
};

function record(): CurrentRunRecord {
  return {
    schema: "canopus.run.v2",
    run_id: "run_12345678",
    status: "completed",
    effect: "none",
    authority: "non_authoritative",
    external_gate_credit: false,
    mission: {
      id: "mission_test",
      target: "target-1",
      digest,
      starting_roots: roots,
    },
    candidate: {
      digest,
      status: "success",
      claim: "Bounded result.",
      caveats: ["This establishes only the bounded result."],
      artifacts: [{ path: "result", kind: "witness", digest, bytes: 1 }],
    },
    verifier: {
      status: "passed",
      sandbox: "macos_sandbox",
      record: {
        argv: ["verify"],
        executable_digest: digest,
        exit_code: 0,
        stdout_digest: digest,
        stderr_digest: digest,
        duration_ms: 1,
      },
    },
    submission: null,
    reproduction: {
      matched: true,
      roots,
      verifier_status: "passed",
      stdout_digest: digest,
      stderr_digest: digest,
    },
    budget: {
      research_elapsed_ms: 1,
      research_processes: 1,
      research_output_bytes: 1,
      prompt_bytes: 1,
      artifact_bytes: 1,
      attempts: 1,
      observed_tokens: 1,
    },
  };
}

test("current projection is explicitly read-only and rebuildable", () => {
  const parsed = parseCurrentRunRecord(record());
  const first = projectCurrentRun(parsed);
  const second = projectCurrentRun(
    parseCurrentRunRecord(JSON.parse(JSON.stringify(record()))),
  );
  assert.deepEqual(first, second);
  assert.equal(first.authority, "read_only_projection");
  assert.equal(first.submitted, false);
  assert.equal(first.clean_clone_reproduced, true);
});

test("current run inspection rejects nested drift instead of casting it", () => {
  const drifted = structuredClone(record()) as unknown as Record<string, unknown>;
  const verifier = (drifted.verifier as Record<string, unknown>).record as Record<string, unknown>;
  verifier.exit_code = "0";
  assert.throws(
    () => parseCurrentRunRecord(drifted),
    /run\.verifier\.record\.exit_code must be an integer/u,
  );

  const legacy = structuredClone(record()) as unknown as Record<string, unknown>;
  legacy.schema = "canopus.run.v1";
  assert.throws(
    () => parseCurrentRunRecord(legacy),
    /run\.schema must be canopus\.run\.v2/u,
  );
});
