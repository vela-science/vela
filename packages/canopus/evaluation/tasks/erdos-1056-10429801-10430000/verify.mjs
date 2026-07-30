#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { lstat, readFile, realpath } from "node:fs/promises";
import process from "node:process";

import {
  CAPSULE_MANIFEST_ROOT,
  DOCKER_ROOT,
  VERIFIER_BINARY_ROOT,
  VERIFIER_IMAGE,
  VERIFIER_IMAGE_DIGEST,
  sha256,
} from "./task.mjs";

function options(argv) {
  const parsed = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (
      !["--artifact", "--binary", "--docker"].includes(key) ||
      value === undefined ||
      parsed.has(key)
    ) {
      throw new Error(`invalid verifier option near ${key ?? "end"}`);
    }
    parsed.set(key, value);
  }
  if (["--artifact", "--binary", "--docker"].some((key) => !parsed.has(key))) {
    throw new Error(
      "usage: verify.mjs --artifact <result.txt> --binary <verifier> " +
      "--docker <docker>",
    );
  }
  return parsed;
}

function execute(argv, timeoutMs = 60_000) {
  return new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), {
      env: { PATH: "/usr/local/bin:/usr/bin:/bin", LANG: "C", LC_ALL: "C" },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    let bytes = 0;
    let settled = false;
    const finish = (callback) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      callback();
    };
    const collect = (target, chunk) => {
      bytes += chunk.length;
      if (bytes > 2 * 1024 * 1024) {
        child.kill("SIGKILL");
        finish(() => reject(new Error("verifier exceeded its output bound")));
        return;
      }
      target.push(chunk);
    };
    child.stdout.on("data", (chunk) => collect(stdout, chunk));
    child.stderr.on("data", (chunk) => collect(stderr, chunk));
    child.on("error", (error) => finish(() => reject(error)));
    child.on("close", (code, signal) => finish(() => resolve({
      code,
      signal,
      stdout: Buffer.concat(stdout),
      stderr: Buffer.concat(stderr),
    })));
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      finish(() => reject(new Error("verifier timed out")));
    }, timeoutMs);
  });
}

const values = options(process.argv.slice(2).filter((value) => value !== "--"));
const [artifact, binary, docker] = await Promise.all([
  realpath(values.get("--artifact")),
  realpath(values.get("--binary")),
  realpath(values.get("--docker")),
]);
for (const [file, expected, label] of [
  [binary, VERIFIER_BINARY_ROOT, "binary"],
  [docker, DOCKER_ROOT, "Docker client"],
]) {
  const metadata = await lstat(file);
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    sha256(await readFile(file)) !== expected
  ) {
    throw new Error(`verifier ${label} identity drifted`);
  }
}
const manifestBytes = await readFile(new URL("./capsule.json", import.meta.url));
if (sha256(manifestBytes) !== CAPSULE_MANIFEST_ROOT) {
  throw new Error("verifier capsule manifest identity drifted");
}
const artifactBytes = await readFile(artifact);
if (artifactBytes.length === 0 || artifactBytes.length > 64 * 1024) {
  throw new Error("verification artifact exceeds its byte contract");
}
const image = `${VERIFIER_IMAGE}@${VERIFIER_IMAGE_DIGEST}`;
const inspect = await execute([docker, "image", "inspect", image], 30_000);
if (inspect.code !== 0) {
  throw new Error(`exact verifier image is not installed: ${image}`);
}
const result = await execute([
  docker,
  "run",
  "--rm",
  "--platform",
  "linux/amd64",
  "--network",
  "none",
  "--read-only",
  "--cap-drop",
  "ALL",
  "--security-opt",
  "no-new-privileges",
  "--memory",
  "1g",
  "--cpus",
  "1",
  "--pids-limit",
  "64",
  "--mount",
  `type=bind,src=${binary},dst=/verifier,readonly`,
  "--mount",
  `type=bind,src=${artifact},dst=/artifact,readonly`,
  "--entrypoint",
  "/verifier",
  image,
  "/artifact",
]);
if (result.code !== 0) {
  throw new Error(
    `bounded verifier failed: exit=${String(result.code)} ` +
    `signal=${String(result.signal)} stderr_sha256=${sha256(result.stderr)}`,
  );
}
process.stdout.write(result.stdout);
