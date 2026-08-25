#!/usr/bin/env node
/** Independent JavaScript reducer for the proof-history interchange format. */

import fs from "node:fs";

const INPUT_FORMAT = "theory-of-standing.proof-history.v1";
const RESULT_FORMAT = "theory-of-standing.proof-result.v2";
const INVALID_FORMAT = "theory-of-standing.proof-invalid.v1";
const STANDING = new Set(["accepted", "unassessed", "superseded", "retracted"]);

class FormatError extends Error {}

function fail(message) {
  throw new FormatError(message);
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function exact(value, keys, where) {
  if (!isObject(value)) fail(`${where}: expected object`);
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${where}: expected exact keys ${JSON.stringify(expected)}`);
  }
  return value;
}

function nat(value, where) {
  if (!Number.isSafeInteger(value) || value < 0) {
    fail(`${where}: expected nonnegative safe integer`);
  }
  return value;
}

function natMap(value, where) {
  if (!isObject(value)) fail(`${where}: expected object`);
  const result = new Map();
  for (const [key, item] of Object.entries(value)) {
    if (!/^(0|[1-9][0-9]*)$/.test(key)) fail(`${where}: invalid resource id`);
    const resource = Number(key);
    nat(resource, `${where}: resource id`);
    result.set(resource, nat(item, `${where}.${key}`));
  }
  return result;
}

function validateAction(value, where) {
  if (!isObject(value) || typeof value.kind !== "string") fail(`${where}: invalid action`);
  if (value.kind === "accept" || value.kind === "reject") {
    exact(value, ["kind", "claim"], where);
    nat(value.claim, `${where}.claim`);
  } else if (value.kind === "correct") {
    exact(value, ["kind", "prior_decision", "predecessor", "replacement"], where);
    nat(value.prior_decision, `${where}.prior_decision`);
    nat(value.predecessor, `${where}.predecessor`);
    nat(value.replacement, `${where}.replacement`);
  } else {
    fail(`${where}: unsupported action`);
  }
  return value;
}

function validateHistory(value) {
  const history = exact(
    value,
    [
      "format",
      "repository",
      "authorized_performers",
      "initial_versions",
      "descriptive_dependencies",
      "records",
    ],
    "history",
  );
  if (history.format !== INPUT_FORMAT) fail("history.format: unsupported format");
  nat(history.repository, "history.repository");
  if (!Array.isArray(history.authorized_performers)) {
    fail("history.authorized_performers: expected array");
  }
  history.authorized_performers.forEach((actor, index) =>
    nat(actor, `history.authorized_performers[${index}]`),
  );
  const canonicalActors = [...new Set(history.authorized_performers)].sort((a, b) => a - b);
  if (JSON.stringify(canonicalActors) !== JSON.stringify(history.authorized_performers)) {
    fail("history.authorized_performers: expected sorted unique values");
  }
  history.initial_versions = natMap(history.initial_versions, "history.initial_versions");
  if (!Array.isArray(history.descriptive_dependencies)) {
    fail("history.descriptive_dependencies: expected array");
  }
  history.descriptive_dependencies.forEach((dependency, index) => {
    exact(dependency, ["dependent", "depends_on"], `dependency[${index}]`);
    nat(dependency.dependent, `dependency[${index}].dependent`);
    nat(dependency.depends_on, `dependency[${index}].depends_on`);
  });
  if (!Array.isArray(history.records)) fail("history.records: expected array");
  const decisionIds = new Set();
  history.records.forEach((record, index) => {
    const where = `record[${index}]`;
    if (!isObject(record) || typeof record.kind !== "string") fail(`${where}: invalid record`);
    if (record.kind === "submission") {
      exact(record, ["kind", "claim", "producer", "scope", "authenticated"], where);
      nat(record.claim, `${where}.claim`);
      nat(record.producer, `${where}.producer`);
      nat(record.scope, `${where}.scope`);
      if (typeof record.authenticated !== "boolean") fail(`${where}.authenticated: expected boolean`);
    } else if (record.kind === "verification") {
      exact(record, ["kind", "claim", "scope", "property", "outcome"], where);
      nat(record.claim, `${where}.claim`);
      nat(record.scope, `${where}.scope`);
      nat(record.property, `${where}.property`);
      if (record.outcome !== "pass" && record.outcome !== "fail") {
        fail(`${where}.outcome: unsupported outcome`);
      }
    } else if (record.kind === "decision") {
      exact(
        record,
        ["kind", "id", "repository", "authority_label", "performer", "expected_root", "read_set", "action"],
        where,
      );
      for (const key of ["id", "repository", "authority_label", "performer", "expected_root"]) {
        nat(record[key], `${where}.${key}`);
      }
      if (decisionIds.has(record.id)) fail(`${where}.id: duplicate Decision id`);
      decisionIds.add(record.id);
      record.read_set = natMap(record.read_set, `${where}.read_set`);
      validateAction(record.action, `${where}.action`);
    } else {
      fail(`${where}: unsupported kind`);
    }
  });
  return history;
}

function stableStringify(value) {
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  if (isObject(value)) {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableStringify(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function projection(history, state) {
  const corrected = new Set(
    state.events
      .filter((event) => event.action.kind === "correct")
      .map((event) => event.action.predecessor),
  );
  return [...state.standing.keys()]
    .sort((a, b) => a - b)
    .map((claim) => ({
      claim,
      status: history.descriptive_dependencies.some(
        (dependency) => dependency.dependent === claim && corrected.has(dependency.depends_on),
      )
        ? "needs_reassessment"
        : "unaffected",
    }));
}

function emit(history, state) {
  const standing = [...state.standing.entries()]
    .sort(([left], [right]) => left - right)
    .map(([claim, status]) => ({ claim, status }));
  if (standing.some((item) => !STANDING.has(item.status))) throw new Error("noncanonical Standing");
  const result = {
    events: state.events,
    format: RESULT_FORMAT,
    reassessment: projection(history, state),
    rejections: state.rejections,
    repository: history.repository,
    root: state.root,
    standing,
  };
  return `${stableStringify(result)}\n`;
}

function reduceHistory(history) {
  const state = {
    events: [],
    rejections: [],
    root: 0,
    standing: new Map(),
    submissions: [],
    verifications: [],
    versions: history.initial_versions,
  };
  for (let recordIndex = 0; recordIndex < history.records.length; recordIndex += 1) {
    const record = history.records[recordIndex];
    if (record.kind === "submission") {
      if (!record.authenticated) continue;
      state.submissions.push([record.claim, record.scope]);
      if (!state.standing.has(record.claim)) state.standing.set(record.claim, "unassessed");
      state.root += 1;
      continue;
    }
    if (record.kind === "verification") {
      if (!state.submissions.some(([claim, scope]) => claim === record.claim && scope === record.scope)) {
        continue;
      }
      state.verifications.push([record.claim, record.outcome]);
      state.root += 1;
      continue;
    }

    let code = null;
    if (record.repository !== history.repository) code = "wrong_repository";
    else if (!history.authorized_performers.includes(record.performer)) code = "unauthorized";
    else if (record.authority_label !== record.performer) code = "misattributed";
    else if (record.expected_root !== state.root) code = "stale_root";
    else {
      for (const [resource, version] of record.read_set) {
        if (state.versions.get(resource) !== version) {
          code = "stale_read_set";
          break;
        }
      }
    }

    const action = record.action;
    let eligible;
    if (action.kind === "accept") {
      eligible = state.submissions.some(([claim]) => claim === action.claim)
        && state.verifications.some(([claim, outcome]) => claim === action.claim && outcome === "pass");
    } else if (action.kind === "reject") {
      eligible = state.submissions.some(([claim]) => claim === action.claim);
    } else {
      eligible = state.submissions.some(([claim]) => claim === action.replacement)
        && state.verifications.some(
          ([claim, outcome]) => claim === action.replacement && outcome === "pass",
        );
    }
    if (code === null && !eligible) code = "ineligible";

    if (code === null && action.kind === "correct") {
      const validReference = state.events.some(
        (event) => event.decision_id === action.prior_decision
          && event.repository === record.repository
          && event.action.kind === "accept"
          && event.action.claim === action.predecessor,
      ) && state.standing.get(action.predecessor) === "accepted";
      if (!validReference) code = "invalid_correction_reference";
    }

    if (code !== null) {
      state.rejections.push({ code, record_index: recordIndex });
      continue;
    }

    if (action.kind === "accept") state.standing.set(action.claim, "accepted");
    if (action.kind === "correct") {
      state.standing.set(action.predecessor, "superseded");
      state.standing.set(action.replacement, "accepted");
    }
    state.events.push({
      action,
      authority_label: record.authority_label,
      decision_id: record.id,
      performer: record.performer,
      repository: record.repository,
    });
    state.root += 1;
  }
  return { bytes: emit(history, state), exitCode: 0 };
}

function main() {
  if (process.argv.length !== 3) {
    process.stderr.write("usage: reducer.mjs HISTORY.json\n");
    return 64;
  }
  try {
    const history = validateHistory(JSON.parse(fs.readFileSync(process.argv[2], "utf8")));
    const result = reduceHistory(history);
    process.stdout.write(result.bytes);
    return result.exitCode;
  } catch (error) {
    if (!(error instanceof FormatError) && !(error instanceof SyntaxError)) throw error;
    process.stderr.write(`${error.message}\n`);
    process.stdout.write(`${stableStringify({ code: "invalid_format", format: INVALID_FORMAT })}\n`);
    return 2;
  }
}

process.exitCode = main();
