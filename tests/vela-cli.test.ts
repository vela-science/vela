import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import type { Mission, MissionV1 } from "../src/contracts/mission.js";
import {
  DEFAULT_VELA_COMMAND_TIMEOUT_MS,
  strictBaselineFromCheck,
  validateLandResult,
  VelaClient,
  VelaClientError,
  type CommandRunner,
} from "../src/vela/cli.js";
import { sha256Bytes } from "../src/util/canonical.js";

const gitCommit = "b".repeat(40);
const gitTree = "c".repeat(40);
const root = `sha256:${"a".repeat(64)}`;
const velaBinaryDigest = sha256Bytes(readFileSync(process.execPath));

test("large-frontier Vela operations retain a bounded transaction-safe timeout", () => {
  assert.equal(DEFAULT_VELA_COMMAND_TIMEOUT_MS, 120_000);
});

function result(argv: readonly string[], stdout: unknown, exitCode = 0): Awaited<ReturnType<CommandRunner>> {
  return {
    argv: [...argv],
    exitCode,
    signal: null,
    stdout: Buffer.from(typeof stdout === "string" ? stdout : JSON.stringify(stdout)),
    stderr: Buffer.alloc(0),
    durationMs: 1,
  };
}

function mission(): Mission {
  return {
    schema: "canopus.mission.v0",
    id: "mission_vela_client",
    target: "target-1",
    vela_version: "0.800.19",
    vela_sha256: root,
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
      vela_event_log: root,
      vela_snapshot: root,
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
      executable_sha256: root,
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

function fakeRunner(options: { version?: string; checkRoot?: string; proofRoot?: string } = {}): {
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
      return result(argv, `vela ${options.version ?? "0.800.19"}\n`);
    }
    if (argv[1] === "check") {
      return result(argv, {
        ok: true,
        summary: { strict: true, status: "pass", errors: 0, invalid_findings: 0 },
        checks: [
          { id: "schema", status: "pass" },
          { id: "signals", status: "pass", failed: 0, blockers: [] },
          { id: "events", status: "pass" },
          { id: "state_integrity", status: "pass" },
          { id: "active_policy", status: "pass" },
          { id: "policy_readiness", status: "pass" },
          { id: "policy_lane", status: "pass" },
        ],
        replay: {
          event_log_hash: root.slice(7),
          current_hash: (options.checkRoot ?? root).slice(7),
          replayed_hash: (options.checkRoot ?? root).slice(7),
          source_hash: (options.checkRoot ?? root).slice(7),
        },
      });
    }
    if (argv[1] === "proof") {
      const proofRoot = options.proofRoot ?? root;
      return result(argv, {
        ok: true,
        event_log_hash: root,
        snapshot_hash: proofRoot,
        proof: { event_log_hash: root, frontier_hash: proofRoot },
      });
    }
    if (argv[1] === "next" || argv[1] === "work") {
      return result(argv, { ok: true, command: argv[1] });
    }
    if (argv[1] === "land") {
      return result(argv, {
        ok: true,
        command: "land",
        operation_id: "vop_test",
        receipt_root: root,
        record_id: "vrr_test",
        proposal_id: "vpr_test",
        finding_id: "vf_test",
        accepted_event_count_before: 12,
        accepted_event_count_after: 12,
        accepted_event_delta: 0,
        route: "deferred",
        original_route: null,
        detail: "human review required",
        publication: { state: "committed_local", commit: gitCommit },
      });
    }
    throw new Error(`unexpected command: ${argv.join(" ")}`);
  };
  return { runner, calls, environments };
}

function debtCheck(blockers: Array<Record<string, unknown>>): Record<string, unknown> {
  const status = blockers.length === 0 ? "pass" : "fail";
  return {
    ok: blockers.length === 0,
    summary: { strict: true, status, errors: 0, invalid_findings: 0 },
    checks: [
      { id: "schema", status: "pass" },
      { id: "signals", status, failed: blockers.length, blockers },
      { id: "events", status: "pass" },
      { id: "state_integrity", status: "pass" },
      { id: "active_policy", status: "pass" },
      { id: "policy_readiness", status: "pass" },
      { id: "policy_lane", status: "pass" },
    ],
    replay: {
      event_log_hash: root.slice(7),
      current_hash: root.slice(7),
      replayed_hash: root.slice(7),
      source_hash: root.slice(7),
    },
  };
}

