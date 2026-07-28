import assert from "node:assert/strict";
import { link, mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { sha256Bytes } from "../src/util/canonical.js";
import { sha256RegularFile } from "../src/util/files.js";

test("sha256RegularFile streams a bounded regular file", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-file-digest-"));
  const file = path.join(root, "binary");
  const bytes = Buffer.alloc(2 * 1024 * 1024 + 17, 0x5a);
  await writeFile(file, bytes, { mode: 0o700 });
  assert.equal(await sha256RegularFile(file, bytes.length), sha256Bytes(bytes));
  await assert.rejects(sha256RegularFile(file, bytes.length - 1), /exceeds/u);
});

test("sha256RegularFile rejects multiply linked inputs", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-file-link-"));
  const file = path.join(root, "binary");
  await writeFile(file, "binary\n", { mode: 0o700 });
  await link(file, path.join(root, "alias"));
  await assert.rejects(sha256RegularFile(file, 1024), /singly linked/u);
});
