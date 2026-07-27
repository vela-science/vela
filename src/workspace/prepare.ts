import {
  chmod,
  lstat,
  mkdir,
  readdir,
  realpath,
  rm,
} from "node:fs/promises";
import path from "node:path";

import { GIT_OBJECT_RE } from "../contracts/validation.js";
import { isolatedEnvironment, runCommand, type CommandRunner } from "../util/command.js";

export class WorkspaceError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = "WorkspaceError";
  }
}

export interface WorkspacePaths {
  root: string;
  input: string;
  frontier: string;
  work: string;
  output: string;
  artifacts: string;
  /** Empty, isolated HOME for the untrusted synthesis worker. */
  home: string;
  /** Control-plane HOME where Vela may mint an agent-only session key. */
  velaHome: string;
  /** Empty HOME visible to the sandboxed verifier. */
  verifierHome: string;
}

export interface PrepareWorkspaceOptions {
  sourceRepo: string;
  runRoot: string;
  gitCommit: string;
  gitTree: string;
  timeoutMs?: number;
  maxOutputBytes?: number;
  maxEntries?: number;
  runner?: CommandRunner;
}

function isBelow(child: string, parent: string): boolean {
  const relative = path.relative(parent, child);
  return relative !== "" && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

async function assertFreshOutsideRoot(sourceRepo: string, runRoot: string): Promise<string> {
  const source = await realpath(sourceRepo);
  const target = path.resolve(runRoot);
  const missing: string[] = [];
  let existing = target;
  while (true) {
    try {
      existing = await realpath(existing);
      break;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
      const parent = path.dirname(existing);
      if (parent === existing) throw error;
      missing.unshift(path.basename(existing));
      existing = parent;
    }
  }
  const stableTarget = path.join(existing, ...missing);
  if (stableTarget === source || isBelow(stableTarget, source) || isBelow(source, stableTarget)) {
    throw new WorkspaceError("run root and source repository must not overlap");
  }
  try {
    await lstat(stableTarget);
    throw new WorkspaceError("run root already exists");
  } catch (error) {
    if (error instanceof WorkspaceError) throw error;
    const code = (error as NodeJS.ErrnoException).code;
    if (code !== "ENOENT") throw error;
  }
  return stableTarget;
}

async function sealTree(root: string, maxEntries: number): Promise<void> {
  let entries = 0;
  const visit = async (current: string): Promise<void> => {
    const stat = await lstat(current);
    entries += 1;
    if (entries > maxEntries) {
      throw new WorkspaceError(`checkout exceeds ${maxEntries} filesystem entries`);
    }
    if (stat.isSymbolicLink()) {
      throw new WorkspaceError(`checkout contains a symbolic link: ${path.relative(root, current)}`);
    }
    if (stat.isFile()) {
      if (stat.nlink !== 1) {
        throw new WorkspaceError(`checkout contains a multiply linked file: ${path.relative(root, current)}`);
      }
      await chmod(current, stat.mode & 0o111 ? 0o555 : 0o444);
      return;
    }
    if (!stat.isDirectory()) {
      throw new WorkspaceError(`checkout contains a non-regular entry: ${path.relative(root, current)}`);
    }
    const children = await readdir(current);
    for (const child of children) {
      await visit(path.join(current, child));
    }
    await chmod(current, 0o555);
  };
  await visit(root);
}

async function makeWritableTree(root: string): Promise<void> {
  const stat = await lstat(root);
  if (stat.isDirectory()) {
    await chmod(root, 0o700);
    for (const child of await readdir(root)) {
      await makeWritableTree(path.join(root, child));
    }
  } else if (!stat.isSymbolicLink()) {
    await chmod(root, 0o600);
  }
}

async function gitText(
  runner: CommandRunner,
  argv: readonly string[],
  cwd: string,
  home: string,
  timeoutMs: number,
  maxOutputBytes: number,
): Promise<string> {
  const result = await runner({
    argv,
    cwd,
    env: isolatedEnvironment(home),
    timeoutMs,
    maxOutputBytes,
  });
  if (result.exitCode !== 0) {
    throw new WorkspaceError(`${argv.join(" ")} exited ${result.exitCode}: ${result.stderr.toString("utf8")}`);
  }
  return result.stdout.toString("utf8").trim();
}

export async function prepareWorkspace(options: PrepareWorkspaceOptions): Promise<WorkspacePaths> {
  if (!GIT_OBJECT_RE.test(options.gitCommit) || !GIT_OBJECT_RE.test(options.gitTree)) {
    throw new WorkspaceError("workspace requires full Git commit and tree object IDs");
  }
  const root = await assertFreshOutsideRoot(options.sourceRepo, options.runRoot);
  const input = path.join(root, "input");
  const frontier = path.join(root, "frontier");
  const work = path.join(root, "work");
  const output = path.join(root, "output");
  const artifacts = path.join(root, "artifacts");
  const home = path.join(root, "home");
  const velaHome = path.join(root, "vela-home");
  const verifierHome = path.join(root, "verifier-home");
  const runner = options.runner ?? runCommand;
  const timeoutMs = options.timeoutMs ?? 120_000;
  const maxOutputBytes = options.maxOutputBytes ?? 8 * 1024 * 1024;

  await mkdir(root, { mode: 0o700 });
  await Promise.all([
    mkdir(work, { mode: 0o700 }),
    mkdir(output, { mode: 0o700 }),
    mkdir(artifacts, { mode: 0o700 }),
    mkdir(home, { mode: 0o700 }),
    mkdir(velaHome, { mode: 0o700 }),
    mkdir(verifierHome, { mode: 0o700 }),
  ]);

  try {
    await gitText(
      runner,
      ["git", "clone", "--no-hardlinks", "--no-checkout", "--", await realpath(options.sourceRepo), input],
      root,
      home,
      timeoutMs,
      maxOutputBytes,
    );
    await gitText(
      runner,
      ["git", "clone", "--no-hardlinks", "--no-checkout", "--", await realpath(options.sourceRepo), frontier],
      root,
      home,
      timeoutMs,
      maxOutputBytes,
    );
    await gitText(
      runner,
      ["git", "checkout", "--detach", options.gitCommit],
      input,
      home,
      timeoutMs,
      maxOutputBytes,
    );
    // Vela publishes exact scientific deltas to the current ref. Keep the
    // immutable input detached, but give the isolated control clone its own
    // disposable local branch so inspection never depends on caller state.
    await gitText(
      runner,
      ["git", "checkout", "-B", "canopus-frontier", options.gitCommit],
      frontier,
      home,
      timeoutMs,
      maxOutputBytes,
    );
    const [commit, tree, status, frontierCommit, frontierTree, frontierStatus] = await Promise.all([
      gitText(runner, ["git", "rev-parse", "--verify", "HEAD^{commit}"], input, home, timeoutMs, maxOutputBytes),
      gitText(runner, ["git", "rev-parse", "--verify", "HEAD^{tree}"], input, home, timeoutMs, maxOutputBytes),
      gitText(runner, ["git", "status", "--porcelain=v1", "--untracked-files=all"], input, home, timeoutMs, maxOutputBytes),
      gitText(runner, ["git", "rev-parse", "--verify", "HEAD^{commit}"], frontier, home, timeoutMs, maxOutputBytes),
      gitText(runner, ["git", "rev-parse", "--verify", "HEAD^{tree}"], frontier, home, timeoutMs, maxOutputBytes),
      gitText(runner, ["git", "status", "--porcelain=v1", "--untracked-files=all"], frontier, home, timeoutMs, maxOutputBytes),
    ]);
    if (
      commit !== options.gitCommit ||
      tree !== options.gitTree ||
      frontierCommit !== options.gitCommit ||
      frontierTree !== options.gitTree
    ) {
      throw new WorkspaceError(
        `checkout root mismatch: expected ${options.gitCommit}/${options.gitTree}, observed ${commit}/${tree} and ${frontierCommit}/${frontierTree}`,
      );
    }
    if (status !== "" || frontierStatus !== "") {
      throw new WorkspaceError("exact checkout is unexpectedly dirty");
    }
    await sealTree(input, options.maxEntries ?? 200_000);
    return { root, input, frontier, work, output, artifacts, home, velaHome, verifierHome };
  } catch (error) {
    await cleanupWorkspace({
      root,
      input,
      frontier,
      work,
      output,
      artifacts,
      home,
      velaHome,
      verifierHome,
    });
    throw error;
  }
}

export async function cleanupWorkspace(paths: WorkspacePaths): Promise<void> {
  try {
    await makeWritableTree(paths.root);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
  await rm(paths.root, { recursive: true, force: true });
}
