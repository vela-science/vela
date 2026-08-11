#!/usr/bin/env node
import { createHash } from "node:crypto";
import {
  closeSync,
  constants,
  fstatSync,
  lstatSync,
  openSync,
  readFileSync,
} from "node:fs";
import { fileURLToPath } from "node:url";
import http from "node:http";
import { resolve } from "node:path";
import { Readable } from "node:stream";
import {
  DefaultResourceLoader,
  ModelRuntime,
  SessionManager,
  SettingsManager,
  createAgentSession,
} from "@earendil-works/pi-coding-agent";
import {
  createReadOnlyCredentialStore,
  loadFrozenOAuth,
  sanitizeProviderEnvironment,
} from "./auth-preflight.mjs";

const CWD = "/workspace";
const AGENT_DIR = "/nonexistent/pi-agent";
const PROVIDER = "openai-codex";
const MODEL = "gpt-5.6-sol";
const MAXIMUM_REQUEST_BYTES = 262_144;
const ROOT = /^sha256:[0-9a-f]{64}$/u;
const UUID4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
const RUN = /^block-[12]-(?:profile|baseline)$/u;
const BROKER_SOCKET = "/broker/inference.sock";
const FORBIDDEN_EVENT_PREFIXES = [
  "auto_retry_",
  "compaction_",
  "summarization_retry_",
  "tool_execution_",
];

export class ParticipantContractError extends Error {
  constructor(message) {
    super(message);
    this.name = "ParticipantContractError";
  }
}

function requireContract(condition, message) {
  if (!condition) throw new ParticipantContractError(message);
}

