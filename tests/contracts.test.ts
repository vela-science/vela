import assert from "node:assert/strict";
import test from "node:test";

import { parseCandidate } from "../src/contracts/candidate.js";
import { parseMission } from "../src/contracts/mission.js";
import { parseCandidateDraft } from "../src/candidate/validate.js";
import { contentDigest } from "../src/util/canonical.js";

const hex = "a".repeat(64);
const digest = `sha256:${hex}`;

function mission(): Record<string, unknown> {
  return {
    schema: "canopus.mission.v0",
    id: "mission_contract_test",
    target: "target-1",
    vela_version: "0.800.19",
    vela_sha256: digest,
    frontier: "frontier",
    actor: "agent:canopus-test",
    role: "producer",
    claim_type: "computational",
    replayability: "exact",
    objective: "Produce one bounded witness.",
    completion_condition: "The declared verifier exits zero on frozen bytes.",
    roots: {
      git_commit: "b".repeat(40),
      git_tree: "c".repeat(40),
      vela_repository: digest,
    },
    allowed_paths: ["proof/Main.lean", "artifacts/witness.json"],
    budgets: {
      max_research_wall_time_ms: 30_000,
      max_research_processes: 4,
      max_research_output_bytes: 1_048_576,
      max_prompt_bytes: 1_048_576,
      max_artifact_bytes: 4_194_304,
      max_attempts: 2,
      max_observed_tokens: 20_000,
    },
    verifier: {
      argv: ["frontier/verifier", "proof/Main.lean"],
      executable_sha256: digest,
      cwd: "frontier",
      timeout_ms: 20_000,
      max_output_bytes: 1_048_576,
      network: "deny",
      writes: "deny",
    },
    scientific_chain: {
      predicted_observable: "The frozen witness passes the declared finite check.",
      performed_test: "lake env lean proof/Main.lean",
    },
    landing: {
      expected_routes: ["defer"],
      max_accepted_delta: 0,
    },
  };
}

function candidate(): Record<string, unknown> {
  return {
    schema: "canopus.candidate.v0",
    mission_id: "mission_contract_test",
    status: "success",
    claim: "The frozen witness passes the declared finite verifier.",
    artifacts: [
      {
        path: "artifacts/witness.json",
        kind: "witness",
        digest,
        bytes: 128,
      },
    ],
    observations: ["The bounded search returned one candidate."],
    tests: [
      {
        argv: ["verify", "artifacts/witness.json"],
        executable_digest: digest,
        exit_code: 0,
        stdout_digest: digest,
        stderr_digest: digest,
        duration_ms: 10,
      },
    ],
    costs: {
      wall_time_ms: 100,
      attempt: 1,
      input_tokens: 20,
      output_tokens: 30,
    },
    caveats: ["This result does not establish optimality or acceptance."],
    engine: {
      name: "fake",
      version: "1",
      binary_sha256: null,
      model: null,
      configuration_sha256: digest,
    },
  };
}

function missionV1(): Record<string, unknown> {
  return {
    ...mission(),
    schema: "canopus.mission.v1",
    target_packet: { path: "packets/target.json", sha256: digest },
    worker: {
      kind: "codex_tools_native",
      platform: "darwin",
      codex_version: "codex-cli 0.144.5",
      codex_sha256: digest,
      permission_profile_path: "contract/native-worker.config.toml",
      permission_profile_sha256: digest,
      workspace: "target_packet_only",
      output_schema_sha256: digest,
      model: "gpt-5.2-codex",
      network: "provider_only",
      tools: ["shell", "apply_patch"],
    },
    verifier: {
      ...(mission().verifier as object),
      capsule_path: "capsules/verifier",
      capsule_sha256: digest,
      image: digest,
    },
  };
}

function permitMissionV1(): Record<string, unknown> {
  const input = missionV1();
  const profileRoot = `sha256:${"b".repeat(64)}`;
  const resultContract = {
    schema: "canopus.result-contract.v1",
    target: "target-1",
    claim_exact: "The exact positive claim.",
    claim_type: "computational",
    replayability: "exact",
    candidate_status: "success",
    verifier_status: "passed",
    required_artifact_kinds: ["witness"],
  };
  input.landing = { expected_routes: ["permit"], max_accepted_delta: 1 };
  input.profile = { name: "sidon-a24-improve", root: profileRoot };
  input.result_contract = resultContract;
  input.execution_binding = {
    schema: "vela.execution-binding.v1",
    packet_root: digest,
    profile_root: profileRoot,
    verifier_capsule_root: digest,
    result_contract_root: contentDigest(resultContract),
  };
  return input;
}

test("mission v0 round-trips a bounded exact-root contract", () => {
  assert.deepEqual(parseMission(mission()), mission());
});

test("mission parser accepts an attested Vela prerelease identity", () => {
  const input = mission();
  input.vela_version = "0.930.0-rc.12";
  assert.equal(parseMission(input).vela_version, "0.930.0-rc.12");
});

