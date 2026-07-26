import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { ActivityStore, readActivity } from "../src/activity/store.js";
import { canonicalJson, contentDigest } from "../src/util/canonical.js";

test("activity store appends a content-addressed chain and replays it", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-activity-"));
  const file = path.join(root, "run.jsonl");
  const store = await ActivityStore.open(file, "run_12345678");
  const first = await store.append("run.started", { mission_id: "mission_test" });
  const second = await store.append("engine.completed", { status: "success" });
  assert.equal(first.sequence, 0);
  assert.equal(second.previous, first.event_digest);
  const replayed = await readActivity(file);
  assert.deepEqual(replayed, [first, second]);

  const resumed = await ActivityStore.open(file, "run_12345678");
  const third = await resumed.append("run.completed", { ok: true });
  assert.equal(third.sequence, 2);
});

test("activity replay rejects tamper, truncation, and unknown fields", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-activity-tamper-"));
  const file = path.join(root, "run.jsonl");
  const store = await ActivityStore.open(file, "run_abcdefgh");
  await store.append("run.started", { mission_id: "mission_test" });
  const line = (await readFile(file, "utf8")).trim();
  const value = JSON.parse(line) as Record<string, unknown>;
  value.payload = { mission_id: "changed" };
  await writeFile(file, `${JSON.stringify(value)}\n`);
  await assert.rejects(readActivity(file), /invalid event digest/u);

  await writeFile(file, line.slice(0, -3));
  await assert.rejects(readActivity(file), /partial record/u);

  const original = JSON.parse(line) as Record<string, unknown>;
  original.authority = "accepted";
  await writeFile(file, `${JSON.stringify(original)}\n`);
  await assert.rejects(readActivity(file), /invalid field set/u);
});

test("historical withdrawal activity replays but cannot be newly emitted", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-activity-historical-"));
  const file = path.join(root, "run.jsonl");
  const body = {
    schema: "canopus.activity.v0" as const,
    run_id: "run_historical",
    sequence: 0,
    at: "2026-07-17T00:00:00Z",
    type: "withdrawal_capability.retained" as const,
    previous: null,
    payload: {
      proposal_id: "vpr_0123456789abcdef",
      authority: "producer_withdrawal_only",
    },
  };
  await writeFile(
    file,
    canonicalJson({ ...body, event_digest: contentDigest(body) }),
  );
  const events = await readActivity(file);
  assert.equal(events.length, 1);
  assert.equal(events[0]?.type, "withdrawal_capability.retained");

  const active = await ActivityStore.open(path.join(root, "active.jsonl"), "run_active01");
  await assert.rejects(
    active.append(
      "withdrawal_capability.retained" as never,
      { proposal_id: "vpr_0123456789abcdef" },
    ),
    /retired or unknown activity type/u,
  );
});
