import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  ARTIFACT_SCHEMA,
  FIGURE_S5_ROOT,
  SOURCE_FILES,
  TASK_ID,
  assertSafeArchiveEntries,
  buildPacket,
  packetBytes,
  parseArtifact,
  parseReplayEvidence,
  verifyArtifactAgainstReplay,
} from "../evaluation/tasks/core-bench-1108125/task.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));

function artifact(overrides = {}) {
  return {
    schema: ARTIFACT_SCHEMA,
    task_id: TASK_ID,
    forestgroup_mean: 0.34,
    gender_mean: 0.46,
    income_mean: 1,
    eigen_trend: "decrease",
    ...overrides,
  };
}

function replayStdout(overrides = {}) {
  const values = {
    forestgroup: "0.34",
    gender: "0.46",
    income: "1.00",
    figure: FIGURE_S5_ROOT,
    ...overrides,
  };
  return [
    `forestgroup 173 ${values.forestgroup} 0.48 0 1`,
    `gender      173 ${values.gender} 0.50 0 1`,
    `income      172 ${values.income} 0.78 0.09 4.39`,
    `CANOPUS_FIGURE_S5 ${values.figure}`,
    "",
  ].join("\n");
}

test("scientific task artifact is closed and exactly scoped", () => {
  assert.deepEqual(parseArtifact(artifact()), artifact());
  assert.throws(
    () => parseArtifact({ ...artifact(), accepted: true }),
    /must contain exactly/u,
  );
  assert.throws(
    () => parseArtifact(artifact({ eigen_trend: "universal" })),
    /eigen_trend is unsupported/u,
  );
});

test("task packet contains only the answer-safe source allowlist", () => {
  const files = new Map(SOURCE_FILES.map((file) => [file, Buffer.from(`${file}\n`)]));
  const packet = buildPacket(files);
  assert.deepEqual(packet.files.map((file) => file.path), SOURCE_FILES);
  assert.equal(packet.files.some((file) => file.path.startsWith("results/")), false);
  assert.equal(packet.constraints.precomputed_results, "not_provided");
  assert.equal(packet.constraints.verifier, "not_exposed");
  assert.equal(packet.output.additional_fields, "forbidden");
  assert.deepEqual(Object.keys(packet.output.fields), [
    "schema",
    "task_id",
    "forestgroup_mean",
    "gender_mean",
    "income_mean",
    "eigen_trend",
  ]);
  assert.equal(
    packet.output.fields.forestgroup_mean.rounding,
    "nearest two decimal places",
  );
  assert.equal("exact" in packet.output.fields.forestgroup_mean, false);
  assert.equal("exact" in packet.output.fields.gender_mean, false);
  assert.equal("exact" in packet.output.fields.income_mean, false);
  assert.deepEqual(packet.output.fields.eigen_trend.enum, ["decrease", "increase"]);
  assert.deepEqual(packetBytes(packet), packetBytes(packet));
  files.set("results/output", Buffer.from("answer\n"));
  assert.throws(() => buildPacket(files), /source allowlist mismatch/u);
});

test("archive path audit rejects traversal and unexpected roots", () => {
  assert.doesNotThrow(() => assertSafeArchiveEntries([
    "capsule-1108125/",
    "capsule-1108125/code/analysis.R",
  ]));
  assert.throws(
    () => assertSafeArchiveEntries(["capsule-1108125/code/../../answer"]),
    /unsafe path/u,
  );
  assert.throws(
    () => assertSafeArchiveEntries(["other-capsule/code/main.R"]),
    /unexpected root/u,
  );
});

test("replay parser and artifact comparison fail closed on result drift", () => {
  const replay = parseReplayEvidence(replayStdout());
  assert.deepEqual(replay, {
    forestgroup_mean: 0.34,
    gender_mean: 0.46,
    income_mean: 1,
    figure_s5_root: FIGURE_S5_ROOT,
  });
  assert.doesNotThrow(() => verifyArtifactAgainstReplay(artifact(), replay));
  assert.throws(
    () => verifyArtifactAgainstReplay(
      artifact(),
      parseReplayEvidence(replayStdout({ income: "1.01" })),
    ),
    /replay income_mean drifted/u,
  );
  assert.throws(
    () => verifyArtifactAgainstReplay(artifact({ gender_mean: 0.47 }), replay),
    /artifact gender_mean does not match/u,
  );
  assert.throws(
    () => verifyArtifactAgainstReplay(
      artifact(),
      parseReplayEvidence(replayStdout({ figure: `sha256:${"0".repeat(64)}` })),
    ),
    /FigureS5 root drifted/u,
  );
});

test("preparation rejects bytes other than the registered source archive", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "canopus-task-prepare-"));
  const archive = path.join(root, "wrong.tar.gz");
  const output = path.join(root, "output");
  writeFileSync(archive, "wrong archive\n");
  const result = spawnSync(
    process.execPath,
    [
      "evaluation/tasks/core-bench-1108125/prepare.mjs",
      "--archive",
      archive,
      "--output",
      output,
    ],
    {
      cwd: packageRoot,
      encoding: "utf8",
    },
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /source archive root drifted/u);
});
