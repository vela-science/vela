#!/usr/bin/env bun

import { mkdir, lstat, readFile, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  REPOSITORY_INDEX_PATH,
  SOURCE_PACKET_PATH,
  SOURCE_PACKET_ROOT,
  SOURCE_REPOSITORY_ROOT,
  TARGET_INDEX_FILE_ROOT,
  TARGET_INDEX_PATH,
  TASK_ID,
  TASK_PACKET_ROOT,
  buildPacket,
  packetBytes,
  sha256,
} from "./task.mjs";
import {
  assertOutputOutsideFrontier,
  inspectCheckout,
  readBoundFile,
} from "./source.mjs";

function options(argv) {
  const parsed = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (
      !["--frontier", "--output"].includes(key) ||
      value === undefined ||
      parsed.has(key)
    ) {
      throw new Error(`invalid Erdős task option near ${key ?? "end"}`);
    }
    parsed.set(key, value);
  }
  if (!parsed.has("--frontier") || !parsed.has("--output")) {
    throw new Error(
      "usage: prepare.mjs --frontier <clean-erdos-frontier> --output <new-directory>",
    );
  }
  return parsed;
}

const values = options(process.argv.slice(2).filter((value) => value !== "--"));
const requestedFrontier = path.resolve(values.get("--frontier"));
const requestedMetadata = await lstat(requestedFrontier);
if (!requestedMetadata.isDirectory() || requestedMetadata.isSymbolicLink()) {
  throw new Error("Erdős Frontier path must be a real directory, not a symlink");
}
const frontier = await realpath(requestedFrontier);
const output = path.resolve(values.get("--output"));
assertOutputOutsideFrontier(frontier, output);

await inspectCheckout(frontier);
const [repositoryIndexBytes, targetIndexBytes, targetPacketBytes] =
  await Promise.all([
    readBoundFile(
      frontier,
      REPOSITORY_INDEX_PATH,
      SOURCE_REPOSITORY_ROOT,
      2 * 1024 * 1024,
    ),
    readBoundFile(
      frontier,
      TARGET_INDEX_PATH,
      TARGET_INDEX_FILE_ROOT,
      64 * 1024,
    ),
    readBoundFile(
      frontier,
      SOURCE_PACKET_PATH,
      SOURCE_PACKET_ROOT,
      64 * 1024,
    ),
  ]);
const packet = buildPacket({
  repositoryIndexBytes,
  targetIndexBytes,
  targetPacketBytes,
});
const bytes = packetBytes(packet);
const observedRoot = sha256(bytes);
if (observedRoot !== TASK_PACKET_ROOT) {
  throw new Error(
    `evaluation task packet root drifted: expected ${TASK_PACKET_ROOT}, observed ${observedRoot}`,
  );
}
const retainedPacketBytes = await readFile(new URL("./packet.json", import.meta.url));
if (!bytes.equals(retainedPacketBytes)) {
  throw new Error("retained evaluation task packet does not match the rooted source projection");
}
await inspectCheckout(frontier);

await mkdir(output, { recursive: false, mode: 0o700 });
const packetFile = path.join(output, "packet.json");
await writeFile(packetFile, bytes, { flag: "wx", mode: 0o600 });
process.stdout.write(`${JSON.stringify({
  ok: true,
  command: "eval:task:prepare",
  task_id: TASK_ID,
  source_commit: packet.source.repository.checkout_commit,
  source_tree: packet.source.repository.checkout_tree,
  source_repository_root: packet.source.repository.repository_root,
  target_index_root: packet.source.target_index.index_root,
  source_packet_root: packet.source.target.packet_root,
  packet: packetFile,
  packet_root: observedRoot,
})}\n`);
