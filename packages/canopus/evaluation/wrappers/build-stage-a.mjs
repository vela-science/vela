#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { lstat } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

function options(argv) {
  if (argv.length !== 2 || argv[0] !== "--output") {
    throw new Error("usage: build-stage-a.mjs --output <new-file>");
  }
  return path.resolve(argv[1]);
}

async function execute(argv) {
  return await new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), {
      cwd: path.resolve(import.meta.dirname, "../.."),
      env: {
        PATH: process.env.PATH ?? "",
        HOME: process.env.HOME ?? "",
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
      if (bytes > 8 * 1024 * 1024) {
        child.kill("SIGKILL");
        reject(new Error("wrapper build exceeded its output bound"));
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
          `wrapper build exited ${String(code)}: ` +
          `${Buffer.concat(stderr).toString("utf8").trim()}`,
        ));
        return;
      }
      resolve();
    });
  });
}

const output = options(process.argv.slice(2).filter((value) => value !== "--"));
try {
  await lstat(output);
  throw new Error("wrapper output already exists");
} catch (error) {
  if (!(error instanceof Error) || !("code" in error) || error.code !== "ENOENT") {
    throw error;
  }
}
await execute([
  process.execPath,
  "build",
  "evaluation/wrappers/stage-a.mjs",
  "--target",
  "bun",
  "--outfile",
  output,
]);
process.stdout.write(`${JSON.stringify({
  ok: true,
  command: "eval:wrapper:build",
  wrapper: output,
})}\n`);