function debtMission(check: Record<string, unknown>): MissionV1 {
  const base = mission();
  return {
    ...base,
    schema: "canopus.mission.v1",
    target_packet: { path: "packet.json", sha256: root },
    strict_baseline: strictBaselineFromCheck(check),
    worker: {
      kind: "codex_tools_native",
      platform: "darwin",
      codex_version: "codex-cli 0.144.5",
      codex_sha256: root,
      permission_profile_path: "contract/native-worker.config.toml",
      permission_profile_sha256: root,
      workspace: "target_packet_only",
      output_schema_sha256: root,
      model: "gpt-5.2-codex",
      network: "provider_only",
      tools: ["shell", "apply_patch"],
    },
    verifier: {
      ...base.verifier,
      capsule_path: "capsule",
      capsule_sha256: root,
      image: root,
    },
  };
}

function client(runner: CommandRunner): VelaClient {
  return new VelaClient({
    binary: process.execPath,
    expectedVersion: "0.800.19",
    expectedSha256: velaBinaryDigest,
    home: "/tmp/canopus-home",
    runner,
  });
}

test("Vela client proves Git, replay, and proof roots", async () => {
  const fake = fakeRunner();
  const inspection = await client(fake.runner).assertRoots("/repo", "frontier", mission().roots);
  assert.deepEqual(inspection.roots, mission().roots);
  assert.equal(fake.calls.some((argv) => argv.includes("sign")), false);
  assert.equal(fake.environments.some((env) => env.VELA_AGENT_KEY_HEX !== undefined), false);
  assert.equal(fake.environments.every((env) => env.VELA_NO_KEY_ACCESS === "1"), true);
});

test("Vela client forwards only the repository-authority agent socket", async () => {
  const fake = fakeRunner();
  const vela = new VelaClient({
    binary: process.execPath,
    expectedVersion: "0.800.19",
    expectedSha256: velaBinaryDigest,
    home: "/tmp/canopus-home",
    repositoryAuthorityAgentSocket: "/private/tmp/ssh-agent.sock",
    runner: fake.runner,
  });
  await vela.assertRoots("/repo", "frontier", mission().roots);
  assert.equal(
    fake.environments.every(
      (environment) => environment.SSH_AUTH_SOCK === "/private/tmp/ssh-agent.sock",
    ),
    true,
  );
  assert.equal(fake.environments.some((environment) => environment.VELA_AGENT_KEY_HEX !== undefined), false);
});

test("Vela client rejects a relative repository-authority agent socket", () => {
  const fake = fakeRunner();
  assert.throws(
    () =>
      new VelaClient({
        binary: process.execPath,
        expectedVersion: "0.800.19",
        expectedSha256: velaBinaryDigest,
        home: "/tmp/canopus-home",
        repositoryAuthorityAgentSocket: "agent.sock",
        runner: fake.runner,
      }),
    /must be one absolute path/u,
  );
});

test("Vela 0.915 uses strict replay roots for a minimal frontier without proof/latest", async () => {
  const fake = fakeRunner({ version: "0.915.1" });
  const vela = new VelaClient({
    binary: process.execPath,
    expectedVersion: "0.915.1",
    expectedSha256: velaBinaryDigest,
    home: "/tmp/canopus-home",
    runner: fake.runner,
  });
  const inspection = await vela.assertRoots("/repo", "frontier", mission().roots);
  assert.deepEqual(inspection.roots, mission().roots);
  assert.equal(inspection.proof.command, "status_root_projection");
  assert.equal(fake.calls.some((argv) => argv[1] === "proof"), false);
});

test("Vela 0.930 prereleases use compact roots without requiring a retired proof bundle", async () => {
  const fake = fakeRunner({ version: "0.930.0-rc.9" });
  const vela = new VelaClient({
    binary: process.execPath,
    expectedVersion: "0.930.0-rc.9",
    expectedSha256: velaBinaryDigest,
    home: "/tmp/canopus-home",
    runner: fake.runner,
  });
  const inspection = await vela.assertRoots("/repo", "frontier", mission().roots);
  assert.deepEqual(inspection.roots, mission().roots);
  assert.equal(inspection.proof.command, "status_root_projection");
  assert.equal(fake.calls.some((argv) => argv[1] === "proof"), false);
});

test("Vela client serializes strict check before proof verification", async () => {
  const fake = fakeRunner();
  let checkFinished = false;
  const runner: CommandRunner = async (options) => {
    if (options.argv[1] === "check") {
      await new Promise((resolve) => setTimeout(resolve, 10));
      const observed = await fake.runner(options);
      checkFinished = true;
      return observed;
    }
    if (options.argv[1] === "proof" && !checkFinished) {
      return result(options.argv, { error: "proof raced strict check" }, 1);
    }
    return await fake.runner(options);
  };
  const inspection = await client(runner).inspect("/repo", "frontier");
  assert.deepEqual(inspection.roots, mission().roots);
});

