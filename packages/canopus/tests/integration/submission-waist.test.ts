import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { exportSubmission } from "../../src/product/submission.js";
import { submitBundle } from "../../src/product/submit.js";
import { sha256Bytes } from "../../src/util/canonical.js";
import { writeCurrentRunFixture } from "../helpers/current-run-fixture.js";

function command(
  executable: string,
  argv: string[],
  cwd: string,
  env: NodeJS.ProcessEnv,
): string {
  return execFileSync(executable, argv, {
    cwd,
    env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

test("Canopus Submission crosses the released Vela writer with zero accepted delta", {
  skip: process.env.CANOPUS_VELA_BIN === undefined
    ? "set CANOPUS_VELA_BIN to run the cross-repository writer gate"
    : false,
  concurrency: false,
  timeout: 30_000,
}, async (t) => {
  const vela = path.resolve(process.env.CANOPUS_VELA_BIN!);
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-submission-waist-"));
  const home = path.join(root, "home");
  const frontier = path.join(root, "frontier");
  const remote = path.join(root, "remote.git");
  const replay = path.join(root, "replay");
  await Promise.all([mkdir(home), mkdir(frontier)]);
  const fixtureNonce = path.basename(root);
  command("ssh-keygen", [
    "-q", "-t", "ed25519", "-N", "", "-C", "canopus disposable authority",
    "-f", path.join(root, "authority"),
  ], root, process.env);
  const agent = command("ssh-agent", ["-s"], root, process.env);
  const socket = agent.match(/SSH_AUTH_SOCK=([^;]+);/)?.[1];
  const pid = agent.match(/SSH_AGENT_PID=([0-9]+);/)?.[1];
  assert.ok(socket && pid, "ssh-agent did not expose a bounded fixture session");
  const env = {
    ...process.env,
    HOME: home,
    NO_COLOR: "1",
    VELA_ADVICE: "0",
    SSH_AUTH_SOCK: socket,
    SSH_AGENT_PID: pid,
  };
  t.after(() => {
    try {
      command("ssh-agent", ["-k"], root, env);
    } catch {
      // The test session may already have stopped the disposable agent.
    }
  });
  command("ssh-add", [path.join(root, "authority")], root, env);
  command("git", ["config", "--global", "user.name", "Canopus Interop Fixture"], root, env);
  command("git", ["config", "--global", "user.email", "fixture@vela.invalid"], root, env);
  command(vela, [
    "init", frontier,
    "--name", `Canopus Interop ${fixtureNonce}`,
    "--scope", `Exercise one disposable Canopus Submission ${fixtureNonce}.`,
    "--json",
  ], root, env);
  command("git", ["init", "-q", "--bare", remote], root, env);
  command("git", ["init", "-q"], frontier, env);
  command("git", ["branch", "-M", "main"], frontier, env);
  command("git", ["add", "--all"], frontier, env);
  command("git", ["commit", "-q", "-m", "Initialize disposable Frontier"], frontier, env);
  command("git", ["remote", "add", "origin", remote], frontier, env);
  command("git", ["push", "-q", "-u", "origin", "main"], frontier, env);
  command("git", ["--git-dir", remote, "symbolic-ref", "HEAD", "refs/heads/main"], root, env);
  const initialized = JSON.parse(command(vela, [
    "authority", "init", frontier,
    "--reason", "Establish ephemeral authority for the Canopus writer gate.",
    "--json",
  ], root, env)) as { frontier_id: string; authority_record_root: string };
  const trustAnchor = path.join(
    os.homedir(),
    ".vela",
    "trust",
    "authorities",
    `${initialized.frontier_id}.json`,
  );
  t.after(async () => await rm(trustAnchor, { force: true }));
  command(vela, [
    "authority", "trust", "pin", frontier,
    "--record-root", initialized.authority_record_root,
    "--json",
  ], root, env);
  command("git", ["push", "-q", "origin", "main"], frontier, env);
  await mkdir(path.join(frontier, "domain"), { recursive: true });
  await writeFile(path.join(frontier, "domain", "source.json"), "{\"open\":[1056]}");
  command("git", ["add", "domain/source.json"], frontier, env);
  command("git", ["commit", "-q", "-m", "Add target source"], frontier, env);
  const targetSourceCommit = command("git", ["rev-parse", "HEAD^{commit}"], frontier, env);
  await mkdir(path.join(frontier, "site", "problems"), { recursive: true });
  await writeFile(
    path.join(frontier, "site", "problems", "1056.json"),
    "{\"problem\":1056,\"schema\":\"erdos-frontier.problem-work.v1\"}",
  );
  const candidateDirectory = path.join(frontier, ".vela", "tmp");
  const candidate = path.join(candidateDirectory, "target-index-candidate.json");
  await mkdir(candidateDirectory, { recursive: true });
  await writeFile(candidate, JSON.stringify({
    schema: "vela.target-index-candidate.v1",
    frontier_id: initialized.frontier_id,
    source: {
      git_commit: targetSourceCommit,
      input_paths: ["domain/source.json"],
    },
    targets: [{
      id: "erdos:1056",
      title: "Erdős 1056",
      why: "Exercise registration with an exact current Target Index.",
      state: "open",
      rank: 1,
      objective: "Produce one bounded artifact.",
      labels: ["erdos", "open"],
      packet: {
        schema: "erdos-frontier.problem-work.v1",
        path: "site/problems/1056.json",
      },
    }],
  }, null, 2));
  command(vela, [
    "target-index", "seal", frontier,
    "--candidate", candidate,
    "--apply",
    "--json",
  ], root, env);
  await rm(candidate);
  command("git", ["add", "targets.json", "site/problems/1056.json"], frontier, env);
  command("git", ["commit", "-q", "-m", "Seal target index"], frontier, env);
  command("git", ["push", "-q", "origin", "main"], frontier, env);

  const before = JSON.parse(command(vela, ["status", frontier, "--json"], root, env)) as {
    roots: { repository: string };
  };
  const gitCommit = command("git", ["rev-parse", "HEAD^{commit}"], frontier, env);
  const gitTree = command("git", ["rev-parse", "HEAD^{tree}"], frontier, env);
  const version = command(vela, ["--version"], root, env).split(/\s+/u).at(-1)!;
  const fixture = await writeCurrentRunFixture({
    root: path.join(root, "product"),
    artifact: Buffer.from("{\"value\":42}\n"),
    velaVersion: version,
    velaSha256: sha256Bytes(await readFile(vela)),
    gitCommit,
    gitTree,
    roots: {
      git_commit: gitCommit,
      git_tree: gitTree,
      vela_repository: before.roots.repository,
    },
  });
  const attempt = JSON.parse(command(vela, [
    "start", "erdos:1056",
    "--frontier", frontier,
    "--artifact-class", "witness",
    "--max-submissions", "1",
    "--max-verifications", "1",
    "--as", fixture.mission.actor,
    "--json",
  ], root, env)) as { attempt: { id: string } };
  const bundle = path.join(root, "bundle");
  await exportSubmission({
    runFile: fixture.runFile,
    outputRoot: bundle,
    attempt: attempt.attempt.id,
    now: new Date("2026-07-27T12:00:00Z"),
  });
  command("git", ["commit", "--allow-empty", "-q", "-m", "Advance current Frontier"], frontier, env);

  const saved = {
    HOME: process.env.HOME,
    SSH_AUTH_SOCK: process.env.SSH_AUTH_SOCK,
    SSH_AGENT_PID: process.env.SSH_AGENT_PID,
  };
  Object.assign(process.env, {
    HOME: env.HOME,
    SSH_AUTH_SOCK: env.SSH_AUTH_SOCK,
    SSH_AGENT_PID: env.SSH_AGENT_PID,
  });
  let result;
  try {
    result = await submitBundle({
      bundle,
      frontier,
      velaBinary: vela,
      attempt: attempt.attempt.id,
    });
  } finally {
    for (const [key, value] of Object.entries(saved)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
  }
  assert.equal(result.accepted_event_delta, 0);
  assert.equal(result.route, "pending_review");
  assert.notEqual(result.source_commit_before, result.source_commit_after);
  assert.equal(result.registration_binary_version, `vela ${version}`);
  assert.equal(result.registration_binary_sha256, sha256Bytes(await readFile(vela)));
  assert.equal(command("git", ["status", "--porcelain=v1", "--untracked-files=all"], frontier, env), "");

  const after = JSON.parse(command(vela, ["status", frontier, "--json"], root, env)) as {
    roots: { repository: string };
    counts: { pending_review: number };
  };
  assert.notEqual(after.roots.repository, before.roots.repository);
  assert.equal(after.counts.pending_review, 1);
  command("git", ["clone", "-q", "--no-hardlinks", frontier, replay], root, env);
  const replayed = JSON.parse(command(vela, ["status", replay, "--json"], root, env)) as {
    roots: { repository: string };
  };
  assert.equal(replayed.roots.repository, after.roots.repository);
});
