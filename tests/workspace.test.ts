import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { chmod, lstat, mkdtemp, mkdir, readFile, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

import { cleanupWorkspace, prepareWorkspace } from "../src/workspace/prepare.js";

const exec = promisify(execFile);

async function git(cwd: string, ...args: string[]): Promise<string> {
  const result = await exec("git", args, { cwd, encoding: "utf8" });
  return result.stdout.trim();
}

async function repository(withSymlink = false): Promise<{
  parent: string;
  repo: string;
  commit: string;
  tree: string;
}> {
  const parent = await mkdtemp(path.join(os.tmpdir(), "canopus-workspace-test-"));
  const repo = path.join(parent, "source");
  await mkdir(repo);
  await git(repo, "init", "-b", "main");
  await git(repo, "config", "user.name", "Canopus Test");
  await git(repo, "config", "user.email", "canopus@example.invalid");
  await writeFile(path.join(repo, "witness.txt"), "frozen\n", { mode: 0o600 });
  if (withSymlink) await symlink("witness.txt", path.join(repo, "alias.txt"));
  await git(repo, "add", "witness.txt", ...(withSymlink ? ["alias.txt"] : []));
  await git(repo, "commit", "--no-gpg-sign", "-m", "fixture");
  return {
    parent,
    repo,
    commit: await git(repo, "rev-parse", "HEAD^{commit}"),
    tree: await git(repo, "rev-parse", "HEAD^{tree}"),
  };
}

test("workspace clones only the exact committed root and seals it read-only", async () => {
  const fixture = await repository();
  await writeFile(path.join(fixture.repo, "uncommitted.txt"), "must not leak\n");
  const paths = await prepareWorkspace({
    sourceRepo: fixture.repo,
    runRoot: path.join(fixture.parent, "run"),
    gitCommit: fixture.commit,
    gitTree: fixture.tree,
  });
  assert.equal(await readFile(path.join(paths.input, "witness.txt"), "utf8"), "frozen\n");
  assert.equal(await git(paths.frontier, "branch", "--show-current"), "canopus-frontier");
  assert.equal(await git(paths.input, "branch", "--show-current"), "");
  await assert.rejects(readFile(path.join(paths.input, "uncommitted.txt")), /ENOENT/u);
  const stat = await lstat(path.join(paths.input, "witness.txt"));
  assert.equal(stat.mode & 0o222, 0);
  await cleanupWorkspace(paths);
  await assert.rejects(lstat(paths.root), /ENOENT/u);
});

test("workspace rejects source/run overlap", async () => {
  const fixture = await repository();
  await assert.rejects(
    prepareWorkspace({
      sourceRepo: fixture.repo,
      runRoot: path.join(fixture.repo, ".runs", "one"),
      gitCommit: fixture.commit,
      gitTree: fixture.tree,
    }),
    /must not overlap/u,
  );
});

test("workspace rejects a tree containing symlinks", async () => {
  const fixture = await repository(true);
  await assert.rejects(
    prepareWorkspace({
      sourceRepo: fixture.repo,
      runRoot: path.join(fixture.parent, "run"),
      gitCommit: fixture.commit,
      gitTree: fixture.tree,
    }),
    /symbolic link/u,
  );
});

test("workspace rejects a mismatched tree", async () => {
  const fixture = await repository();
  await assert.rejects(
    prepareWorkspace({
      sourceRepo: fixture.repo,
      runRoot: path.join(fixture.parent, "run"),
      gitCommit: fixture.commit,
      gitTree: "d".repeat(40),
    }),
    /checkout root mismatch/u,
  );
});
