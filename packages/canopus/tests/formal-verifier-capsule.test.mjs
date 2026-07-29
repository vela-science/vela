import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const capsule = fileURLToPath(
  new URL("../capsules/formal-erdos-505-test-dim-one/verifier", import.meta.url),
);

function rejected(bytes) {
  const root = mkdtempSync(path.join(os.tmpdir(), "canopus-formal-capsule-"));
  const artifact = path.join(root, "candidate.lean");
  writeFileSync(artifact, bytes);
  chmodSync(artifact, 0o444);
  return spawnSync(capsule, [artifact], {
    encoding: "utf8",
    timeout: 10_000,
    env: { PATH: process.env.PATH ?? "/usr/bin:/bin" },
  });
}

test("formal capsule fails before Lean on malformed candidate envelopes", () => {
  for (const [candidate, message] of [
    ["", /1\.\.131072 bytes/u],
    ["theorem bad : True := by trivial\n", /begin with a Lean 'by' term/u],
    ["by\n  sorry\n", /forbidden trust-bypassing token/u],
    ["by\n  exact (Classical.choice (show Nonempty True from ⟨True.intro⟩))\naxiom escape : False\n",
      /forbidden trust-bypassing token/u],
  ]) {
    const result = rejected(candidate);
    assert.equal(result.status, 2);
    assert.match(result.stderr, message);
  }
});
