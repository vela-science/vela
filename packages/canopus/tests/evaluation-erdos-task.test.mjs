import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  ARTIFACT_PATH,
  RANGE_END,
  RANGE_START,
  SOURCE_PACKET_ROOT,
  VERIFIER_BINARY_ROOT,
  buildPacket,
} from "../evaluation/tasks/erdos-1056-10429401-10429600/task.mjs";

test("Erdős Stage A task binds the first uncovered exact range", () => {
  assert.equal(RANGE_START, 10_429_401);
  assert.equal(RANGE_END, 10_429_600);
  assert.equal(
    SOURCE_PACKET_ROOT,
    "sha256:517c16cc9c59d7f91aeaea4287e0ce49000c7545199e86ea632c0a2e91faf30b",
  );
  assert.equal(
    ARTIFACT_PATH,
    "artifacts/erdos1056-k15-range-10429401-10429600.txt",
  );
  assert.equal(
    VERIFIER_BINARY_ROOT,
    "sha256:68f64c3dc4bc55e98927f65ba509e5c571944239337864bbf631546ac259cdf4",
  );
});

test("Erdős Stage A packet preparation fails closed on source drift", () => {
  assert.throws(
    () => buildPacket(Buffer.from("{}\n")),
    /source packet root drifted/u,
  );
});

test("Erdős Stage A source contains no preflight answer", () => {
  const source = readFileSync(
    fileURLToPath(
      new URL(
        "../evaluation/tasks/erdos-1056-10429401-10429600/task.mjs",
        import.meta.url,
      ),
    ),
    "utf8",
  );
  for (const leakedAnswer of [
    "10429427",
    "3828577",
    "max_multiplicity=11",
    "cuts=342793",
  ]) {
    assert.equal(source.includes(leakedAnswer), false, leakedAnswer);
  }
});