function exactKeys(value, keys, label) {
  requireContract(value !== null && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  requireContract(
    JSON.stringify(Object.keys(value).sort()) === JSON.stringify([...keys].sort()),
    `${label} key set is invalid`,
  );
}

function rawRoot(data) {
  return `sha256:${createHash("sha256").update(data).digest("hex")}`;
}

function readRequest(path) {
  const resolved = resolve(path);
  const pathBefore = lstatSync(resolved);
  requireContract(pathBefore.isFile() && !pathBefore.isSymbolicLink(), "request is not a regular file");
  const descriptor = openSync(resolved, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
  try {
    const before = fstatSync(descriptor);
    requireContract(before.isFile() && before.size <= MAXIMUM_REQUEST_BYTES, "request type or size is invalid");
    requireContract((before.mode & 0o777) === 0o444, "request mode must be 0444");
    const data = readFileSync(descriptor);
    const after = fstatSync(descriptor);
    const pathAfter = lstatSync(resolved);
    requireContract(
      before.dev === after.dev && before.ino === after.ino && before.size === after.size && before.mtimeMs === after.mtimeMs &&
        before.dev === pathAfter.dev && before.ino === pathAfter.ino && data.length === before.size,
      "request changed while reading",
    );
    let value;
    try {
      value = JSON.parse(data.toString("utf8"));
    } catch (error) {
      throw new ParticipantContractError(`request is not valid UTF-8 JSON: ${error instanceof Error ? error.message : String(error)}`);
    }
    requireContract(data.equals(Buffer.from(`${JSON.stringify(value)}\n`, "utf8")), "request must use exact compact JSON encoding");
    return value;
  } finally {
    closeSync(descriptor);
  }
}

function validateRequest(value) {
  exactKeys(
    value,
    [
      "schema",
      "experiment_id",
      "run_id",
      "arm",
      "session_id",
      "provider",
      "model",
      "thinking_level",
      "system_prompt",
      "system_prompt_raw_root",
      "user_message",
      "user_message_raw_root",
      "input_manifest_raw_root",
      "answer_schema_raw_root",
      "embedded_scientific_input_count",
      "embedded_answer_schema",
      "output_contract",
      "authority_effect",
      "claim_credit",
    ],
    "request",
  );
  requireContract(value.schema === "vela.claim-dependency-pi-participant-request.v1", "request schema is invalid");
  requireContract(value.experiment_id === "synthetic-counterfactual-erdos-321-v0", "experiment ID is invalid");
  requireContract(typeof value.run_id === "string" && RUN.test(value.run_id), "run ID is invalid");
  requireContract(["disciplined-git-ro-crate", "rooted-source-plus-profile"].includes(value.arm), "arm is invalid");
  requireContract(UUID4.test(value.session_id), "session ID is invalid");
  requireContract(value.provider === PROVIDER && value.model === MODEL && value.thinking_level === "high", "model selection is invalid");
  requireContract(typeof value.system_prompt === "string" && value.system_prompt.length > 0, "system prompt is invalid");
  requireContract(typeof value.user_message === "string" && value.user_message.length > 0, "user message is invalid");
  requireContract(ROOT.test(value.system_prompt_raw_root) && value.system_prompt_raw_root === rawRoot(Buffer.from(value.system_prompt, "utf8")), "system prompt root is invalid");
  requireContract(ROOT.test(value.user_message_raw_root) && value.user_message_raw_root === rawRoot(Buffer.from(value.user_message, "utf8")), "user message root is invalid");
  requireContract(ROOT.test(value.input_manifest_raw_root) && ROOT.test(value.answer_schema_raw_root), "contract roots are invalid");
  const expectedCount = value.arm === "disciplined-git-ro-crate" ? 7 : 8;
  requireContract(value.embedded_scientific_input_count === expectedCount && value.embedded_answer_schema === true, "embedded input count is invalid");
  requireContract(value.output_contract === "last_assistant_text_only", "output contract is invalid");
  requireContract(value.authority_effect === "none" && value.claim_credit === false, "request authority boundary is invalid");
  return value;
}

function validateResources(loader, extensionsResult) {
  requireContract(loader.getExtensions() === extensionsResult, "extension result identity drifted");
  requireContract(extensionsResult.extensions.length === 0, "extensions must be empty");
  requireContract(loader.getSkills().skills.length === 0, "skills must be empty");
  requireContract(loader.getPrompts().prompts.length === 0, "prompt templates must be empty");
  requireContract(loader.getThemes().themes.length === 0, "themes must be empty");
  requireContract(loader.getAgentsFiles().agentsFiles.length === 0, "context files must be empty");
  requireContract(loader.getAppendSystemPrompt().length === 0, "append system prompt must be empty");
}

function validateEvents(events) {
  for (const type of events) {
    requireContract(!FORBIDDEN_EVENT_PREFIXES.some((prefix) => type.startsWith(prefix)), `forbidden participant event: ${type}`);
    requireContract(!type.includes("tool"), `tool event is forbidden: ${type}`);
  }
  requireContract(events.filter((type) => type === "agent_start").length === 1, "exactly one agent_start is required");
  requireContract(events.filter((type) => type === "agent_end").length === 1, "exactly one agent_end is required");
  requireContract(events.includes("agent_settled"), "agent_settled is required");
}

function brokerFetch(url, init = {}) {
  requireContract(String(url) === "https://chatgpt.com/backend-api/codex/responses", "broker fetch target drifted");
  requireContract(init.method === "POST", "broker fetch method drifted");
  const body = Buffer.from(init.body ?? "");
  const headers = Object.fromEntries(new Headers(init.headers).entries());
  headers["content-length"] = String(body.length);
  headers.connection = "close";
  return new Promise((resolveResponse, rejectResponse) => {
    const request = http.request(
      {
        socketPath: BROKER_SOCKET,
        path: "/inference",
        method: "POST",
        headers,
        signal: init.signal,
      },
      (response) => {
        resolveResponse(
          new Response(Readable.toWeb(response), {
            status: response.statusCode ?? 500,
            statusText: response.statusMessage,
            headers: response.headers,
          }),
        );
      },
    );
    request.on("error", rejectResponse);
    request.end(body);
  });
}

export async function runParticipant({ requestPath, authPath, auditSink = () => {}, captureMode = false }) {
  requireContract(process.cwd() === CWD, `participant cwd must be ${CWD}`);
  if (!captureMode) requireContract(process.env.VELA_PI_BROKER_SOCKET === BROKER_SOCKET, "exact Unix-socket egress broker is required");
  const removedEnvironment = sanitizeProviderEnvironment();
  const request = validateRequest(readRequest(requestPath));
  auditSink({
    schema: "vela.claim-dependency-pi-participant-audit.v1",
    kind: "start",
    run_id: request.run_id,
    arm: request.arm,
    session_id: request.session_id,
    system_prompt_raw_root: request.system_prompt_raw_root,
    user_message_raw_root: request.user_message_raw_root,
    sanitized_environment_names: removedEnvironment,
    model_visible_filesystem_inputs: 0,
    active_tools: [],
    transport_boundary: captureMode ? "network_dead_fetch_capture" : "one_request_unix_socket_broker",
  });
  const credential = loadFrozenOAuth(authPath);
  const credentials = createReadOnlyCredentialStore(credential);
  const settingsManager = SettingsManager.inMemory({
    transport: "sse",
    compaction: { enabled: false },
    retry: { enabled: false, maxRetries: 0, provider: { maxRetries: 0, maxRetryDelayMs: 0 } },
    defaultProjectTrust: "never",
    packages: [],
    extensions: [],
    skills: [],
    prompts: [],
    themes: [],
    images: { blockImages: true },
  });
  const resourceLoader = new DefaultResourceLoader({
    cwd: CWD,
    agentDir: AGENT_DIR,
    settingsManager,
    noExtensions: true,
    noSkills: true,
    noPromptTemplates: true,
    noThemes: true,
    noContextFiles: true,
    systemPrompt: request.system_prompt,
  });
  await resourceLoader.reload();
  const modelRuntime = await ModelRuntime.create({
    credentials,
    modelsPath: null,
    allowModelNetwork: false,
    refreshOnCreate: false,
  });
  const model = modelRuntime.getModel(PROVIDER, MODEL);
  requireContract(model !== undefined, "qualified model is unavailable in the pinned catalog");
  requireContract(model.provider === PROVIDER && model.id === MODEL && model.api === "openai-codex-responses", "qualified model identity drifted");
  const sessionManager = SessionManager.inMemory(CWD, { id: request.session_id });
  const { session, extensionsResult } = await createAgentSession({
    cwd: CWD,
    agentDir: AGENT_DIR,
    modelRuntime,
    model,
    thinkingLevel: "high",
    noTools: "all",
    tools: [],
    resourceLoader,
    sessionManager,
    settingsManager,
  });
  const events = [];
  const unsubscribe = session.subscribe((event) => {
    events.push(event.type);
    auditSink({
      schema: "vela.claim-dependency-pi-participant-audit.v1",
      kind: "pi_event",
      sequence: events.length,
      event_type: event.type,
    });
  });
  try {
    validateResources(resourceLoader, extensionsResult);
    requireContract(session.getActiveToolNames().length === 0, "active tools must be empty");
    requireContract(session.sessionFile === undefined, "session persistence is forbidden");
    requireContract(session.sessionId === request.session_id, "session ID drifted");
    requireContract(session.systemPrompt === `${request.system_prompt}\nCurrent working directory: ${CWD}`, "effective system prompt drifted");
    session.setAutoRetryEnabled(false);
    session.setAutoCompactionEnabled(false);
    const originalFetch = globalThis.fetch;
    if (!captureMode) globalThis.fetch = brokerFetch;
    try {
      await session.prompt(request.user_message, { expandPromptTemplates: false });
    } finally {
      globalThis.fetch = originalFetch;
    }
    await session.waitForIdle();
    validateEvents(events);
    requireContract(session.retryAttempt === 0 && session.isCompacting === false && session.pendingMessageCount === 0, "post-run continuation state is invalid");
    requireContract(session.getActiveToolNames().length === 0, "tools changed during the run");
    const messages = session.messages;
    requireContract(messages.length === 2 && messages[0].role === "user" && messages[1].role === "assistant", "participant session must contain exactly one user and one assistant message");
    const answer = session.getLastAssistantText();
    requireContract(typeof answer === "string" && answer.length > 0, "last assistant text is missing");
    const stats = session.getSessionStats();
    requireContract(stats.userMessages === 1 && stats.assistantMessages === 1 && stats.toolCalls === 0 && stats.toolResults === 0, "session statistics violate the one-message/no-tool contract");
    const finalAudit = {
      schema: "vela.claim-dependency-pi-participant-audit.v1",
      kind: "final",
      run_id: request.run_id,
      arm: request.arm,
      session_id: request.session_id,
      message_roles: messages.map((message) => message.role),
      session_counts: {
        user_messages: stats.userMessages,
        assistant_messages: stats.assistantMessages,
        tool_calls: stats.toolCalls,
        tool_results: stats.toolResults,
        total_messages: stats.totalMessages,
      },
      usage: {
        input: stats.tokens.input,
        output: stats.tokens.output,
        cache_read: stats.tokens.cacheRead,
        cache_write: stats.tokens.cacheWrite,
        total: stats.tokens.total,
      },
      active_tools: session.getActiveToolNames(),
      retry_attempt: session.retryAttempt,
      compacting: session.isCompacting,
      pending_messages: session.pendingMessageCount,
      effective_system_prompt_raw_root: rawRoot(Buffer.from(session.systemPrompt, "utf8")),
      user_message_raw_root: request.user_message_raw_root,
      answer_raw_root: rawRoot(Buffer.from(answer, "utf8")),
      event_count: events.length,
      event_types: [...events],
      sanitized_environment_names: removedEnvironment,
    };
    auditSink(finalAudit);
    return Object.freeze({
      answer,
      request,
      effectiveSystemPrompt: session.systemPrompt,
      eventTypes: Object.freeze([...events]),
      removedEnvironment,
      stats: Object.freeze({
        userMessages: stats.userMessages,
        assistantMessages: stats.assistantMessages,
        toolCalls: stats.toolCalls,
        toolResults: stats.toolResults,
        totalMessages: stats.totalMessages,
      }),
      finalAudit: Object.freeze(finalAudit),
    });
  } finally {
    unsubscribe();
    session.dispose();
  }
}

function argumentMap(argv) {
  const result = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    requireContract(argv[index]?.startsWith("--") && argv[index + 1] !== undefined, "arguments must be --name value pairs");
    requireContract(!result.has(argv[index]), `duplicate argument: ${argv[index]}`);
    result.set(argv[index], argv[index + 1]);
  }
  return result;
}

async function main(argv) {
  const args = argumentMap(argv);
  requireContract(args.size === 2 && args.has("--request") && args.has("--auth"), "usage: participant.mjs --request PATH --auth PATH");
  const result = await runParticipant({
    requestPath: args.get("--request"),
    authPath: args.get("--auth"),
    auditSink: (record) => process.stderr.write(`${JSON.stringify(record)}\n`),
  });
  process.stdout.write(result.answer);
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try {
    await main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${JSON.stringify({ schema: "vela.claim-dependency-pi-participant-audit.v1", kind: "error", error_class: error instanceof Error ? error.name : "UnknownError" })}\n`);
    process.exitCode = 1;
  }
}
