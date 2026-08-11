#!/usr/bin/env node
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { zstdDecompressSync } from "node:zlib";
import { loadFrozenOAuth } from "./auth-preflight.mjs";
import { runParticipant } from "./participant.mjs";

class CaptureContractError extends Error {
  constructor(message) {
    super(message);
    this.name = "CaptureContractError";
  }
}

function requireCapture(condition, message) {
  if (!condition) throw new CaptureContractError(message);
}

function argumentMap(argv) {
  const result = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    requireCapture(argv[index]?.startsWith("--") && argv[index + 1] !== undefined, "arguments must be --name value pairs");
    requireCapture(!result.has(argv[index]), `duplicate argument: ${argv[index]}`);
    result.set(argv[index], argv[index + 1]);
  }
  return result;
}

function rawRoot(data) {
  return `sha256:${createHash("sha256").update(data).digest("hex")}`;
}

function exactKeys(value, keys, label) {
  requireCapture(value !== null && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  requireCapture(JSON.stringify(Object.keys(value).sort()) === JSON.stringify([...keys].sort()), `${label} key set drifted`);
}

function exactUserInput(body, expected) {
  requireCapture(Array.isArray(body.input) && body.input.length === 1, "request must contain exactly one input message");
  const message = body.input[0];
  exactKeys(message, ["role", "content"], "input message");
  requireCapture(message.role === "user", "request input role must be user");
  requireCapture(Array.isArray(message.content) && message.content.length === 1, "user input must contain exactly one content part");
  exactKeys(message.content[0], ["type", "text"], "input text part");
  requireCapture(message.content[0].type === "input_text" && message.content[0].text === expected, "user input text drifted");
}

function sseResponse() {
  const answer = "{}";
  const events = [
    { type: "response.created", response: { id: "resp_capture", status: "in_progress" } },
    {
      type: "response.output_item.added",
      output_index: 0,
      item: { id: "msg_capture", type: "message", role: "assistant", status: "in_progress", content: [], phase: "final_answer" },
    },
    { type: "response.output_text.delta", output_index: 0, content_index: 0, delta: answer },
    {
      type: "response.output_item.done",
      output_index: 0,
      item: {
        id: "msg_capture",
        type: "message",
        role: "assistant",
        status: "completed",
        phase: "final_answer",
        content: [{ type: "output_text", text: answer, annotations: [] }],
      },
    },
    {
      type: "response.completed",
      response: {
        id: "resp_capture",
        status: "completed",
        output: [],
        usage: {
          input_tokens: 1,
          input_tokens_details: { cached_tokens: 0 },
          output_tokens: 1,
          output_tokens_details: { reasoning_tokens: 0 },
          total_tokens: 2,
        },
      },
    },
  ];
  const stream = `${events.map((event) => `data: ${JSON.stringify(event)}\n\n`).join("")}data: [DONE]\n\n`;
  return new Response(stream, { status: 200, headers: { "content-type": "text/event-stream" } });
}

export async function captureRequest({ requestPath, authPath }) {
  let syntheticCredential;
  try {
    syntheticCredential = loadFrozenOAuth(authPath);
  } catch (error) {
    if (error instanceof CaptureContractError) throw error;
    throw new CaptureContractError("capture fixture cannot be read");
  }
  const calls = [];
  const originalFetch = globalThis.fetch;
  const originalEnvironment = new Map(
    ["OPENAI_BASE_URL", "HTTPS_PROXY"].map((name) => [name, Object.hasOwn(process.env, name) ? process.env[name] : undefined]),
  );
  process.env.OPENAI_BASE_URL = "https://ambient.invalid";
  process.env.HTTPS_PROXY = "http://ambient.invalid";
  globalThis.fetch = async (url, init) => {
    const headers = new Headers(init?.headers);
    const encoded = Buffer.from(await new Response(init?.body).arrayBuffer());
    const decoded = headers.get("content-encoding") === "zstd" ? zstdDecompressSync(encoded) : encoded;
    calls.push({
      url: String(url),
      method: init?.method,
      headers,
      encoded,
      decoded,
      body: JSON.parse(decoded.toString("utf8")),
    });
    return sseResponse();
  };
  let result;
  try {
    result = await runParticipant({ requestPath, authPath, captureMode: true });
  } finally {
    globalThis.fetch = originalFetch;
    for (const [name, value] of originalEnvironment) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
  }
  requireCapture(calls.length === 1, "capture must observe exactly one fetch");
  const request = result.request;
  const call = calls[0];
  const body = call.body;
  exactKeys(
    body,
    ["model", "store", "stream", "instructions", "input", "text", "include", "prompt_cache_key", "tool_choice", "parallel_tool_calls", "reasoning"],
    "provider request body",
  );
  exactKeys(body.text, ["verbosity"], "text options");
  exactKeys(body.reasoning, ["effort", "summary"], "reasoning options");
  requireCapture(call.url === "https://chatgpt.com/backend-api/codex/responses", "provider URL drifted");
  requireCapture(call.method === "POST", "provider method drifted");
  requireCapture(call.headers.get("content-encoding") === "zstd", "request must be zstd encoded");
  const expectedHeaders = [
    "accept",
    "authorization",
    "chatgpt-account-id",
    "content-encoding",
    "content-type",
    "openai-beta",
    "originator",
    "session-id",
    "user-agent",
    "x-client-request-id",
  ];
  requireCapture(JSON.stringify([...call.headers.keys()].sort()) === JSON.stringify(expectedHeaders), "provider header-name set drifted");
  requireCapture(call.headers.get("authorization") === `Bearer ${syntheticCredential.access}`, "synthetic OAuth bearer header drifted");
  requireCapture(call.headers.get("chatgpt-account-id") === syntheticCredential.accountId, "synthetic account header drifted");
  requireCapture(call.headers.get("accept") === "text/event-stream", "accept header drifted");
  requireCapture(call.headers.get("content-type") === "application/json", "content-type header drifted");
  requireCapture(call.headers.get("openai-beta") === "responses=experimental", "OpenAI beta header drifted");
  requireCapture(call.headers.get("originator") === "pi", "originator header drifted");
  requireCapture(call.headers.get("session-id") === request.session_id && call.headers.get("x-client-request-id") === request.session_id, "session headers drifted");
  requireCapture(/^pi \(linux [^;]+; x64\)$/u.test(call.headers.get("user-agent") ?? ""), "reviewed Pi linux/amd64 user-agent policy drifted");
  requireCapture(body.model === "gpt-5.6-sol", "request model drifted");
  requireCapture(body.store === false && body.stream === true, "store/stream contract drifted");
  requireCapture(body.instructions === `${request.system_prompt}\nCurrent working directory: /workspace`, "request instructions drifted");
  exactUserInput(body, request.user_message);
  requireCapture(body.text?.verbosity === "low", "text verbosity drifted");
  requireCapture(JSON.stringify(body.include) === JSON.stringify(["reasoning.encrypted_content"]), "response include set drifted");
  requireCapture(body.reasoning?.effort === "high" && body.reasoning?.summary === "auto", "reasoning contract drifted");
  requireCapture(body.prompt_cache_key === request.session_id, "prompt cache key drifted");
  requireCapture(body.tool_choice === "auto" && body.parallel_tool_calls === true, "provider no-tool defaults drifted");
  requireCapture(!Object.hasOwn(body, "tools"), "request must not contain tools");
  requireCapture(!Object.hasOwn(body, "previous_response_id"), "request must not continue a prior response");
  requireCapture(result.answer === "{}", "capture response drifted");
  requireCapture(result.stats.userMessages === 1 && result.stats.assistantMessages === 1 && result.stats.toolCalls === 0 && result.stats.toolResults === 0, "capture session counts drifted");
  requireCapture(result.removedEnvironment.includes("OPENAI_BASE_URL") && result.removedEnvironment.includes("HTTPS_PROXY"), "provider/proxy environment was not sanitized");
  return {
    schema: "vela.claim-dependency-pi-request-capture.v1",
    run_id: request.run_id,
    arm: request.arm,
    request_raw_root: rawRoot(Buffer.from(`${JSON.stringify(request)}\n`, "utf8")),
    fetch_count: calls.length,
    url: call.url,
    method: call.method,
    content_encoding: call.headers.get("content-encoding"),
    encoded_request_raw_root: rawRoot(call.encoded),
    decoded_request_raw_root: rawRoot(call.decoded),
    instructions_raw_root: rawRoot(Buffer.from(body.instructions, "utf8")),
    user_message_raw_root: rawRoot(Buffer.from(request.user_message, "utf8")),
    response_raw_root: rawRoot(Buffer.from(result.answer, "utf8")),
    model: body.model,
    reasoning: body.reasoning,
    text: body.text,
    prompt_cache_key: body.prompt_cache_key,
    input_message_count: body.input.length,
    tool_definition_count: 0,
    continuation_present: false,
    session_counts: result.stats,
    event_types: result.eventTypes,
    sanitized_environment_names: result.removedEnvironment,
    external_network_calls: 0,
    authority_effect: "none",
    claim_credit: false,
  };
}

async function main(argv) {
  const args = argumentMap(argv);
  requireCapture(args.size === 2 && args.has("--request") && args.has("--auth"), "usage: request-capture.mjs --request PATH --auth PATH");
  const report = await captureRequest({ requestPath: args.get("--request"), authPath: args.get("--auth") });
  process.stdout.write(`${JSON.stringify(report)}\n`);
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try {
    await main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`request-capture: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
