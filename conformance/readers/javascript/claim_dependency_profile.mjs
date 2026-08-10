#!/usr/bin/env node

/* Independent positive/negative reducer for claim-dependency-profile.v0. */
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { canonical } from "./canonical.mjs";

const digest = (value) => `sha256:${createHash("sha256").update(canonical(value)).digest("hex")}`;
const fail = (code) => { throw new Error(code); };
const same = (left, right) => canonical(left) === canonical(right);
const clone = (value) => JSON.parse(JSON.stringify(value));
const depKey = (item) => [item.source.claim_id, item.source.claim_root, item.target.claim_id, item.target.claim_root];

function validate(profile, state) {
  if (!same(Object.keys(profile).sort(), ["dependencies", "does_not_establish", "experiment_id", "nodes", "profile_version", "schema", "scope"]))
    fail("profile_schema_unsupported");
  if (profile.schema !== "vela.claim-dependency-profile.v0" || profile.profile_version !== 0)
    fail("profile_schema_unsupported");
  const scope = profile.scope;
  const context = [scope.repository_id, scope.repository_origin_root];
  if (!Number.isInteger(scope.max_claims) || !Number.isInteger(scope.max_dependencies)
      || typeof scope.complete_claim_set !== "boolean" || typeof scope.complete_dependency_set !== "boolean")
    fail("profile_scope_invalid");
  if (!Array.isArray(profile.does_not_establish) || profile.does_not_establish.length < 3
      || profile.does_not_establish.some((item) => typeof item !== "string" || !item.trim())
      || !same(profile.does_not_establish, [...new Set(profile.does_not_establish)].sort()))
    fail("profile_does_not_establish_invalid");
  if (profile.nodes.length > scope.max_claims) fail("profile_claim_bound_exceeded");
  const nodes = new Map();
  for (const node of profile.nodes) {
    if (nodes.has(node.claim_id)) fail("profile_node_duplicate");
    if (!same([node.repository_id, node.repository_origin_root], context)) fail("profile_repository_context_mismatch");
    nodes.set(node.claim_id, node);
  }
  const nodeKeys = profile.nodes.map((item) => [item.claim_id, item.claim_root]);
  if (!same(nodeKeys, [...nodeKeys].sort())) fail("profile_node_order_invalid");
  const keys = [];
  if (profile.dependencies.length > scope.max_dependencies) fail("profile_dependency_bound_exceeded");
  for (const item of profile.dependencies) {
    if (item.kind !== "requires") fail("profile_dependency_kind_unsupported");
    for (const endpoint of [item.source, item.target]) {
      if (!same([endpoint.repository_id, endpoint.repository_origin_root], context)) fail("profile_repository_context_mismatch");
      const node = nodes.get(endpoint.claim_id);
      if (!node) fail("profile_dependency_endpoint_missing");
      if (node.claim_root !== endpoint.claim_root) fail("profile_dependency_endpoint_root_mismatch");
    }
    if (item.source.claim_id === item.target.claim_id && item.source.claim_root === item.target.claim_root)
      fail("profile_dependency_self_reference");
    keys.push(depKey(item));
  }
  if (new Set(keys.map(canonical)).size !== keys.length) fail("profile_dependency_duplicate");
  if (!same(keys, [...keys].sort())) fail("profile_dependency_order_invalid");
  if (state.schema !== "vela.claim-dependency-state.v0") fail("state_schema_unsupported");
  if (!same([state.repository_id, state.repository_origin_root], context)) fail("state_repository_context_mismatch");
  const ids = state.claims.map((item) => item.claim_id);
  if (new Set(ids).size !== ids.length) fail("state_claim_duplicate");
  if (!same(ids, [...ids].sort())) fail("state_claim_order_invalid");
  return nodes;
}