test("Vela client accepts only an exact registered strict blocker set", async () => {
  const blockers = [
    { id: "sig_b", kind: "missing_conditions", reason: "b", severity: "warning" },
    { id: "sig_a", kind: "missing_conditions", reason: "a", severity: "warning" },
    { id: "sig_c", kind: "unsigned_registered_actor", reason: "c", severity: "error" },
  ];
  const check = debtCheck(blockers);
  const activeMission = debtMission(check);
  const fake = fakeRunner();
  const runner: CommandRunner = async (options) => {
    if (options.argv[1] === "check") return result(options.argv, check, 1);
    return await fake.runner(options);
  };
  const inspection = await client(runner).assertRoots(
    "/repo",
    "frontier",
    activeMission.roots,
    activeMission.strict_baseline,
  );
  assert.equal(inspection.check.ok, false);
  assert.deepEqual(activeMission.strict_baseline.rule_counts, [
    { rule: "missing_conditions", count: 2 },
    { rule: "unsigned_registered_actor", count: 1 },
  ]);

  const drifted = debtCheck([...blockers, {
    id: "sig_d",
    kind: "missing_conditions",
    reason: "new debt",
    severity: "warning",
  }]);
  const driftRunner: CommandRunner = async (options) => {
    if (options.argv[1] === "check") return result(options.argv, drifted, 1);
    return await fake.runner(options);
  };
  await assert.rejects(
    client(driftRunner).assertRoots(
      "/repo",
      "frontier",
      activeMission.roots,
      activeMission.strict_baseline,
    ),
    (error: unknown) => error instanceof VelaClientError && error.code === "root_mismatch",
  );
});

test("Vela client rejects non-signal failures despite a registered debt baseline", async () => {
  const check = debtCheck([
    { id: "sig_a", kind: "missing_conditions", reason: "a", severity: "warning" },
  ]);
  const activeMission = debtMission(check);
  const checks = check.checks as Array<Record<string, unknown>>;
  const events = checks.find((entry) => entry.id === "events");
  assert.ok(events);
  events.status = "fail";
  const fake = fakeRunner();
  const runner: CommandRunner = async (options) => {
    if (options.argv[1] === "check") return result(options.argv, check, 1);
    return await fake.runner(options);
  };
  await assert.rejects(
    client(runner).assertRoots(
      "/repo",
      "frontier",
      activeMission.roots,
      activeMission.strict_baseline,
    ),
    /failed outside the registered signals baseline/u,
  );
});

test("Vela client reports bounded structured errors and only digests raw streams", async () => {
  const fake = fakeRunner();
  const runner: CommandRunner = async (options) => {
    if (options.argv[1] === "proof") {
      return {
        ...result(
          options.argv,
          {
            state_integrity: {
              structural_errors: [
                { message: "proof conflict with sk-never-display-123456789" },
              ],
            },
          },
          1,
        ),
        stderr: Buffer.from("Bearer never-display-this"),
      };
    }
    return await fake.runner(options);
  };
  await assert.rejects(
    client(runner).inspect("/repo", "frontier"),
    (error: unknown) => {
      assert.ok(error instanceof VelaClientError);
      assert.match(error.message, /proof conflict with \[secret-redacted\]/u);
      assert.match(error.message, /stdout_sha256=sha256:/u);
      assert.match(error.message, /stderr_sha256=sha256:/u);
      assert.doesNotMatch(error.message, /never-display-this/u);
      return true;
    },
  );
});

test("Vela client rejects the wrong released binary version", async () => {
  const fake = fakeRunner({ version: "0.800.13" });
  await assert.rejects(
    client(fake.runner).inspect("/repo", "frontier"),
    (error: unknown) => error instanceof VelaClientError && error.code === "version_mismatch",
  );
});

test("Vela client rejects a version-spoofing binary digest", async () => {
  const fake = fakeRunner();
  const spoofed = new VelaClient({
    binary: process.execPath,
    expectedVersion: "0.800.19",
    expectedSha256: root,
    home: "/tmp/canopus-home",
    runner: fake.runner,
  });
  await assert.rejects(
    spoofed.inspect("/repo", "frontier"),
    (error: unknown) => error instanceof VelaClientError && error.code === "version_mismatch",
  );
  assert.equal(fake.calls.length, 0);
});

test("Vela client rejects check/proof root disagreement", async () => {
  const fake = fakeRunner({ proofRoot: `sha256:${"d".repeat(64)}` });
  await assert.rejects(
    client(fake.runner).inspect("/repo", "frontier"),
    (error: unknown) => error instanceof VelaClientError && error.code === "root_mismatch",
  );
});

