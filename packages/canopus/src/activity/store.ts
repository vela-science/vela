import { constants } from "node:fs";
import { mkdir, open } from "node:fs/promises";
import path from "node:path";

import { canonicalJson, contentDigest } from "../util/canonical.js";
import { readBoundedRegularFile } from "../util/files.js";
import {
  ACTIVITY_SCHEMA,
  ACTIVITY_TYPES,
  WRITABLE_ACTIVITY_TYPES,
  type ActivityEvent,
  type ActivityEventBody,
  type ActivityType,
  type WritableActivityType,
} from "./events.js";

const MAX_ACTIVITY_BYTES = 64 * 1024 * 1024;
const MAX_EVENT_BYTES = 1024 * 1024;

function parseEvent(value: unknown, line: number): ActivityEvent {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`activity line ${line} is not an object`);
  }
  const event = value as Record<string, unknown>;
  const expected = new Set([
    "schema",
    "run_id",
    "sequence",
    "at",
    "type",
    "previous",
    "payload",
    "event_digest",
  ]);
  if (Object.keys(event).some((key) => !expected.has(key)) || Object.keys(event).length !== expected.size) {
    throw new Error(`activity line ${line} has an invalid field set`);
  }
  if (
    event.schema !== ACTIVITY_SCHEMA ||
    typeof event.run_id !== "string" ||
    event.run_id.length === 0 ||
    typeof event.sequence !== "number" ||
    !Number.isSafeInteger(event.sequence) ||
    event.sequence < 0 ||
    typeof event.at !== "string" ||
    Number.isNaN(Date.parse(event.at)) ||
    typeof event.type !== "string" ||
    !ACTIVITY_TYPES.includes(event.type as ActivityType) ||
    (event.previous !== null && typeof event.previous !== "string") ||
    typeof event.payload !== "object" ||
    event.payload === null ||
    Array.isArray(event.payload) ||
    typeof event.event_digest !== "string"
  ) {
    throw new Error(`activity line ${line} is malformed`);
  }
  const body: ActivityEventBody = {
    schema: ACTIVITY_SCHEMA,
    run_id: event.run_id,
    sequence: event.sequence,
    at: event.at,
    type: event.type as ActivityType,
    previous: event.previous,
    payload: event.payload as Record<string, unknown>,
  };
  const expectedDigest = contentDigest(body);
  if (event.event_digest !== expectedDigest) {
    throw new Error(`activity line ${line} has an invalid event digest`);
  }
  return { ...body, event_digest: expectedDigest };
}

export async function readActivity(file: string): Promise<ActivityEvent[]> {
  let bytes: Buffer;
  try {
    bytes = await readBoundedRegularFile(file, MAX_ACTIVITY_BYTES);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw error;
  }
  const text = bytes.toString("utf8");
  if (text.length !== 0 && !text.endsWith("\n")) {
    throw new Error("activity log ends with a partial record");
  }
  const lines = text.split("\n").filter((line) => line.length > 0);
  const events: ActivityEvent[] = [];
  for (const [index, line] of lines.entries()) {
    if (Buffer.byteLength(line) > MAX_EVENT_BYTES) throw new Error(`activity line ${index + 1} is oversized`);
    let value: unknown;
    try {
      value = JSON.parse(line) as unknown;
    } catch {
      throw new Error(`activity line ${index + 1} is invalid JSON`);
    }
    const event = parseEvent(value, index + 1);
    const previous = events.at(-1);
    if (event.sequence !== index || event.previous !== (previous?.event_digest ?? null)) {
      throw new Error(`activity line ${index + 1} breaks the append-only chain`);
    }
    if (previous !== undefined && event.run_id !== previous.run_id) {
      throw new Error(`activity line ${index + 1} changes run_id`);
    }
    events.push(event);
  }
  return events;
}

export class ActivityStore {
  readonly #file: string;
  readonly #runId: string;
  #events: ActivityEvent[];
  #queue: Promise<void> = Promise.resolve();

  private constructor(file: string, runId: string, events: ActivityEvent[]) {
    this.#file = file;
    this.#runId = runId;
    this.#events = events;
  }

  public static async open(file: string, runId: string): Promise<ActivityStore> {
    if (!/^run_[a-z0-9-]{8,128}$/u.test(runId)) throw new Error("invalid run_id");
    await mkdir(path.dirname(file), { recursive: true, mode: 0o700 });
    const events = await readActivity(file);
    if (events.some((event) => event.run_id !== runId)) {
      throw new Error("activity log belongs to a different run");
    }
    return new ActivityStore(file, runId, events);
  }

  public get tip(): string | null {
    return this.#events.at(-1)?.event_digest ?? null;
  }

  public get events(): readonly ActivityEvent[] {
    return this.#events;
  }

  public async append(
    type: WritableActivityType,
    payload: Record<string, unknown>,
  ): Promise<ActivityEvent> {
    let resolveEvent: (event: ActivityEvent) => void;
    let rejectEvent: (error: unknown) => void;
    const result = new Promise<ActivityEvent>((resolve, reject) => {
      resolveEvent = resolve;
      rejectEvent = reject;
    });
    this.#queue = this.#queue.then(async () => {
      try {
        if (!WRITABLE_ACTIVITY_TYPES.includes(type)) {
          throw new Error(`retired or unknown activity type ${type}`);
        }
        const body: ActivityEventBody = {
          schema: ACTIVITY_SCHEMA,
          run_id: this.#runId,
          sequence: this.#events.length,
          at: new Date().toISOString(),
          type,
          previous: this.tip,
          payload,
        };
        const event: ActivityEvent = { ...body, event_digest: contentDigest(body) };
        const line = canonicalJson(event);
        if (Buffer.byteLength(line) > MAX_EVENT_BYTES) throw new Error("activity event is oversized");
        const noFollow = "O_NOFOLLOW" in constants ? constants.O_NOFOLLOW : 0;
        const handle = await open(
          this.#file,
          constants.O_WRONLY | constants.O_APPEND | constants.O_CREAT | noFollow,
          0o600,
        );
        try {
          await handle.writeFile(line);
          await handle.sync();
        } finally {
          await handle.close();
        }
        this.#events = [...this.#events, event];
        resolveEvent(event);
      } catch (error) {
        rejectEvent(error);
      }
    });
    return await result;
  }
}
