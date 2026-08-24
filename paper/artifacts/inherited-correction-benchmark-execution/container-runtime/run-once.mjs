#!/usr/bin/env node
import Ajv2020 from "ajv/dist/2020.js";
import { createHash } from "node:crypto";
import { readFileSync, renameSync, statSync, writeFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";
import { configArgs, STRICT_OVERRIDES } from "./strict-config.mjs";

const expectedKeys = (value, keys, label) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${label}_not_object`);
  const got = Object.keys(value).sort();
  const want = [...keys].sort();
  if (JSON.stringify(got) !== JSON.stringify(want)) throw new Error(`${label}_fields`);
};
const bytes = (path) => readFileSync(path);
const json = (path) => JSON.parse(bytes(path).toString("utf8"));
const sha = (data) => `sha256:${createHash("sha256").update(data).digest("hex")}`;
const canonical = (value) => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object") return `{${Object.keys(value).sort().map(k => `${JSON.stringify(k)}:${canonical(value[k])}`).join(",")}}`;
  return JSON.stringify(value);
};
const root = (value) => sha(Buffer.from(canonical(value), "utf8"));
const writeExclusive = (name, data) => writeFileSync(join("/evidence", name), data, { flag: "wx", mode: 0o600 });
const now = () => new Date().toISOString();
const TRUST_BUNDLE_PATH = "/etc/ssl/certs/ca-certificates.crt";

function compileResponseSchema(schema) {
  return new Ajv2020({ allErrors: true, strict: true }).compile(schema);
}

function validateResponseAgainstSchema(schema, response) {
  const validate = compileResponseSchema(schema);
  return { valid: validate(response), errors: validate.errors };
}

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exitCode = 2;
}

function verifyBindings(runId, permit, config, assignment, promptBytes, schemaBytes, trustBundleBytes) {
  expectedKeys(permit, ["schema", "status", "expires_at", "registration_root", "image_digest", "participant_configuration_root", "assignment_root", "run_id", "condition", "participant_instance_id", "prompt_root", "packet_root", "trust_bundle_bytes", "attempt"], "permit");
  if (permit.schema !== "vela.inherited-correction-launch-permit.v1" || permit.status !== "authorized") throw new Error("permit_not_authorized");
  if (Date.parse(permit.expires_at) <= Date.now()) throw new Error("permit_expired");
  if (permit.run_id !== runId || permit.attempt !== 1) throw new Error("permit_run_or_attempt");
  if (permit.image_digest !== process.env.QUALIFIED_IMAGE_DIGEST) throw new Error("permit_image_digest");
  if (permit.participant_configuration_root !== root(config)) throw new Error("permit_configuration_root");
  if (permit.assignment_root !== root(assignment)) throw new Error("permit_assignment_root");
  if (permit.prompt_root !== sha(promptBytes) || config.prompt_root !== sha(promptBytes)) throw new Error("permit_prompt_root");
  if (config.response_schema_bytes !== sha(schemaBytes)) throw new Error("schema_bytes");
  if (process.env.SSL_CERT_FILE !== TRUST_BUNDLE_PATH || config.trust_bundle_path !== TRUST_BUNDLE_PATH) throw new Error("trust_bundle_path");
  if (permit.trust_bundle_bytes !== sha(trustBundleBytes) || config.trust_bundle_bytes !== sha(trustBundleBytes)) throw new Error("trust_bundle_bytes");
  if (config.strict_overrides_root !== root(STRICT_OVERRIDES)) throw new Error("strict_overrides_root");
  const exact = assignment.assignments.find(item => item.run_id === runId);
  if (!exact || exact.condition !== permit.condition || exact.participant_instance_id !== permit.participant_instance_id || exact.packet_root !== permit.packet_root) throw new Error("permit_assignment_binding");
  if (config.registration_root !== permit.registration_root || assignment.registration_root !== permit.registration_root) throw new Error("registration_binding");
  if (config.image_digest !== permit.image_digest || assignment.image_digest !== permit.image_digest) throw new Error("image_binding");
  if (config.model !== "gpt-5.6-sol" || config.reasoning_effort !== "high" || config.service_tier !== "default") throw new Error("participant_configuration");
  if (config.timeout_seconds !== 600 || config.output_token_ceiling !== 8192 || config.attempt !== 1 || config.retries !== 0 || config.tools !== "none") throw new Error("runtime_configuration");
  return exact;
}

function inspectEvents(raw) {
  const events = raw.toString("utf8").split(/\n/).filter(Boolean).map((line, index) => {
    try { return JSON.parse(line); } catch { throw new Error(`provider_jsonl_line_${index + 1}`); }
  });
  const types = events.map(e => String(e.type || ""));
  const items = events.map(e => e.item).filter(x => x && typeof x === "object");
  const itemTypes = items.map(x => String(x.type || ""));
  const forbidden = [...types, ...itemTypes].filter(t => /tool|command|patch|file_change|web_search|computer|compact|resume|continu/i.test(t));
  if (forbidden.length) throw new Error(`forbidden_provider_event:${forbidden.join(",")}`);
  if (types.filter(x => x === "thread.started").length !== 1) throw new Error("thread_count");
  if (types.filter(x => x === "turn.started").length !== 1 || types.filter(x => x === "turn.completed").length !== 1) throw new Error("turn_count");
  const agentMessages = items.filter(x => x.type === "agent_message" || x.type === "message");
  if (agentMessages.length !== 1) throw new Error("response_count");
  const usageEvents = events.filter(e => e.usage && typeof e.usage === "object");
  if (usageEvents.length < 1) throw new Error("usage_missing");
  const usage = usageEvents.at(-1).usage;
  for (const key of ["input_tokens", "cached_input_tokens", "output_tokens"]) {
    if (!Number.isInteger(usage[key]) || usage[key] < 0) throw new Error(`usage_${key}`);
  }
  if (usage.output_tokens > 8192) throw new Error("output_token_ceiling");
  return { usage, event_count: events.length, response_count: agentMessages.length, tool_calls: 0, turn_count: 1, compactions: 0 };
}

function forbiddenEvent(event) {
  const type = String(event?.type || "");
  const itemType = String(event?.item?.type || "");
  return /tool|command|patch|file_change|web_search|computer|compact|resume|continu/i.test(`${type}:${itemType}`);
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length !== 2 || args[0] !== "--run-id" || !/^[-a-z0-9]+$/.test(args[1])) throw new Error("usage: run-once --run-id EXACT_RUN_ID");
  const runId = args[1];
  if (basename(process.cwd()) !== "work" || statSync("/work").uid !== 10001) throw new Error("working_directory");
  const hold = json("/permit/hold-state.json");
  expectedKeys(hold, ["schema", "status", "reason", "updated_at"], "hold");
  if (hold.schema !== "vela.inherited-correction-hold.v1" || hold.status !== "release") throw new Error("launch_on_hold");
  const permitPath = `/permit/${runId}.permit.json`;
  const consumedPath = `/permit/${runId}.permit.consumed.json`;
  const permit = json(permitPath);
  const config = json("/input/participant-configuration.json");
  const assignment = json("/input/assignment.json");
  const promptBytes = bytes("/input/prompt.txt");
  const schemaBytes = bytes("/input/response-schema.json");
  const trustBundleBytes = bytes(TRUST_BUNDLE_PATH);
  const exact = verifyBindings(runId, permit, config, assignment, promptBytes, schemaBytes, trustBundleBytes);

  // Atomic rename on the permit volume is the irreversible pre-provider consume.
  renameSync(permitPath, consumedPath);
  writeExclusive("launch.json", `${JSON.stringify({ schema: "vela.inherited-correction-launch.v1", run_id: runId, participant_instance_id: exact.participant_instance_id, condition: exact.condition, permit_bytes: sha(bytes(consumedPath)), consumed_at: now() }, null, 2)}\n`);

  const outputPath = "/evidence/participant-response.raw.json";
  const cliArgs = [
    "exec", "--strict-config", "--ignore-user-config", "--ignore-rules", "--ephemeral", "--skip-git-repo-check",
    "--model", "gpt-5.6-sol", "-c", 'model_reasoning_effort="high"', "-c", 'service_tier="default"',
    ...configArgs(),
    "--sandbox", "read-only", "--cd", "/work", "--output-schema", "/input/response-schema.json",
    "--output-last-message", outputPath, "--json", "-"
  ];
  const startedAt = now();
  const start = process.hrtime.bigint();
  const child = spawn("codex", cliArgs, { cwd: "/work", env: { CODEX_HOME: "/codex-home", PATH: process.env.PATH, HOME: "/tmp", SSL_CERT_FILE: TRUST_BUNDLE_PATH }, stdio: ["pipe", "pipe", "pipe"], detached: true });
  const stdout = [];
  const stderr = [];
  let streamBuffer = "";
  let forbiddenObserved = null;
  child.stdout.on("data", chunk => {
    stdout.push(chunk);
    streamBuffer += chunk.toString("utf8");
    const lines = streamBuffer.split("\n");
    streamBuffer = lines.pop();
    for (const line of lines) {
      if (!line) continue;
      try {
        const event = JSON.parse(line);
        if (forbiddenEvent(event) && !forbiddenObserved) {
          forbiddenObserved = `${String(event.type || "")}:${String(event.item?.type || "")}`;
          try { process.kill(-child.pid, "SIGKILL"); } catch {}
        }
      } catch {}
    }
  });
  child.stderr.on("data", chunk => stderr.push(chunk));
  child.stdin.end(promptBytes);
  let timedOut = false;
  const timer = setTimeout(() => { timedOut = true; try { process.kill(-child.pid, "SIGKILL"); } catch {} }, 600_000);
  const exitCode = await new Promise((resolve, reject) => { child.on("error", reject); child.on("close", resolve); });
  clearTimeout(timer);
  const elapsed = Number(process.hrtime.bigint() - start) / 1e9;
  const eventBytes = Buffer.concat(stdout);
  const stderrBytes = Buffer.concat(stderr);
  writeExclusive("provider-events.jsonl", eventBytes);
  writeExclusive("provider-stderr.txt", stderrBytes);
  let status = "completed";
  let validationError = null;
  let eventReceipt = null;
  try {
    if (forbiddenObserved) throw new Error(`forbidden_provider_event:${forbiddenObserved}`);
    if (timedOut) throw new Error("timeout");
    if (exitCode !== 0) throw new Error(`provider_exit_${exitCode}`);
    eventReceipt = inspectEvents(eventBytes);
    const response = JSON.parse(bytes(outputPath).toString("utf8"));
    const schema = JSON.parse(schemaBytes.toString("utf8"));
    const validate = compileResponseSchema(schema);
    if (!validate(response)) throw new Error(`response_schema:${JSON.stringify(validate.errors)}`);
    if (config.expected_response_root && config.expected_response_root !== root(response)) throw new Error("expected_response_root");
    const retained = Buffer.concat([eventBytes, stderrBytes, bytes(outputPath)]).toString("utf8");
    if (/access_token|refresh_token|id_token|OPENAI_API_KEY|Bearer\s|sk-[A-Za-z0-9]/.test(retained)) throw new Error("credential_shaped_capture");
  } catch (error) {
    status = "non_result";
    validationError = String(error.message || error);
  }
  const receipt = {
    schema: "vela.inherited-correction-terminal-receipt.v1", run_id: runId, condition: exact.condition,
    participant_instance_id: exact.participant_instance_id, attempt: 1, status, validation_error: validationError,
    provider_started_at: startedAt, provider_completed_at: now(), duration_seconds: elapsed,
    timeout_seconds: 600, process_exit_code: exitCode, process_timed_out: timedOut,
    registration_root: permit.registration_root, image_digest: permit.image_digest,
    participant_configuration_root: permit.participant_configuration_root, assignment_root: permit.assignment_root,
    trust_bundle_bytes: permit.trust_bundle_bytes,
    prompt_root: permit.prompt_root, packet_root: permit.packet_root, provider_events_bytes: sha(eventBytes),
    provider_stderr_bytes: sha(stderrBytes), response_bytes: statSafe(outputPath), event_receipt: eventReceipt,
    cumulative_provider_usage_is_telemetry_only: true, credential_retained: false
  };
  writeExclusive("terminal-receipt.json", `${JSON.stringify(receipt, null, 2)}\n`);
  if (status !== "completed") throw new Error(`terminal_non_result:${validationError}`);
}

function statSafe(path) {
  try { return sha(bytes(path)); } catch { return null; }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(error => fail(String(error.message || error)));
}

export { compileResponseSchema, forbiddenEvent, inspectEvents, root, sha, validateResponseAgainstSchema, verifyBindings };
