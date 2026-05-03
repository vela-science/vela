#!/usr/bin/env -S bun run
//
// Vela reducer — third implementation, TypeScript stdlib-only.
//
// What this proves: the per-kind reducer mutation rules are protocol,
// not a Rust artifact and not a Python artifact. Three independent
// implementations of the reducer (this TypeScript one, the stdlib
// Python one in clients/python/vela_reducer.py, and the Rust one in
// crates/vela-protocol/src/reducer.rs) must produce byte-equivalent
// post-replay finding state from the same canonical event log on the
// same genesis findings. If any pair disagrees, one of the three is
// wrong.
//
// Usage:
//   bun  clients/typescript/vela_reducer.ts <fixture-or-dir> [--json]
//   node --experimental-strip-types clients/typescript/vela_reducer.ts <fixture-or-dir>
//   deno run --allow-read clients/typescript/vela_reducer.ts <fixture-or-dir>
//
// Exit codes:
//   0  — every fixture's expected_states matched after TS replay
//   1  — at least one fixture mismatched (cross-implementation drift)
//   2  — fixture directory empty, malformed, or unreadable
//
// This implementation deliberately uses only Node-compatible stdlib
// (fs, path) so a reviewer can read it end to end and reason about
// whether it's doing the same thing the Rust + Python reducers do.
// The matching Rust source is documented inline next to each apply_*
// function; the matching Python source has identical function names.
//
// Fixture schema: vela.science/schema/cross-impl-reducer-fixture/v1
// Generator: crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs

import { readFileSync, statSync, readdirSync } from "node:fs";
import { join, resolve, basename } from "node:path";
import { argv, exit, stdout, stderr } from "node:process";

// ── Shared types ───────────────────────────────────────────────────

type Json = string | number | boolean | null | Json[] | { [k: string]: Json };
type Finding = { [k: string]: Json } & { id?: string };
type Event = {
  id?: string;
  kind?: string;
  payload?: { [k: string]: Json };
  target?: { id?: string };
  actor?: { id?: string };
  timestamp?: string;
  reason?: string;
  [k: string]: Json | undefined;
};

// ── Per-kind reducer rules ─────────────────────────────────────────
//
// Each function mirrors a `fn apply_finding_*` in the Rust source at
// crates/vela-protocol/src/reducer.rs and the Python reducer at
// clients/python/vela_reducer.py. The mutation rules are kept in
// sync by the cross-impl fixture test:
//   crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs

// ReviewState → contested mapping. Mirrors `ReviewState::implies_contested`
// in bundle.rs:1278-1288.
const _CONTESTED_REVIEW_STATES = new Set([
  "contested",
  "needs_revision",
  "rejected",
]);

function _findFinding(state: Finding[], findingId: string): Finding | undefined {
  return state.find((f) => f.id === findingId);
}

function _ensureFlags(f: Finding): { [k: string]: Json } {
  if (!f.flags || typeof f.flags !== "object" || Array.isArray(f.flags)) {
    f.flags = {};
  }
  return f.flags as { [k: string]: Json };
}

function _ensureAnnotations(f: Finding): Json[] {
  if (!Array.isArray(f.annotations)) f.annotations = [];
  return f.annotations as Json[];
}

function _ensureConfidence(f: Finding): { [k: string]: Json } {
  if (
    !f.confidence ||
    typeof f.confidence !== "object" ||
    Array.isArray(f.confidence)
  ) {
    f.confidence = {};
  }
  return f.confidence as { [k: string]: Json };
}

function _deepClone<T>(x: T): T {
  return JSON.parse(JSON.stringify(x));
}

// Key-order-independent JSON for cross-impl comparison. The Python and
// Rust effect rows can serialize keys in any order; what matters is
// the value at each key. Sort keys at every level before stringifying.
function canonicalJson(x: unknown): string {
  function sort(v: unknown): unknown {
    if (Array.isArray(v)) return v.map(sort);
    if (v && typeof v === "object") {
      const obj = v as { [k: string]: unknown };
      const out: { [k: string]: unknown } = {};
      for (const k of Object.keys(obj).sort()) out[k] = sort(obj[k]);
      return out;
    }
    return v;
  }
  return JSON.stringify(sort(x));
}

// Mirror of reducer.rs::apply_finding_asserted.
// For v0.3+ frontiers a genesis event may carry the finding inline at
// payload.finding; for legacy frontiers the finding is already in
// state from genesis and this is a no-op.
function applyFindingAsserted(state: Finding[], event: Event): void {
  const payload = event.payload ?? {};
  const finding = payload.finding as Finding | undefined;
  if (!finding) return;
  if (state.some((f) => f.id === finding.id)) return;
  state.push(_deepClone(finding));
}

