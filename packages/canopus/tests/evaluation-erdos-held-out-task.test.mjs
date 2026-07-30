import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  ACCEPTED_COVERAGE_END,
  ARTIFACT_PATH,
  CAPSULE_MANIFEST_ROOT,
  PENDING_PRODUCER_COVERAGE_END,
  RANGE_END,
  RANGE_START,
  SOURCE_CHECKOUT_COMMIT,
  SOURCE_CHECKOUT_TREE,
  SOURCE_PACKET_ROOT,
  SOURCE_REPOSITORY_ROOT,
  TARGET_INDEX_ROOT,
  TASK_ID,
  TASK_PACKET_ROOT,
  VERIFIER_BINARY_ROOT,
  VERIFIER_SOURCE_ROOT,
  buildCapsuleManifest,
  capsuleManifestBytes,
  sha256,
} from "../evaluation/tasks/erdos-1056-10429801-10430000/task.mjs";
import {
  assertCheckoutIdentity,
  assertOutputOutsideFrontier,
  readBoundFile,
  safeRelativePath,
} from "../evaluation/tasks/erdos-1056-10429801-10430000/source.mjs";

const taskRoot = fileURLToPath(
  new URL(
    "../evaluation/tasks/erdos-1056-10429801-10430000/",
    import.meta.url,
  ),
);

test("held-out Erdős packet binds the exact current source and next range", () => {
  const bytes = readFileSync(path.join(taskRoot, "packet.json"));
  const packet = JSON.parse(bytes.toString("utf8"));

  assert.equal(TASK_ID, "erdos:1056:10429801-10430000");
  assert.equal(RANGE_START, 10_429_801);
  assert.equal(RANGE_END, 10_430_000);
  assert.equal(ACCEPTED_COVERAGE_END, 10_429_600);
  assert.equal(PENDING_PRODUCER_COVERAGE_END, 10_429_800);
  assert.equal(
    ARTIFACT_PATH,
    "artifacts/erdos1056-k15-range-10429801-10430000.txt",
  );
  assert.equal(sha256(bytes), TASK_PACKET_ROOT);
  assert.equal(packet.task_id, TASK_ID);
  assert.equal(packet.source.repository.checkout_commit, SOURCE_CHECKOUT_COMMIT);
  assert.equal(packet.source.repository.checkout_tree, SOURCE_CHECKOUT_TREE);
  assert.equal(
    packet.source.repository.repository_root,
    SOURCE_REPOSITORY_ROOT,
  );
  assert.equal(packet.source.target_index.index_root, TARGET_INDEX_ROOT);
  assert.equal(packet.source.target.packet_root, SOURCE_PACKET_ROOT);
  assert.equal(packet.constraints.answer_access, "held_out");
  assert.equal(packet.constraints.precomputed_result, "not_provided");
  assert.equal(packet.constraints.verifier, "not_exposed");
});

test("held-out verifier capsule is exact, bounded, and separate", () => {
  const bytes = readFileSync(path.join(taskRoot, "capsule.json"));
  const manifest = JSON.parse(bytes.toString("utf8"));

  assert.equal(sha256(bytes), CAPSULE_MANIFEST_ROOT);
  assert.deepEqual(bytes, capsuleManifestBytes(buildCapsuleManifest()));
  assert.equal(manifest.source.root, VERIFIER_SOURCE_ROOT);
  assert.equal(manifest.executable.root, VERIFIER_BINARY_ROOT);
  assert.equal(manifest.build.platform, "linux/amd64");
  assert.equal(manifest.execution.network, "deny");
  assert.equal(manifest.execution.root_filesystem, "read_only");
  assert.equal(manifest.execution.authority, "none");
  assert.deepEqual(
    manifest.build.argv.filter((value) => value.startsWith("-DCANOPUS_RANGE_")),
    [
      "-DCANOPUS_RANGE_START=10429801",
      "-DCANOPUS_RANGE_END=10430000",
    ],
  );
});

test("held-out task directory contains no answer or output artifact", () => {
  const allowed = [
    "README.md",
    "build-verifier.mjs",
    "capsule.json",
    "packet.json",
    "prepare.mjs",
    "source.mjs",
    "task.mjs",
    "verify.mjs",
  ];
  assert.deepEqual(readdirSync(taskRoot).sort(), allowed);

  const inspectable = allowed.filter((file) =>
    [".json", ".md", ".mjs"].some((extension) => file.endsWith(extension)));
  const text = inspectable
    .map((file) => readFileSync(path.join(taskRoot, file), "utf8"))
    .join("\n");
  for (const leakedResult of [
    /status=(?:witness|negative)(?:["\n])/u,
    /primes_tested=\d/u,
    /max_multiplicity=\d/u,
    /best_p=\d/u,
    /best_residue=\d/u,
    /cuts=\d/u,
  ]) {
    assert.equal(leakedResult.test(text), false, String(leakedResult));
  }
  assert.equal(
    allowed.some((file) =>
      /(?:answer|artifact|output|preflight|result|run)\.(?:json|txt|log)$/u.test(file)),
    false,
  );
});

test("source roots and checkout metadata fail closed on drift", () => {
  const clean = {
    commit: SOURCE_CHECKOUT_COMMIT,
    tree: SOURCE_CHECKOUT_TREE,
    origin: "https://github.com/vela-science/erdos-frontier.git",
    status: "",
  };
  assert.doesNotThrow(() => assertCheckoutIdentity(clean));
  assert.throws(
    () => assertCheckoutIdentity({ ...clean, commit: "0".repeat(40) }),
    /checkout commit drifted/u,
  );
  assert.throws(
    () => assertCheckoutIdentity({ ...clean, status: " M targets.json" }),
    /checkout is dirty/u,
  );
  assert.notEqual(SOURCE_PACKET_ROOT, TARGET_INDEX_ROOT);
  assert.notEqual(TASK_PACKET_ROOT, CAPSULE_MANIFEST_ROOT);
  assert.notEqual(VERIFIER_SOURCE_ROOT, VERIFIER_BINARY_ROOT);
});

test("source paths reject traversal, symlinks, and Frontier-local output", async () => {
  assert.equal(safeRelativePath("targets/erdos-1056.json"), "targets/erdos-1056.json");
  for (const unsafe of [
    "/tmp/answer",
    "../answer",
    "targets/../answer",
    "targets\\answer",
    "targets//answer",
  ]) {
    assert.throws(() => safeRelativePath(unsafe), /unsafe source-relative path/u);
  }

  const root = mkdtempSync(path.join(os.tmpdir(), "canopus-held-out-path-"));
  const outside = path.join(root, "outside.json");
  const source = path.join(root, "source");
  const link = path.join(source, "target.json");
  writeFileSync(outside, "{}\n");
  mkdirSync(source);
  symlinkSync(outside, link);
  await assert.rejects(
    readBoundFile(source, "target.json", sha256(readFileSync(outside)), 1024),
    /path substitution/u,
  );
  assert.throws(
    () => assertOutputOutsideFrontier(source, path.join(source, "generated")),
    /outside the canonical Frontier/u,
  );
  assert.doesNotThrow(() =>
    assertOutputOutsideFrontier(source, path.join(root, "generated")));
});
