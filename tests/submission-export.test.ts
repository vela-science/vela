import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, readFile, readdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { exportSubmission, verifySubmission, type SubmissionV1 } from "../src/product/submission.js";
import { submitBundle } from "../src/product/submit.js";
import {
  canonicalJson,
  contentDigest,
  protocolDigest,
  sha256Bytes,
} from "../src/util/canonical.js";
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
      vela_repository: `sha256:${"a".repeat(64)}`,
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

test("review-only export preserves an absent optional execution binding", async () => {
  const home = await mkdtemp(path.join(os.tmpdir(), "canopus-export-unbound-"));
  const artifact = Buffer.from("{\"bounded\":\"negative\"}\n");
  const fixture = await writeCurrentRunFixture({
    root: path.join(home, "product"),
    artifact,
    velaVersion: "0.930.0-rc.13",
    velaSha256: sha256Bytes(artifact),
    gitCommit: "e".repeat(40),
    gitTree: "f".repeat(40),
    roots: {
      git_commit: "e".repeat(40),
      git_tree: "f".repeat(40),
      vela_repository: `sha256:${"a".repeat(64)}`,
    },
    includeExecutionBinding: false,
  });
  const output = path.join(home, "submission");
  await exportSubmission({
    runFile: fixture.runFile,
    outputRoot: output,
    now: new Date("2026-07-27T12:00:00Z"),
  });
  const submission = JSON.parse(
    await readFile(path.join(output, "submission.json"), "utf8"),
  ) as SubmissionV1;
  verifySubmission(submission);
  assert.equal(submission.execution_binding, undefined);
  assert.match(
    submission.verification_requirements[0] ?? "",
    new RegExp(fixture.mission.verifier.capsule_sha256, "u"),
  );
});

test("export fails closed on stale verifier wording and preserves an explicit bounded correction", async () => {
  const home = await mkdtemp(path.join(os.tmpdir(), "canopus-export-correction-"));
  const artifact = Buffer.from("status=negative\nrange=1..10\n");
  const fixture = await writeCurrentRunFixture({
    root: path.join(home, "product"),
    artifact,
    velaVersion: "0.940.6",
    velaSha256: sha256Bytes(artifact),
    gitCommit: "e".repeat(40),
    gitTree: "f".repeat(40),
    roots: {
      git_commit: "e".repeat(40),
      git_tree: "f".repeat(40),
      vela_repository: `sha256:${"a".repeat(64)}`,
    },
    includeExecutionBinding: false,
    candidateClaim:
      "The exact bounded search found no witness; frozen verification remains pending outside this workspace.",
    candidateCaveats: [
      "Verifier execution was not performed here; Canopus will run it after exit.",
      "Canopus produced this record; it is not a Verification Record or Decision.",
    ],
  });
  const originalRun = await readFile(fixture.runFile);
  await assert.rejects(
    exportSubmission({
      runFile: fixture.runFile,
      outputRoot: path.join(home, "blocked"),
    }),
    /requires --claim and --scope-limit/u,
  );

  const output = path.join(home, "corrected");
  await exportSubmission({
    runFile: fixture.runFile,
    outputRoot: output,
    correctedClaim:
      "The exact bounded search over 1..10 found no witness; the retained frozen-verifier replay passed.",
    scopeLimit: "This bounded negative result is not a universal nonexistence result.",
    now: new Date("2026-07-28T12:00:00Z"),
  });
  const submission = JSON.parse(
    await readFile(path.join(output, "submission.json"), "utf8"),
  ) as SubmissionV1;
  verifySubmission(submission);
  assert.equal(
    submission.claim.assertion,
    "The exact bounded search over 1..10 found no witness; the retained frozen-verifier replay passed.",
  );
  assert.deepEqual(submission.caveats, [
    "The worker handed off without verifier authority; Canopus subsequently recorded the separate verifier outcome.",
    "Canopus produced this record; it is not a Verification Record or Decision.",
    "This bounded negative result is not a universal nonexistence result.",
    "The Submission wording corrects a stale post-run Claim after verifier passage; the immutable Run remains unchanged.",
  ]);
  assert.deepEqual(await readFile(fixture.runFile), originalRun);
});

