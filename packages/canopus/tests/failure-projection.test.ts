import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

import {
  parseFailureRecord,
  projectFailure,
  type HistoricalFailureRecord,
} from "../src/projection/failure.js";

const digest = `sha256:${"a".repeat(64)}`;
const execFileAsync = promisify(execFile);

function record(): HistoricalFailureRecord {
  return {
    schema: "canopus.failure.v0",
    run_id: "run_deadbeef",
    error: "worker returned null; verifier and landing were not run",
    phase: "engine_non_success",
    landing_observed: false,
    landing_recovery: null,
    activity_tip: digest,
    authority: "non_authoritative",
  };
}

test("historical failed run projection is read-only and makes landing uncertainty explicit", () => {
  assert.deepEqual(parseFailureRecord(record()), record());
  const projection = projectFailure(record());
  assert.equal(projection.authority, "read_only_projection");
  assert.equal(projection.status, "failed");
  assert.equal(projection.effect, "none");

  const observed = record();
  observed.phase = "receipt_binding";
  observed.landing_observed = true;
  observed.landing_recovery = "landing-recovery.json";
  assert.equal(projectFailure(observed).effect, "historical_landing_observed");
});

test("current failed run records expose only a nonmutating effect", () => {
  const current = {
    schema: "canopus.failure.v1",
    run_id: "run_current_failure",
    error: "worker stopped",
    phase: "worker",
    effect: "none",
    activity_tip: digest,
    authority: "non_authoritative",
  } as const;
  assert.deepEqual(parseFailureRecord(current), current);
  assert.equal(projectFailure(parseFailureRecord(current)).effect, "none");
});

test("built vela agent show accepts a current failed run record", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-agent-failure-show-"));
  const failurePath = path.join(root, "failure.json");
  await writeFile(
    failurePath,
    `${JSON.stringify({
      schema: "canopus.failure.v1",
      run_id: "run_current_failure",
      error: "worker stopped",
      phase: "worker",
      effect: "none",
      activity_tip: digest,
      authority: "non_authoritative",
    })}\n`,
    "utf8",
  );
  const helper = fileURLToPath(new URL("../vela-agent", import.meta.url));
  const { stdout } = await execFileAsync("bun", [helper, "show", failurePath]);
  const result = JSON.parse(stdout) as {
    ok: boolean;
    projection: {
      authority: string;
      status: string;
      effect: string;
    };
  };
  assert.equal(result.ok, true);
  assert.equal(result.projection.authority, "read_only_projection");
  assert.equal(result.projection.status, "failed");
  assert.equal(result.projection.effect, "none");
});

test("failed run projection rejects drift and missing recovery evidence", () => {
  const drifted = structuredClone(record()) as unknown as Record<string, unknown>;
  drifted.accepted_state_effect = "unchanged";
  assert.throws(() => parseFailureRecord(drifted), /accepted_state_effect is not allowed/u);

  const contradictory = record();
  contradictory.landing_observed = true;
  assert.throws(() => projectFailure(contradictory), /landing observation and recovery evidence disagree/u);
});
