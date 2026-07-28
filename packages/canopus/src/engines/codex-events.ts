import { objectAt } from "../contracts/validation.js";
import type { EngineUsage } from "./engine.js";

const EVENT_TYPES = new Set([
  "thread.started",
  "turn.started",
  "item.started",
  "item.updated",
  "item.completed",
  "turn.completed",
]);
const PASSIVE_ITEMS = new Set(["agent_message", "reasoning", "todo_list"]);
const ACTION_ITEMS = new Set(["command_execution", "file_change"]);
const FORBIDDEN_COMMAND = /(?:^|[\s;&|])(?:vela\s+sign|git\s+push|gh\s+|curl\s+|wget\s+|ssh\s+)/iu;
const SECRET_ASSIGNMENT = /\b(?:api[_-]?key|access[_-]?token|authorization|password|secret)\b\s*[:=]\s*[^\s,;]+/giu;
const SECRET_TOKEN = /\b(?:sk|sess|key)-[A-Za-z0-9_-]{8,}\b/gu;
const BEARER_TOKEN = /\bBearer\s+[A-Za-z0-9._~+\/-]+=*/giu;
const URL_WITH_QUERY = /https?:\/\/[^\s?#]+[?#][^\s]*/giu;
const MAX_FAILURE_LINES = 256;
const MAX_FAILURE_MESSAGE_CHARS = 512;

export interface CodexEventSummary {
  usage: EngineUsage;
  eventTypes: string[];
  actionTypes: string[];
}

export interface CodexStructuralSummary {
  lines: number;
  parsed_lines: number;
  invalid_lines: number;
  event_types: Record<string, number>;
  item_types: Record<string, number>;
}

function safeEventLabel(value: unknown): string {
  return typeof value === "string" &&
      (EVENT_TYPES.has(value) || value === "error" || value === "turn.failed")
    ? value
    : "other";
}

function safeItemLabel(value: unknown): string {
  return typeof value === "string" &&
      (PASSIVE_ITEMS.has(value) || ACTION_ITEMS.has(value) || value === "error")
    ? value
    : "other";
}

function increment(counts: Record<string, number>, label: string): void {
  counts[label] = (counts[label] ?? 0) + 1;
}

/**
 * Describe only the shape of a Codex JSONL stream. Values that could contain
 * prompts, paths, commands, host data, or credentials are never returned.
 */
export function summarizeCodexStructure(value: string): CodexStructuralSummary {
  const summary: CodexStructuralSummary = {
    lines: 0,
    parsed_lines: 0,
    invalid_lines: 0,
    event_types: {},
    item_types: {},
  };
  for (const line of value.split("\n")) {
    if (line.length === 0) continue;
    summary.lines += 1;
    if (Buffer.byteLength(line) > 1_048_576) {
      summary.invalid_lines += 1;
      continue;
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(line) as unknown;
    } catch {
      summary.invalid_lines += 1;
      continue;
    }
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      summary.invalid_lines += 1;
      continue;
    }
    const event = parsed as Record<string, unknown>;
    summary.parsed_lines += 1;
    increment(summary.event_types, safeEventLabel(event.type));
    if (typeof event.item === "object" && event.item !== null && !Array.isArray(event.item)) {
      increment(
        summary.item_types,
        safeItemLabel((event.item as Record<string, unknown>).type),
      );
    }
  }
  summary.event_types = Object.fromEntries(Object.entries(summary.event_types).sort());
  summary.item_types = Object.fromEntries(Object.entries(summary.item_types).sort());
  return summary;
}

function safeDiagnostic(value: string): string {
  const normalized = value
    .replace(/[\u0000-\u001f\u007f]+/gu, " ")
    .replace(URL_WITH_QUERY, "[url-with-query-redacted]")
    .replace(BEARER_TOKEN, "Bearer [redacted]")
    .replace(SECRET_TOKEN, "[secret-redacted]")
    .replace(SECRET_ASSIGNMENT, "[secret-assignment-redacted]")
    .replace(/\s+/gu, " ")
    .trim();
  return [...normalized].slice(0, MAX_FAILURE_MESSAGE_CHARS).join("");
}

/**
 * Extract a bounded diagnostic from Codex's documented JSONL failure events.
 * Raw stderr and unstructured stdout are intentionally excluded because they
 * can contain credential, host, or prompt material.
 */
