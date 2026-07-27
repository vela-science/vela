import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, readFile, readdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import type { MissionV1, PositiveResultContractV1 } from "../src/contracts/mission.js";
import { exportSubmission, verifySubmission, type SubmissionV1 } from "../src/product/submission.js";
import { submitBundle } from "../src/product/submit.js";
import { canonicalJson, contentDigest, protocolDigest, sha256Bytes } from "../src/util/canonical.js";

const digest = `sha256:${"a".repeat(64)}`;

function fixtureMission(artifactDigest: string): MissionV1 {
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
  return {
    schema: "canopus.mission.v1",
    id: "mission_submission_export_fixture",
    target: "fixture:1",
    vela_version: "0.930.0-rc.12",
    vela_sha256: artifactDigest,
    frontier: ".",
    actor: "agent:canopus-export-fixture",
    role: "producer",
    claim_type: "computational",
    replayability: "exact",
    objective: "Produce the exact bounded fixture output.",
    completion_condition: "The frozen verifier accepts the witness.",
    roots: {
      git_commit: "e".repeat(40),
      git_tree: "f".repeat(40),
      vela_event_log: digest,
      vela_snapshot: digest,
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
    strict_baseline: {
      status: "pass",
      blocker_count: 0,
      blockers_root: digest,
      rule_counts: [],
    },
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
    execution_binding: {
      schema: "vela.execution-binding.v1",
      packet_root: packetRoot,
      profile_root: profileRoot,
      verifier_capsule_root: capsuleRoot,
      result_contract_root: contentDigest(resultContract),
    },
    result_contract: resultContract,
  };
}

test("export creates an authenticated portable Submission without mutating Vela", async () => {
  const home = await mkdtemp(path.join(os.tmpdir(), "canopus-export-home-"));
  const product = path.join(home, "product");
  const runRoot = path.join(product, "run");
  const missionRoot = path.join(product, "mission");
  const artifactRoot = path.join(runRoot, "artifacts");
  await Promise.all([
    mkdir(artifactRoot, { recursive: true }),
    mkdir(missionRoot, { recursive: true }),
  ]);
  const artifact = Buffer.from("{\"value\":42}\n");
  const artifactDigest = sha256Bytes(artifact);
  const mission = fixtureMission(artifactDigest);
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
      digest: digest,
      status: "success",
      claim: "The exact bounded fixture returned 42.",
      artifacts: [{
        path: "result.json",
        kind: "witness",
        digest: artifactDigest,
        bytes: artifact.length,
      }],
      caveats: ["This establishes only the exact bounded fixture."],
    },
    verifier: { status: "passed", sandbox: {}, record: {} },
    submission: null,
    reproduction: {
      matched: true,
      roots: mission.roots,
      verifier_status: "passed",
      stdout_digest: digest,
      stderr_digest: digest,
    },
    budget: {},
  };
  await writeFile(path.join(missionRoot, "mission.json"), canonicalJson(mission));
  await writeFile(path.join(runRoot, "run.json"), canonicalJson(run));
  await writeFile(path.join(artifactRoot, artifactDigest.slice(7)), artifact);
  const output = path.join(home, "submission");
  const result = await exportSubmission({
    runFile: path.join(runRoot, "run.json"),
    outputRoot: output,
    now: new Date("2026-07-26T12:00:00Z"),
  });
  const submission = JSON.parse(await readFile(path.join(output, "submission.json"), "utf8")) as SubmissionV1;
  verifySubmission(submission);
  assert.equal(protocolDigest(submission), result.submission_root);
  assert.equal(submission.provenance.source_run, "run_export_fixture");
  assert.equal(submission.producer_checks.length, 0);
  assert.equal(submission.artifacts[0]?.path, `records/artifacts/sha256/${artifactDigest.slice(7)}`);
  assert.equal(
    (await readdir(output, { recursive: true })).some((entry) => String(entry).includes("private-key")),
    false,
  );

  const manifestFile = path.join(output, "manifest.json");
  const manifest = JSON.parse(await readFile(manifestFile, "utf8")) as {
    artifacts: Array<{ source: string }>;
  };
  manifest.artifacts[0]!.source = "../escape";
  await chmod(manifestFile, 0o600);
  await writeFile(manifestFile, canonicalJson(manifest));
  const frontier = path.join(home, "frontier");
  await mkdir(frontier);
  await assert.rejects(
    submitBundle({ bundle: output, frontier }),
    /safe relative POSIX path/u,
  );
});
