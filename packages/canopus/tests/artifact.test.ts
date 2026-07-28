import assert from "node:assert/strict";
import { chmod, link, mkdir, mkdtemp, readFile, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  freezeArtifact,
  sealArtifactStore,
  verifyFrozenArtifact,
} from "../src/artifact/freeze.js";

async function roots(): Promise<{ root: string; output: string; artifacts: string }> {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-artifact-"));
  const output = path.join(root, "output");
  const artifacts = path.join(root, "artifacts");
  await Promise.all([mkdir(output), mkdir(artifacts)]);
  return { root, output, artifacts };
}

test("artifact freeze copies bounded bytes to their content address", async () => {
  const dirs = await roots();
  await writeFile(path.join(dirs.output, "witness.json"), "{\"value\":1}\n");
  const frozen = await freezeArtifact({
    sourceRoot: dirs.output,
    artifactRoot: dirs.artifacts,
    path: "witness.json",
    kind: "witness",
    maxBytes: 1024,
  });
  assert.equal(frozen.artifact.bytes, 12);
  assert.equal(path.basename(frozen.frozenPath), frozen.artifact.digest.slice(7));
  assert.equal(await readFile(frozen.frozenPath, "utf8"), "{\"value\":1}\n");
  await sealArtifactStore(dirs.artifacts);
  await verifyFrozenArtifact(frozen, 1024);
});

test("artifact freeze rejects oversize, traversal, symlink, and hardlink inputs", async () => {
  const oversize = await roots();
  await writeFile(path.join(oversize.output, "large"), "12345");
  await assert.rejects(
    freezeArtifact({
      sourceRoot: oversize.output,
      artifactRoot: oversize.artifacts,
      path: "large",
      kind: "data",
      maxBytes: 4,
    }),
    /exceeds 4 bytes/u,
  );
  await assert.rejects(
    freezeArtifact({
      sourceRoot: oversize.output,
      artifactRoot: oversize.artifacts,
      path: "../large",
      kind: "data",
      maxBytes: 10,
    }),
    /remain below its root/u,
  );

  const linked = await roots();
  await writeFile(path.join(linked.output, "source"), "same");
  await symlink("source", path.join(linked.output, "symbolic"));
  await assert.rejects(
    freezeArtifact({
      sourceRoot: linked.output,
      artifactRoot: linked.artifacts,
      path: "symbolic",
      kind: "data",
      maxBytes: 10,
    }),
    /symbolic link/u,
  );
  await link(path.join(linked.output, "source"), path.join(linked.output, "hard"));
  await assert.rejects(
    freezeArtifact({
      sourceRoot: linked.output,
      artifactRoot: linked.artifacts,
      path: "hard",
      kind: "data",
      maxBytes: 10,
    }),
    /singly linked/u,
  );
});

test("frozen artifact verification detects post-freeze mutation", async () => {
  const dirs = await roots();
  await writeFile(path.join(dirs.output, "result"), "original");
  const frozen = await freezeArtifact({
    sourceRoot: dirs.output,
    artifactRoot: dirs.artifacts,
    path: "result",
    kind: "data",
    maxBytes: 100,
  });
  await chmod(frozen.frozenPath, 0o600);
  await writeFile(frozen.frozenPath, "tampered");
  await assert.rejects(verifyFrozenArtifact(frozen, 100), /no longer matches/u);
});

