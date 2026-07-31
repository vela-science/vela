import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  canonicalJcs,
  canonicalJson,
  protocolDigest,
  sha256Bytes,
  verifySubmission,
  type SubmissionV1,
} from "./index.js";

test("canonical encodings sort nested object keys", () => {
  const value = { z: 1, a: { y: true, b: null } };
  assert.equal(canonicalJcs(value), '{"a":{"b":null,"y":true},"z":1}');
  assert.equal(canonicalJson(value), '{"a":{"b":null,"y":true},"z":1}\n');
});

test("protocol roots use JCS without the retained-record newline", () => {
  assert.equal(protocolDigest({ value: 1 }), sha256Bytes('{"value":1}'));
});

test("the shared validator accepts the independent current Submission fixture", async () => {
  const fixture = JSON.parse(
    await readFile(
      new URL("../../../conformance/current-objects/submission.json", import.meta.url),
      "utf8",
    ),
  ) as SubmissionV1;
  assert.doesNotThrow(() => verifySubmission(fixture));
});
