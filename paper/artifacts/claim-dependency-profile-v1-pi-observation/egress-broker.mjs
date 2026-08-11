#!/usr/bin/env node
import { createHash } from "node:crypto";
import { chmodSync, closeSync, constants, fstatSync, lstatSync, openSync, readFileSync, unlinkSync } from "node:fs";
import http from "node:http";
import https from "node:https";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { zstdDecompressSync } from "node:zlib";
import { loadFrozenOAuth, sanitizeProviderEnvironment } from "./auth-preflight.mjs";

const TARGET = "https://chatgpt.com/backend-api/codex/responses";
const MAXIMUM_REQUEST_BYTES = 2_097_152;
const UUID4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

class BrokerContractError extends Error {
  constructor(message) {
    super(message);
    this.name = "BrokerContractError";
  }
}

function requireBroker(condition, message) {
  if (!condition) throw new BrokerContractError(message);
}

function exactKeys(value, keys, label) {
  requireBroker(value !== null && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  requireBroker(JSON.stringify(Object.keys(value).sort()) === JSON.stringify([...keys].sort()), `${label} key set drifted`);
}

function rawRoot(data) {
  return `sha256:${createHash("sha256").update(data).digest("hex")}`;
}

function readFrozenRequest(path) {
  let descriptor;
  try {
    const beforePath = lstatSync(path);
    requireBroker(beforePath.isFile() && !beforePath.isSymbolicLink(), "broker request must be regular");
    descriptor = openSync(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
    const before = fstatSync(descriptor);
    requireBroker(before.isFile() && (before.mode & 0o777) === 0o444 && before.size <= 262_144, "broker request mode or size drifted");
    const data = readFileSync(descriptor);
    const after = fstatSync(descriptor);
    const afterPath = lstatSync(path);
    requireBroker(
      before.dev === after.dev && before.ino === after.ino && before.size === after.size && before.mtimeMs === after.mtimeMs &&
        before.dev === afterPath.dev && before.ino === afterPath.ino && data.length === before.size,
      "broker request identity changed",
    );
    requireBroker(data.length <= 262_144, "broker request is too large");
    const value = JSON.parse(data.toString("utf8"));
    requireBroker(data.equals(Buffer.from(`${JSON.stringify(value)}\n`, "utf8")), "broker request encoding drifted");
    return value;
  } catch (error) {
    if (error instanceof BrokerContractError) throw error;
    throw new BrokerContractError("broker request cannot be read");
  } finally {
    if (descriptor !== undefined) {
      try { closeSync(descriptor); } catch {}
    }
  }
}

function jwtAccount(access) {
  const parts = access.split(".");
  requireBroker(parts.length === 3, "broker bearer is not a JWT");
  let payload;
  try {
    payload = JSON.parse(Buffer.from(parts[1], "base64url").toString("utf8"));
  } catch {
    throw new BrokerContractError("broker JWT payload is invalid");
  }
  const accountId = payload?.["https://api.openai.com/auth"]?.chatgpt_account_id;
  requireBroker(typeof accountId === "string" && accountId.length > 0, "broker JWT account binding is missing");
  requireBroker(Number.isSafeInteger(payload.exp) && payload.exp * 1000 - Date.now() >= 21_600_000, "broker JWT validity is insufficient");
  return accountId;
}

function validateProviderBody(body, frozenRequest) {
  exactKeys(body, ["model", "store", "stream", "instructions", "input", "text", "include", "prompt_cache_key", "tool_choice", "parallel_tool_calls", "reasoning"], "broker provider body");
  requireBroker(body.model === "gpt-5.6-sol" && body.store === false && body.stream === true, "broker model/store/stream drifted");
  requireBroker(body.instructions === `${frozenRequest.system_prompt}\nCurrent working directory: /workspace`, "broker instructions drifted");
  requireBroker(Array.isArray(body.input) && body.input.length === 1, "broker input count drifted");
  exactKeys(body.input[0], ["role", "content"], "broker input message");
  requireBroker(body.input[0].role === "user", "broker input role drifted");
  requireBroker(Array.isArray(body.input[0].content) && body.input[0].content.length === 1, "broker input content count drifted");
  exactKeys(body.input[0].content[0], ["type", "text"], "broker input text");
  requireBroker(body.input[0].content[0].type === "input_text" && body.input[0].content[0].text === frozenRequest.user_message, "broker user message drifted");
  requireBroker(JSON.stringify(body.text) === JSON.stringify({ verbosity: "low" }), "broker text options drifted");
  requireBroker(JSON.stringify(body.reasoning) === JSON.stringify({ effort: "high", summary: "auto" }), "broker reasoning options drifted");
  requireBroker(JSON.stringify(body.include) === JSON.stringify(["reasoning.encrypted_content"]), "broker include set drifted");
  requireBroker(body.prompt_cache_key === frozenRequest.session_id, "broker prompt cache key drifted");
  requireBroker(body.tool_choice === "auto" && body.parallel_tool_calls === true, "broker provider defaults drifted");
}

function validateProviderHeaders(headers, frozenRequest, credential) {
  const expected = [
    "accept", "authorization", "chatgpt-account-id", "connection", "content-encoding", "content-length", "content-type",
    "host", "openai-beta", "originator", "session-id", "user-agent", "x-client-request-id",
  ];
  requireBroker(JSON.stringify(Object.keys(headers).sort()) === JSON.stringify(expected), "broker header-name set drifted");
  requireBroker(headers.accept === "text/event-stream" && headers["content-type"] === "application/json", "broker media headers drifted");
  requireBroker(headers["content-encoding"] === "zstd" && headers["openai-beta"] === "responses=experimental", "broker encoding/beta headers drifted");
  requireBroker(headers.originator === "pi" && headers.connection === "close", "broker originator/connection drifted");
  requireBroker(headers.host === "localhost", "broker Unix-request host header drifted");
  requireBroker(headers["session-id"] === frozenRequest.session_id && headers["x-client-request-id"] === frozenRequest.session_id, "broker session headers drifted");
  requireBroker(UUID4.test(headers["session-id"]), "broker session ID is invalid");
  requireBroker(/^pi \(linux [^;]+; x64\)$/u.test(headers["user-agent"]), "broker user-agent policy drifted");
  requireBroker(headers.authorization === `Bearer ${credential.access}`, "broker authorization header does not match the derived inference credential");
  const access = credential.access;
  requireBroker(jwtAccount(access) === headers["chatgpt-account-id"] && headers["chatgpt-account-id"] === credential.accountId, "broker bearer/account binding drifted");
  requireBroker(Number(headers["content-length"]) > 0 && Number(headers["content-length"]) <= MAXIMUM_REQUEST_BYTES, "broker content length is invalid");
  return Object.fromEntries(
    Object.entries(headers).filter(([name]) => !["connection", "content-length", "host"].includes(name)),
  );
}

function collectRequest(request) {
  return new Promise((resolveBody, rejectBody) => {
    const chunks = [];
    let bytes = 0;
    request.on("data", (chunk) => {
      bytes += chunk.length;
      if (bytes > MAXIMUM_REQUEST_BYTES) {
        rejectBody(new BrokerContractError("broker request body is too large"));
        request.destroy();
        return;
      }
      chunks.push(chunk);
    });
    request.on("end", () => resolveBody(Buffer.concat(chunks)));
    request.on("error", () => rejectBody(new BrokerContractError("broker request read failed")));
  });
}

export function createBroker({ socketPath, frozenRequest, credential, audit = () => {} }) {
  let requestCount = 0;
  let closed = false;
  const server = http.createServer(async (request, response) => {
    requestCount += 1;
    try {
      requireBroker(requestCount === 1, "broker accepts exactly one request");
      requireBroker(request.method === "POST" && request.url === "/inference", "broker method/path drifted");
      const forwardHeaders = validateProviderHeaders(request.headers, frozenRequest, credential);
      const encoded = await collectRequest(request);
      requireBroker(encoded.length === Number(request.headers["content-length"]), "broker request length drifted");
      let decoded;
      try { decoded = zstdDecompressSync(encoded); } catch { throw new BrokerContractError("broker request zstd decoding failed"); }
      const providerBody = JSON.parse(decoded.toString("utf8"));
      validateProviderBody(providerBody, frozenRequest);
      audit({
        schema: "vela.claim-dependency-pi-egress-broker-audit.v1",
        kind: "validated_request",
        request_count: 1,
        target: TARGET,
        encoded_request_raw_root: rawRoot(encoded),
        decoded_request_raw_root: rawRoot(decoded),
        header_names: Object.keys(request.headers).sort(),
      });
      const upstream = https.request(TARGET, {
        method: "POST",
        headers: { ...forwardHeaders, "content-length": String(encoded.length), connection: "close" },
        agent: false,
      });
      upstream.setTimeout(900_000, () => upstream.destroy(new Error("upstream timeout")));
      upstream.on("response", (upstreamResponse) => {
        response.writeHead(upstreamResponse.statusCode ?? 502, {
          "content-type": upstreamResponse.headers["content-type"] ?? "text/event-stream",
          connection: "close",
        });
        const digest = createHash("sha256");
        let responseBytes = 0;
        upstreamResponse.on("data", (chunk) => {
          responseBytes += chunk.length;
          digest.update(chunk);
          response.write(chunk);
        });
        upstreamResponse.on("end", () => {
          response.end();
          audit({
            schema: "vela.claim-dependency-pi-egress-broker-audit.v1",
            kind: "completed",
            request_count: 1,
            status: upstreamResponse.statusCode ?? 502,
            response_bytes: responseBytes,
            response_raw_root: `sha256:${digest.digest("hex")}`,
            additional_requests: 0,
          });
          server.close();
        });
      });
      upstream.on("error", () => {
        if (!response.headersSent) response.writeHead(502, { "content-type": "text/plain", connection: "close" });
        response.end("bounded broker upstream failure");
        audit({ schema: "vela.claim-dependency-pi-egress-broker-audit.v1", kind: "upstream_error", request_count: 1 });
        server.close();
      });
      upstream.end(encoded);
    } catch (error) {
      response.writeHead(400, { "content-type": "text/plain", connection: "close" });
      response.end("bounded broker request refused");
      audit({
        schema: "vela.claim-dependency-pi-egress-broker-audit.v1",
        kind: "refused",
        request_count: requestCount,
        error_class: error instanceof Error ? error.name : "UnknownError",
      });
      server.close();
    }
  });
  const cleanup = () => {
    if (closed) return;
    closed = true;
    try { unlinkSync(socketPath); } catch {}
  };
  server.on("close", cleanup);
  return { server, cleanup };
}

function argumentMap(argv) {
  const result = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    requireBroker(argv[index]?.startsWith("--") && argv[index + 1] !== undefined, "arguments must be --name value pairs");
    requireBroker(!result.has(argv[index]), `duplicate argument: ${argv[index]}`);
    result.set(argv[index], argv[index + 1]);
  }
  return result;
}

async function main(argv) {
  const args = argumentMap(argv);
  requireBroker(args.size === 3 && args.has("--socket") && args.has("--request") && args.has("--auth"), "usage: egress-broker.mjs --socket PATH --request PATH --auth PATH");
  sanitizeProviderEnvironment();
  const socketPath = args.get("--socket");
  const frozenRequest = readFrozenRequest(args.get("--request"));
  const credential = loadFrozenOAuth(args.get("--auth"));
  const audit = (record) => process.stdout.write(`${JSON.stringify(record)}\n`);
  const { server, cleanup } = createBroker({ socketPath, frozenRequest, credential, audit });
  process.once("SIGTERM", () => { server.close(); cleanup(); });
  process.once("SIGINT", () => { server.close(); cleanup(); });
  await new Promise((resolveListen, rejectListen) => {
    server.once("error", () => rejectListen(new BrokerContractError("broker listen failed")));
    server.listen(socketPath, () => {
      chmodSync(socketPath, 0o600);
      audit({ schema: "vela.claim-dependency-pi-egress-broker-audit.v1", kind: "ready", request_count: 0, target: TARGET });
      resolveListen();
    });
  });
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try {
    await main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${JSON.stringify({ schema: "vela.claim-dependency-pi-egress-broker-audit.v1", kind: "fatal", error_class: error instanceof Error ? error.name : "UnknownError" })}\n`);
    process.exitCode = 1;
  }
}
