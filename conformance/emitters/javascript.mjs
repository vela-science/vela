#!/usr/bin/env node

import {
  createHash,
  createPrivateKey,
  createPublicKey,
  sign,
  verify,
} from "node:crypto";
import { lstatSync, readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

const PKCS8_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const SPKI_PREFIX = Buffer.from("302a300506032b6570032100", "hex");

function usage(message) {
  if (message) process.stderr.write(`${message}\n\n`);
  process.stderr.write(
    "Usage:\n" +
      "  node conformance/emitters/javascript.mjs submission --draft <json> --seed-file <path> --output <json>\n" +
      "  node conformance/emitters/javascript.mjs verification --draft <json> --seed-file <path> --output <json>\n",
  );
  process.exit(message ? 2 : 0);
}

function parseArgs(argv) {
  const kind = argv[2];
  if (!["submission", "verification"].includes(kind)) usage("object kind is required");
  const options = {};
  for (let index = 3; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!flag?.startsWith("--") || value === undefined) usage(`invalid argument ${flag ?? ""}`);
    options[flag.slice(2)] = value;
  }
  for (const required of ["draft", "seed-file", "output"]) {
    if (!options[required]) usage(`--${required} is required`);
  }
  return { kind, options };
}

function canonical(value) {
  if (value === null || typeof value === "string" || typeof value === "boolean") {
    return JSON.stringify(value);
  }
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) throw new Error("canonical values require safe integers");
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
      .join(",")}}`;
  }
  throw new Error(`unsupported canonical JSON value: ${typeof value}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function readJson(path) {
  const bytes = readFileSync(path);
  if (bytes.length > 8 * 1024 * 1024) throw new Error(`${basename(path)} exceeds 8 MiB`);
  return JSON.parse(bytes.toString("utf8"));
}

function readSeed(path) {
  const absolute = resolve(path);
  const stat = lstatSync(absolute);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error("seed file must be a regular file");
  if ((stat.mode & 0o077) !== 0) throw new Error("seed file permissions must be 0600 or stricter");
  const encoded = readFileSync(absolute, "utf8").trim();
  if (!/^[0-9a-f]{64}$/.test(encoded)) {
    throw new Error("seed file must contain exactly one lowercase 32-byte hex seed");
  }
  return Buffer.from(encoded, "hex");
}

function keyPair(seed) {
  const privateKey = createPrivateKey({
    key: Buffer.concat([PKCS8_PREFIX, seed]),
    format: "der",
    type: "pkcs8",
  });
  const publicKey = createPublicKey(privateKey);
  const spki = publicKey.export({ format: "der", type: "spki" });
  if (!Buffer.isBuffer(spki) || !spki.subarray(0, SPKI_PREFIX.length).equals(SPKI_PREFIX)) {
    throw new Error("derived key is not Ed25519");
  }
  return { privateKey, publicKey, publicBytes: spki.subarray(SPKI_PREFIX.length) };
}

function buildBinding(actorId, createdAt, keys) {
  let binding = {
    schema: "vela.identity_binding.v0.1",
    binding_id: "",
    actor_id: actorId,
    actor_class: "agent",
    public_key_hex: keys.publicBytes.toString("hex"),
    created_at: createdAt,
    signature: "",
  };
  const preimage = canonical(binding);
  binding = {
    ...binding,
    binding_id: `vib_${sha256(preimage).slice(0, 16)}`,
    signature: sign(null, Buffer.from(preimage), keys.privateKey).toString("hex"),
  };
  if (!verify(null, Buffer.from(preimage), keys.publicKey, Buffer.from(binding.signature, "hex"))) {
    throw new Error("identity binding self-check failed");
  }
  return binding;
}

function buildSubmission(draft, keys) {
  const producer = draft?.provenance?.producer;
  const emittedAt = draft?.provenance?.emitted_at;
  if (typeof producer !== "string" || !producer.startsWith("agent:")) {
    throw new Error("submission provenance.producer must start with agent:");
  }
  if (typeof emittedAt !== "string" || emittedAt.length === 0) {
    throw new Error("submission provenance.emitted_at is required");
  }
  const binding = buildBinding(producer, emittedAt, keys);
  let object = {
    schema: "vela.submission.v1",
    submission_id: "",
    ...draft,
    authentication: {
      algorithm: "ed25519",
      identity_binding: binding,
      signature: "",
    },
  };
  const preimage = canonical(object);
  object = {
    ...object,
    submission_id: `vsb_${sha256(preimage).slice(0, 16)}`,
    authentication: {
      ...object.authentication,
      signature: sign(null, Buffer.from(preimage), keys.privateKey).toString("hex"),
    },
  };
  return object;
}

function buildVerification(draft, keys) {
  const verifier = draft?.verifier;
  const createdAt = draft?.started_at;
  if (typeof verifier !== "string" || verifier.length === 0) {
    throw new Error("verification verifier is required");
  }
  if (typeof createdAt !== "string" || createdAt.length === 0) {
    throw new Error("verification started_at is required");
  }
  const binding = buildBinding(verifier, createdAt, keys);
  let object = {
    schema: "vela.verification-record.v1",
    verification_record_id: "",
    ...draft,
    authentication: {
      algorithm: "ed25519",
      identity_binding: binding,
      signature: "",
    },
  };
  const preimage = canonical(object);
  object = {
    ...object,
    verification_record_id: `vvr_${sha256(preimage).slice(0, 16)}`,
    authentication: {
      ...object.authentication,
      signature: sign(null, Buffer.from(preimage), keys.privateKey).toString("hex"),
    },
  };
  return object;
}

const { kind, options } = parseArgs(process.argv);
const keys = keyPair(readSeed(options["seed-file"]));
const draft = readJson(options.draft);
const object = kind === "submission" ? buildSubmission(draft, keys) : buildVerification(draft, keys);
const bytes = `${canonical(object)}\n`;
writeFileSync(options.output, bytes, { encoding: "utf8", flag: "wx", mode: 0o444 });
process.stdout.write(
  `${canonical({
    schema: "vela.reference-emission-result.v1",
    kind,
    id: kind === "submission" ? object.submission_id : object.verification_record_id,
    root: `sha256:${sha256(bytes.slice(0, -1))}`,
    output: resolve(options.output),
  })}\n`,
);
