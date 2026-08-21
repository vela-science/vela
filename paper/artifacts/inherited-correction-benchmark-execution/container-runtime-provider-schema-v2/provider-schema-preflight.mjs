#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { deriveProviderSchema, validateResponseAgainstSchema } from "./run-once.mjs";

const load = path => JSON.parse(readFileSync(path, "utf8"));
const registered = load("/input/response-schema.json");
const provider = load("/input/provider-response-schema.json");
const derived = deriveProviderSchema(registered);
if (JSON.stringify(provider) !== JSON.stringify(derived)) throw new Error("provider_schema_not_exact_derivative");
const valid = load("/input/valid-response.json");
const duplicate = structuredClone(valid);
duplicate.evidence_bindings[1] = structuredClone(duplicate.evidence_bindings[0]);
const checks = {
  provider_accepts_valid: validateResponseAgainstSchema(provider, valid).valid,
  registered_accepts_valid: validateResponseAgainstSchema(registered, valid).valid,
  provider_accepts_duplicate: validateResponseAgainstSchema(provider, duplicate).valid,
  registered_rejects_duplicate: !validateResponseAgainstSchema(registered, duplicate).valid,
};
if (Object.values(checks).some(value => value !== true)) throw new Error("provider_schema_semantics_check_failed");
process.stdout.write(`${JSON.stringify({
  schema: "vela.inherited-correction-provider-schema-preflight.v1",
  provider_contact_possible: false,
  container_network: "none",
  exact_deleted_json_pointers: ["/properties/evidence_bindings/uniqueItems"],
  checks,
}, null, 2)}\n`);
