import { createHash } from "node:crypto";

function normalize(value: unknown, at: string): unknown {
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "boolean"
  ) {
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError(`${at} contains a non-finite number`);
    }
    return value;
  }
  if (Array.isArray(value)) {
    return value.map((item, index) => normalize(item, `${at}[${index}]`));
  }
  if (typeof value === "object") {
    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) {
      throw new TypeError(`${at} must contain plain JSON objects`);
    }
    const input = value as Record<string, unknown>;
    const output: Record<string, unknown> = {};
    for (const key of Object.keys(input).sort()) {
      const item = input[key];
      if (item === undefined) {
        throw new TypeError(`${at}.${key} is undefined`);
      }
      output[key] = normalize(item, `${at}.${key}`);
    }
    return output;
  }
  throw new TypeError(`${at} contains a non-JSON value`);
}

export function canonicalJson(value: unknown): string {
  return `${JSON.stringify(normalize(value, "$"))}\n`;
}

// RFC 8785 uses ECMAScript JSON serialization over recursively sorted object
// keys and does not append a record newline. Vela protocol objects use these bytes.
export function canonicalJcs(value: unknown): string {
  return JSON.stringify(normalize(value, "$"));
}

export function sha256Bytes(value: Uint8Array | string): string {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

export function contentDigest(value: unknown): string {
  return sha256Bytes(canonicalJson(value));
}

// Vela protocol objects use compact canonical JSON bytes, without the record
// newline used by Canopus-owned manifests and activity records.
export function protocolDigest(value: unknown): string {
  return sha256Bytes(canonicalJcs(value));
}