// Mirror of reducer.rs::apply_finding_reviewed.
// Sets flags.review_state from the snake_case status; sets
// flags.contested per ReviewState::implies_contested.
// Accepts both 'accepted' and 'approved' (Rust accepts both).
function applyFindingReviewed(state: Finding[], event: Event): void {
  const payload = event.payload ?? {};
  const status = payload.status;
  if (typeof status !== "string") {
    throw new Error("finding.reviewed missing payload.status");
  }
  const findingId = event.target?.id ?? "";
  const f = _findFinding(state, findingId);
  if (!f) {
    throw new Error(`finding.reviewed targets unknown finding ${findingId}`);
  }
  const flags = _ensureFlags(f);
  if (status === "accepted" || status === "approved") {
    flags.review_state = "accepted";
    flags.contested = false;
  } else if (status === "contested") {
    flags.review_state = "contested";
    flags.contested = true;
  } else if (status === "needs_revision") {
    flags.review_state = "needs_revision";
    flags.contested = true;
  } else if (status === "rejected") {
    flags.review_state = "rejected";
    flags.contested = true;
  } else {
    throw new Error(`unsupported review status ${JSON.stringify(status)}`);
  }
}

// Mirror of reducer.rs::apply_finding_annotation.
// Idempotent on annotation_id. Adds an Annotation with id, text,
// author=event.actor.id, timestamp=event.timestamp.
function applyFindingAnnotation(state: Finding[], event: Event): void {
  const payload = event.payload ?? {};
  const text = payload.text;
  const annotationId = payload.annotation_id;
  if (typeof text !== "string" || typeof annotationId !== "string") {
    throw new Error("annotation event missing text or annotation_id");
  }
  const findingId = event.target?.id ?? "";
  const f = _findFinding(state, findingId);
  if (!f) {
    throw new Error(`annotation event targets unknown finding ${findingId}`);
  }
  const annotations = _ensureAnnotations(f);
  if (
    annotations.some((a) => (a as { [k: string]: Json }).id === annotationId)
  ) {
    return;
  }
  annotations.push({
    id: annotationId,
    text,
    author: event.actor?.id ?? "",
    timestamp: event.timestamp ?? "",
    provenance: payload.provenance ?? null,
  });
}

// Mirror of reducer.rs::apply_finding_confidence_revised.
// Sets confidence.score, basis, method=expert_judgment.
function applyFindingConfidenceRevised(state: Finding[], event: Event): void {
  const payload = event.payload ?? {};
  const newScore = payload.new_score;
  const previous = (payload.previous_score as number | undefined) ?? 0.0;
  if (typeof newScore !== "number") {
    throw new Error("finding.confidence_revised missing payload.new_score");
  }
  const findingId = event.target?.id ?? "";
  const f = _findFinding(state, findingId);
  if (!f) {
    throw new Error(
      `confidence_revised targets unknown finding ${findingId}`,
    );
  }
  const conf = _ensureConfidence(f);
  conf.score = newScore;
  conf.basis =
    `expert revision from ${previous.toFixed(3)} to ${newScore.toFixed(3)}: ` +
    `${event.reason ?? ""}`;
  conf.method = "expert_judgment";
}

// Mirror of reducer.rs::apply_finding_rejected. Sets contested=true.
function applyFindingRejected(state: Finding[], event: Event): void {
  const findingId = event.target?.id ?? "";
  const f = _findFinding(state, findingId);
  if (!f) {
    throw new Error(`finding.rejected targets unknown finding ${findingId}`);
  }
  _ensureFlags(f).contested = true;
}

// Mirror of reducer.rs::apply_finding_retracted. Sets retracted=true.
function applyFindingRetracted(state: Finding[], event: Event): void {
  const findingId = event.target?.id ?? "";
  const f = _findFinding(state, findingId);
  if (!f) {
    throw new Error(`finding.retracted targets unknown finding ${findingId}`);
  }
  _ensureFlags(f).retracted = true;
}

// Mirror of reducer.rs::apply_finding_dependency_invalidated.
// Sets contested=true and appends a deterministic annotation whose
// id encodes the upstream cascade event and the depth.
//
// Rust shape:
//   annotation_id = format!("ann_dep_{}_{}", &event.id[4..], depth);
// The "vev_" prefix on event.id is stripped by [4..] — TS does
// the same with .slice(4).
function applyFindingDependencyInvalidated(
  state: Finding[],
  event: Event,
): void {
  const payload = event.payload ?? {};
  const upstream = (payload.upstream_finding_id as string | undefined) ?? "?";
  const depth = (payload.depth as number | undefined) ?? 1;
  const findingId = event.target?.id ?? "";
  const f = _findFinding(state, findingId);
  if (!f) {
    throw new Error(
      `finding.dependency_invalidated targets unknown finding ${findingId}`,
    );
  }
  _ensureFlags(f).contested = true;
  const eventId = event.id ?? "";
  const eventTail = eventId.startsWith("vev_") ? eventId.slice(4) : eventId;
  const annotationId = `ann_dep_${eventTail}_${depth}`;
  const annotations = _ensureAnnotations(f);
  if (
    annotations.some((a) => (a as { [k: string]: Json }).id === annotationId)
  ) {
    return;
  }
  annotations.push({
    id: annotationId,
    text: `Upstream ${upstream} retracted (cascade depth ${depth}).`,
    author: event.actor?.id ?? "",
    timestamp: event.timestamp ?? "",
    provenance: null,
  });
}

