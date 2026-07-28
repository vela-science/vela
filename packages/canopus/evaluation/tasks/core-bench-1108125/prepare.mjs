#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  SOURCE_ARCHIVE_ROOT,
  SOURCE_FILES,
  assertSafeArchiveEntries,
  buildPacket,
  packetBytes,
} from "./task.mjs";

function parseOptions(argv) {
  const options = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!["--archive", "--output"].includes(key) || value === undefined || options.has(key)) {
      throw new Error(`invalid task-preparation option near ${key ?? "end"}`);
    }
    options.set(key, value);
  }
  if (!options.has("--archive") || !options.has("--output")) {
    throw new Error("usage: prepare.mjs --archive <capsule.tar.gz> --output <new-directory>");
  }
  return options;
}

async function execute(argv, maxBytes = 8 * 1024 * 1024) {
  return await new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), {
      env: {
        PATH: "/usr/bin:/bin",
        LANG: "C",
        LC_ALL: "C",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    let bytes = 0;
    const collect = (target, chunk) => {
      bytes += chunk.length;
      if (bytes > maxBytes) {
        child.kill("SIGKILL");
        reject(new Error("task preparation command exceeded its output bound"));
        return;
      }
      target.push(chunk);
    };
    child.stdout.on("data", (chunk) => collect(stdout, chunk));
    child.stderr.on("data", (chunk) => collect(stderr, chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(
          `${argv[0]} exited ${String(code)}: ${Buffer.concat(stderr).toString("utf8").trim()}`,
        ));
        return;
      }
      resolve(Buffer.concat(stdout));
    });
  });
}

function sha256(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

const options = parseOptions(process.argv.slice(2).filter((value) => value !== "--"));
const archive = await realpath(options.get("--archive"));
const archiveBytes = await readFile(archive);
const observedArchiveRoot = sha256(archiveBytes);
if (observedArchiveRoot !== SOURCE_ARCHIVE_ROOT) {
  throw new Error(
    `source archive root drifted: expected ${SOURCE_ARCHIVE_ROOT}, ` +
    `observed ${observedArchiveRoot}`,
  );
}

const listing = (await execute(["/usr/bin/tar", "-tzf", archive]))
  .toString("utf8")
  .split("\n")
  .filter((entry) => entry.length > 0);
assertSafeArchiveEntries(listing);

const files = new Map();
for (const relative of SOURCE_FILES) {
  const bytes = await execute([
    "/usr/bin/tar",
    "-xOzf",
    archive,
    `capsule-1108125/${relative}`,
  ]);
  files.set(relative, bytes);
}

const packet = buildPacket(files);
const bytes = packetBytes(packet);
const output = path.resolve(options.get("--output"));
await mkdir(output, { recursive: false, mode: 0o700 });
const packetFile = path.join(output, "packet.json");
await writeFile(packetFile, bytes, { flag: "wx", mode: 0o600 });
process.stdout.write(`${JSON.stringify({
  ok: true,
  command: "eval:task:prepare",
  source_root: observedArchiveRoot,
  packet: packetFile,
  packet_root: sha256(bytes),
  projected_files: packet.files.length,
})}\n`);
