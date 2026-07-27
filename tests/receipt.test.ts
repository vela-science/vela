import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { freezeArtifact, type FrozenArtifactLocation } from "../src/artifact/freeze.js";
import type { Candidate } from "../src/contracts/candidate.js";
import { parseMission, type Mission } from "../src/contracts/mission.js";
import {
  finalizeCandidate,
  installFrozenArtifacts,
  mapCandidateToReceipt,
} from "../src/receipt/map.js";
import type { EngineResult } from "../src/engines/engine.js";
import type { VerifierOutcome } from "../src/verifier/run.js";
import { contentDigest } from "../src/util/canonical.js";

const digest = `sha256:${"a".repeat(64)}`;
const empty = `sha256:${"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}`;

function mission(): Mission {
  return {
    schema: "canopus.mission.v0",
    id: "mission_receipt",
    target: "target-1",
    vela_version: "0.800.19",
    vela_sha256: digest,
    frontier: "frontier",
    actor: "agent:canopus-test",
    role: "producer",
    claim_type: "computational",
    replayability: "exact",
    objective: "Produce one witness.",
    completion_condition: "The verifier exits zero.",
    roots: {
      git_commit: "b".repeat(40), git_tree: "c".repeat(40),
      vela_event_log: digest, vela_snapshot: digest,
    },
    allowed_paths: ["artifacts/witness.json"],
    budgets: {
      max_research_wall_time_ms: 1000, max_research_processes: 4,
      max_research_output_bytes: 4096, max_prompt_bytes: 4096,
      max_artifact_bytes: 4096, max_attempts: 1, max_observed_tokens: 1000,
    },
    verifier: {
      argv: ["frontier/verifier", "{artifact:artifacts/witness.json}"], executable_sha256: digest, cwd: "frontier",
      timeout_ms: 1000, max_output_bytes: 4096, network: "deny", writes: "deny",
    },
    scientific_chain: {
      predicted_observable: "The frozen witness passes the exact verifier.",
      performed_test: "verify frozen witness",
    },
    landing: { expected_routes: ["defer"], max_accepted_delta: 0 },
  };
}

function exactPermitMission(): Mission {
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
  return parseMission({
    ...mission(),
    schema: "canopus.mission.v1",
    target_packet: { path: "packet/target.json", sha256: digest },
    strict_baseline: {
      status: "pass",
      blocker_count: 0,
      blockers_root: digest,
      rule_counts: [],
    },
    worker: {
      kind: "codex_tools_native",
      platform: "darwin",
      codex_version: "codex-cli 0.144.6",
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
      ...mission().verifier,
      capsule_path: "capsule/verifier",
      capsule_sha256: digest,
      image: digest,
    },
    landing: { expected_routes: ["permit"], max_accepted_delta: 1 },
    result_contract: resultContract,
    execution_binding: {
      schema: "vela.execution-binding.v1",
      packet_root: digest,
      profile_root: `sha256:${"b".repeat(64)}`,
      verifier_capsule_root: digest,
      result_contract_root: contentDigest(resultContract),
    },
  });
}

function verifier(status: VerifierOutcome["status"] = "passed"): VerifierOutcome {
  return {
    status,
    record: {
      argv: ["verify", "frozen"],
      executable_digest: digest,
      exit_code: status === "passed" ? 0 : 1,
      stdout_digest: empty,
      stderr_digest: empty,
      duration_ms: 1,
    },
    sandbox: "macos_sandbox",
    ...(status === "error" ? { error: "timeout" } : {}),
  };
}

function engine(): EngineResult {
  return {
    draft: {
      schema: "canopus.engine-output.v0",
      status: "success",
      claim: "The bounded witness satisfies the declared predicate.",
      artifacts: [
        {
          path: "artifacts/witness.json",
          kind: "witness",
          encoding: "utf8",
          content: "{\"witness\":true}\n",
        },
      ],
      observations: ["One witness was produced."],
      caveats: ["No optimality claim is made."],
    },
    engine: {
      name: "fake",
      version: "1",
      binary_sha256: null,
      model: null,
      configuration_sha256: digest,
    },
    usage: { input_tokens: 1, cached_input_tokens: 0, output_tokens: 2, reasoning_output_tokens: 0 },
    wallTimeMs: 3,
    eventTypes: ["fake.completed"], actionTypes: [], eventsDigest: digest, stderrDigest: empty,
  };
}

function frozen(): FrozenArtifactLocation[] {
  return [{
    artifact: { path: "artifacts/witness.json", kind: "witness", digest, bytes: 10 },
    frozenPath: "/frozen/a",
  }];
}

