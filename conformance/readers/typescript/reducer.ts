#!/usr/bin/env -S bun run
//
// Vela reducer — third implementation, TypeScript stdlib-only.
//
// What this proves: the per-kind reducer mutation rules are protocol,
// not a Rust artifact and not a Python artifact. Three independent
// implementations of the reducer (this TypeScript one, the stdlib
// Python one in conformance/readers/python/reducer.py, and the Rust one in
// crates/vela-protocol/src/reducer.rs) must produce byte-equivalent
// post-replay finding state from the same canonical event log on the
// same genesis findings. If any pair disagrees, one of the three is
// wrong.
//
// Usage:
//   bun conformance/readers/typescript/reducer.ts <fixture-or-dir> [--json]
//   node --experimental-strip-types conformance/readers/typescript/reducer.ts <fixture-or-dir>
//   deno run --allow-read conformance/readers/typescript/reducer.ts <fixture-or-dir>
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
// Fixture schema: vela.science/schema/cross-impl-reducer-fixture/v3
// Generator: crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs

import { readFileSync, statSync, readdirSync } from "node:fs";
import { join, resolve, basename } from "node:path";
import { argv, exit, stdout, stderr } from "node:process";
import {
  createHash,
  createPublicKey,
  verify as verifySignature,
} from "node:crypto";

// ── Shared types ───────────────────────────────────────────────────

type Json = string | number | boolean | null | Json[] | { [k: string]: Json };
type Finding = { [k: string]: Json } & { id?: string };
type Artifact = { [k: string]: Json } & { id?: string };
type Event = {
  id?: string;
  kind?: string;
  payload?: { [k: string]: Json };
  target?: { id?: string; type?: string };
  actor?: { id?: string };
  timestamp?: string;
  reason?: string;
  [k: string]: Json | undefined;
};

// Full reducer state. The TS reducer used to track findings only; it
// now includes current non-finding collections so `tier.set` and
// `artifact.*` events participate in the cross-impl byte-equivalence
// promise.
interface ReducerState {
  findings: Finding[];
  artifacts: Artifact[];
}

// ── Per-kind reducer rules ─────────────────────────────────────────
//
// Each function mirrors a `fn apply_finding_*` in the Rust source at
// crates/vela-protocol/src/reducer.rs and the Python reducer at
// conformance/readers/python/reducer.py. The mutation rules are kept in
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

// ── Signed repository-boundary validation ──────────────────────────
//
// `frontier.repository_bound` is reducer-neutral but not
// validation-neutral. The exact envelope, content address, v1 signature, and
// one linear repository-identity chain are checked before the reducer no-op.

const BOUNDARY_SCHEMA = "vela.frontier-repository-boundary.v1";
const FRONTIER_CREATED_SCHEMA_V1 = "vela.frontier-created.v1";
const FRONTIER_PROFILE_SCHEMA_V1 = "vela.frontier-profile.v1";
const FRONTIER_IDENTITY_SCHEMA_V1 = "vela.frontier-identity.v1";
const EVENT_SCHEMA = "vela.event.v0.1";
const NULL_HASH = "sha256:null";
const EVENT_PAYLOAD_TYPE = "application/vnd.vela.event+json";
const SHA256_ROOT = /^sha256:[0-9a-f]{64}$/;
const LOWER_HEX_64 = /^[0-9a-f]{64}$/;
const FRONTIER_ID = /^vfr_[0-9a-f]{16}$/;
const RFC3339 =
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/;
const BOUNDARY_PAYLOAD_KEYS = new Set([
  "schema",
  "mode",
  "frontier_id",
  "identity_root",
  "observed_profile_root",
  "dependency_root",
  "dependencies",
  "previous_identity_event_root",
  "legacy_identity_preimage_root",
  "administrator_actor_id",
  "administrator_public_key",
  "administrator_algorithm",
  "trust_mode",
  "git_object_format",
  "anchor_git_commit",
  "anchor_git_tree",
  "anchor_event_log_root",
  "anchor_event_count",
  "anchor_snapshot_root",
  "anchor_snapshot_schema",
  "anchor_proposal_root",
  "anchor_actor_registry_root",
  "anchor_artifact_registry_root",
  "anchor_canonical_store_root",
]);
const DEPENDENCY_KEYS = new Set([
  "frontier_id",
  "identity_root",
  "scientific_state_root",
  "git_object_format",
  "git_commit",
  "git_tree",
]);
const FRONTIER_CREATED_PAYLOAD_KEYS = new Set([
  "schema",
  "name_at_creation",
  "creator",
  "profile_schema",
  "dependency_root",
  "created_at",
]);
const BOUNDARY_CASE_IDS = new Set([
  "valid_linear_chain",
  "valid_native_genesis_chain",
  "missing_native_genesis",
  "invalid_native_genesis",
  "unsigned",
  "corrupt_signature",
  "event_id_drift",
  "fixed_envelope_drift",
  "empty_reason",
  "invalid_timestamp",
  "missing_parent",
  "fork",
  "rollback",
]);

type JsonObject = { [key: string]: Json };
type BoundaryRecord = { event: Event; payload: JsonObject };

function objectValue(value: unknown, label: string): JsonObject {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as JsonObject;
}

function hasExactKeys(
  value: JsonObject,
  expected: Set<string>,
): boolean {
  const actual = Object.keys(value);
  return (
    actual.length === expected.size &&
    actual.every((key) => expected.has(key))
  );
}

function sha256Hex(bytes: Uint8Array | string): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalRoot(value: unknown): string {
  return `sha256:${sha256Hex(canonicalJson(value))}`;
}