function applyEvent(state: Finding[], event: Event): void {
  const kind = event.kind ?? "";
  if (kind === "frontier.created") {
    return; // structural anchor, no mutation
  } else if (kind === "finding.asserted") {
    applyFindingAsserted(state, event);
  } else if (kind === "finding.reviewed") {
    applyFindingReviewed(state, event);
  } else if (kind === "finding.noted" || kind === "finding.caveated") {
    applyFindingAnnotation(state, event);
  } else if (kind === "finding.confidence_revised") {
    applyFindingConfidenceRevised(state, event);
  } else if (kind === "finding.rejected") {
    applyFindingRejected(state, event);
  } else if (kind === "finding.retracted") {
    applyFindingRetracted(state, event);
  } else if (kind === "finding.dependency_invalidated") {
    applyFindingDependencyInvalidated(state, event);
  } else {
    throw new Error(`reducer: unsupported event kind ${JSON.stringify(kind)}`);
  }
}

// ── Reducer-effects digest ─────────────────────────────────────────
//
// Mirror of `finding_state` in
// crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs.
// Captures only the fields the reducer mutates so cross-impl agreement
// is testable without serializing the full Project struct.

interface EffectRow {
  id: string;
  retracted: boolean;
  contested: boolean;
  review_state: string;
  confidence_score: string;
  annotation_ids: string[];
}

function reducerEffects(state: Finding[]): EffectRow[] {
  const sorted = [...state].sort((a, b) =>
    (a.id ?? "").localeCompare(b.id ?? ""),
  );
  return sorted.map((f) => {
    const flags = (f.flags ?? {}) as { [k: string]: Json };
    const reviewState = (flags.review_state as string | undefined) ?? "none";
    const confidence = (f.confidence ?? {}) as { [k: string]: Json };
    const annotations = (f.annotations ?? []) as { id?: string }[];
    const annotationIds = annotations
      .map((a) => a.id ?? "")
      .sort((x, y) => x.localeCompare(y));
    // Format score to 6 decimals so f64 noise can't cross the
    // implementation boundary. Rust uses `format!("{:.6}", score)`,
    // Python matches with `f"{score:.6f}"`, TS with `.toFixed(6)`.
    const score = Number(confidence.score ?? 0.0);
    return {
      id: f.id ?? "",
      retracted: Boolean(flags.retracted ?? false),
      contested: Boolean(flags.contested ?? false),
      review_state: reviewState,
      confidence_score: score.toFixed(6),
      annotation_ids: annotationIds,
    };
  });
}

// ── Fixture verification ───────────────────────────────────────────

interface FixtureResult {
  path: string;
  frontierIdx: number;
  findings: number;
  events: number;
  cascadeDepth: number;
  matched: number;
  diffs: { id: string; issue: string; expected?: EffectRow; actual?: EffectRow }[];
  ok: boolean;
  error: string | null;
}

function verifyFixture(path: string): FixtureResult {
  const result: FixtureResult = {
    path,
    frontierIdx: -1,
    findings: 0,
    events: 0,
    cascadeDepth: 0,
    matched: 0,
    diffs: [],
    ok: false,
    error: null,
  };
  let fx: { [k: string]: Json };
  try {
    fx = JSON.parse(readFileSync(path, "utf8"));
  } catch (e) {
    result.error = `unreadable fixture: ${(e as Error).message}`;
    return result;
  }
  if (fx.fixture_version !== "1") {
    result.error = `unsupported fixture_version ${JSON.stringify(fx.fixture_version)}; expected '1'`;
    return result;
  }
  result.frontierIdx = Number(fx.frontier_idx ?? -1);
  const stats = (fx.stats ?? {}) as { [k: string]: Json };
  result.findings = Number(stats.findings ?? 0);
  result.events = Number(stats.events ?? 0);
  result.cascadeDepth = Number(stats.cascade_depth ?? 0);

  const state: Finding[] = _deepClone(
    (fx.genesis_findings as Finding[]) ?? [],
  );
  const eventLog = (fx.event_log as Event[]) ?? [];
  const expected = (fx.expected_states as EffectRow[]) ?? [];

  for (const event of eventLog) {
    try {
      applyEvent(state, event);
    } catch (e) {
      result.error =
        `reducer error on event ${event.id ?? "?"} (${event.kind ?? "?"}): ` +
        (e as Error).message;
      return result;
    }
  }

  const actual = reducerEffects(state);
  const actualById = new Map(actual.map((r) => [r.id, r]));
  const expectedById = new Map(expected.map((r) => [r.id, r]));
  const allIds = [
    ...new Set([...actualById.keys(), ...expectedById.keys()]),
  ].sort();

  for (const fid of allIds) {
    const a = actualById.get(fid);
    const e = expectedById.get(fid);
    if (!a) {
      result.diffs.push({ id: fid, issue: "missing in ts output", expected: e });
    } else if (!e) {
      result.diffs.push({ id: fid, issue: "extra in ts output", actual: a });
    } else if (canonicalJson(a) !== canonicalJson(e)) {
      result.diffs.push({ id: fid, issue: "mismatch", expected: e, actual: a });
    } else {
      result.matched += 1;
    }
  }

  result.ok = result.diffs.length === 0 && result.matched === expected.length;
  return result;
}