test("Vela client rechecks roots before work", async () => {
  const fake = fakeRunner({ checkRoot: `sha256:${"e".repeat(64)}` });
  await assert.rejects(
    client(fake.runner).work(mission(), "/repo", "task-1", mission().roots),
    /check\/proof snapshot mismatch|Vela snapshot mismatch/u,
  );
  assert.equal(fake.calls.some((argv) => argv[1] === "work"), false);
});

test("Vela client delegates Receipt v1 authoring to the released CLI", async () => {
  const fake = fakeRunner();
  const vela = client(fake.runner);
  const observation = await vela.landAuthoredCommand(
    mission(),
    "/repo",
    {
      claim: "The frozen result passed.",
      claimType: "computational",
      replayability: "exact",
      artifacts: [{ path: "artifact.json", kind: "witness" }],
      caveats: ["Human acceptance remains separate."],
      predictedObservable: "The exact verifier exits zero.",
      performedTest: "verify artifact.json",
      result: "The declared verifier exited zero.",
      evidence: [`artifact:${root}`],
      counterevidence: [],
      executionBinding: {
        schema: "vela.execution-binding.v1",
        packet_root: root,
        profile_root: `sha256:${"b".repeat(64)}`,
        verifier_capsule_root: `sha256:${"c".repeat(64)}`,
        result_contract_root: `sha256:${"d".repeat(64)}`,
      },
      work: "task-1",
    },
    mission().roots,
  );
  const landed = vela.validateLandResult(mission(), vela.parseLandCommand(observation));
  assert.equal(landed.route, "defer");
  assert.equal(landed.acceptedEventDelta, 0);
  const land = fake.calls.find((argv) => argv[1] === "land");
  assert.ok(land?.includes("--predicted-observable"));
  assert.ok(land?.includes("--performed-test"));
  assert.ok(land?.includes("--evidence"));
  assert.ok(land?.includes("--packet-root"));
  assert.ok(land?.includes("--profile-root"));
  assert.ok(land?.includes("--verifier-capsule-root"));
  assert.ok(land?.includes("--result-contract-root"));
  assert.equal(land?.includes("--push"), false);
  assert.equal(land?.includes("sign"), false);
});

test("Vela client rejects an oversized authored argv before the effectful command", async () => {
  const fake = fakeRunner();
  await assert.rejects(
    client(fake.runner).landAuthoredCommand(
      mission(),
      "/repo",
      {
        claim: "Bounded result.",
        claimType: "computational",
        replayability: "exact",
        artifacts: [{ path: "artifact.json", kind: "witness" }],
        caveats: Array.from({ length: 60 }, (_, index) => `caveat-${index}`),
        predictedObservable: "The verifier exits zero.",
        performedTest: "verify artifact.json",
        result: "Passed.",
        evidence: [],
        counterevidence: [],
      },
      mission().roots,
    ),
    /maximum is 128/u,
  );
  assert.equal(fake.calls.some((argv) => argv[1] === "land"), false);
});

test("Vela client preserves policy-admitted exact-retry historical counts", async () => {
  const raw = {
      ok: true,
      command: "land",
      operation_id: "vop_retry",
      receipt_root: root,
      record_id: "vrr_retry",
      proposal_id: "vpr_retry",
      finding_id: "vf_retry",
      accepted_event_count_before: 12,
      accepted_event_count_after: 13,
      accepted_event_delta: 1,
      route: "exact_retry",
      original_route: "policy_admitted",
      detail: "reused durable policy_admitted result",
      publication: { state: "committed_local" },
  };
  const activeMission = mission();
  activeMission.landing = { expected_routes: ["permit"], max_accepted_delta: 1 };
  const landed = validateLandResult(activeMission, raw);
  assert.equal(landed.route, "exact_retry");
  assert.equal(landed.originalRoute, "permit");
  assert.equal(landed.acceptedEventDelta, 1);
});

test("Vela client preserves journal-free exact retry with unknown historical counts", async () => {
  const raw = {
      ok: true,
      command: "land",
      operation_id: "vop_retry",
      receipt_root: root,
      record_id: "vrr_retry",
      proposal_id: "vpr_retry",
      finding_id: "vf_retry",
      accepted_event_count_before: null,
      accepted_event_count_after: null,
      accepted_event_delta: null,
      route: "exact_retry",
      original_route: "deferred",
      detail: "reused durable deferred result",
      publication: { state: "committed_local" },
  };
  const activeMission = mission();
  const landed = validateLandResult(activeMission, raw);
  assert.equal(landed.route, "exact_retry");
  assert.equal(landed.originalRoute, "defer");
  assert.equal(landed.acceptedEventDelta, null);
});

test("Vela client exposes no signer command", () => {
  assert.equal("sign" in VelaClient.prototype, false);
  assert.equal("accept" in VelaClient.prototype, false);
});
