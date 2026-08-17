#!/usr/bin/env node

/* An independent emitter for the current signed Vela objects.
 *
 * Nothing here imports the Rust implementation. It reads the published
 * contract — RFC 8785 canonical JSON, SHA-256 roots, DSSE envelopes over
 * versioned payload types — and must land on the same bytes.
 *
 * The object is a DSSE envelope: the payload is the canonical scientific
 * content, the signature covers the DSSE pre-authentication encoding of those
 * exact bytes, and the identifier is derived from the envelope's own root
 * rather than stored inside it.
 */

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

const KINDS = {
  submission: {
    schema: "vela.submission.v3",
    payloadType: "application/vnd.vela.submission.v3+json",
    prefix: "vsb",
  },
  verification: {
    schema: "vela.verification-record.v2",
    payloadType: "application/vnd.vela.verification-record.v2+json",
    prefix: "vvr",
  },
};

function usage(message) {
  if (message) process.stderr.write(`${message}\n\n`);
  process.stderr.write(
    "Usage:\n" +
      "  node conformance/emitters/javascript.mjs <submission|verification> \\\n" +
      "    --draft <json> --seed-file <path> --actor <id> --actor-class <human|agent|org> \\\n" +
      "    --declared-at <rfc3339> --output <json>\n",
  );
  process.exit(message ? 2 : 0);
}

function parseArgs(argv) {
  const kind = argv[2];
  if (!Object.hasOwn(KINDS, kind)) usage("object kind is required");
  const options = {};
  for (let index = 3; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!flag?.startsWith("--") || value === undefined) usage(`invalid argument ${flag ?? ""}`);
    options[flag.slice(2)] = value;
  }
  for (const required of ["draft", "seed-file", "actor", "actor-class", "declared-at", "output"]) {
    if (!options[required]) usage(`--${required} is required`);
  }
  if (!["human", "agent", "org"].includes(options["actor-class"])) {
    usage("--actor-class must be human, agent or org");
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

/* DSSE Pre-Authentication Encoding:
   `DSSEv1 SP LEN(payloadType) SP payloadType SP LEN(payload) SP payload`. */
function pae(payloadType, payload) {
  const type = Buffer.from(payloadType, "utf8");
  return Buffer.concat([
    Buffer.from("DSSEv1 ", "utf8"),
    Buffer.from(String(type.length), "utf8"),
    Buffer.from(" ", "utf8"),
    type,
    Buffer.from(" ", "utf8"),
    Buffer.from(String(payload.length), "utf8"),
    Buffer.from(" ", "utf8"),
    payload,
  ]);
}

/* A draft may not supply anything the emitter produces.
 *
 * The spread below puts draft keys after `schema` and `identity`, so a draft
 * carrying either would sign a payload under a type or an actor nobody asked
 * for. `python.py` refuses identically. */
function refuseSignedFields(draft) {
  const supplied = ["schema", "identity"].filter((field) => field in draft);
  if (supplied.length) {
    throw new Error(
      `draft supplies ${supplied.join(", ")}, which the emitter produces. Pass a draft, not a signed object.`,
    );
  }
}

function build(kind, draft, keys, options) {
  const { schema, payloadType } = KINDS[kind];
  refuseSignedFields(draft);
  if (kind === "submission" && draft?.provenance?.producer !== options.actor) {
    throw new Error("submission provenance.producer must be the declared signer");
  }
  const object = {
    schema,
    identity: {
      schema: "vela.signer-identity.v1",
      actor_id: options.actor,
      actor_class: options["actor-class"],
      public_key_hex: keys.publicBytes.toString("hex"),
      declared_at: options["declared-at"],
    },
    ...draft,
  };
  const payload = Buffer.from(canonical(object), "utf8");
  const signature = sign(null, pae(payloadType, payload), keys.privateKey);
  if (!verify(null, pae(payloadType, payload), keys.publicKey, signature)) {
    throw new Error("envelope self-check failed");
  }
  return {
    payloadType,
    payload: payload.toString("base64"),
    signatures: [
      { keyid: keys.publicBytes.toString("hex"), sig: signature.toString("base64") },
    ],
  };
}

const { kind, options } = parseArgs(process.argv);
const keys = keyPair(readSeed(options["seed-file"]));
const envelope = build(kind, readJson(options.draft), keys, options);

/* The retained bytes are the canonical envelope exactly, with no trailing
   newline: the published root is over the file a reader is handed, and a byte
   the root does not cover would make the two disagree. */
const bytes = Buffer.from(canonical(envelope), "utf8");
writeFileSync(options.output, bytes, { flag: "wx", mode: 0o444 });
const root = `sha256:${sha256(bytes)}`;
process.stdout.write(
  `${canonical({
    schema: "vela.reference-emission-result.v1",
    kind,
    id: `${KINDS[kind].prefix}_${root.slice("sha256:".length, "sha256:".length + 16)}`,
    root,
    output: resolve(options.output),
  })}\n`,
);