export function summarizeCodexFailure(value: string): string {
  const diagnostics: string[] = [];
  const lines = value.split("\n").filter((line) => line.length > 0).slice(0, MAX_FAILURE_LINES);
  for (const line of lines) {
    if (Buffer.byteLength(line) > 1_048_576) continue;
    let event: Record<string, unknown>;
    try {
      const parsed = JSON.parse(line) as unknown;
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) continue;
      event = parsed as Record<string, unknown>;
    } catch {
      continue;
    }
    let message: unknown;
    if (event.type === "error") {
      message = event.message;
    } else if (event.type === "turn.failed") {
      const error = event.error;
      if (typeof error === "object" && error !== null && !Array.isArray(error)) {
        message = (error as Record<string, unknown>).message;
      }
    } else if (
      (event.type === "item.started" ||
        event.type === "item.updated" ||
        event.type === "item.completed") &&
      typeof event.item === "object" &&
      event.item !== null &&
      !Array.isArray(event.item)
    ) {
      const item = event.item as Record<string, unknown>;
      if (item.type === "error") message = item.message;
    }
    if (typeof message !== "string" || message.length === 0) continue;
    const diagnostic = safeDiagnostic(message);
    if (diagnostic.length > 0 && !diagnostics.includes(diagnostic)) diagnostics.push(diagnostic);
    if (diagnostics.length === 3) break;
  }
  return diagnostics.length === 0
    ? "no structured Codex failure event"
    : diagnostics.join("; ");
}

function usageInteger(value: unknown, at: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${at} must be a nonnegative integer`);
  }
  return value;
}

export function parseCodexEvents(value: string): CodexEventSummary {
  const eventTypes = new Set<string>();
  const actionTypes = new Set<string>();
  const itemKinds = new Map<string, string>();
  let usage: EngineUsage | undefined;

  const lines = value.split("\n").filter((line) => line.length > 0);
  if (lines.length === 0) throw new Error("Codex returned no JSONL events");
  for (const [index, line] of lines.entries()) {
    if (Buffer.byteLength(line) > 1_048_576) throw new Error(`Codex event ${index} is oversized`);
    let parsed: unknown;
    try {
      parsed = JSON.parse(line) as unknown;
    } catch {
      throw new Error(`Codex event ${index} is invalid JSON`);
    }
    const event = objectAt(parsed, `codex_event[${index}]`);
    const type = event.type;
    if (typeof type !== "string" || !EVENT_TYPES.has(type)) {
      throw new Error(`Codex event ${index} has unknown type ${String(type)}`);
    }
    eventTypes.add(type);
    if (type === "thread.started" && (typeof event.thread_id !== "string" || event.thread_id === "")) {
      throw new Error("Codex thread.started event has no thread_id");
    }
    if (type === "turn.completed") {
      const rawUsage = objectAt(event.usage, "codex turn.completed.usage");
      usage = {
        input_tokens: usageInteger(rawUsage.input_tokens, "usage.input_tokens"),
        cached_input_tokens: usageInteger(
          rawUsage.cached_input_tokens,
          "usage.cached_input_tokens",
        ),
        output_tokens: usageInteger(rawUsage.output_tokens, "usage.output_tokens"),
        reasoning_output_tokens: usageInteger(
          rawUsage.reasoning_output_tokens,
          "usage.reasoning_output_tokens",
        ),
      };
    }
    if (type === "item.started" || type === "item.updated" || type === "item.completed") {
      const item = objectAt(event.item, `codex ${type}.item`);
      if (typeof item.id !== "string" || item.id === "" || typeof item.type !== "string") {
        throw new Error(`Codex ${type} has a malformed item`);
      }
      const previous = itemKinds.get(item.id);
      if (previous !== undefined && previous !== item.type) {
        throw new Error(`Codex item ${item.id} changed type`);
      }
      itemKinds.set(item.id, item.type);
      if (!PASSIVE_ITEMS.has(item.type) && !ACTION_ITEMS.has(item.type)) {
        throw new Error(`Codex emitted unsupported action type ${item.type}`);
      }
      if (ACTION_ITEMS.has(item.type)) actionTypes.add(item.type);
      if (type === "item.completed" && item.type === "command_execution") {
        if (typeof item.command !== "string" || item.command === "") {
          throw new Error("Codex completed command has no command text");
        }
        if (FORBIDDEN_COMMAND.test(item.command)) {
          throw new Error(`Codex attempted a forbidden external or custody action: ${item.command}`);
        }
      }
    }
  }
  for (const required of ["thread.started", "turn.started", "turn.completed"]) {
    if (!eventTypes.has(required)) throw new Error(`Codex event stream is missing ${required}`);
  }
  if (usage === undefined) throw new Error("Codex event stream has no usage record");
  return {
    usage,
    eventTypes: [...eventTypes].sort(),
    actionTypes: [...actionTypes].sort(),
  };
}