function eventContent(event: Event): JsonObject {
  return {
    schema: (event.schema as Json) ?? null,
    kind: (event.kind as Json) ?? null,
    target: (event.target as Json) ?? null,
    actor: (event.actor as Json) ?? null,
    timestamp: (event.timestamp as Json) ?? null,
    reason: (event.reason as Json) ?? null,
    before_hash: (event.before_hash as Json) ?? null,
    after_hash: (event.after_hash as Json) ?? null,
    payload: (event.payload as Json) ?? null,
    caveats: (event.caveats as Json) ?? null,
  };
}

function eventContentRoot(event: Event): string {
  return canonicalRoot(eventContent(event));
}

function eventId(event: Event): string {
  return `vev_${eventContentRoot(event).slice(7, 23)}`;
}

function eventSigningBytes(event: Event, version: "v0" | "v1"): Buffer {
  const body = Buffer.from(
    canonicalJson({
      schema: event.schema ?? null,
      id: event.id ?? null,
      kind: event.kind ?? null,
      target: event.target ?? null,
      actor: event.actor ?? null,
      timestamp: event.timestamp ?? null,
      reason: event.reason ?? null,
      before_hash: event.before_hash ?? null,
      after_hash: event.after_hash ?? null,
      payload: event.payload ?? null,
      caveats: event.caveats ?? null,
    }),
    "utf8",
  );
  if (version === "v0") return body;
  const media = Buffer.from(EVENT_PAYLOAD_TYPE, "ascii");
  return Buffer.concat([
    Buffer.from(`DSSEv1 ${media.length} `, "ascii"),
    media,
    Buffer.from(` ${body.length} `, "ascii"),
    body,
  ]);
}

function requireText(field: string, value: unknown): string {
  if (
    typeof value !== "string" ||
    value.trim().length === 0 ||
    value.normalize("NFC") !== value ||
    /[\u0000-\u001f\u007f-\u009f]/u.test(value)
  ) {
    throw new Error(`${field} must be non-empty NFC text without controls`);
  }
  return value;
}

function requireRoot(field: string, value: unknown): string {
  if (typeof value !== "string" || !SHA256_ROOT.test(value)) {
    throw new Error(`${field} must be a full lowercase sha256 root`);
  }
  return value;
}

function requireGitObject(
  field: string,
  value: unknown,
  objectFormat: unknown,
): string {
  const length =
    objectFormat === "sha1" ? 40 : objectFormat === "sha256" ? 64 : 0;
  if (
    length === 0 ||
    typeof value !== "string" ||
    !new RegExp(`^[0-9a-f]{${length}}$`).test(value)
  ) {
    throw new Error(`${field} is not a lowercase ${String(objectFormat)} Git object`);
  }
  return value;
}

function validateDependency(dependencyValue: unknown): [string, string] {
  const dependency = objectValue(
    dependencyValue,
    "repository boundary dependency",
  );
  if (!hasExactKeys(dependency, DEPENDENCY_KEYS)) {
    throw new Error(
      "repository boundary dependency has an open or incomplete shape",
    );
  }
  const frontierId = dependency.frontier_id;
  if (typeof frontierId !== "string" || !FRONTIER_ID.test(frontierId)) {
    throw new Error("dependency.frontier_id is invalid");
  }
  const identityRoot = requireRoot(
    "dependency.identity_root",
    dependency.identity_root,
  );
  requireRoot(
    "dependency.scientific_state_root",
    dependency.scientific_state_root,
  );
  const objectFormat = dependency.git_object_format;
  if (objectFormat !== "sha1" && objectFormat !== "sha256") {
    throw new Error("dependency.git_object_format is invalid");
  }
  requireGitObject(
    "dependency.git_commit",
    dependency.git_commit,
    objectFormat,
  );
  requireGitObject("dependency.git_tree", dependency.git_tree, objectFormat);
  return [frontierId, identityRoot];
}