test("export rejects arbitrary Claim replacement and control characters", async () => {
  const home = await mkdtemp(path.join(os.tmpdir(), "canopus-export-guard-"));
  const artifact = Buffer.from("{\"value\":42}\n");
  const fixture = await writeCurrentRunFixture({
    root: path.join(home, "product"),
    artifact,
    velaVersion: "0.940.6",
    velaSha256: sha256Bytes(artifact),
    gitCommit: "e".repeat(40),
    gitTree: "f".repeat(40),
    roots: {
      git_commit: "e".repeat(40),
      git_tree: "f".repeat(40),
      vela_repository: `sha256:${"a".repeat(64)}`,
    },
  });
  await assert.rejects(
    exportSubmission({
      runFile: fixture.runFile,
      outputRoot: path.join(home, "arbitrary"),
      correctedClaim: "A different Claim.",
      scopeLimit: "A limit.",
    }),
    /allowed only for a retained Run Claim/u,
  );

  const controlFixture = await writeCurrentRunFixture({
    root: path.join(home, "control-product"),
    artifact,
    velaVersion: "0.940.6",
    velaSha256: sha256Bytes(artifact),
    gitCommit: "e".repeat(40),
    gitTree: "f".repeat(40),
    roots: {
      git_commit: "e".repeat(40),
      git_tree: "f".repeat(40),
      vela_repository: `sha256:${"a".repeat(64)}`,
    },
    candidateClaim: "A bounded result\u0015with a control byte.",
  });
  await assert.rejects(
    exportSubmission({
      runFile: controlFixture.runFile,
      outputRoot: path.join(home, "control"),
    }),
    /requires --claim and --scope-limit/u,
  );
});

test("export reads an immutable predecessor-root Run without rewriting it", async () => {
  const home = await mkdtemp(path.join(os.tmpdir(), "canopus-export-predecessor-"));
  const fixture = await writeCurrentRunFixture({
    root: path.join(home, "fixture"),
    artifact: Buffer.from("{\"value\":42}\n"),
    velaVersion: "0.930.0-rc.13",
    velaSha256: `sha256:${"9".repeat(64)}`,
    gitCommit: "b".repeat(40),
    gitTree: "c".repeat(40),
    roots: {
      git_commit: "b".repeat(40),
      git_tree: "c".repeat(40),
      vela_repository: `sha256:${"a".repeat(64)}`,
    },
    candidateClaim:
      "The bounded result completed; independent verifier replay remains pending.",
  });
  const missionFile = path.join(
    path.dirname(path.dirname(fixture.runFile)),
    "mission",
    "mission.json",
  );
  const mission = JSON.parse(await readFile(missionFile, "utf8")) as Record<string, unknown>;
  const predecessorRoots = {
    git_commit: "b".repeat(40),
    git_tree: "c".repeat(40),
    vela_event_log: `sha256:${"d".repeat(64)}`,
    vela_snapshot: `sha256:${"e".repeat(64)}`,
  };
  mission.roots = predecessorRoots;
  mission.strict_baseline = {
    status: "fail",
    blocker_count: 1,
    blockers_root: `sha256:${"f".repeat(64)}`,
    rule_counts: [{ count: 1, rule: "legacy_fixture" }],
  };
  await writeFile(missionFile, canonicalJson(mission));

  const run = JSON.parse(await readFile(fixture.runFile, "utf8")) as Record<string, unknown>;
  const runMission = run.mission as Record<string, unknown>;
  const reproduction = run.reproduction as Record<string, unknown>;
  runMission.digest = contentDigest(mission);
  runMission.starting_roots = predecessorRoots;
  reproduction.roots = predecessorRoots;
  await writeFile(fixture.runFile, canonicalJson(run));
  const exactRunBytes = await readFile(fixture.runFile);

  const result = await exportSubmission({
    runFile: fixture.runFile,
    outputRoot: path.join(home, "submission"),
    correctedClaim: "The exact bounded predecessor-era fixture returned 42.",
    scopeLimit: "This establishes only the exact retained fixture.",
    now: new Date("2026-07-28T12:00:00.000Z"),
  });
  const manifest = JSON.parse(await readFile(result.manifest, "utf8")) as {
    run_root: string;
    source: Record<string, string>;
  };
  assert.equal(manifest.run_root, sha256Bytes(exactRunBytes));
  assert.deepEqual(manifest.source, {
    git_commit: "b".repeat(40),
    git_tree: "c".repeat(40),
    vela_version: "0.930.0-rc.13",
    vela_sha256: `sha256:${"9".repeat(64)}`,
  });
  assert.deepEqual(await readFile(fixture.runFile), exactRunBytes);
});
