import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import type { Mission } from "../src/contracts/mission.js";
import {
  DEFAULT_VELA_COMMAND_TIMEOUT_MS,
  VelaClient,
  VelaClientError,
  type CommandRunner,
} from "../src/vela/cli.js";
import { sha256Bytes } from "../src/util/canonical.js";

const gitCommit = "b".repeat(40);
const gitTree = "c".repeat(40);
const repositoryRoot = `sha256:${"a".repeat(64)}`;
const originRoot = `sha256:${"d".repeat(64)}`;
const keysetRoot = `sha256:${"e".repeat(64)}`;
const policyRoot = `sha256:${"f".repeat(64)}`;
const velaBinaryDigest = sha256Bytes(readFileSync(process.execPath));

test("large-frontier Vela operations retain a bounded transaction-safe timeout", () => {
  assert.equal(DEFAULT_VELA_COMMAND_TIMEOUT_MS, 600_000);
});

function result(
  argv: readonly string[],
  stdout: unknown,
  exitCode = 0,
  stderr = "",
): Awaited<ReturnType<CommandRunner>> {
  return {
    argv: [...argv],
    exitCode,
    signal: null,
    stdout: Buffer.from(typeof stdout === "string" ? stdout : JSON.stringify(stdout)),
    stderr: Buffer.from(stderr),
    durationMs: 1,
  };
}

function mission(): Mission {
  return {
    schema: "canopus.mission.v0",
    id: "mission_vela_client",
    target: "target-1",
    vela_version: "0.940.7",
    vela_sha256: repositoryRoot,
    frontier: "frontier",
    actor: "agent:canopus-test",
    role: "producer",
    claim_type: "computational",
    replayability: "exact",
    objective: "Produce a finite witness.",
    completion_condition: "The verifier passes.",
    roots: {
      git_commit: gitCommit,
      git_tree: gitTree,
      vela_repository: repositoryRoot,
    },
    allowed_paths: ["artifact.json"],
    budgets: {
      max_research_wall_time_ms: 1000,
      max_research_processes: 2,
      max_research_output_bytes: 4096,
      max_prompt_bytes: 4096,
      max_artifact_bytes: 4096,
      max_attempts: 1,
      max_observed_tokens: 1000,
    },
    verifier: {
      argv: ["frontier/verifier", "artifact.json"],
      executable_sha256: repositoryRoot,
      cwd: "frontier",
      timeout_ms: 1000,
      max_output_bytes: 4096,
      network: "deny",
      writes: "deny",
    },
    scientific_chain: {
      predicted_observable: "The declared verifier exits zero.",
      performed_test: "verify artifact.json",
    },
    landing: { expected_routes: ["defer"], max_accepted_delta: 0 },
  };
}

interface FakeOptions {
  version?: string;
  status?: Record<string, unknown>;
  repository?: Record<string, unknown>;
  next?: Record<string, unknown>;
  repositoryExitCode?: number;
  repositoryStderr?: string;
}

function validStatus(): Record<string, unknown> {
  return {
    schema: "vela.status.v1",
    ok: true,
    command: "status",
    frontier: { id: "vfr_fixture", name: "Fixture", profile_root: repositoryRoot },
    git: { commit: gitCommit, tree: gitTree },
    integrity: {
      replay: "verified",
      strict: "pass",
      blocker_count: 0,
      blockers_by_code: {},
    },
    roots: {
      origin: originRoot,
      repository: repositoryRoot,
      authority_keyset: keysetRoot,
      authority_policy: policyRoot,
    },
    counts: {},
    next_action: "vela repository verify . --json",
  };
}

function validRepository(): Record<string, unknown> {
  return {
    schema: "vela.repository-verification.v2",
    ok: true,
    command: "repository verify",
    frontier: "frontier",
    frontier_id: "vfr_fixture",
    git_commit: gitCommit,
    git_tree: gitTree,
    origin_id: "vor_fixture",
    origin_root: originRoot,
    repository_root: repositoryRoot,
    authority_keyset_root: keysetRoot,
    authority_policy_root: policyRoot,
    counts: {},
  };
}

function fakeRunner(options: FakeOptions = {}): {
  runner: CommandRunner;
  calls: string[][];
  environments: NodeJS.ProcessEnv[];
} {
  const calls: string[][] = [];
  const environments: NodeJS.ProcessEnv[] = [];
  const runner: CommandRunner = async (command) => {
    const argv = [...command.argv];
    calls.push(argv);
    environments.push(command.env);
    if (argv[0] === "git") {
      return result(argv, `${argv.at(-1) === "HEAD^{tree}" ? gitTree : gitCommit}\n`);
    }
    if (argv[1] === "--version") {
      return result(argv, `vela ${options.version ?? "0.940.7"}\n`);
    }
    if (argv[1] === "status") {
      return result(argv, options.status ?? validStatus());
    }
    if (argv[1] === "repository") {
      return result(
        argv,
        options.repository ?? validRepository(),
        options.repositoryExitCode ?? 0,
        options.repositoryStderr,
      );
    }
    if (argv[1] === "next") {
      return result(argv, options.next ?? {
        schema: "vela.offer.v1",
        ok: true,
        command: "next",
        frontier_id: "vfr_fixture",
        epoch_id: "vre_fixture",
        repository_root: repositoryRoot,
        target_index_root: `sha256:${"1".repeat(64)}`,
        availability: { configured: 1, stale: 0, fresh: 1, returned: 1 },
        targets: [{ target_id: "target-1", rank: 1 }],
      });
    }
    throw new Error(`unexpected command: ${argv.join(" ")}`);
  };
  return { runner, calls, environments };
}