function validateBoundaryPayload(payloadValue: unknown): JsonObject {
  const payload = objectValue(payloadValue, "repository boundary payload");
  if (!hasExactKeys(payload, BOUNDARY_PAYLOAD_KEYS)) {
    throw new Error(
      "repository boundary payload has an open or incomplete shape",
    );
  }
  if (payload.schema !== BOUNDARY_SCHEMA) {
    throw new Error("repository boundary payload schema mismatch");
  }
  const mode = payload.mode;
  const trustMode = payload.trust_mode;
  if (
    mode !== "temporalize_existing" &&
    mode !== "update_dependencies"
  ) {
    throw new Error("repository boundary mode is invalid");
  }
  if (
    trustMode !== "tofu" &&
    trustMode !== "genesis" &&
    trustMode !== "previous_boundary"
  ) {
    throw new Error("repository boundary trust_mode is invalid");
  }
  const frontierId = payload.frontier_id;
  if (typeof frontierId !== "string" || !FRONTIER_ID.test(frontierId)) {
    throw new Error("repository boundary frontier_id is invalid");
  }
  for (const field of [
    "identity_root",
    "observed_profile_root",
    "dependency_root",
    "anchor_event_log_root",
    "anchor_snapshot_root",
    "anchor_proposal_root",
    "anchor_actor_registry_root",
    "anchor_artifact_registry_root",
    "anchor_canonical_store_root",
  ]) {
    requireRoot(`payload.${field}`, payload[field]);
  }
  for (const field of [
    "previous_identity_event_root",
    "legacy_identity_preimage_root",
  ]) {
    if (payload[field] !== null) requireRoot(`payload.${field}`, payload[field]);
  }
  if (!Array.isArray(payload.dependencies)) {
    throw new Error("payload.dependencies must be an array");
  }
  const dependencies = payload.dependencies;
  const dependencyKeys = dependencies.map(validateDependency);
  const sortedKeys = [...dependencyKeys].sort((left, right) =>
    left[0] === right[0]
      ? left[1].localeCompare(right[1])
      : left[0].localeCompare(right[0]),
  );
  if (
    canonicalJson(dependencyKeys) !== canonicalJson(sortedKeys) ||
    new Set(dependencyKeys.map((key) => `${key[0]}\0${key[1]}`)).size !==
      dependencyKeys.length
  ) {
    throw new Error("payload.dependencies must be uniquely sorted");
  }
  if (canonicalRoot(dependencies) !== payload.dependency_root) {
    throw new Error("payload.dependency_root does not match dependencies");
  }
  const actorId = payload.administrator_actor_id;
  if (
    typeof actorId !== "string" ||
    (!actorId.startsWith("reviewer:") && !actorId.startsWith("steward:"))
  ) {
    throw new Error("payload administrator must be a reviewer or steward");
  }
  const publicKey = payload.administrator_public_key;
  if (typeof publicKey !== "string" || !LOWER_HEX_64.test(publicKey)) {
    throw new Error("payload administrator public key is invalid");
  }
  if (payload.administrator_algorithm !== "ed25519") {
    throw new Error("payload administrator algorithm must be ed25519");
  }
  const objectFormat = payload.git_object_format;
  if (objectFormat !== "sha1" && objectFormat !== "sha256") {
    throw new Error("payload.git_object_format is invalid");
  }
  requireGitObject(
    "payload.anchor_git_commit",
    payload.anchor_git_commit,
    objectFormat,
  );
  requireGitObject(
    "payload.anchor_git_tree",
    payload.anchor_git_tree,
    objectFormat,
  );
  const count = payload.anchor_event_count;
  if (!Number.isSafeInteger(count) || Number(count) <= 0) {
    throw new Error("payload.anchor_event_count must be positive");
  }
  requireText("payload.anchor_snapshot_schema", payload.anchor_snapshot_schema);

  if (mode === "temporalize_existing") {
    if (
      trustMode !== "tofu" ||
      payload.previous_identity_event_root !== null ||
      payload.legacy_identity_preimage_root === null
    ) {
      throw new Error("temporal boundary trust fields are inconsistent");
    }
    const legacyOrigin = {
      schema: "vela.legacy-frontier-origin.v1",
      frontier_id: frontierId,
      legacy_identity_preimage_root: payload.legacy_identity_preimage_root,
      git_object_format: objectFormat,
      anchor_git_commit: payload.anchor_git_commit,
      anchor_git_tree: payload.anchor_git_tree,
      anchor_event_log_root: payload.anchor_event_log_root,
      anchor_event_count: count,
    };
    const identity = {
      schema: "vela.frontier-identity.v1",
      frontier_id: frontierId,
      origin: "legacy_boundary",
      origin_commitment: canonicalRoot(legacyOrigin),
      legacy_identity_preimage_root: payload.legacy_identity_preimage_root,
    };
    if (canonicalRoot(identity) !== payload.identity_root) {
      throw new Error("temporal boundary identity_root is invalid");
    }
  } else if (
    (trustMode !== "genesis" && trustMode !== "previous_boundary") ||
    payload.previous_identity_event_root === null
  ) {
    throw new Error("dependency update trust fields are inconsistent");
  }
  return payload;
}

function validateBoundaryEvent(eventValue: unknown): BoundaryRecord & {
  root: string;
} {
  const event = objectValue(eventValue, "repository boundary event") as Event;
  if (
    event.schema !== EVENT_SCHEMA ||
    event.kind !== "frontier.repository_bound"
  ) {
    throw new Error("repository boundary event schema or kind mismatch");
  }
  if (event.id !== eventId(event)) {
    throw new Error(
      "repository boundary event id does not match canonical content",
    );
  }
  const target = objectValue(event.target, "repository boundary target");
  const actor = objectValue(event.actor, "repository boundary actor");
  if (target.type !== "frontier") {
    throw new Error("repository boundary target.type must be frontier");
  }
  if (actor.type !== "human") {
    throw new Error("repository boundary actor.type must be human");
  }
  if (
    event.before_hash !== NULL_HASH ||
    event.after_hash !== NULL_HASH
  ) {
    throw new Error(
      "repository boundary must use null before_hash and after_hash",
    );
  }
  requireText("event.reason", event.reason);
  if (
    typeof event.timestamp !== "string" ||
    !RFC3339.test(event.timestamp) ||
    Number.isNaN(Date.parse(event.timestamp))
  ) {
    throw new Error("repository boundary timestamp must be RFC3339");
  }
  if (
    !Array.isArray(event.caveats) ||
    !event.caveats.every((value) => typeof value === "string")
  ) {
    throw new Error("repository boundary caveats must be strings");
  }
  const payload = validateBoundaryPayload(event.payload);
  if (target.id !== payload.frontier_id) {
    throw new Error(
      "repository boundary target does not match payload frontier",
    );
  }
  if (actor.id !== payload.administrator_actor_id) {
    throw new Error(
      "repository boundary actor does not match payload administrator",
    );
  }
  const rawSignature = event.signature;
  if (typeof rawSignature !== "string") {
    throw new Error("repository boundary must carry an event signature");
  }
  const version = rawSignature.startsWith("v1:") ? "v1" : "v0";
  const signatureHex =
    version === "v1" ? rawSignature.slice(3) : rawSignature;
  if (!/^[0-9a-f]{128}$/.test(signatureHex)) {
    throw new Error("repository boundary signature encoding is invalid");
  }
  const publicKeyBytes = Buffer.from(
    payload.administrator_public_key as string,
    "hex",
  );
  const spki = Buffer.concat([
    Buffer.from("302a300506032b6570032100", "hex"),
    publicKeyBytes,
  ]);
  const publicKey = createPublicKey({
    key: spki,
    format: "der",
    type: "spki",
  });
  if (
    !verifySignature(
      null,
      eventSigningBytes(event, version),
      publicKey,
      Buffer.from(signatureHex, "hex"),
    )
  ) {
    throw new Error("repository boundary event signature does not verify");
  }
  return { event, payload, root: eventContentRoot(event) };
}