test("mission parser rejects an incomplete Vela prerelease identity", () => {
  const input = mission();
  input.vela_version = "0.930.0-";
  assert.throws(() => parseMission(input), /vela_version has an invalid format/u);
});

test("mission v1 round-trips a zero-delta tool-worker contract", () => {
  assert.deepEqual(parseMission(missionV1()), missionV1());
});

test("mission v1 optionally binds the exact producer profile for old-record replay", () => {
  const historical = missionV1();
  assert.equal((parseMission(historical) as { profile?: unknown }).profile, undefined);

  const current = missionV1();
  current.profile = { name: "formal-erdos-505", root: digest };
  assert.deepEqual(parseMission(current), current);
});

test("mission v1 rejects unbound Permit", () => {
  const permit = missionV1();
  permit.landing = { expected_routes: ["permit"], max_accepted_delta: 1 };
  assert.throws(() => parseMission(permit), /Permit requires an exact execution binding/u);
});

test("mission v1 Permit is exact-root-bound and substitution fails closed", () => {
  const intended = permitMissionV1();
  assert.deepEqual(parseMission(intended), intended);

  const packet = structuredClone(intended);
  (packet.execution_binding as { packet_root: string }).packet_root =
    `sha256:${"c".repeat(64)}`;
  assert.throws(() => parseMission(packet), /does not match its retained packet/u);

  const capsule = structuredClone(intended);
  (capsule.execution_binding as { verifier_capsule_root: string }).verifier_capsule_root =
    `sha256:${"d".repeat(64)}`;
  assert.throws(() => parseMission(capsule), /does not match its retained packet/u);

  const contract = structuredClone(intended);
  (contract.result_contract as { target: string }).target = "target-2";
  assert.throws(() => parseMission(contract), /does not match its retained packet/u);

  const profile = structuredClone(intended);
  (profile.profile as { root: string }).root = `sha256:${"e".repeat(64)}`;
  assert.throws(() => parseMission(profile), /retained producer profile/u);
});

test("mission v1 rejects missing capsules and widened worker tools", () => {
  const missing = missionV1();
  delete (missing.verifier as { capsule_path?: string }).capsule_path;
  assert.throws(() => parseMission(missing), /capsule_path must be a string/u);

  const tools = missionV1();
  (tools.worker as { tools: string[] }).tools = ["shell", "apply_patch", "browser"];
  assert.throws(() => parseMission(tools), /tools length must be 2\.\.2/u);
});

test("mission rejects unknown fields", () => {
  const input = mission();
  input.verdict = "accepted";
  assert.throws(() => parseMission(input), /verdict is not allowed/u);
});

test("mission rejects short roots", () => {
  const input = mission();
  input.roots = { ...(input.roots as object), vela_repository: "sha256:abcd" };
  assert.throws(() => parseMission(input), /vela_repository has an invalid format/u);
});

test("mission rejects path escape and a shell-bearing verifier executable", () => {
  const escaped = mission();
  escaped.allowed_paths = ["../human-key"];
  assert.throws(() => parseMission(escaped), /remain below its root/u);

  const shell = mission();
  (shell.verifier as { argv: string[] }).argv[0] = "vela sign";
  assert.throws(() => parseMission(shell), /relative verifier capsule without shell syntax/u);
});

test("candidate v0 round-trips frozen evidence", () => {
  assert.deepEqual(parseCandidate(candidate()), candidate());
});

test("candidate has no verdict or authority field", () => {
  const input = candidate();
  input.verdict = "accepted";
  assert.throws(() => parseCandidate(input), /verdict is not allowed/u);
});

test("candidate rejects unfrozen artifacts", () => {
  const input = candidate();
  input.artifacts = [
    { path: "artifacts/witness.json", kind: "witness", digest: "sha256:abcd", bytes: 1 },
  ];
  assert.throws(() => parseCandidate(input), /digest has an invalid format/u);
});

test("engine and frozen candidate artifact caps fit one authored Vela command", () => {
  const artifact = {
    path: "artifact.json",
    kind: "witness",
    encoding: "utf8",
    content: "{}\n",
  };
  assert.throws(
    () => parseCandidateDraft({
      schema: "canopus.engine-output.v0",
      status: "success",
      claim: "bounded",
      artifacts: Array.from({ length: 9 }, (_, index) => ({
        ...artifact,
        path: `artifact-${index}.json`,
      })),
      observations: [],
      caveats: [],
    }),
    /artifacts length must be 0\.\.8/u,
  );
  const oversized = candidate();
  oversized.artifacts = Array.from({ length: 11 }, (_, index) => ({
    path: `artifact-${index}.json`, kind: "witness", digest, bytes: 1,
  }));
  assert.throws(() => parseCandidate(oversized), /artifacts length must be 0\.\.10/u);
});

test("candidate repairs name an immutable parent", () => {
  const input = candidate();
  input.repair = { parent_candidate: digest, reason: "Verifier rejected the first form." };
  assert.deepEqual(parseCandidate(input), input);
});
