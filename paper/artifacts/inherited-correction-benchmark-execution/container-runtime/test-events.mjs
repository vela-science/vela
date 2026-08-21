import assert from "node:assert/strict";
import { inspectEvents } from "./run-once.mjs";

const stream = (item, usage = { input_tokens: 90000, cached_input_tokens: 80000, output_tokens: 100, reasoning_output_tokens: 20 }) => Buffer.from([
  JSON.stringify({ type: "thread.started", thread_id: "test" }),
  JSON.stringify({ type: "turn.started" }),
  JSON.stringify({ type: "item.completed", item }),
  JSON.stringify({ type: "turn.completed", usage }),
].join("\n") + "\n");

const valid = inspectEvents(stream({ id: "one", type: "agent_message", text: "{}" }));
assert.equal(valid.response_count, 1);
assert.equal(valid.tool_calls, 0);
assert.equal(valid.usage.input_tokens, 90000, "cumulative input is telemetry, not invalidation");

assert.throws(() => inspectEvents(stream({ id: "one", type: "command_execution" })), /forbidden_provider_event/);
assert.throws(() => inspectEvents(stream({ id: "one", type: "agent_message" }, { input_tokens: 1, cached_input_tokens: 0, output_tokens: 8193 })), /output_token_ceiling/);
assert.throws(() => inspectEvents(Buffer.from(stream({ id: "one", type: "agent_message" })).toString().replace("turn.completed", "turn.compacted")), /forbidden_provider_event/);

process.stdout.write("event contract tests passed\n");
