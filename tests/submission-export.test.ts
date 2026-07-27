import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, readFile, readdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { exportSubmission, verifySubmission, type SubmissionV1 } from "../src/product/submission.js";
import { submitBundle } from "../src/product/submit.js";
import { canonicalJson, protocolDigest, sha256Bytes } from "../src/util/canonical.js";
import { writeCurrentRunFixture } from "./helpers/current-run-fixture.js";

test("export creates an authenticated portable Submission without mutating Vela", async () => {
  const home = await mkdtemp(path.join(os.tmpdir(), "canopus-export-home-"));
  const product = path.join(home, "product");
  const artifact = Buffer.from("{\"value\":42}\n");
  const artifactDigest = sha256Bytes(artifact);
  const fixture = await writeCurrentRunFixture({
    root: product,
    artifact,
    velaVersion: "0.930.0-rc.12",
    velaSha256: artifactDigest,
    gitCommit: "e".repeat(40),
    gitTree: "f".repeat(40),
    roots: {
      git_commit: "e".repeat(40),
      git_tree: "f".repeat(40),
      vela_event_log: `sha256:${"a".repeat(64)}`,
      vela_snapshot: `sha256:${"a".repeat(64)}`,
    },
  });
  const output = path.join(home, "submission");
  const result = await exportSubmission({
    runFile: fixture.runFile,
    outputRoot: output,
    now: new Date("2026-07-26T12:00:00Z"),
  });
  const submission = JSON.parse(await readFile(path.join(output, "submission.json"), "utf8")) as SubmissionV1;
  verifySubmission(submission);
  assert.equal(protocolDigest(submission), result.submission_root);
  assert.equal(submission.provenance.source_run, "run_export_fixture");
  assert.equal(submission.producer_checks.length, 0);
  assert.equal(submission.artifacts[0]?.path, `records/artifacts/sha256/${artifactDigest.slice(7)}`);
  assert.equal(
    (await readdir(output, { recursive: true })).some((entry) => String(entry).includes("private-key")),
    false,
  );

  const manifestFile = path.join(output, "manifest.json");
  const manifest = JSON.parse(await readFile(manifestFile, "utf8")) as {
    artifacts: Array<{ source: string }>;
  };
  manifest.artifacts[0]!.source = "../escape";
  await chmod(manifestFile, 0o600);
  await writeFile(manifestFile, canonicalJson(manifest));
  const frontier = path.join(home, "frontier");
  await mkdir(frontier);
  await assert.rejects(
    submitBundle({ bundle: output, frontier }),
    /safe relative POSIX path/u,
  );
});