function validateProfileV1GenesisEvent(eventValue: unknown): {
  event: Event;
  frontierId: string;
  identityRoot: string;
  root: string;
} {
  const event = objectValue(
    eventValue,
    "Profile v1 frontier.created",
  ) as Event;
  if (event.schema !== EVENT_SCHEMA || event.kind !== "frontier.created") {
    throw new Error("Profile v1 frontier.created schema or kind mismatch");
  }
  if (event.id !== eventId(event)) {
    throw new Error(
      "Profile v1 frontier.created id does not match canonical content",
    );
  }
  const target = objectValue(event.target, "Profile v1 frontier.created target");
  const actor = objectValue(event.actor, "Profile v1 frontier.created actor");
  if (target.type !== "frontier" || actor.type !== "frontier") {
    throw new Error(
      "Profile v1 frontier.created target and actor must be frontiers",
    );
  }
  if (event.before_hash !== NULL_HASH || event.after_hash !== NULL_HASH) {
    throw new Error("Profile v1 frontier.created must use null state hashes");
  }
  if (
    event.signature !== null &&
    event.signature !== undefined
  ) {
    throw new Error("Profile v1 frontier.created must be unsigned");
  }
  if (!Array.isArray(event.caveats) || event.caveats.length !== 0) {
    throw new Error("Profile v1 frontier.created must carry no caveats");
  }

  const payload = objectValue(
    event.payload,
    "Profile v1 frontier.created payload",
  );
  if (!hasExactKeys(payload, FRONTIER_CREATED_PAYLOAD_KEYS)) {
    throw new Error(
      "Profile v1 frontier.created payload has an open or incomplete shape",
    );
  }
  if (payload.schema !== FRONTIER_CREATED_SCHEMA_V1) {
    throw new Error("Profile v1 frontier.created payload schema mismatch");
  }
  const name = payload.name_at_creation;
  const creator = payload.creator;
  if (
    typeof name !== "string" ||
    name.length === 0 ||
    typeof creator !== "string" ||
    creator.length === 0
  ) {
    throw new Error("Profile v1 frontier.created identity text is invalid");
  }
  if (payload.profile_schema !== FRONTIER_PROFILE_SCHEMA_V1) {
    throw new Error("Profile v1 frontier.created profile schema mismatch");
  }
  if (payload.dependency_root !== canonicalRoot([])) {
    throw new Error(
      "Profile v1 frontier.created dependency root is not empty",
    );
  }
  const createdAt = payload.created_at;
  if (
    typeof createdAt !== "string" ||
    !RFC3339.test(createdAt) ||
    Number.isNaN(Date.parse(createdAt))
  ) {
    throw new Error(
      "Profile v1 frontier.created created_at must be RFC3339",
    );
  }
  if (
    target.id !== name ||
    actor.id !== creator ||
    event.timestamp !== createdAt
  ) {
    throw new Error(
      "Profile v1 frontier.created identity disagrees with its event core",
    );
  }

  const root = eventContentRoot(event);
  const frontierId = `vfr_${root.slice(7, 23)}`;
  const identityRoot = canonicalRoot({
    schema: FRONTIER_IDENTITY_SCHEMA_V1,
    frontier_id: frontierId,
    origin: "genesis",
    origin_commitment: root,
    legacy_identity_preimage_root: null,
  });
  return { event, frontierId, identityRoot, root };
}

function validateRepositoryBoundaryEventSet(eventsValue: unknown): void {
  if (!Array.isArray(eventsValue)) {
    throw new Error("repository boundary event set must be an array");
  }
  const boundaries = new Map<string, BoundaryRecord>();
  for (const eventValue of eventsValue) {
    if (
      !eventValue ||
      typeof eventValue !== "object" ||
      Array.isArray(eventValue) ||
      (eventValue as { kind?: unknown }).kind !== "frontier.repository_bound"
    ) {
      continue;
    }
    const { event, payload, root } = validateBoundaryEvent(eventValue);
    if (boundaries.has(root)) {
      throw new Error("duplicate repository boundary content root");
    }
    boundaries.set(root, { event, payload });
  }
  if (boundaries.size === 0) return;

  const genesis = new Map<
    string,
    ReturnType<typeof validateProfileV1GenesisEvent>
  >();
  for (const eventValue of eventsValue) {
    if (
      !eventValue ||
      typeof eventValue !== "object" ||
      Array.isArray(eventValue)
    ) {
      continue;
    }
    const event = eventValue as Event;
    if (
      event.kind !== "frontier.created" ||
      !event.payload ||
      event.payload.schema !== FRONTIER_CREATED_SCHEMA_V1
    ) {
      continue;
    }
    const native = validateProfileV1GenesisEvent(event);
    if (genesis.has(native.root)) {
      throw new Error(
        "duplicate Profile v1 frontier.created content root",
      );
    }
    genesis.set(native.root, native);
  }
  if (genesis.size > 1) {
    throw new Error(
      "repository boundary graph has multiple Profile v1 genesis events",
    );
  }

  let roots = 0;
  const children = new Map<string, string[]>();
  for (const [root, boundary] of boundaries) {
    const payload = boundary.payload;
    if (payload.mode === "temporalize_existing") {
      roots += 1;
      if (genesis.size !== 0) {
        throw new Error(
          "legacy temporal boundary cannot coexist with Profile v1 frontier.created",
        );
      }
      continue;
    }
    const parentRoot = payload.previous_identity_event_root as string;
    const childRoots = children.get(parentRoot) ?? [];
    childRoots.push(root);
    children.set(parentRoot, childRoots);
    const parent = boundaries.get(parentRoot);
    const nativeParent = genesis.get(parentRoot);
    if (!parent && !nativeParent) {
      throw new Error(
        "repository boundary references a missing identity parent",
      );
    }
    if (nativeParent) {
      roots += 1;
      if (payload.trust_mode !== "genesis") {
        throw new Error("frontier.created parent requires genesis trust");
      }
      if (
        payload.frontier_id !== nativeParent.frontierId ||
        payload.identity_root !== nativeParent.identityRoot ||
        payload.legacy_identity_preimage_root !== null
      ) {
        throw new Error(
          "genesis-chained boundary changed the derived Frontier identity",
        );
      }
      continue;
    }
    if (!parent) {
      throw new Error(
        "repository boundary references a missing identity parent",
      );
    }
    if (payload.trust_mode !== "previous_boundary") {
      throw new Error(
        "repository boundary parent requires previous_boundary trust",
      );
    }
    if (
      Number(payload.anchor_event_count) <=
      Number(parent.payload.anchor_event_count)
    ) {
      throw new Error(
        "repository boundary anchor_event_count did not advance",
      );
    }
    for (const field of [
      "frontier_id",
      "identity_root",
      "legacy_identity_preimage_root",
      "administrator_actor_id",
      "administrator_public_key",
      "administrator_algorithm",
    ]) {
      if (payload[field] !== parent.payload[field]) {
        throw new Error(
          "repository boundary changed immutable identity fields",
        );
      }
    }
  }
  if ([...children.values()].some((values) => values.length > 1)) {
    throw new Error("repository boundary graph contains a fork");
  }
  if (roots !== 1) {
    throw new Error("repository boundary graph must have exactly one root");
  }
  for (const start of boundaries.keys()) {
    let current = start;
    const visited = new Set<string>();
    while (boundaries.has(current)) {
      if (visited.has(current)) {
        throw new Error("repository boundary graph contains a cycle");
      }
      visited.add(current);
      const payload = boundaries.get(current)!.payload;
      if (payload.mode === "temporalize_existing") break;
      const parent = payload.previous_identity_event_root as string;
      if (genesis.has(parent)) break;
      current = parent;
    }
  }
}

