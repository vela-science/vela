#!/usr/bin/env node
import { createHash, X509Certificate } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const TRUST_BUNDLE_PATH = "/etc/ssl/certs/ca-certificates.crt";
const sha = data => `sha256:${createHash("sha256").update(data).digest("hex")}`;
const args = process.argv.slice(2);
const mode = args[0];
const outputIndex = args.indexOf("--output");
const output = outputIndex >= 0 ? args[outputIndex + 1] : null;
const expected = process.env.EXPECTED_TRUST_BUNDLE_SHA256;
if (!["positive", "missing", "corrupt"].includes(mode) || !output || !expected) {
  throw new Error("usage: trust-preflight positive|missing|corrupt --output DIR with EXPECTED_TRUST_BUNDLE_SHA256");
}

let content = null;
let error = null;
let certificates = [];
try {
  if (mode === "missing") content = readFileSync("/tmp/absent-ca-bundle.crt");
  else if (mode === "corrupt") content = Buffer.from("not a PEM certificate bundle\n", "utf8");
  else content = readFileSync(TRUST_BUNDLE_PATH);
  const blocks = content.toString("utf8").match(/-----BEGIN CERTIFICATE-----[\s\S]*?-----END CERTIFICATE-----/g) || [];
  if (blocks.length === 0) throw new Error("no_certificates");
  certificates = blocks.map(value => new X509Certificate(value));
  if (sha(content) !== expected) throw new Error("trust_bundle_digest");
} catch (cause) {
  error = String(cause.code || cause.message || cause);
}

const expectedSuccess = mode === "positive";
const passed = expectedSuccess ? error === null && certificates.length >= 100 : error !== null;
mkdirSync(output, { recursive: true });
writeFileSync(join(output, "provider-events.jsonl"), "", { flag: "wx", mode: 0o600 });
const receipt = {
  schema: "vela.inherited-correction-trust-preflight.v1",
  mode,
  expected_success: expectedSuccess,
  trust_check_passed: passed,
  trust_bundle_path: TRUST_BUNDLE_PATH,
  expected_trust_bundle_bytes: expected,
  observed_trust_bundle_bytes: content ? sha(content) : null,
  certificate_count: certificates.length,
  validation_error: error,
  provider_contact_possible: false,
  container_network: "none",
  provider_events_bytes: sha(Buffer.alloc(0)),
};
writeFileSync(join(output, "receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`, { flag: "wx", mode: 0o600 });
if (!passed) process.exit(2);
