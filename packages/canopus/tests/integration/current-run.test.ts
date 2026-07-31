import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { chmod, copyFile, mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

import type { Mission, MissionRoots } from "../../src/contracts/mission.js";
import { FakeEngine } from "../../src/engines/fake.js";
import {
  snapshotWorkerWorkspace,
  stageWorkerWorkspaceEvidence,
} from "../../src/engines/workspace-evidence.js";
import { projectCurrentRun } from "../../src/projection/current-run.js";
import { runCanopus, validateTargetOffer, type VelaPort } from "../../src/run.js";
import type { CommandRunner } from "../../src/util/command.js";
import { sha256Bytes } from "../../src/util/canonical.js";
import type { VelaCommandResponse, VelaInspection } from "../../src/vela/types.js";

const exec = promisify(execFile);
const scientificRoot = `sha256:${"a".repeat(64)}`;

async function git(cwd: string, ...args: string[]): Promise<string> {
  const result = await exec("git", args, { cwd, encoding: "utf8" });
  return result.stdout.trim();
}

async function sourceRepository(): Promise<{
  parent: string;
  repo: string;
  roots: MissionRoots;
  verifierDigest: string;
}> {
  const parent = await mkdtemp(path.join(os.tmpdir(), "canopus-current-run-"));
  const repo = path.join(parent, "source");
  await mkdir(path.join(repo, "frontier"), { recursive: true });
  await git(repo, "init", "-b", "main");
  await git(repo, "config", "user.name", "Canopus Test");
  await git(repo, "config", "user.email", "canopus@example.invalid");
  await writeFile(path.join(repo, "frontier/base.txt"), "accepted base\n");
  const verifier = path.join(repo, "frontier", "verifier");
  await copyFile("/usr/bin/true", verifier);
  await chmod(verifier, 0o555);
  const verifierDigest = sha256Bytes(await readFile(verifier));
  await git(repo, "add", "frontier/base.txt", "frontier/verifier");
  await git(repo, "commit", "--no-gpg-sign", "-m", "accepted base");
  return {
    parent,
    repo,
    verifierDigest,
    roots: {
      git_commit: await git(repo, "rev-parse", "HEAD^{commit}"),
      git_tree: await git(repo, "rev-parse", "HEAD^{tree}"),
      vela_repository: scientificRoot,
    },
  };
}

class FakeVela implements VelaPort {
  public nextCalls = 0;

  public async assertRoots(repoRoot: string, _frontier: string, expected: MissionRoots): Promise<VelaInspection> {
    assert.equal(await git(repoRoot, "rev-parse", "HEAD^{commit}"), expected.git_commit);
    assert.equal(await git(repoRoot, "rev-parse", "HEAD^{tree}"), expected.git_tree);
    return {
      version: "0.930.0",
      roots: expected,
      status: { ok: true },
      repository: { ok: true },
    };
  }

  public async next(mission: Mission): Promise<VelaCommandResponse> {
    this.nextCalls += 1;
    return {
      ok: true,
      value: {
        ok: true,
        command: "next",
        targets: [{ target_id: mission.target, rank: 0 }],
      },
    };
  }
}

function mission(source: Awaited<ReturnType<typeof sourceRepository>>): Mission {
  return {
    schema: "canopus.mission.v0",
    id: "mission_current_run_fixture",
    target: "finite:42",
    vela_version: "0.930.0",
    vela_sha256: scientificRoot,
    frontier: "frontier",
    actor: "agent:canopus-test",
    role: "producer",
    claim_type: "computational",
    replayability: "exact",
    objective: "Produce the bounded value 42.",
    completion_condition: "The frozen result contains value 42.",
    roots: source.roots,
    allowed_paths: ["result.json"],
    budgets: {
      max_research_wall_time_ms: 30_000,
      max_research_processes: 4,
      max_research_output_bytes: 1_048_576,
      max_prompt_bytes: 1_048_576,
      max_artifact_bytes: 1_048_576,
      max_attempts: 1,
      max_observed_tokens: 1000,
    },
    verifier: {
      argv: ["frontier/verifier", "{artifact:result.json}"],
      executable_sha256: source.verifierDigest,
      cwd: "frontier",
      timeout_ms: 1000,
      max_output_bytes: 4096,
      network: "deny",
      writes: "deny",
    },
    scientific_chain: {
      predicted_observable: "The frozen JSON object has value 42.",
      performed_test: "verify frozen result.json",
    },
    landing: { expected_routes: ["defer"], max_accepted_delta: 0 },
  };
}

test("target offer validation requires one exact target", () => {
  assert.deepEqual(validateTargetOffer("a", {
    ok: true,
    value: { command: "next", targets: [{ target_id: "a" }, { target_id: "b" }] },
  }), { index: 0, id: "a" });
  assert.throws(() => validateTargetOffer("a", {
    ok: true,
    value: { command: "next", targets: [{ target_id: "a" }, { target_id: "a" }] },
  }), /exactly once/u);
});

test("current Run verifies and reproduces with zero frontier mutation", async () => {
  const source = await sourceRepository();
  const active = mission(source);
  const vela = new FakeVela();
  const engine = new FakeEngine(async (context) => {
    const worker = path.join(context.paths.work, "worker");
    await mkdir(worker, { recursive: true });
    const baseline = await snapshotWorkerWorkspace(worker);
    await writeFile(path.join(worker, "search.cpp"), "int main() { return 0; }\n");
    await stageWorkerWorkspaceEvidence({
      workspace: worker,
      runRoot: context.paths.root,
      baseline,
      excludedPaths: active.allowed_paths,
      secrets: [],
    });
    return {
      schema: "canopus.engine-output.v0",
      status: "success",
      claim: "The bounded result has value 42.",
      artifacts: [
        { path: "result.json", kind: "witness", encoding: "utf8", content: "{\"value\":42}\n" },
      ],
      observations: ["The finite construction completed."],
      caveats: ["This is a bounded fixture, not a general theorem."],
    };
  });
  const verifierRunner: CommandRunner = async (options) => ({
    argv: [...options.argv],
    exitCode: 0,
    signal: null,
    stdout: Buffer.from("value=42\n"),
    stderr: Buffer.alloc(0),
    durationMs: 1,
  });
  const result = await runCanopus({
    mission: active,
    sourceRepo: source.repo,
    runRoot: path.join(source.parent, "run"),
    vela,
    engine,
    verifierRunner,
  });
  assert.equal(result.record.schema, "canopus.run.v2");
  assert.equal(result.record.effect, "none");
  assert.equal(result.record.submission, null);
  assert.equal(result.record.reproduction.matched, true);
  assert.equal(result.projection.submitted, false);
  assert.deepEqual(projectCurrentRun(result.record), result.projection);
  assert.equal(vela.nextCalls, 1);
  assert.equal(await git(source.repo, "rev-parse", "HEAD^{commit}"), source.roots.git_commit);
  assert.match(await readFile(path.join(result.paths.root, "activity.jsonl"), "utf8"), /work\.skipped/u);
  await assert.rejects(
    readFile(path.join(result.paths.root, ".worker-evidence-staging", "manifest.json")),
    /ENOENT/u,
  );
  await assert.rejects(
    readFile(path.join(result.paths.root, "failure-evidence", "manifest.json")),
    /ENOENT/u,
  );
});

test("failed verification promotes bounded worker source evidence", async () => {
  const source = await sourceRepository();
  const active = mission(source);
  const runRoot = path.join(source.parent, "failed-run");
  const engine = new FakeEngine(async (context) => {
    const worker = path.join(context.paths.work, "worker");
    await mkdir(worker, { recursive: true });
    const baseline = await snapshotWorkerWorkspace(worker);
    await writeFile(path.join(worker, "search.cpp"), "int main() { return 0; }\n");
    await stageWorkerWorkspaceEvidence({
      workspace: worker,
      runRoot: context.paths.root,
      baseline,
      excludedPaths: active.allowed_paths,
      secrets: [],
    });
    return {
      schema: "canopus.engine-output.v0",
      status: "success",
      claim: "The bounded result has value 42.",
      artifacts: [
        { path: "result.json", kind: "witness", encoding: "utf8", content: "{\"value\":42}\n" },
      ],
      observations: ["The finite construction completed."],
      caveats: ["This is a bounded fixture, not a general theorem."],
    };
  });
  const verifierRunner: CommandRunner = async (options) => ({
    argv: [...options.argv],
    exitCode: 1,
    signal: null,
    stdout: Buffer.from("wrong value\n"),
    stderr: Buffer.alloc(0),
    durationMs: 1,
  });

  await assert.rejects(
    runCanopus({
      mission: active,
      sourceRepo: source.repo,
      runRoot,
      vela: new FakeVela(),
      engine,
      verifierRunner,
    }),
    /verifier returned failed/u,
  );
  assert.equal(
    await readFile(path.join(runRoot, "failure-evidence", "files", "search.cpp"), "utf8"),
    "int main() { return 0; }\n",
  );
  const activity = await readFile(path.join(runRoot, "activity.jsonl"), "utf8");
  assert.match(activity, /failure_evidence/u);
  assert.match(activity, /"retained_files":1/u);
  await assert.rejects(
    readFile(path.join(runRoot, ".worker-evidence-staging", "manifest.json")),
    /ENOENT/u,
  );
});
