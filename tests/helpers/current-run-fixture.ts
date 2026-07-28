import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import type { MissionRoots, MissionV1, PositiveResultContractV1 } from "../../src/contracts/mission.js";
import { canonicalJson, contentDigest, sha256Bytes } from "../../src/util/canonical.js";

const digest = `sha256:${"a".repeat(64)}`;

export async function writeCurrentRunFixture(options: {
  root: string;
  artifact: Buffer;
  velaVersion: string;
  velaSha256: string;
  gitCommit: string;
  gitTree: string;
  roots: MissionRoots;
  actor?: string;
  includeExecutionBinding?: boolean;
}): Promise<{ runFile: string; mission: MissionV1 }> {
  const actor = options.actor ?? "agent:canopus-export-fixture";
  const artifactDigest = sha256Bytes(options.artifact);
  const resultContract: PositiveResultContractV1 = {
    schema: "canopus.result-contract.v1",
    target: "fixture:1",
    claim_exact: "The exact bounded fixture returned 42.",
    claim_type: "computational",
    replayability: "exact",
    candidate_status: "success",
    verifier_status: "passed",
    required_artifact_kinds: ["witness"],
  };
  const packetRoot = `sha256:${"b".repeat(64)}`;
  const profileRoot = `sha256:${"c".repeat(64)}`;
  const capsuleRoot = `sha256:${"d".repeat(64)}`;
  const mission: MissionV1 = {
    schema: "canopus.mission.v1",
    id: "mission_submission_export_fixture",
    target: "fixture:1",
    vela_version: options.velaVersion,
    vela_sha256: options.velaSha256,
    frontier: ".",
    actor,
    role: "producer",
    claim_type: "computational",
    replayability: "exact",
    objective: "Produce the exact bounded fixture output.",
    completion_condition: "The frozen verifier accepts the witness.",
    roots: {
      ...options.roots,
      git_commit: options.gitCommit,
      git_tree: options.gitTree,
    },
    allowed_paths: ["result.json"],
    budgets: {
      max_research_wall_time_ms: 60_000,
      max_research_processes: 2,
      max_research_output_bytes: 1_048_576,
      max_prompt_bytes: 1_048_576,
      max_artifact_bytes: 1_048_576,
      max_attempts: 1,
      max_observed_tokens: 10_000,
    },
    scientific_chain: {
      predicted_observable: "The result equals 42.",
      performed_test: "fixture-verifier result.json",
    },
    landing: { expected_routes: ["defer"], max_accepted_delta: 0 },
    target_packet: { path: "packet/target.json", sha256: packetRoot },
    profile: { name: "fixture", root: profileRoot },
    worker: {
      kind: "codex_tools_native",
      platform: "darwin",
      codex_version: "codex-cli 0.145.0",
      codex_sha256: digest,
      permission_profile_path: "contract/config.toml",
      permission_profile_sha256: digest,
      workspace: "target_packet_only",
      output_schema_sha256: digest,
      model: "gpt-5.6",
      network: "provider_only",
      tools: ["shell", "apply_patch"],
    },
    verifier: {
      argv: ["capsule/verifier", "{artifact:result.json}"],
      executable_sha256: capsuleRoot,
      cwd: "site",
      timeout_ms: 30_000,
      max_output_bytes: 1_048_576,
      network: "deny",
      writes: "deny",
      capsule_path: "capsule/verifier",
      capsule_sha256: capsuleRoot,
      image: digest,
    },
    ...(options.includeExecutionBinding === false
      ? {}
      : {
          execution_binding: {
            schema: "vela.execution-binding.v1" as const,
            packet_root: packetRoot,
            profile_root: profileRoot,
            verifier_capsule_root: capsuleRoot,
            result_contract_root: contentDigest(resultContract),
          },
          result_contract: resultContract,
        }),
  };
  const runRoot = path.join(options.root, "run");
  const missionRoot = path.join(options.root, "mission");
  const artifactRoot = path.join(runRoot, "artifacts");
  await Promise.all([
    mkdir(artifactRoot, { recursive: true }),
    mkdir(missionRoot, { recursive: true }),
  ]);
  const run = {
    schema: "canopus.run.v2",
    run_id: "run_export_fixture",
    status: "completed",
    effect: "none",
    authority: "non_authoritative",
    external_gate_credit: false,
    mission: {
      id: mission.id,
      target: mission.target,
      digest: contentDigest(mission),
      starting_roots: mission.roots,
    },
    candidate: {
      digest,
      status: "success",
      claim: "The exact bounded fixture returned 42.",
      artifacts: [{
        path: "result.json",
        kind: "witness",
        digest: artifactDigest,
        bytes: options.artifact.length,
      }],
      caveats: ["This establishes only the exact bounded fixture."],
    },
    verifier: {
      status: "passed",
      sandbox: "macos_sandbox",
      record: {
        argv: ["capsule/verifier", "result.json"],
        executable_digest: capsuleRoot,
        exit_code: 0,
        stdout_digest: digest,
        stderr_digest: digest,
        duration_ms: 1,
      },
    },
    submission: null,
    reproduction: {
      matched: true,
      roots: mission.roots,
      verifier_status: "passed",
      stdout_digest: digest,
      stderr_digest: digest,
    },
    budget: {
      research_elapsed_ms: 1,
      research_processes: 1,
      research_output_bytes: options.artifact.length,
      prompt_bytes: 1,
      artifact_bytes: options.artifact.length,
      attempts: 1,
      observed_tokens: 1,
    },
  };
  await writeFile(path.join(missionRoot, "mission.json"), canonicalJson(mission));
  await writeFile(path.join(runRoot, "run.json"), canonicalJson(run));
  await writeFile(path.join(artifactRoot, artifactDigest.slice(7)), options.artifact);
  return { runFile: path.join(runRoot, "run.json"), mission };
}