function renderText(results: FixtureResult[]): string {
  const lines: string[] = [];
  lines.push("vela reducer (typescript · stdlib · third implementation)");
  for (const r of results) {
    const status = r.ok ? "ok" : "FAIL";
    lines.push(
      `  ${status.padEnd(4)} · frontier ${String(r.frontierIdx).padStart(2, "0")} · ` +
        `${r.matched}/${r.findings} findings · ${r.events} events · ` +
        `cascade depth ${r.cascadeDepth}`,
    );
    if (r.error) lines.push(`          error: ${r.error}`);
    for (const d of r.diffs.slice(0, 5)) {
      lines.push(`          · ${d.id}: ${d.issue}`);
      if (d.expected && d.actual) {
        const exp = d.expected as unknown as { [k: string]: Json };
        const act = d.actual as unknown as { [k: string]: Json };
        const allKeys = [
          ...new Set([...Object.keys(exp), ...Object.keys(act)]),
        ].sort();
        for (const k of allKeys) {
          if (JSON.stringify(exp[k]) !== JSON.stringify(act[k])) {
            lines.push(
              `              ${k}: expected=${JSON.stringify(exp[k])} actual=${JSON.stringify(act[k])}`,
            );
          }
        }
      }
    }
    if (r.diffs.length > 5) {
      lines.push(`          (… ${r.diffs.length - 5} more)`);
    }
  }
  if (results.every((r) => r.ok)) {
    lines.push("");
    lines.push("reducer: ok");
    lines.push(
      "  every event-log replay through the typescript reducer produced",
    );
    lines.push(
      "  the same per-finding state the rust and python reducers produced.",
    );
    lines.push(
      "  the per-kind mutation rules are now confirmed across three",
    );
    lines.push("  independent implementations.");
  }
  return lines.join("\n");
}

function collectFixtures(target: string): string[] {
  const abs = resolve(target);
  let stat;
  try {
    stat = statSync(abs);
  } catch {
    return [];
  }
  if (stat.isFile()) return [abs];
  if (stat.isDirectory()) {
    return readdirSync(abs)
      .filter((f) => f.startsWith("cascade-fixture-") && f.endsWith(".json"))
      .sort()
      .map((f) => join(abs, f));
  }
  return [];
}

function main(args: string[]): number {
  let jsonMode = false;
  const positional: string[] = [];
  for (const a of args) {
    if (a === "--json") jsonMode = true;
    else if (a === "-h" || a === "--help") {
      stdout.write(
        "usage: vela_reducer.ts <fixture-or-dir> [--json]\n" +
          "  Verify byte-equivalent reducer state against the rust implementation.\n",
      );
      return 0;
    } else positional.push(a);
  }
  const target = positional[0];
  if (!target) {
    stderr.write("error: missing fixture path\n");
    return 2;
  }

  const fixtures = collectFixtures(target);
  if (fixtures.length === 0) {
    stderr.write(`error: no cascade-fixture-*.json found at ${target}\n`);
    return 2;
  }

  const results = fixtures.map(verifyFixture);

  if (jsonMode) {
    stdout.write(
      JSON.stringify(
        {
          ok: results.every((r) => r.ok),
          fixtures: results.map((r) => ({
            path: basename(r.path),
            frontier_idx: r.frontierIdx,
            ok: r.ok,
            findings: r.findings,
            events: r.events,
            cascade_depth: r.cascadeDepth,
            matched: r.matched,
            diffs: r.diffs,
            error: r.error,
          })),
          verifier:
            "vela_reducer.ts · typescript stdlib · third implementation",
        },
        null,
        2,
      ) + "\n",
    );
  } else {
    stdout.write(renderText(results) + "\n");
  }

  return results.every((r) => r.ok) ? 0 : 1;
}

exit(main(argv.slice(2)));
