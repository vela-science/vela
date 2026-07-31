import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { validateRequestedChange } from "./index.js";

interface Case {
  name: string;
  valid: boolean;
  value: unknown;
}

test("requested-change vocabulary matches the shared current-object matrix", async () => {
  const cases = JSON.parse(
    await readFile(
      new URL("../../../conformance/current-objects/requested-change-cases.json", import.meta.url),
      "utf8",
    ),
  ) as Case[];

  for (const entry of cases) {
    if (entry.valid) {
      assert.doesNotThrow(() => validateRequestedChange(entry.value), entry.name);
    } else {
      assert.throws(() => validateRequestedChange(entry.value), entry.name);
    }
  }
});