function verifyRepositoryBoundaryContract(fixture: JsonObject): void {
  const eventLog = fixture.event_log;
  if (
    Array.isArray(eventLog) &&
    eventLog.some(
      (event) =>
        Boolean(event) &&
        typeof event === "object" &&
        !Array.isArray(event) &&
        (event as { kind?: unknown }).kind === "frontier.repository_bound",
    )
  ) {
    validateRepositoryBoundaryEventSet(eventLog);
  }
  const vectors = fixture.repository_boundary_validation;
  if (fixture.scenario !== "frontier_repository_bound") {
    if (vectors !== undefined) {
      throw new Error(
        "repository boundary vectors appear on the wrong fixture",
      );
    }
    return;
  }
  const vectorObject = objectValue(
    vectors,
    "repository boundary conformance vectors",
  );
  if (
    vectorObject.schema !==
      "vela.frontier-repository-boundary-conformance.v1" ||
    vectorObject.signature_version !== "v1" ||
    !Array.isArray(vectorObject.cases)
  ) {
    throw new Error("repository boundary conformance vectors are missing");
  }
  const cases = vectorObject.cases;
  const ids = new Set(
    cases.map((entry) =>
      objectValue(entry, "repository boundary conformance case").id,
    ),
  );
  if (
    cases.length !== BOUNDARY_CASE_IDS.size ||
    ids.size !== BOUNDARY_CASE_IDS.size ||
    [...ids].some(
      (id) => typeof id !== "string" || !BOUNDARY_CASE_IDS.has(id),
    )
  ) {
    throw new Error("repository boundary conformance case inventory drifted");
  }
  for (const entry of cases) {
    const testCase = objectValue(
      entry,
      "repository boundary conformance case",
    );
    if (typeof testCase.expected_valid !== "boolean") {
      throw new Error("repository boundary conformance case is malformed");
    }
    let actualValid = true;
    try {
      validateRepositoryBoundaryEventSet(testCase.events);
    } catch {
      actualValid = false;
    }
    if (actualValid !== testCase.expected_valid) {
      throw new Error(
        `repository boundary case ${JSON.stringify(testCase.id)} ` +
          `expected valid=${String(testCase.expected_valid)}, got ${String(actualValid)}`,
      );
    }
  }
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

// Mirror of reducer.rs::apply_finding_contribution_recorded. Appends a
// claim-granularity attribution to provenance.contributions, idempotent on
// (unit, agent_id, role). Normalizes to Rust's serialized shape so the
// canonical snapshot hash matches: model/model_version drop when null,
// basis drops when empty (serde skip_serializing_if).
function applyFindingContributionRecorded(state: Finding[], event: Event): void {
  const findingId = event.target?.id ?? "";
  const f = _findFinding(state, findingId);
  if (!f) {
    throw new Error(
      `finding.contribution.recorded targets unknown finding ${findingId}`,
    );
  }
  const raw = event.payload?.contribution;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    throw new Error("finding.contribution.recorded missing payload.contribution");
  }
  const src = raw as { [k: string]: Json };
  const c: { [k: string]: Json } = {
    unit: src.unit,
    unit_type: src.unit_type,
    agent_kind: src.agent_kind,
    agent_id: src.agent_id,
    role: src.role,
  };
  if (src.model != null) c.model = src.model;
  if (src.model_version != null) c.model_version = src.model_version;
  if (typeof src.basis === "string" && src.basis.length > 0) c.basis = src.basis;

  if (
    f.provenance == null ||
    typeof f.provenance !== "object" ||
    Array.isArray(f.provenance)
  ) {
    f.provenance = {};
  }
  const prov = f.provenance as { [k: string]: Json };
  if (!Array.isArray(prov.contributions)) prov.contributions = [];
  const list = prov.contributions as Json[];
  const dup = list.some((x) => {
    const o = x as { [k: string]: Json };
    return o.unit === c.unit && o.agent_id === c.agent_id && o.role === c.role;
  });
  if (!dup) list.push(c);
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

// v0.49+v0.50+v0.51 mirror functions: each mutates the appropriate
// sub-collection in ReducerState. Idempotent on duplicate ids.

function applyArtifactAsserted(state: Artifact[], event: Event): void {
  const payload = event.payload ?? {};
  const artifact = payload.artifact as Artifact | undefined;
  if (!artifact) return;
  if (state.some((a) => a.id === artifact.id)) return;
  state.push(_deepClone(artifact));
}

function applyArtifactReviewed(state: Artifact[], event: Event): void {
  const payload = event.payload ?? {};
  const status = payload.status;
  if (typeof status !== "string") {
    throw new Error("artifact.reviewed missing payload.status");
  }
  const id = event.target?.id ?? "";
  const artifact = state.find((a) => a.id === id);
  if (!artifact) {
    throw new Error(`artifact.reviewed targets unknown id ${id}`);
  }
  if (status === "accepted" || status === "approved") {
    artifact.review_state = "accepted";
  } else if (
    status === "contested" ||
    status === "needs_revision" ||
    status === "rejected"
  ) {
    artifact.review_state = status;
  } else {
    throw new Error(`unsupported review status ${JSON.stringify(status)}`);
  }
}

function applyArtifactRetracted(state: Artifact[], event: Event): void {
  const id = event.target?.id ?? "";
  const artifact = state.find((a) => a.id === id);
  if (!artifact) {
    throw new Error(`artifact.retracted targets unknown id ${id}`);
  }
  artifact.retracted = true;
}

function applyFindingSpanRepaired(findings: Finding[], event: Event): void {
  if (event.target?.type !== "finding") {
    throw new Error("finding.span_repaired target.type must be 'finding'");
  }
  const findingId = event.target?.id ?? "";
  if (!findingId) throw new Error("finding.span_repaired missing target.id");
  const payload = event.payload ?? {};
  const section = payload.section;
  const text = payload.text;
  if (typeof section !== "string" || section.length === 0) {
    throw new Error("finding.span_repaired missing payload.section");
  }
  if (typeof text !== "string" || text.length === 0) {
    throw new Error("finding.span_repaired missing payload.text");
  }
  const finding = findings.find((f) => f.id === findingId);
  if (!finding) {
    throw new Error(`finding.span_repaired targets unknown finding ${findingId}`);
  }
  const evidence = (finding.evidence ?? {}) as { [k: string]: Json };
  finding.evidence = evidence;
  const spans = Array.isArray(evidence.evidence_spans)
    ? (evidence.evidence_spans as Json[])
    : [];
  evidence.evidence_spans = spans;
  const alreadyPresent = spans.some((span) => {
    if (!span || typeof span !== "object" || Array.isArray(span)) return false;
    return span.section === section && span.text === text;
  });
  if (!alreadyPresent) spans.push({ section, text });
}

// v0.51: tier.set mutates access_tier on the matched object. The
// payload carries object_type so the dispatcher knows which
// collection to mutate; we re-check inside this function for
// independent verification.
function applyTierSet(state: ReducerState, event: Event): void {
  const payload = event.payload ?? {};
  const objType = payload.object_type;
  const objId = payload.object_id;
  const newTier = payload.new_tier;
  if (
    typeof objType !== "string" ||
    typeof objId !== "string" ||
    typeof newTier !== "string"
  ) {
    throw new Error(
      "tier.set requires payload.{object_type, object_id, new_tier}",
    );
  }
  if (!["public", "restricted", "classified"].includes(newTier)) {
    throw new Error(`tier.set invalid new_tier ${JSON.stringify(newTier)}`);
  }
  let collection: { id?: string; access_tier?: Json }[];
  if (objType === "finding") collection = state.findings;
  else if (objType === "artifact") collection = state.artifacts;
  else throw new Error(`tier.set unsupported object_type ${objType}`);
  const obj = collection.find((o) => o.id === objId);
  if (!obj) {
    throw new Error(`tier.set targets unknown ${objType} ${objId}`);
  }
  obj.access_tier = newTier;
}

function applyEvent(state: ReducerState, event: Event): void {
  const kind = event.kind ?? "";
  if (kind === "frontier.created") return; // structural anchor
  else if (kind === "finding.asserted")
    applyFindingAsserted(state.findings, event);
  else if (kind === "finding.reviewed")
    applyFindingReviewed(state.findings, event);
  else if (kind === "finding.noted" || kind === "finding.caveated")
    applyFindingAnnotation(state.findings, event);
  else if (kind === "finding.confidence_revised")
    applyFindingConfidenceRevised(state.findings, event);
  else if (kind === "finding.rejected")
    applyFindingRejected(state.findings, event);
  else if (kind === "finding.retracted")
    applyFindingRetracted(state.findings, event);
  else if (kind === "finding.contribution.recorded")
    applyFindingContributionRecorded(state.findings, event);
  else if (kind === "finding.dependency_invalidated")
    applyFindingDependencyInvalidated(state.findings, event);
  else if (kind === "artifact.asserted")
    applyArtifactAsserted(state.artifacts, event);
  else if (kind === "artifact.reviewed")
    applyArtifactReviewed(state.artifacts, event);
  else if (kind === "artifact.retracted")
    applyArtifactRetracted(state.artifacts, event);
  else if (kind === "tier.set") applyTierSet(state, event);
  else if (kind === "finding.span_repaired")
    applyFindingSpanRepaired(state.findings, event);
  // v0.82: cross-impl reducer parity for newer protocol kinds. The
  // following events do not touch any field the TS effect-digest
  // captures (id, retracted, contested, review_state,
  // confidence_score, annotation_ids, access_tier on findings; the
  // analogous projections on artifacts). The Rust reducer is canonical
  // and recomputes derived structures from the event log directly
  // (attestations). Treating them as no-ops here keeps the
  // third-implementation reducer-effects digest byte-identical with
  // Rust + Python.
  else if (kind === "attestation.recorded") return; // audit-only
  // Side-table / federation arms. Each mutates a collection the Rust +
  // Python reducers keep outside the digested collections
  // (released_diff_packs, verdict_conflicts, contradictions,
  // evidence_atoms, and the frontier-observation log). The cross-impl
  // effect-digest covers findings / artifacts only, so these are
  // no-ops here. Mirrors reducer.rs::apply_diff_pack_released /
  // apply_diff_pack_reviewed / apply_verdict_conflict_resolved /
  // apply_contradiction_resolved and the v0.39+ federation no-ops.
  else if (kind === "diff_pack.released") return;
  else if (kind === "diff_pack.reviewed") return;
  else if (kind === "verdict_conflict.resolved") return;
  else if (kind === "contradiction.resolved") return;
  else if (kind === "evidence_atom.locator_repaired") return;
  else if (kind === "frontier.synced_with_peer") return;
  else if (kind === "frontier.conflict_detected") return;
  else if (kind === "frontier.conflict_resolved") return;
  // verifier attachment bound to a finding: mutates the Project-level
  // verifier_attachments sidecar; no-op on the findings digest. Rust mirror
  // is reducer.rs::apply_verifier_attachment_added.
  else if (kind === "verifier_attachment.added") return;
  // Supersession: flip flags.superseded on the OLD (target) finding.
  // The replacement's body enters via loader genesis seeding, never the
  // reducer (thin payload). Rust mirror: reducer.rs::apply_finding_superseded.
  else if (kind === "finding.superseded") {
    applyFindingSuperseded(state.findings, event);
  }
  // Statement-faithfulness attestation: side-table upsert in Rust; no-op
  // on the findings digest here. Rust mirror: apply_statement_attested.
  else if (kind === "statement.attested") return;
  // Obligation lease + priority registration: side-table upserts in Rust;
  // no-ops on the findings digest here.
  else if (kind === "attempt.claimed" || kind === "statement.registered")
    return;
  // Causal re-grading from payload.after ({claim, grade}). Rust mirror:
  // reducer.rs::apply_assertion_reinterpreted_causal.
  else if (kind === "assertion.reinterpreted_causal") {
    applyAssertionReinterpretedCausal(state.findings, event);
  }
  // Audit-only / writerless kinds: validated at emit, no projected
  // state on replay (explicit no-op arms in the Rust reducer).
  else if (
    kind === "frontier.observation_reviewed" ||
    kind === "correction_return.review" ||
    kind === "key.revoke" ||
    kind === "review.accepted" ||
    kind === "review.rejected" ||
    kind === "review.revision_requested" ||
    kind === "actor.registration_activated" ||
    kind === "proposal.withdrawn" ||
    kind === "frontier.repository_bound"
  )
    return;
  // policy.auto_admitted (Phase 1A): deterministic machine-verified admission
  // audit record. No-op on the findings digest (mirror of the Rust + Python
  // reducers); the verifier attachments define trust.
  else if (kind === "policy.auto_admitted") return;
  else
    throw new Error(`reducer: unsupported event kind ${JSON.stringify(kind)}`);
}

// ── Reducer-effects digest ─────────────────────────────────────────
//
// Mirror of `finding_state` in
// crates/vela-protocol/tests/cross_impl_reducer_fixtures.rs.
// Captures only the fields the reducer mutates so cross-impl agreement
// is testable without serializing the full Project struct.

interface FindingEffectRow {
  id: string;
  retracted: boolean;
  contested: boolean;
  review_state: string;
  confidence_score: string;
  annotation_ids: string[];
  access_tier: string;
}

interface ArtifactEffectRow {
  id: string;
  kind: string;
  retracted: boolean;
  review_state: string;
  access_tier: string;
}

function findingEffects(findings: Finding[]): FindingEffectRow[] {
  const sorted = [...findings].sort((a, b) =>
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
    const score = Number(confidence.score ?? 0.0);
    const accessTier = (f.access_tier as string | undefined) ?? "public";
    return {
      id: f.id ?? "",
      retracted: Boolean(flags.retracted ?? false),
      contested: Boolean(flags.contested ?? false),
      review_state: reviewState,
      confidence_score: score.toFixed(6),
      annotation_ids: annotationIds,
      access_tier: accessTier,
    };
  });
}

