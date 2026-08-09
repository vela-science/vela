#!/usr/bin/env node

/* Clean-room RFC 8785 vector reader using the ECMAScript primitives the
 * standard normatively selects for string and number serialization. It shares
 * no code with the Rust minter or the Python reader. */

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

function requireUnicodeScalar(text) {
  for (let index = 0; index < text.length; index += 1) {
    const unit = text.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = text.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        throw new Error("I-JSON strings may not contain a lone high surrogate");
      }
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      throw new Error("I-JSON strings may not contain a lone low surrogate");
    }
  }
}

export function canonical(value) {
  if (value === null || typeof value === "boolean") return JSON.stringify(value);
  if (typeof value === "string") {
    requireUnicodeScalar(value);
    return JSON.stringify(value);
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error("I-JSON numbers must be finite");
    if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
      throw new Error("protocol integers must be IEEE-754 interoperable integers");
    }
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (typeof value === "object") {
    return `{${Object.keys(value)
      // ECMAScript's default string comparison is the UTF-16 code-unit order
      // RFC 8785 requires for property names.
      .sort()
      .map((key) => {
        requireUnicodeScalar(key);
        return `${JSON.stringify(key)}:${canonical(value[key])}`;
      })
      .join(",")}}`;
  }
  throw new Error(`unsupported JSON value: ${typeof value}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function main(argv) {
  const corpus = resolve(argv[2] ?? "conformance/canonical-hashing.json");
  const document = JSON.parse(readFileSync(corpus, "utf8"));
  if (document.format_id !== "RFC8785" || !Array.isArray(document.vectors)) {
    throw new Error("canonical vector corpus has the wrong format or shape");
  }

  let failures = 0;
  for (const vector of document.vectors) {
    const encoded = canonical(vector.input);
    const digest = sha256(Buffer.from(encoded, "utf8"));
    if (encoded !== vector.canonical || digest !== vector.sha256) {
      failures += 1;
      process.stderr.write(`FAIL ${vector.name}: JavaScript canonical bytes or root diverged\n`);
    }
  }
  process.stdout.write(
    `javascript-canonical-hashing: ${document.vectors.length} vectors, ${failures} FAILED\n`,
  );
  return failures === 0 ? 0 : 1;
}

if (process.argv[1] && import.meta.url === new URL(`file://${resolve(process.argv[1])}`).href) {
  try {
    process.exitCode = main(process.argv);
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 2;
  }
}
