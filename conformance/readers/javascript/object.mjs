#!/usr/bin/env node

/* Verify one current Vela producer or verifier object without Vela or Rust. */

import { createHash, createPublicKey, verify as verifySignature } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { canonical } from "./canonical.mjs";

const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");
const KINDS = {
  "application/vnd.vela.submission.v3+json": {
    kind: "submission",
    schema: "vela.submission.v3",
    prefix: "vsb",
    fields: [
      "artifacts",
      "caveats",
      "claim",
      "identity",
      "producer_checks",
      "provenance",
      "replayability",
      "requested_change",
      "schema",
      "verification_requirements",
    ],
  },
  "application/vnd.vela.verification-record.v2+json": {
    kind: "verification",
    schema: "vela.verification-record.v2",
    prefix: "vvr",
  },
};

function decodeBase64(field, value) {
  if (typeof value !== "string" || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${field} is not canonical base64`);
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.toString("base64") !== value) throw new Error(`${field} is not canonical base64`);
  return decoded;
}

function pae(payloadType, payload) {
  const type = Buffer.from(payloadType, "utf8");
  return Buffer.concat([
    Buffer.from(`DSSEv1 ${type.length} `, "utf8"),
    type,
    Buffer.from(` ${payload.length} `, "utf8"),
    payload,
  ]);
}

function verify(path) {
  const raw = readFileSync(path);
  if (raw.length > 8 * 1024 * 1024) throw new Error("object exceeds 8 MiB");
  const envelope = JSON.parse(raw.toString("utf8"));
  if (!Buffer.from(canonical(envelope), "utf8").equals(raw)) {
    throw new Error("envelope is not canonical RFC 8785 JSON");
  }
  const contract = KINDS[envelope.payloadType];
  if (!contract) throw new Error(`unsupported payload type: ${envelope.payloadType}`);
  if (!Array.isArray(envelope.signatures) || envelope.signatures.length !== 1) {
    throw new Error("current objects require exactly one DSSE signature");
  }

  const payload = decodeBase64("payload", envelope.payload);
  const record = JSON.parse(payload.toString("utf8"));
  if (!Buffer.from(canonical(record), "utf8").equals(payload)) {
    throw new Error("payload is not canonical RFC 8785 JSON");
  }
  if (record.schema !== contract.schema) throw new Error(`payload schema is not ${contract.schema}`);
  if (contract.fields) {
    const actual = Object.keys(record).sort();
    const expected = [...contract.fields].sort();
    if (actual.length !== expected.length || actual.some((field, index) => field !== expected[index])) {
      throw new Error(`${contract.schema} payload fields are not the exact closed set`);
    }
  }
  const identity = record.identity;
  if (!identity || !/^[0-9a-f]{64}$/.test(identity.public_key_hex)) {
    throw new Error("identity public key is not 32-byte lowercase hex");
  }
  const signature = envelope.signatures[0];
  if (signature.keyid !== identity.public_key_hex) {
    throw new Error("DSSE keyid does not match the payload identity");
  }
  const publicKey = createPublicKey({
    key: Buffer.concat([SPKI_PREFIX, Buffer.from(identity.public_key_hex, "hex")]),
    format: "der",
    type: "spki",
  });
  if (!verifySignature(null, pae(envelope.payloadType, payload), publicKey, decodeBase64("signature", signature.sig))) {
    throw new Error("Ed25519 signature did not verify");
  }

  const root = `sha256:${createHash("sha256").update(raw).digest("hex")}`;
  return {
    schema: "vela.reference-read-result.v1",
    kind: contract.kind,
    id: `${contract.prefix}_${root.slice("sha256:".length, "sha256:".length + 16)}`,
    root,
    payload_schema: contract.schema,
    signer: identity.actor_id,
  };
}

try {
  if (process.argv.length !== 3) throw new Error("usage: object.mjs <signed-object.json>");
  process.stdout.write(`${canonical(verify(resolve(process.argv[2])))}\n`);
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
