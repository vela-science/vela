import assert from "node:assert/strict";
import { mkdir, mkdtemp } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { BudgetTracker } from "../src/budget/enforce.js";
import type { Mission } from "../src/contracts/mission.js";
import {
  parseCodexEvents,
  summarizeCodexFailure,
  summarizeCodexStructure,
} from "../src/engines/codex-events.js";
import type { CandidateDraft } from "../src/engines/engine.js";
import { FakeEngine } from "../src/engines/fake.js";

const digest = `sha256:${"a".repeat(64)}`;
const draft: CandidateDraft = {
  schema: "canopus.engine-output.v0",
  status: "success",
  claim: "The bounded artifact was produced.",
  artifacts: [
    { path: "result.json", kind: "witness", encoding: "utf8", content: "{\"value\":42}\n" },
  ],
  observations: ["One result was found."],
  caveats: ["Acceptance remains outside this run."],
};

function mission(): Mission {
  return {
    schema: "canopus.mission.v0",
    id: "mission_engine",
    target: "target-1",
    vela_version: "0.800.19",
    vela_sha256: digest,
    frontier: "frontier",
    actor: "agent:canopus-test",
    role: "producer",
    claim_type: "computational",
    replayability: "exact",
    objective: "Produce one result.",
    completion_condition: "A frozen verifier passes.",
    roots: {
      git_commit: "b".repeat(40),
      git_tree: "c".repeat(40),
      vela_event_log: digest,
      vela_snapshot: digest,
    },
    allowed_paths: ["result.json"],
    budgets: {
      max_research_wall_time_ms: 10_000,
      max_research_processes: 4,
      max_research_output_bytes: 1_048_576,
      max_prompt_bytes: 1_048_576,
      max_artifact_bytes: 1_048_576,
      max_attempts: 2,
      max_observed_tokens: 10_000,
    },
    verifier: {
      argv: ["frontier/verifier", "{artifact:result.json}"],
      executable_sha256: digest,
      cwd: "frontier",
      timeout_ms: 1000,
      max_output_bytes: 4096,
      network: "deny",
      writes: "deny",
    },
    scientific_chain: {
      predicted_observable: "The frozen result passes the declared verifier.",
      performed_test: "python3 frozen result",
    },
    landing: { expected_routes: ["defer"], max_accepted_delta: 0 },
  };
}

async function paths() {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-engine-"));
  const value = {
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
  await Promise.all(Object.values(value).slice(1).map((entry) => mkdir(entry)));
  return value;
}

function events(command?: string): string {
  const stream: Array<Record<string, unknown>> = [
    { type: "thread.started", thread_id: "thread-1" },
    { type: "turn.started" },
    {
      type: "item.completed",
      item: { id: "message-1", type: "agent_message", text: "done" },
    },
    {
      type: "turn.completed",
      usage: {
        input_tokens: 100,
        cached_input_tokens: 50,
        output_tokens: 20,
        reasoning_output_tokens: 10,
      },
    },
  ];
  if (command !== undefined) {
    stream.splice(2, 0, {
      type: "item.completed",
      item: { id: "command-1", type: "command_execution", command },
    });
  }
  return stream.map((event) => JSON.stringify(event)).join("\n") + "\n";
}

test("fake engine obeys the same bounded draft contract", async () => {
  const workspace = await paths();
  const activeMission = mission();
  const result = await new FakeEngine(draft).run({
    mission: activeMission,
    briefing: {},
    paths: workspace,
    budget: new BudgetTracker(activeMission.budgets),
  });
  assert.deepEqual(result.draft, draft);
  assert.equal(result.engine.name, "fake");
});

test("Codex event parser rejects unknown, malformed, and custody actions", () => {
  assert.throws(() => parseCodexEvents('{"type":"future.event"}\n'), /unknown type/u);
  assert.throws(
    () => parseCodexEvents(events("vela sign")),
    /forbidden external or custody action/u,
  );
  assert.throws(
    () =>
      parseCodexEvents(
        events("python3 verify.py").replace(
          '"command":"python3 verify.py"',
          '"type":"command_execution"',
        ),
      ),
    /no command text/u,
  );
});

test("Codex failure diagnostics are structured, bounded, and secret-redacted", () => {
  const stream = [
    JSON.stringify({ type: "thread.started", thread_id: "thread-failed" }),
    JSON.stringify({
      type: "turn.failed",
      error: {
        message:
          "Provider rejected api_key=sk-example-secret-123456789 at https://example.test/run?token=private",
      },
    }),
  ].join("\n");
  const diagnostic = summarizeCodexFailure(stream);
  assert.match(diagnostic, /Provider rejected/u);
  assert.match(diagnostic, /redacted/u);
  assert.doesNotMatch(diagnostic, /sk-example/u);
  assert.doesNotMatch(diagnostic, /token=private/u);
  assert.equal(summarizeCodexFailure("not json\n"), "no structured Codex failure event");
  assert.ok(diagnostic.length <= 512);
});

test("Codex structural diagnostics expose no event or item content", () => {
  const secret = "secret-that-must-not-appear";
  const stream = [
    JSON.stringify({ type: "thread.started", thread_id: secret }),
    JSON.stringify({
      type: "item.completed",
      item: { id: secret, type: "command_execution", command: `print ${secret}` },
    }),
    JSON.stringify({ type: secret, item: { type: secret } }),
    "not json",
  ].join("\n");
  const summary = summarizeCodexStructure(stream);
  assert.deepEqual(summary, {
    lines: 4,
    parsed_lines: 3,
    invalid_lines: 1,
    event_types: { "item.completed": 1, other: 1, "thread.started": 1 },
    item_types: { command_execution: 1, other: 1 },
  });
  assert.doesNotMatch(JSON.stringify(summary), new RegExp(secret, "u"));
});
