import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { parseMission, type MissionV1 } from "../src/contracts/mission.js";
import {
  packetFromTarget,
  parseVelaVersionOutput,
  assertVerifierWorkingDirectory,
  selectedProducerTarget,
  validateMissionBundle,
} from "../src/mission/prepare.js";
import { canonicalJson, contentDigest, sha256Bytes } from "../src/util/canonical.js";

const digest = `sha256:${"a".repeat(64)}`;

test("verifier cwd must exist below the sealed source before a model call", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "vela-agent-verifier-cwd-"));
  await mkdir(path.join(root, "targets"));
  await assertVerifierWorkingDirectory(root, "targets");
  await assert.rejects(
    assertVerifierWorkingDirectory(root, "site"),
    /does not exist in the sealed source checkout/u,
  );
  await symlink(os.tmpdir(), path.join(root, "escape"));
  await assert.rejects(
    assertVerifierWorkingDirectory(root, "escape"),
    /not a real directory below the sealed source checkout/u,
  );
});

test("mission preparation accepts the exact stable and prerelease Vela identities", () => {
  assert.equal(parseVelaVersionOutput("vela 0.930.0"), "0.930.0");
  assert.equal(parseVelaVersionOutput("vela 0.930.0-rc.12"), "0.930.0-rc.12");
  assert.throws(() => parseVelaVersionOutput("vela latest"), /invalid version/u);
  assert.throws(() => parseVelaVersionOutput("vela 0.930.0 rc.9"), /invalid version/u);
});

test("mission preparation consumes only the current Vela producer offer", () => {
  const producer = {
    lane: "produce",
    target_id: "fixture:bounded-check",
    packet: {
      schema: "fixture.work.v1",
      path: "packet/target.json",
      sha256: digest,
    },
  };
  assert.equal(
    selectedProducerTarget({
      targets: [
        { lane: "review", target_id: "vpr_fixture" },
        producer,
      ],
    }),
    producer,
  );
  assert.equal(
    selectedProducerTarget({ targets: [producer] }, "fixture:bounded-check"),
    producer,
  );
  assert.deepEqual(
    packetFromTarget(producer, {
      target: "fixture:bounded-check",
      schema: "fixture.work.v1",
    }),
    {
      path: "packet/target.json",
      sha256: digest,
    },
  );
  assert.throws(
    () => selectedProducerTarget({
      targets: [{ lane: "attack", target_id: "legacy:target" }],
    }),
    /no producer target/u,
  );
  assert.throws(
    () => packetFromTarget({
      lane: "produce",
      target_id: "erdos:1056",
      task: { packet_ref: producer.packet },
    }),
    /producer packet/u,
  );
});

function mission(
  capsuleDigest: string,
  packetDigest: string,
  outputSchemaDigest: string,
  permissionProfileDigest: string,
): MissionV1 {
  return parseMission({
    schema: "canopus.mission.v1",
    id: "mission_v1_bundle",
    target: "fixture:bounded-check",
    vela_version: "0.800.23",
    vela_sha256: digest,
    frontier: ".",
    actor: "agent:canopus",
    role: "producer",
    claim_type: "computational",
    replayability: "exact",
    objective: "Evaluate one exact finite synthetic obligation.",
    completion_condition: "The frozen capsule verifies the exact artifact.",
    roots: {
      git_commit: "b".repeat(40),
      git_tree: "c".repeat(40),
      vela_repository: digest,
    },
    target_packet: { path: "packet/target.json", sha256: packetDigest },
    allowed_paths: ["artifacts/result.json"],
    budgets: {
      max_research_wall_time_ms: 60_000,
      max_research_processes: 4,
      max_research_output_bytes: 1_048_576,
      max_prompt_bytes: 1_048_576,
      max_artifact_bytes: 1_048_576,
      max_attempts: 1,
      max_observed_tokens: 20_000,
    },
    worker: {
      kind: "codex_tools_native",
      platform: "darwin",
      codex_version: "codex-cli 0.144.5",
      codex_sha256: digest,
      permission_profile_path: "contract/native-worker.config.toml",
      permission_profile_sha256: permissionProfileDigest,
      workspace: "target_packet_only",
      output_schema_sha256: outputSchemaDigest,
      model: "gpt-5.2-codex",
      network: "provider_only",
      tools: ["shell", "apply_patch"],
    },
    verifier: {
      argv: ["capsule/verifier", "{artifact:artifacts/result.json}"],
      executable_sha256: capsuleDigest,
      cwd: "site",
      timeout_ms: 30_000,
      max_output_bytes: 1_048_576,
      network: "deny",
      writes: "deny",
      capsule_path: "capsule/verifier",
      capsule_sha256: capsuleDigest,
      image: digest,
    },
    scientific_chain: {
      predicted_observable: "The exact synthetic result matches the bounded packet.",
      performed_test: "capsule/verifier artifacts/result.json",
    },
    landing: { expected_routes: ["defer"], max_accepted_delta: 0 },
  }) as MissionV1;
}

test("mission v1 validates one portable exact-byte bundle and detects drift", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-mission-v1-"));
  await Promise.all([
    mkdir(path.join(root, "capsule")),
    mkdir(path.join(root, "packet")),
    mkdir(path.join(root, "contract")),
  ]);
  const capsuleBytes = Buffer.from("#!/usr/bin/env python3\nraise SystemExit(0)\n");
  const packetBytes = Buffer.from("{\"task\":\"bounded-check\"}\n");
  const outputSchemaBytes = Buffer.from("{\"type\":\"object\"}\n");
  const permissionProfileBytes = Buffer.from(
    'default_permissions = "vela-agent-worker"\n[permissions.vela-agent-worker.filesystem]\n":minimal" = "read"\n',
  );
  const capsule = path.join(root, "capsule", "verifier");
  await Promise.all([
    writeFile(capsule, capsuleBytes, { mode: 0o555 }),
    writeFile(path.join(root, "packet", "target.json"), packetBytes),
    writeFile(path.join(root, "contract", "engine-output.v0.json"), outputSchemaBytes),
    writeFile(path.join(root, "contract", "native-worker.config.toml"), permissionProfileBytes),
  ]);
  await chmod(capsule, 0o555);
  const active = mission(
    sha256Bytes(capsuleBytes),
    sha256Bytes(packetBytes),
    sha256Bytes(outputSchemaBytes),
    sha256Bytes(permissionProfileBytes),
  );
  await writeFile(path.join(root, "mission.json"), canonicalJson(active));
  await writeFile(path.join(root, "bundle-manifest.json"), canonicalJson({
    schema: "canopus.mission-bundle.v1",
    authority: "non_authoritative",
    mission_sha256: contentDigest(active),
  }));
  await validateMissionBundle(active, root);

  await writeFile(path.join(root, "packet", "target.json"), "{\"task\":\"drifted\"}\n");
  await assert.rejects(validateMissionBundle(active, root), /target packet drifted/u);
});

test("Mission v1 accepts only explicit Linux verifier platforms", () => {
  const active = mission(digest, digest, digest, digest);
  const bound = structuredClone(active) as MissionV1;
  bound.verifier.platform = "linux/amd64";
  const reparsed = parseMission(bound);
  assert.equal(reparsed.schema, "canopus.mission.v1");
  assert.equal((reparsed as MissionV1).verifier.platform, "linux/amd64");

  const invalid = structuredClone(bound) as unknown as Record<string, unknown>;
  (invalid.verifier as Record<string, unknown>).platform = "linux/s390x";
  assert.throws(() => parseMission(invalid), /mission\.verifier\.platform/u);
});
