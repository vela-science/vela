#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import { buildPacket, packetBytes } from "./task.mjs";

function options(argv) {
  const parsed = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!["--source", "--output"].includes(key) || value === undefined || parsed.has(key)) {
      throw new Error(`invalid Erdős task option near ${key ?? "end"}`);
    }
    parsed.set(key, value);
  }
  if (!parsed.has("--source") || !parsed.has("--output")) {
    throw new Error("usage: prepare.mjs --source <problem-packet.json> --output <new-directory>");
  }
  return parsed;
}

const values = options(process.argv.slice(2).filter((value) => value !== "--"));
const source = await realpath(values.get("--source"));
const packet = buildPacket(await readFile(source));
const bytes = packetBytes(packet);
const output = path.resolve(values.get("--output"));
await mkdir(output, { recursive: false, mode: 0o700 });
const packetFile = path.join(output, "packet.json");
await writeFile(packetFile, bytes, { flag: "wx", mode: 0o600 });
process.stdout.write(`${JSON.stringify({
  ok: true,
  command: "eval:task:prepare",
  task_id: packet.task_id,
  source_root: packet.source.packet_root,
  packet: packetFile,
  packet_root: `sha256:${createHash("sha256").update(bytes).digest("hex")}`,
})}\n`);
