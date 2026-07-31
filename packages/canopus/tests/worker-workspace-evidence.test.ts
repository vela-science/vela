import assert from "node:assert/strict";
import { access, mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  discardWorkerWorkspaceEvidence,
  promoteWorkerWorkspaceEvidence,
  readWorkerWorkspaceEvidence,
  snapshotWorkerWorkspace,
  stageWorkerWorkspaceEvidence,
} from "../src/engines/workspace-evidence.js";

async function fixture(): Promise<{ root: string; workspace: string; runRoot: string }> {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-worker-evidence-"));
  const workspace = path.join(root, "workspace");
  const runRoot = path.join(root, "run");
  await Promise.all([
    mkdir(workspace, { mode: 0o700 }),
    mkdir(runRoot, { mode: 0o700 }),
  ]);
  return { root, workspace, runRoot };
}

test("worker failure evidence retains only bounded new source and build files", async () => {
  const { workspace, runRoot } = await fixture();
  await writeFile(path.join(workspace, "packet.json"), "{\"task\":true}\n");
  const baseline = await snapshotWorkerWorkspace(workspace);

  await mkdir(path.join(workspace, "src"));
  await writeFile(path.join(workspace, "src", "search.cpp"), "int main() { return 0; }\n");
  await writeFile(path.join(workspace, "result.txt"), "candidate\n");
  await writeFile(path.join(workspace, "notes.md"), "private scratch notes\n");
  await writeFile(path.join(workspace, "binary.cpp"), Buffer.from([0xff, 0xfe, 0xfd]));
  await writeFile(path.join(workspace, "secret.py"), "token = 'sk-abcdefghijklmnopqrstuvwxyz012345'\n");
  await writeFile(path.join(workspace, "oversized.rs"), "x".repeat(64 * 1024 + 1));

  const staged = await stageWorkerWorkspaceEvidence({
    workspace,
    runRoot,
    baseline,
    excludedPaths: ["result.txt"],
    secrets: [],
  });
  assert.deepEqual(staged.retained, [{
    path: path.join("src", "search.cpp"),
    digest: staged.retained[0]?.digest,
    bytes: 25,
  }]);
  assert.equal(staged.omitted.baseline_or_contract, 2);
  assert.equal(staged.omitted.unsupported_type, 1);
  assert.equal(staged.omitted.non_utf8, 1);
  assert.equal(staged.omitted.sensitive, 1);
  assert.equal(staged.omitted.oversized, 1);

  const promoted = await promoteWorkerWorkspaceEvidence(runRoot);
  assert.ok(promoted);
  assert.equal(promoted.retained_files, 1);
  assert.equal(promoted.retained_bytes, 25);
  assert.match(promoted.root, /^sha256:[0-9a-f]{64}$/u);
  assert.equal(
    await readFile(path.join(runRoot, "failure-evidence", "files", "src", "search.cpp"), "utf8"),
    "int main() { return 0; }\n",
  );
  assert.deepEqual(await readWorkerWorkspaceEvidence(runRoot), staged);
  await assert.rejects(
    access(path.join(runRoot, "failure-evidence", "files", "secret.py")),
    /ENOENT/u,
  );
  await assert.rejects(
    access(path.join(runRoot, "failure-evidence", "files", "notes.md")),
    /ENOENT/u,
  );
  await assert.rejects(
    access(path.join(runRoot, "failure-evidence", "files", "packet.json")),
    /ENOENT/u,
  );
  await assert.rejects(
    access(path.join(runRoot, "failure-evidence", "files", "result.txt")),
    /ENOENT/u,
  );
});

test("successful runs can discard staged worker evidence", async () => {
  const { workspace, runRoot } = await fixture();
  const baseline = await snapshotWorkerWorkspace(workspace);
  await writeFile(path.join(workspace, "debug.py"), "print('bounded')\n");
  await stageWorkerWorkspaceEvidence({
    workspace,
    runRoot,
    baseline,
    excludedPaths: [],
    secrets: [],
  });
  await discardWorkerWorkspaceEvidence(runRoot);
  await assert.rejects(
    access(path.join(runRoot, ".worker-evidence-staging")),
    /ENOENT/u,
  );
});
