import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { chmod, mkdtemp, readFile, realpath, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import test from "node:test";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

import { parseAgentRunRequest } from "../src/product/attempt.js";
import {
  canonicalJson,
  contentDigest,
  protocolDigest,
  sha256Bytes,
} from "../src/util/canonical.js";

const execFileAsync = promisify(execFile);
const packageRoot = fileURLToPath(new URL("../../", import.meta.url));
const helperPath = path.join(packageRoot, "dist", "vela-agent");

function request(): Record<string, unknown> {
  const target = "erdos:1056";
  const artifact = {
    path: "artifacts/erdos-1056-10430001-10430200.json",
    kind: "computational",
  };
  const mission = {
    schema: "canopus.mission.v1",
    id: "mission_erdos_1056_10430001_10430200",
    target,
    actor: "agent:codex",
    frontier: ".",
    role: "producer",
    allowed_paths: [artifact.path],
    budgets: {
      max_artifact_bytes: 4096,
      max_research_wall_time_ms: 30_000,
    },
    verifier: {
      capsule_path: "verifier",
    },
  };
  const missionBytes = canonicalJson(mission);
  const missionReference = {
    path: "execution/erdos-1056/mission.json",
    size: Buffer.byteLength(missionBytes, "utf8"),
    sha256: sha256Bytes(missionBytes),
  };
  assert.equal(missionReference.sha256, contentDigest(mission));

  const bundle = {
    schema: "vela.agent-execution-bundle.v1",
    authority: "non_authoritative",
    effect: "none",
    target: { id: target },
    mission: missionReference,
    artifact_contract: artifact,
    safeguards: {
      worker_inputs: ["mission", "target_packet"],
      prior_answer_inputs: [],
      duplicate_work: "target_revalidation",
    },
    verifier: {
      image: `ghcr.io/vela-science/canopus-verifier@sha256:${"e".repeat(64)}`,
      capsule: {
        path: "execution/erdos-1056/verifier",
        size: 1024,
        sha256: `sha256:${"f".repeat(64)}`,
      },
      source: {
        path: "execution/erdos-1056/verifier.cpp",
        size: 2048,
        sha256: `sha256:${"0".repeat(64)}`,
      },
      runtime: {
        supported_hosts: ["darwin-arm64", "linux-x64"],
        verifier_platform: "linux/arm64",
      },
      isolation: { network: "deny", writes: "deny" },
    },
  };
  const bundleBytes = canonicalJson(bundle);
  const reference = {
    schema: "vela.agent-execution-bundle.v1",
    path: "execution/erdos-1056/bundle.json",
    size: Buffer.byteLength(bundleBytes, "utf8"),
    sha256: sha256Bytes(bundleBytes),
  };
  assert.equal(reference.sha256, contentDigest(bundle));

  const packetValue = {
    schema: "erdos-frontier.problem-work.v2",
    problem: 1056,
    execution_bundle: reference,
  };
  const packetJson = canonicalJson(packetValue);
  const packetReference = {
    schema: packetValue.schema,
    path: "targets/erdos-1056.json",
    size: Buffer.byteLength(packetJson, "utf8"),
    sha256: sha256Bytes(packetJson),
  };

  const runnerBuild = {
    schema: "vela.agent-helper-build.v1",
    platform: "darwin-arm64",
    runtime: {
      kind: "bun",
      version: "1.3.12",
      size: 61_405_888,
      sha256: `sha256:${"1".repeat(64)}`,
    },
    bundle: {
      format: "esm",
      size: 222_448,
      sha256: `sha256:${"2".repeat(64)}`,
    },
  };
  const runnerBuildRoot = protocolDigest(runnerBuild);

  const preimage = {
    schema: "vela.agent-run-request.internal.v1",
    authority: "none",
    effect: "none",
    frontier: {
      path: "/tmp/frontier",
      id: "vfr_1234567890abcdef",
      origin_id: "vro_1234567890abcdef",
      repository_root: `sha256:${"3".repeat(64)}`,
    },
    attempt: {
      id: `vat_${"4".repeat(64)}`,
      authorization_root: `sha256:${"5".repeat(64)}`,
      actor: "agent:codex",
      created_at: "2026-07-31T00:00:00Z",
      expires_at: "2026-08-01T00:00:00Z",
      controller_build: {
        program: "vela-cli",
        version: "0.950.0",
        binary_sha256: `sha256:${"6".repeat(64)}`,
      },
      runner_build_root: runnerBuildRoot,
      allowed_operations: ["inspect", "run_tool", "write_private_artifact"],
      allowed_artifact_classes: [artifact.kind],
      budget: {
        max_runs: 1,
        max_submissions: 1,
        max_verifications: 1,
        max_artifacts: 2,
        max_artifact_bytes: 4096,
      },
      usage: {
        runs: 1,
        submissions: 0,
        verifications: 0,
        artifacts: 0,
        artifact_bytes: 0,
        registered_submission_ids: [],
        registered_verification_record_ids: [],
      },
      consequence_ceiling: "pending_review",
      task_contract_root: `sha256:${"7".repeat(64)}`,
    },
    target: {
      id: target,
      binding_root: `sha256:${"8".repeat(64)}`,
      target_index_root: `sha256:${"9".repeat(64)}`,
      input_root: `sha256:${"a".repeat(64)}`,
      source: {
        git_object_format: "sha1",
        git_commit: "b".repeat(40),
        git_tree: "c".repeat(40),
      },
      claim_read_set: {
        git_object_format: "sha1",
        git_commit: "b".repeat(40),
        git_tree: "c".repeat(40),
      },
      packet: packetReference,
      packet_json: packetJson,
    },
    execution_bundle: {
      reference,
      value: bundle,
      mission,
    },
    runner_build: runnerBuild,
    output_root: null,
  };
  return { ...preimage, request_root: protocolDigest(preimage) };
}

test("parses one exact v7 authority-free Attempt-bound Agent request", () => {
  const parsed = parseAgentRunRequest(request());
  assert.equal(parsed.authority, "none");
  assert.equal(parsed.effect, "none");
  assert.equal(parsed.attempt.id, `vat_${"4".repeat(64)}`);
  assert.equal(parsed.attempt.budget.max_runs, 1);
  assert.equal(parsed.attempt.usage.runs, 1);
  assert.equal(parsed.execution_bundle.reference.path, "execution/erdos-1056/bundle.json");
  assert.deepEqual(parsed.execution_bundle.value.artifact_contract, {
    path: "artifacts/erdos-1056-10430001-10430200.json",
    kind: "computational",
  });
  assert.equal(protocolDigest(parsed.runner_build), parsed.attempt.runner_build_root);
});

test("rejects request, bundle, Target, packet, and helper-build substitution", () => {
  const changedRequest = request();
  (changedRequest.attempt as Record<string, unknown>).actor = "agent:other";
  assert.throws(() => parseAgentRunRequest(changedRequest), /request root/u);

  const changedBundle = request();
  const execution = changedBundle.execution_bundle as Record<string, unknown>;
  const bundle = execution.value as Record<string, unknown>;
  bundle.target = { id: "formal:505" };
  const { request_root: _, ...changedBundlePreimage } = changedBundle;
  changedBundle.request_root = protocolDigest(changedBundlePreimage);
  assert.throws(() => parseAgentRunRequest(changedBundle), /substituted another Target/u);

  const changedPacket = request();
  const changedTarget = changedPacket.target as Record<string, unknown>;
  changedTarget.packet_json = `${String(changedTarget.packet_json)} `;
  const { request_root: __, ...changedPacketPreimage } = changedPacket;
  changedPacket.request_root = protocolDigest(changedPacketPreimage);
  assert.throws(() => parseAgentRunRequest(changedPacket), /packet bytes/u);

  const changedBuild = request();
  const changedRunner = changedBuild.runner_build as Record<string, unknown>;
  const changedRuntime = changedRunner.runtime as Record<string, unknown>;
  changedRuntime.sha256 = `sha256:${"d".repeat(64)}`;
  const { request_root: ___, ...changedBuildPreimage } = changedBuild;
  changedBuild.request_root = protocolDigest(changedBuildPreimage);
  assert.throws(() => parseAgentRunRequest(changedBuild), /Attempt runner root/u);
});

test("builds one deterministic non-executable bundle and reports its exact identity", async () => {
  const before = await readFile(helperPath);
  await execFileAsync("bun", ["tooling/build-agent-helper.mjs"], { cwd: packageRoot });
  const first = await readFile(helperPath);
  await execFileAsync("bun", ["tooling/build-agent-helper.mjs"], { cwd: packageRoot });
  const second = await readFile(helperPath);
  assert.deepEqual(first, before);
  assert.deepEqual(second, first);
  assert.notEqual(first.subarray(0, 2).toString("utf8"), "#!");
  assert.equal((await stat(helperPath)).mode & 0o111, 0);

  const diagnostic = await mkdtemp(path.join(os.tmpdir(), "vela-agent-doctor-test-"));
  const fakeCodex = path.join(diagnostic, process.platform === "win32" ? "codex.cmd" : "codex");
  if (process.platform === "win32") {
    await writeFile(fakeCodex, "@echo off\r\nexit /b 1\r\n");
  } else {
    await writeFile(
      fakeCodex,
      [
        "#!/bin/sh",
        'if [ "$1" = "--version" ]; then printf "codex-cli 0.145.0\\n"; exit 0; fi',
        'if [ "$1" = "sandbox" ]; then',
        '  printf "true false false false false false false false false false\\n"',
        "  exit 0",
        "fi",
        "exit 1",
        "",
      ].join("\n"),
    );
    await chmod(fakeCodex, 0o700);
  }
  const doctor = await (async () => {
    try {
      return await execFileAsync("bun", [helperPath, "doctor", "--json"], {
        cwd: packageRoot,
        env: {
          ...process.env,
          PATH: `${diagnostic}${path.delimiter}${process.env.PATH ?? ""}`,
        },
      });
    } finally {
      await rm(diagnostic, { recursive: true, force: true });
    }
  })();
  assert.equal(doctor.stderr, "");
  const report = JSON.parse(doctor.stdout) as {
    ok: boolean;
    authority: string;
    effect: string;
    build: {
      schema: string;
      platform: string;
      runtime: { kind: string; version: string; size: number; sha256: string };
      bundle: { format: string; size: number; sha256: string };
    };
    build_root: string;
    custody: {
      preflight: string;
      mode: string;
      placement: {
        default_output: string;
        suitable: boolean;
        system_temporary_output: string;
      };
    };
  };
  assert.equal(report.ok, true);
  assert.equal(report.authority, "none");
  assert.equal(report.effect, "none");
  assert.equal(report.build.bundle.format, "esm");
  assert.equal(report.build.bundle.size, second.length);
  assert.equal(report.build.bundle.sha256, sha256Bytes(second));
  assert.equal(report.build.runtime.kind, "bun");
  assert.equal(report.build.runtime.version, "1.3.12");
  assert.equal(
    report.custody.preflight,
    process.platform === "win32" ? "wsl2_required" : "passed",
  );
  assert.equal(report.custody.placement.default_output, "local_user_home");
  assert.equal(report.custody.placement.suitable, true);
  assert.equal(report.custody.placement.system_temporary_output, "rejected");

  const runtimeProbe = await execFileAsync("bun", [
    "-e",
    "process.stdout.write(process.execPath)",
  ]);
  const runtimePath = await realpath(runtimeProbe.stdout);
  const runtimeBytes = await readFile(runtimePath);
  assert.equal(report.build.runtime.size, runtimeBytes.length);
  assert.equal(report.build.runtime.sha256, sha256Bytes(runtimeBytes));
  assert.equal(report.build_root, protocolDigest(report.build));
});
