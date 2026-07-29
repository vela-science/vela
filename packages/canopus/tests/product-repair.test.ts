import assert from "node:assert/strict";
import { mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { loadRepairInput } from "../src/product/run.js";
import { sha256Bytes } from "../src/util/canonical.js";

test("repair input requires and loads the exact parent candidate bytes", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-repair-"));
  const source = path.join(root, "candidate.lean");
  const bytes = Buffer.from("by\n  exact trivial\n");
  await writeFile(source, bytes);
  const mission = {
    parent_candidate: sha256Bytes(bytes),
    repair_reason: "Repair the exact retained candidate.",
    allowed_paths: ["artifacts/proof.lean"],
    budgets: { max_artifact_bytes: 4096 },
  };

  assert.deepEqual(await loadRepairInput({ mission, source }), {
    path: "artifacts/proof.lean",
    digest: sha256Bytes(bytes),
    bytes,
  });
  await assert.rejects(
    loadRepairInput({ mission }),
    /repair mission requires --repair-from/u,
  );

  const wrong = path.join(root, "wrong.lean");
  await writeFile(wrong, "by\n  contradiction\n");
  await assert.rejects(
    loadRepairInput({ mission, source: wrong }),
    /repair input root mismatch/u,
  );
});

test("non-repair missions reject repair input", async () => {
  await assert.rejects(
    loadRepairInput({
      mission: {
        allowed_paths: ["artifact.json"],
        budgets: { max_artifact_bytes: 4096 },
      },
      source: "/tmp/unused",
    }),
    /valid only for a repair mission/u,
  );
});
