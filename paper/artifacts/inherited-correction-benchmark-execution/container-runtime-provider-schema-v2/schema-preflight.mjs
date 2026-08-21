#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { validateResponseAgainstSchema } from "./run-once.mjs";

const load = path => JSON.parse(readFileSync(path, "utf8"));
const schema = load("/input/response-schema.json");
const cases = {
  valid: true,
  "unknown-field": false,
  "missing-required": false,
  "invalid-enum": false,
  "invalid-shape": false,
};
const observed = {};
for (const [name, expected] of Object.entries(cases)) {
  const result = validateResponseAgainstSchema(
    schema,
    load(`/opt/vela-runner/schema-fixtures/${name}.json`),
  );
  observed[name] = result.valid;
  if (result.valid !== expected) throw new Error(`schema_case_mismatch:${name}`);
}
process.stdout.write(`${JSON.stringify({
  schema: "vela.inherited-correction-schema-preflight.v1",
  validator: "ajv/dist/2020.js@8.17.1",
  provider_contact_possible: false,
  container_network: "none",
  cases: observed,
}, null, 2)}\n`);