function artifactEffects(artifacts: Artifact[]): ArtifactEffectRow[] {
  const sorted = [...artifacts].sort((a, b) =>
    (a.id ?? "").localeCompare(b.id ?? ""),
  );
  return sorted.map((a) => ({
    id: a.id ?? "",
    kind: (a.kind as string | undefined) ?? "",
    retracted: Boolean(a.retracted ?? false),
    review_state: (a.review_state as string | undefined) ?? "none",
    access_tier: (a.access_tier as string | undefined) ?? "public",
  }));
}

// ── Fixture verification ───────────────────────────────────────────

interface FixtureResult {
  path: string;
  frontierIdx: number;
  findings: number;
  artifacts: number;
  events: number;
  cascadeDepth: number;
  matched: number;
  diffs: {
    collection: string;
    id: string;
    issue: string;
    expected?: unknown;
    actual?: unknown;
  }[];
  ok: boolean;
  error: string | null;
}

// v0.106.5+: extended verifier reads fixture_version "4" with all current
// expected collections. Falls back to v1/v2 for backward-compat.
function verifyFixture(path: string): FixtureResult {
  const result: FixtureResult = {
    path,
    frontierIdx: -1,
    findings: 0,
    artifacts: 0,
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
  const fxVersion = String(fx.fixture_version ?? "");
  if (
    fxVersion !== "6" &&
    fxVersion !== "5" &&
    fxVersion !== "4" &&
    fxVersion !== "3" &&
    fxVersion !== "2" &&
    fxVersion !== "1"
  ) {
    result.error = `unsupported fixture_version ${JSON.stringify(fx.fixture_version)}; expected '1', '2', '3', '4', '5', or '6'`;
    return result;
  }
  result.frontierIdx = Number(fx.frontier_idx ?? -1);
  const stats = (fx.stats ?? {}) as { [k: string]: Json };
  result.findings = Number(stats.findings ?? 0);
  result.artifacts = Number(stats.artifacts ?? 0);
  result.events = Number(stats.events ?? 0);
  result.cascadeDepth = Number(stats.cascade_depth ?? 0);

  const state: ReducerState = {
    findings: _deepClone((fx.genesis_findings as Finding[]) ?? []),
    artifacts: [],
  };
  const eventLog = (fx.event_log as Event[]) ?? [];
  const expectedFindings = (fx.expected_states as FindingEffectRow[]) ?? [];
  const expectedArtifacts =
    (fx.expected_artifacts as ArtifactEffectRow[]) ?? [];

  try {
    verifyRepositoryBoundaryContract(fx);
  } catch (e) {
    result.error =
      `repository boundary conformance error: ` + (e as Error).message;
    return result;
  }

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

  // For v1 fixtures, the access_tier field will be missing from
  // expected_states; strip it from actual rows so the comparison
  // doesn't false-fail. v2 fixtures include it.
  const stripV1 = fxVersion === "1";

  const actualF = findingEffects(state.findings).map((r) =>
    stripV1
      ? ({
          id: r.id,
          retracted: r.retracted,
          contested: r.contested,
          review_state: r.review_state,
          confidence_score: r.confidence_score,
          annotation_ids: r.annotation_ids,
        } as unknown as FindingEffectRow)
      : r,
  );
  const actualA = artifactEffects(state.artifacts);

  diffCollection("findings", actualF, expectedFindings, result);
  if (
    fxVersion === "3" ||
    fxVersion === "4" ||
    fxVersion === "5" ||
    fxVersion === "6"
  ) {
    diffCollection("artifacts", actualA, expectedArtifacts, result);
  }

  let totalExpected = expectedFindings.length;
  if (
    fxVersion === "3" ||
    fxVersion === "4" ||
    fxVersion === "5" ||
    fxVersion === "6"
  )
    totalExpected += expectedArtifacts.length;
  result.ok = result.diffs.length === 0 && result.matched === totalExpected;
  return result;
}

function diffCollection(
  name: string,
  actual: { id: string }[],
  expected: { id: string }[],
  result: FixtureResult,
): void {
  const actualById = new Map(actual.map((r) => [r.id, r]));
  const expectedById = new Map(expected.map((r) => [r.id, r]));
  const allIds = [
    ...new Set([...actualById.keys(), ...expectedById.keys()]),
  ].sort();
  for (const id of allIds) {
    const a = actualById.get(id);
    const e = expectedById.get(id);
    if (!a) {
      result.diffs.push({
        collection: name,
        id,
        issue: "missing in ts output",
        expected: e,
      });
    } else if (!e) {
      result.diffs.push({
        collection: name,
        id,
        issue: "extra in ts output",
        actual: a,
      });
    } else if (canonicalJson(a) !== canonicalJson(e)) {
      result.diffs.push({
        collection: name,
        id,
        issue: "mismatch",
        expected: e,
        actual: a,
      });
    } else {
      result.matched += 1;
    }
  }
}

function renderText(results: FixtureResult[]): string {
  const lines: string[] = [];
  lines.push("vela reducer (typescript · stdlib · third implementation)");
  for (const r of results) {
    const status = r.ok ? "ok" : "FAIL";
    const totalExpected = r.findings + r.artifacts;
    lines.push(
      `  ${status.padEnd(4)} · frontier ${String(r.frontierIdx).padStart(2, "0")} · ` +
        `${r.matched}/${totalExpected} (${r.findings}f/${r.artifacts}a) · ` +
        `${r.events} events · cascade depth ${r.cascadeDepth}`,
    );
    if (r.error) lines.push(`          error: ${r.error}`);
    for (const d of r.diffs.slice(0, 5)) {
      lines.push(
        `          · [${d.collection}] ${d.id}: ${d.issue}`,
      );
      if (d.expected && d.actual) {
        const exp = d.expected as { [k: string]: Json };
        const act = d.actual as { [k: string]: Json };
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
      "  the same reducer-effects state the rust and python reducers produced.",
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
            artifacts: r.artifacts,
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

function applyFindingSuperseded(findings: Finding[], event: StateEvent): void {
  const findingId = event.target?.id;
  const f = findings.find((x) => x.id === findingId);
  if (!f) {
    throw new Error(`finding.superseded targets unknown finding ${findingId}`);
  }
  if (!f.flags) f.flags = {} as Finding["flags"];
  (f.flags as { superseded?: boolean }).superseded = true;
}

function applyAssertionReinterpretedCausal(
  findings: Finding[],
  event: StateEvent,
): void {
  const findingId = event.target?.id;
  const payload = (event.payload ?? {}) as {
    after?: { claim?: string; grade?: string | null };
  };
  const claim = payload.after?.claim;
  if (
    claim !== "correlation" &&
    claim !== "mediation" &&
    claim !== "intervention"
  ) {
    throw new Error(`invalid causal claim ${JSON.stringify(claim)}`);
  }
  const grade = payload.after?.grade ?? null;
  if (
    grade !== null &&
    grade !== "rct" &&
    grade !== "quasi_experimental" &&
    grade !== "observational" &&
    grade !== "theoretical"
  ) {
    throw new Error(`invalid causal evidence grade ${JSON.stringify(grade)}`);
  }
  const f = findings.find((x) => x.id === findingId);
  if (!f) {
    throw new Error(
      `assertion.reinterpreted_causal targets unknown finding ${findingId}`,
    );
  }
  const assertion = f.assertion as unknown as {
    causal_claim?: string;
    causal_evidence_grade?: string;
  };
  assertion.causal_claim = claim;
  if (grade !== null) assertion.causal_evidence_grade = grade;
}
