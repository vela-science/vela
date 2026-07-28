import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { chmod, lstat, mkdir, mkdtemp, readFile, realpath, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

import { freezeArtifact, sealArtifactStore } from "../src/artifact/freeze.js";
import { BudgetTracker } from "../src/budget/enforce.js";
import type { Mission, MissionV1 } from "../src/contracts/mission.js";
import { runCommand, type CommandRunner } from "../src/util/command.js";
import { sha256Bytes } from "../src/util/canonical.js";
import { runVerifier } from "../src/verifier/run.js";

const rootDigest = `sha256:${"a".repeat(64)}`;
const exec = promisify(execFile);

const VERIFIER_SOURCE = `
#include <stdio.h>
#include <sys/stat.h>
#include <string.h>
int main(int argc, char **argv) {
  if (argc < 2) return 0;
  if (strcmp(argv[1], "read") == 0 && argc == 4) {
    char a[64] = {0}, b[64] = {0};
    FILE *left = fopen(argv[2], "r");
    FILE *right = fopen(argv[3], "r");
    if (!left || !right || !fgets(a, sizeof(a), left) || !fgets(b, sizeof(b), right)) return 1;
    a[strcspn(a, "\\r\\n")] = 0; b[strcspn(b, "\\r\\n")] = 0;
    printf("%s|%s", a, b); return 0;
  }
  if (strcmp(argv[1], "read-one") == 0 && argc == 3) {
    FILE *source = fopen(argv[2], "r"); return source ? 0 : 1;
  }
  if (strcmp(argv[1], "write") == 0 && argc == 3) {
    FILE *target = fopen(argv[2], "w");
    if (!target) return 1; fputs("x", target); fclose(target); return 0;
  }
  if (strcmp(argv[1], "stat-one") == 0 && argc == 3) {
    struct stat result; return stat(argv[2], &result) == 0 ? 0 : 1;
  }
  return 0;
}
`;

async function fixture(): Promise<{
  mission: Mission;
  paths: {
    root: string;
    input: string;
    frontier: string;
    work: string;
    output: string;
    artifacts: string;
    home: string;
    velaHome: string;
    verifierHome: string;
  };
}> {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-verifier-"));
  const paths = {
    root,
    input: path.join(root, "input"),
    frontier: path.join(root, "frontier"),
    work: path.join(root, "work"),
    output: path.join(root, "output"),
    artifacts: path.join(root, "artifacts"),
    home: path.join(root, "home"),
    velaHome: path.join(root, "vela-home"),
    verifierHome: path.join(root, "verifier-home"),
  };
  await Promise.all(Object.values(paths).slice(1).map((entry) => mkdir(entry)));
  await mkdir(path.join(paths.input, "frontier"));
  const verifier = path.join(paths.input, "frontier", "verifier");
  const source = path.join(root, "verifier.c");
  await writeFile(source, VERIFIER_SOURCE);
  await exec("/usr/bin/clang", ["-Os", "-o", verifier, source]);
  await chmod(verifier, 0o555);
  const verifierDigest = sha256Bytes(await readFile(verifier));
  return {
    paths,
    mission: {
      schema: "canopus.mission.v0",
      id: "mission_verifier",
      target: "target-1",
      vela_version: "0.800.19",
      vela_sha256: rootDigest,
      frontier: "frontier",
      actor: "agent:canopus-test",
      role: "verifier",
      claim_type: "computational",
      replayability: "exact",
      objective: "Verify one artifact.",
      completion_condition: "Verifier exits zero.",
      roots: {
        git_commit: "b".repeat(40),
        git_tree: "c".repeat(40),
        vela_repository: rootDigest,
      },
      allowed_paths: ["witness"],
      budgets: {
        max_research_wall_time_ms: 10_000,
        max_research_processes: 2,
        max_research_output_bytes: 4096,
        max_prompt_bytes: 4096,
        max_artifact_bytes: 4096,
        max_attempts: 1,
        max_observed_tokens: 100,
      },
      verifier: {
        argv: ["frontier/verifier", "{artifact:witness}"],
        executable_sha256: verifierDigest,
        cwd: "frontier",
        timeout_ms: 1000,
        max_output_bytes: 1024,
        network: "deny",
        writes: "deny",
      },
      scientific_chain: {
        predicted_observable: "The frozen proof passes verification.",
        performed_test: "verify frozen proof",
      },
      landing: { expected_routes: ["defer"], max_accepted_delta: 0 },
    },
  };
}

test("verifier runs frozen bytes in the macOS network/write sandbox", async () => {
  const data = await fixture();
  await writeFile(path.join(data.paths.output, "witness"), "proof");
  const artifact = await freezeArtifact({
    sourceRoot: data.paths.output,
    artifactRoot: data.paths.artifacts,
    path: "witness",
    kind: "proof",
    maxBytes: 4096,
  });
  await sealArtifactStore(data.paths.artifacts);
  const calls: string[][] = [];
  const runner: CommandRunner = async (options) => {
    calls.push([...options.argv]);
    return {
      argv: [...options.argv],
      exitCode: 0,
      signal: null,
      stdout: Buffer.from("ok\n"),
      stderr: Buffer.alloc(0),
      durationMs: 2,
    };
  };
  const outcome = await runVerifier({
    mission: data.mission,
    paths: data.paths,
    artifacts: [artifact],
    budget: new BudgetTracker(data.mission.budgets),
    runner,
  });
  assert.equal(outcome.status, "passed");
  assert.equal(calls[0]?.[0], "/usr/bin/sandbox-exec");
  const profile = calls[0]?.[2] ?? "";
  assert.match(profile, /\(deny default\)/u);
  assert.match(profile, /deny network/u);
  assert.match(profile, /\(import "dyld-support\.sb"\)/u);
  assert.doesNotMatch(profile, /\(allow file-read\*\)\s/u);
  assert.match(profile, /allow file-write\* \(literal "\/dev\/null"\)/u);
  assert.doesNotMatch(profile, new RegExp(data.paths.frontier, "u"));
  assert.doesNotMatch(profile, new RegExp(data.paths.output, "u"));
  assert.doesNotMatch(profile, new RegExp(data.paths.velaHome, "u"));
  assert.match(profile, new RegExp(await realpath(data.paths.verifierHome), "u"));
  assert.equal(calls[0]?.at(-1), artifact.frozenPath);
});

test("real verifier sandbox denies host-secret reads and all persistent writes", async () => {
  const data = await fixture();
  const verifierCapsule = path.join(data.paths.input, "frontier", "verifier");
  assert.equal((await lstat(verifierCapsule)).mode & 0o111, 0o111);
  await writeFile(path.join(data.paths.input, "frontier", "visible.txt"), "public\n");
  await writeFile(path.join(data.paths.output, "witness"), "proof\n");
  const artifact = await freezeArtifact({
    sourceRoot: data.paths.output,
    artifactRoot: data.paths.artifacts,
    path: "witness",
    kind: "proof",
    maxBytes: 4096,
  });
  await sealArtifactStore(data.paths.artifacts);
  const keyDirectory = path.join(
    data.paths.velaHome,
    ".vela",
    "agents",
    "agent_canopus-test",
  );
  await mkdir(keyDirectory, { recursive: true });
  const secret = path.join(keyDirectory, "private.key");
  const writeTarget = path.join(data.paths.frontier, "verifier-write.txt");
  await writeFile(secret, "must-not-be-readable\n");
  assert.equal(await readFile(secret, "utf8"), "must-not-be-readable\n");

  const allowedMission = structuredClone(data.mission);
  allowedMission.verifier.argv = [
    "frontier/verifier",
    "read",
    "{artifact:witness}",
    path.join(data.paths.input, "frontier", "visible.txt"),
  ];
  let allowedStderr = "";
  const allowedRunner: CommandRunner = async (options) => {
    const result = await runCommand(options);
    allowedStderr = result.stderr.toString("utf8");
    return result;
  };
  const allowedOutcome = await runVerifier({
    mission: allowedMission,
    paths: data.paths,
    artifacts: [artifact],
    budget: new BudgetTracker(allowedMission.budgets),
    runner: allowedRunner,
  });
  assert.equal(
    allowedOutcome.status,
    "passed",
    `${allowedStderr} exit=${allowedOutcome.record.exit_code}`,
  );
  assert.equal(allowedOutcome.record.stdout_digest, sha256Bytes("proof|public"));

  const readMission = structuredClone(data.mission);
  readMission.verifier.argv = [
    "frontier/verifier",
    "read-one",
    secret,
  ];
  const readOutcome = await runVerifier({
    mission: readMission,
    paths: data.paths,
    artifacts: [],
    budget: new BudgetTracker(readMission.budgets),
  });
  assert.equal(readOutcome.status, "failed");

  const metadataMission = structuredClone(data.mission);
  metadataMission.verifier.argv = [
    "frontier/verifier",
    "stat-one",
    secret,
  ];
  const metadataOutcome = await runVerifier({
    mission: metadataMission,
    paths: data.paths,
    artifacts: [],
    budget: new BudgetTracker(metadataMission.budgets),
  });
  assert.equal(metadataOutcome.status, "failed");

  const writeMission = structuredClone(data.mission);
  writeMission.verifier.argv = [
    "frontier/verifier",
    "write",
    writeTarget,
  ];
  const writeOutcome = await runVerifier({
    mission: writeMission,
    paths: data.paths,
    artifacts: [],
    budget: new BudgetTracker(writeMission.budgets),
  });
  assert.equal(writeOutcome.status, "failed");
  await assert.rejects(lstat(writeTarget), /ENOENT/u);
});

test("verifier records nonzero as an honest failed result", async () => {
  const data = await fixture();
  await writeFile(path.join(data.paths.output, "witness"), "proof");
  const artifact = await freezeArtifact({
    sourceRoot: data.paths.output,
    artifactRoot: data.paths.artifacts,
    path: "witness",
    kind: "proof",
    maxBytes: 4096,
  });
  const runner: CommandRunner = async (options) => ({
    argv: [...options.argv],
    exitCode: 1,
    signal: null,
    stdout: Buffer.alloc(0),
    stderr: Buffer.from("counterexample\n"),
    durationMs: 3,
  });
  const outcome = await runVerifier({
    mission: data.mission,
    paths: data.paths,
    artifacts: [artifact],
    budget: new BudgetTracker(data.mission.budgets),
    runner,
  });
  assert.equal(outcome.status, "failed");
  assert.equal(outcome.record.exit_code, 1);
});

test("verifier rejects an executable absent from the exact input", async () => {
  const data = await fixture();
  data.mission.verifier.argv[0] = "bash";
  await assert.rejects(
    runVerifier({
      mission: data.mission,
      paths: data.paths,
      artifacts: [],
      budget: new BudgetTracker(data.mission.budgets),
    }),
    /executable is unavailable/u,
  );
});

test("mission v1 verifier uses an exact capsule in a network/write-denied container", async () => {
  const data = await fixture();
  const bundleRoot = path.join(data.paths.root, "bundle");
  await mkdir(bundleRoot);
  const capsule = path.join(bundleRoot, "verifier.py");
  await writeFile(capsule, "#!/usr/bin/env python3\nraise SystemExit(0)\n", { mode: 0o555 });
  const capsuleDigest = sha256Bytes(await readFile(capsule));
  await writeFile(path.join(data.paths.output, "witness"), "proof\n");
  const artifact = await freezeArtifact({
    sourceRoot: data.paths.output,
    artifactRoot: data.paths.artifacts,
    path: "witness",
    kind: "proof",
    maxBytes: 4096,
  });
  await sealArtifactStore(data.paths.artifacts);
  const base = data.mission;
  const active: MissionV1 = {
    ...base,
    schema: "canopus.mission.v1",
    target_packet: { path: "packet.json", sha256: rootDigest },
    worker: {
      kind: "codex_tools_native",
      platform: "darwin",
      codex_version: "codex-cli 0.144.5",
      codex_sha256: rootDigest,
      permission_profile_path: "contract/native-worker.config.toml",
      permission_profile_sha256: rootDigest,
      workspace: "target_packet_only",
      output_schema_sha256: rootDigest,
      model: "gpt-test",
      network: "provider_only",
      tools: ["shell", "apply_patch"],
    },
    verifier: {
      argv: ["capsule/verifier", "{artifact:witness}"],
      executable_sha256: capsuleDigest,
      cwd: "frontier",
      timeout_ms: 1000,
      max_output_bytes: 1024,
      network: "deny",
      writes: "deny",
      capsule_path: "verifier.py",
      capsule_sha256: capsuleDigest,
      image: rootDigest,
      platform: "linux/amd64",
    },
  };
  const calls: string[][] = [];
  const runner: CommandRunner = async (options) => {
    calls.push([...options.argv]);
    if (options.argv[1] === "image") {
      return {
        argv: [...options.argv], exitCode: 0, signal: null,
        stdout: Buffer.from(`${rootDigest}\n`), stderr: Buffer.alloc(0), durationMs: 1,
      };
    }
    return {
      argv: [...options.argv], exitCode: 0, signal: null,
      stdout: Buffer.from("verified\n"), stderr: Buffer.alloc(0), durationMs: 2,
    };
  };
  const outcome = await runVerifier({
    mission: active,
    paths: data.paths,
    artifacts: [artifact],
    budget: new BudgetTracker(active.budgets),
    bundleRoot,
    dockerBinary: "/usr/bin/docker",
    runner,
  });
  assert.equal(outcome.status, "passed");
  assert.equal(outcome.sandbox, "container_network_denied");
  const docker = calls[1] ?? [];
  assert.ok(docker.includes("--network=none"));
  assert.deepEqual(docker.slice(2, 4), ["--platform", "linux/amd64"]);
  assert.ok(docker.includes("--read-only"));
  assert.ok(docker.includes("--cap-drop=ALL"));
  assert.ok(docker.includes("--security-opt=no-new-privileges"));
  assert.equal(docker.some((item) => item.includes("dst=/input") && !item.endsWith("readonly")), false);
  assert.equal(docker.some((item) => item.includes("docker.sock")), false);
  assert.deepEqual(outcome.record.argv, ["/capsule/verifier", "/artifacts/0"]);

  await chmod(capsule, 0o755);
  await writeFile(capsule, "#!/usr/bin/env python3\nraise SystemExit(1)\n", { mode: 0o555 });
  await assert.rejects(
    runVerifier({
      mission: active,
      paths: data.paths,
      artifacts: [artifact],
      budget: new BudgetTracker(active.budgets),
      bundleRoot,
      dockerBinary: "/usr/bin/docker",
      runner,
    }),
    /capsule digest does not match/u,
  );
});