function derive(profile, state) {
  const nodes = validate(profile, state);
  const claims = new Map(state.claims.map((item) => [item.claim_id, item]));
  const deps = new Map();
  for (const item of profile.dependencies) {
    if (!deps.has(item.source.claim_id)) deps.set(item.source.claim_id, []);
    deps.get(item.source.claim_id).push(item.target);
  }
  const visiting = new Set(), visited = new Set();
  function visit(id) {
    if (visiting.has(id)) fail("dependency_cycle");
    if (visited.has(id)) return;
    visiting.add(id); for (const target of deps.get(id) ?? []) visit(target.claim_id);
    visiting.delete(id); visited.add(id);
  }
  for (const id of [...deps.keys()].sort()) visit(id);
  const memo = new Map();
  function reduce(id) {
    if (memo.has(id)) return memo.get(id);
    const source = claims.get(id); let rank = 0, layer = null; const evidence = [];
    if (!source) { rank = 2; evidence.push({ code: "source_missing" }); }
    else if (source.availability === "unavailable") { rank = 2; evidence.push({ code: "source_unavailable" }); }
    else if (source.lifecycle === "unaccepted") { rank = 2; evidence.push({ code: "source_unaccepted" }); }
    for (const target of deps.get(id) ?? []) {
      const claim = claims.get(target.claim_id);
      if (!claim) { rank = 2; evidence.push({ code: "target_missing", target_claim_id: target.claim_id }); continue; }
      if (claim.claim_root !== target.claim_root) fail("dependency_target_root_mismatch");
      if (claim.availability === "unavailable") { rank = 2; evidence.push({ code: "target_unavailable", target_claim_id: target.claim_id }); continue; }
      if (claim.lifecycle === "unaccepted") { rank = 2; evidence.push({ code: "target_unaccepted", target_claim_id: target.claim_id }); continue; }
      if (claim.lifecycle === "retired") { rank = Math.max(rank, 1); layer ??= 0; evidence.push({ code: "target_retired", target_claim_id: target.claim_id }); continue; }
      const nested = reduce(target.claim_id);
      if (nested.status === "incomplete") { rank = 2; evidence.push({ code: "transitive_incomplete", target_claim_id: target.claim_id }); }
      else if (nested.status === "review_required") { rank = Math.max(rank, 1); layer = Math.max(layer ?? 0, nested.layer + 1); evidence.push({ code: "transitive_review_required", target_claim_id: target.claim_id }); }
    }
    if (!profile.scope.complete_claim_set) { rank = 2; evidence.push({ code: "claim_set_incomplete" }); }
    if (!profile.scope.complete_dependency_set) { rank = 2; evidence.push({ code: "dependency_set_incomplete" }); }
    const value = { status: ["satisfied", "review_required", "incomplete"][rank], evidence, layer: rank === 1 ? layer : null };
    memo.set(id, value); return value;
  }
  const endpoints = new Set([state.transition.predecessor.claim_id, state.transition.successor.claim_id]);
  const results = [], obligations = [], stale = [];
  for (const node of profile.nodes) {
    if (endpoints.has(node.claim_id)) continue;
    const claim = claims.get(node.claim_id), result = reduce(node.claim_id);
    results.push({ label: node.label, claim_id: node.claim_id, claim_root: node.claim_root,
      dependency_status: result.status, evidence: result.evidence, repair_layer: result.layer });
    if (result.status !== "satisfied" && claim?.verification) stale.push(claim.verification.verification_id);
    if (result.status === "review_required") {
      const preimage = { schema: "vela.claim-dependency-repair-obligation.v0", claim_id: node.claim_id,
        claim_root: node.claim_root, evidence: result.evidence,
        discharge_condition: "Re-establish every exact requires edge against current accepted targets, narrow the Claim, or retract it." };
      obligations.push({ label: node.label, repair_layer: result.layer, obligation_root: digest(preimage),
        claim_id: node.claim_id, claim_root: node.claim_root, evidence: result.evidence,
        discharge_condition: preimage.discharge_condition });
    }
  }
  results.sort((a, b) => a.label.localeCompare(b.label)); obligations.sort((a, b) => a.label.localeCompare(b.label));
  const sets = Object.fromEntries(["satisfied", "review_required", "incomplete"].map((status) =>
    [status, results.filter((item) => item.dependency_status === status).map((item) => item.label).sort()]));
  sets.unaffected = [...sets.satisfied]; sets.stale_verifications = stale.sort(); sets.repair_required = obligations.map((item) => item.label).sort();
  const repair_batches = [...new Set(obligations.map((item) => item.repair_layer))].sort().map((layer) => ({
    batch: layer + 1, repair_layer: layer,
    labels: obligations.filter((item) => item.repair_layer === layer).map((item) => item.label).sort(),
    obligation_roots: obligations.filter((item) => item.repair_layer === layer).map((item) => item.obligation_root).sort(),
  }));
  return { schema: "vela.claim-dependency-projection.v0", experiment_id: profile.experiment_id,
    profile_canonical_root: digest(profile), state_canonical_root: digest(state),
    overall_status: sets.incomplete.length ? "incomplete" : sets.review_required.length ? "review_required" : "satisfied",
    claims: results, sets, repair_obligations: obligations, repair_batches, authority_effect: "none" };
}

function mutate(profile, state, vector) {
  profile = clone(profile); state = clone(state);
  const mutation = vector.mutation, document = vector.document === "profile" ? profile : state;
  let target = document; for (const part of mutation.path.slice(0, -1)) target = target[part];
  const key = mutation.path.at(-1);
  if (mutation.op === "set") target[key] = mutation.value;
  else if (mutation.op === "append_copy") target[key].push(clone(target[key][mutation.index]));
  else if (mutation.op === "swap") [target[key][mutation.indices[0]], target[key][mutation.indices[1]]] = [target[key][mutation.indices[1]], target[key][mutation.indices[0]]];
  else if (mutation.op === "remove_label") target[key] = target[key].filter((item) => item.label !== mutation.label);
  else if (mutation.op === "append_dependency") { target[key].push(clone(mutation.value)); target[key].sort((a, b) => canonical(depKey(a)).localeCompare(canonical(depKey(b)))); }
  return [profile, state];
}

try {
  const base = resolve(process.argv[2]);
  const load = (name) => JSON.parse(readFileSync(resolve(base, name), "utf8"));
  const profile = load("profile.json"), state = load("state.json"), expected = load("expected.json");
  const projection = derive(profile, state);
  const wrapper = { schema: "vela.claim-dependency-profile-expected.v0", profile_canonical_root: digest(profile),
    state_canonical_root: digest(state), projection_canonical_root: digest(projection), projection };
  if (!same(wrapper, expected)) fail("positive_expected_projection_drift");
  for (const vector of load("negative-vectors.json").vectors) {
    const [changedProfile, changedState] = mutate(profile, state, vector);
    if (vector.expected_error) {
      try { derive(changedProfile, changedState); } catch (error) { if (error.message === vector.expected_error) continue; throw error; }
      fail(`${vector.id}:expected_${vector.expected_error}`);
    }
    const changed = derive(changedProfile, changedState);
    for (const [key, value] of Object.entries(vector.expected_sets)) if (!same(changed.sets[key], value)) fail(`${vector.id}:${key}`);
  }
  process.stdout.write(`javascript-claim-dependency-profile-v0: ${digest(profile)} ${digest(projection)} ok\n`);
} catch (error) {
  process.stderr.write(`javascript-claim-dependency-profile-v0: FAIL: ${error.message ?? String(error)}\n`); process.exitCode = 1;
}