test("verified candidate maps to scientific-chain Receipt v1 authoring", () => {
  const candidate = finalizeCandidate({
    mission: mission(), engine: engine(), frozen: frozen(), verifier: verifier(),
    budget: {
      research_elapsed_ms: 5, research_processes: 2, research_output_bytes: 2,
      prompt_bytes: 1, artifact_bytes: 10, attempts: 1, observed_tokens: 3,
    },
  });
  const input = mapCandidateToReceipt(mission(), candidate, verifier(), "target-1");
  assert.equal(input.predictedObservable, "The frozen witness passes the exact verifier.");
  assert.equal(input.performedTest, "verify frozen witness");
  assert.deepEqual(input.evidence, [
    `artifact:${digest}`,
    `verifier_executable:${digest}`,
    `verifier_stdout:${empty}`,
    `verifier_stderr:${empty}`,
  ]);
  assert.deepEqual(input.counterevidence, []);
  assert.equal(input.work, "target-1");
});

test("post-verifier candidate resolves worker-time pending language", () => {
  const pendingEngine = engine();
  pendingEngine.draft.caveats = [
    "Verification by the separate frozen verifier remains pending after producer exit.",
  ];
  const outcome = verifier();
  const candidate = finalizeCandidate({
    mission: mission(), engine: pendingEngine, frozen: frozen(), verifier: outcome,
    budget: {
      research_elapsed_ms: 5, research_processes: 2, research_output_bytes: 2,
      prompt_bytes: 1, artifact_bytes: 10, attempts: 1, observed_tokens: 3,
    },
  });
  assert.equal(candidate.caveats.some((caveat) => /pending/iu.test(caveat)), false);
  assert.equal(candidate.caveats.includes(
    "The worker handed off without verifier authority; Canopus subsequently recorded the separate verifier outcome.",
  ), true);
  const input = mapCandidateToReceipt(mission(), candidate, outcome);
  assert.equal(input.caveats.some((caveat) => /pending/iu.test(caveat)), false);
  assert.equal(input.caveats.includes("Declared verifier outcome: passed."), true);
});

test("failed verifier cannot remain a success candidate", () => {
  const outcome = verifier("failed");
  const candidate = finalizeCandidate({
    mission: mission(), engine: engine(), frozen: frozen(), verifier: outcome,
    budget: {
      research_elapsed_ms: 5, research_processes: 2, research_output_bytes: 2,
      prompt_bytes: 1, artifact_bytes: 10, attempts: 1, observed_tokens: 3,
    },
  });
  assert.equal(candidate.status, "failed");
  assert.match(candidate.claim, /did not pass/u);
  const input = mapCandidateToReceipt(mission(), candidate, outcome);
  assert.deepEqual(input.counterevidence, ["verifier_outcome:failed"]);
});

test("exact Permit candidate emits the registered Vela execution binding", () => {
  const exact = exactPermitMission();
  const outcome = verifier();
  const candidate = finalizeCandidate({
    mission: exact,
    engine: engine(),
    frozen: frozen(),
    verifier: outcome,
    budget: {
      research_elapsed_ms: 5,
      research_processes: 2,
      research_output_bytes: 2,
      prompt_bytes: 1,
      artifact_bytes: 10,
      attempts: 1,
      observed_tokens: 3,
    },
  });
  const input = mapCandidateToReceipt(exact, candidate, outcome, exact.target);
  assert.equal(candidate.claim, "The exact positive claim.");
  assert.equal(input.claim, "The exact positive claim.");
  assert.deepEqual(
    input.executionBinding,
    exact.schema === "canopus.mission.v1" ? exact.execution_binding : undefined,
  );

  const substituted = { ...candidate, claim: "A broader claim the verifier did not establish." };
  assert.throws(
    () => mapCandidateToReceipt(exact, substituted, outcome, exact.target),
    /does not satisfy the exact positive result contract/u,
  );
});

test("frozen artifacts install into the separate landing clone", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-receipt-"));
  const output = path.join(root, "output");
  const store = path.join(root, "store");
  const landing = path.join(root, "landing");
  await Promise.all([mkdir(path.join(output, "artifacts"), { recursive: true }), mkdir(path.join(landing, "frontier"), { recursive: true })]);
  await writeFile(path.join(output, "artifacts/witness.json"), "{\"ok\":true}\n");
  const item = await freezeArtifact({
    sourceRoot: output, artifactRoot: store, path: "artifacts/witness.json", kind: "witness", maxBytes: 4096,
  });
  const installed = await installFrozenArtifacts({
    landingRepo: landing, frontier: "frontier", frozen: [item], maxBytes: 4096,
  });
  assert.equal(await readFile(installed[0] as string, "utf8"), "{\"ok\":true}\n");
  await chmod(installed[0] as string, 0o600);
  await writeFile(installed[0] as string, "tampered");
  await assert.rejects(
    installFrozenArtifacts({ landingRepo: landing, frontier: "frontier", frozen: [item], maxBytes: 4096 }),
    /does not match frozen bytes/u,
  );
});
