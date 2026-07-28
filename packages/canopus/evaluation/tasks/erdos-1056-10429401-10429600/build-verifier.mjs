#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { access, lstat, readFile, realpath } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  RANGE_END,
  RANGE_START,
  VERIFIER_COMPILER_ROOT,
  VERIFIER_ROOT,
  VERIFIER_SOURCE_ROOT,
} from "./task.mjs";

function sha256(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function options(argv) {
  const parsed = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (
      !["--source", "--compiler", "--output"].includes(key) ||
      value === undefined ||
      parsed.has(key)
    ) {
      throw new Error(`invalid verifier-build option near ${key ?? "end"}`);
    }
    parsed.set(key, value);
  }
  if (["--source", "--compiler", "--output"].some((key) => !parsed.has(key))) {
    throw new Error(
      "usage: build-verifier.mjs --source <verifier.cpp> " +
      "--compiler <aarch64-linux-gnu-g++> --output <new-file>",
    );
  }
  return parsed;
}

async function execute(argv) {
  return await new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), {
      env: { PATH: "/usr/bin:/bin", LANG: "C", LC_ALL: "C" },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    let bytes = 0;
    const collect = (target, chunk) => {
      bytes += chunk.length;
      if (bytes > 1_048_576) {
        child.kill("SIGKILL");
        reject(new Error("verifier compiler exceeded its output bound"));
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
          `verifier compiler exited ${String(code)}: ` +
          `${Buffer.concat(stderr).toString("utf8").trim()}`,
        ));
        return;
      }
      resolve();
    });
  });
}

const values = options(process.argv.slice(2).filter((value) => value !== "--"));
const [source, compiler] = await Promise.all([
  realpath(values.get("--source")),
  realpath(values.get("--compiler")),
]);
for (const [file, expected, label] of [
  [source, VERIFIER_SOURCE_ROOT, "source"],
  [compiler, VERIFIER_COMPILER_ROOT, "compiler"],
]) {
  const metadata = await lstat(file);
  if (!metadata.isFile() || metadata.isSymbolicLink() || sha256(await readFile(file)) !== expected) {
    throw new Error(`verifier ${label} identity drifted`);
  }
}
const output = path.resolve(values.get("--output"));
await access(path.dirname(output));
try {
  await lstat(output);
  throw new Error("verifier output already exists");
} catch (error) {
  if (!(error instanceof Error) || !("code" in error) || error.code !== "ENOENT") {
    throw error;
  }
}
await execute([
  compiler,
  "-O3",
  "-std=c++20",
  "-static",
  "-s",
  `-DCANOPUS_RANGE_START=${RANGE_START}`,
  `-DCANOPUS_RANGE_END=${RANGE_END}`,
  source,
  "-o",
  output,
]);
const outputRoot = sha256(await readFile(output));
if (outputRoot !== VERIFIER_ROOT) {
  throw new Error(
    `built verifier root drifted: expected ${VERIFIER_ROOT}, observed ${outputRoot}`,
  );
}
process.stdout.write(`${JSON.stringify({
  ok: true,
  command: "eval:task:build-verifier",
  verifier: output,
  verifier_root: outputRoot,
})}\n`);