function client(runner: CommandRunner, version = "0.940.7"): VelaClient {
  return new VelaClient({
    binary: process.execPath,
    expectedVersion: version,
    expectedSha256: velaBinaryDigest,
    home: "/tmp/canopus-home",
    runner,
  });
}

test("Vela client binds Git to one strict-passing current repository root", async () => {
  const fake = fakeRunner();
  const inspection = await client(fake.runner).assertRoots("/repo", "frontier", mission().roots);
  assert.deepEqual(inspection.roots, mission().roots);
  assert.equal(inspection.status.schema, "vela.status.v1");
  assert.equal(inspection.repository.schema, "vela.repository-verification.v2");
  assert.equal(fake.calls.some((argv) => argv.includes("sign")), false);
  assert.equal(fake.calls.some((argv) => argv[1] === "check" || argv[1] === "proof"), false);
  assert.equal(fake.environments.some((env) => env.VELA_AGENT_KEY_HEX !== undefined), false);
  assert.equal(fake.environments.every((env) => env.VELA_NO_KEY_ACCESS === "1"), true);
});

test("Vela client rejects replay or strict debt instead of registering a baseline", async () => {
  for (const integrity of [
    { replay: "failed", strict: "pass", blocker_count: 0, blockers_by_code: {} },
    {
      replay: "verified",
      strict: "fail",
      blocker_count: 1,
      blockers_by_code: { unsigned_record: 1 },
    },
  ]) {
    const status = validStatus();
    status.integrity = integrity;
    await assert.rejects(
      client(fakeRunner({ status }).runner).inspect("/repo", "frontier"),
      (error: unknown) =>
        error instanceof VelaClientError &&
        error.code === "command_failed" &&
        /must replay and pass strict verification/u.test(error.message),
    );
  }
});

test("Vela client rejects status and verification root drift", async () => {
  const repository = validRepository();
  repository.repository_root = `sha256:${"2".repeat(64)}`;
  await assert.rejects(
    client(fakeRunner({ repository }).runner).inspect("/repo", "frontier"),
    (error: unknown) => error instanceof VelaClientError && error.code === "root_mismatch",
  );

  const status = validStatus();
  (status.git as Record<string, unknown>).tree = "9".repeat(40);
  await assert.rejects(
    client(fakeRunner({ status }).runner).inspect("/repo", "frontier"),
    (error: unknown) => error instanceof VelaClientError && error.code === "root_mismatch",
  );
});

test("Vela client rejects malformed current contract identities", async () => {
  const status = validStatus();
  status.schema = "vela.status.v2";
  await assert.rejects(
    client(fakeRunner({ status }).runner).inspect("/repo", "frontier"),
    /status contract identity is invalid/u,
  );

  const repository = validRepository();
  repository.schema = "vela.repository-verification.v0";
  await assert.rejects(
    client(fakeRunner({ repository }).runner).inspect("/repo", "frontier"),
    /verification contract identity is invalid/u,
  );
});

test("Vela client requires the exact registered binary version", async () => {
  const fake = fakeRunner({ version: "0.940.0" });
  await assert.rejects(
    client(fake.runner).inspect("/repo", "frontier"),
    (error: unknown) => error instanceof VelaClientError && error.code === "version_mismatch",
  );
});

test("Vela offer remains bound to the inspected repository root", async () => {
  const fake = fakeRunner();
  const response = await client(fake.runner).next(mission(), "/repo");
  assert.equal(response.value.schema, "vela.offer.v1");

  const next = {
    schema: "vela.offer.v1",
    ok: true,
    command: "next",
    repository_root: `sha256:${"2".repeat(64)}`,
    targets: [],
  };
  await assert.rejects(
    client(fakeRunner({ next }).runner).next(mission(), "/repo"),
    (error: unknown) => error instanceof VelaClientError && error.code === "root_mismatch",
  );
});

test("Vela client reports bounded structured command errors and hashes raw streams", async () => {
  const fake = fakeRunner({
    repositoryExitCode: 2,
    repository: {
      error: {
        message: "repository conflict with sk-never-display-123456789",
      },
    },
    repositoryStderr: "Bearer never-display-this",
  });
  await assert.rejects(
    client(fake.runner).inspect("/repo", "frontier"),
    (error: unknown) => {
      assert.ok(error instanceof VelaClientError);
      assert.match(error.message, /repository conflict with \[secret-redacted\]/u);
      assert.match(error.message, /stdout_sha256=sha256:/u);
      assert.match(error.message, /stderr_sha256=sha256:/u);
      assert.doesNotMatch(error.message, /never-display/u);
      return true;
    },
  );
});
