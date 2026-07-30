#!/usr/bin/env bun

import { spawn } from "node:child_process";
import {
  lstat,
  mkdir,
  readFile,
  realpath,
} from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  CAPSULE_MANIFEST_ROOT,
  DOCKER_ROOT,
  RANGE_END,
  RANGE_START,
  VERIFIER_BINARY_ROOT,
  VERIFIER_IMAGE,
  VERIFIER_IMAGE_DIGEST,
  VERIFIER_SOURCE_ROOT,
  sha256,
} from "./task.mjs";

function options(argv) {
  const parsed = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (
      !["--source", "--docker", "--output"].includes(key) ||
      value === undefined ||
      parsed.has(key)
    ) {
      throw new Error(`invalid verifier-build option near ${key ?? "end"}`);
    }
    parsed.set(key, value);
  }
  if (["--source", "--docker", "--output"].some((key) => !parsed.has(key))) {
    throw new Error(
      "usage: build-verifier.mjs --source <verifier.cpp> " +
      "--docker <docker> --output <new-file>",
    );
  }
  return parsed;
}

function execute(argv, timeoutMs = 120_000) {
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
        finish(() => reject(new Error("verifier build exceeded its output bound")));
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
      finish(() => reject(new Error("verifier build timed out")));
    }, timeoutMs);
  });
}

const values = options(process.argv.slice(2).filter((value) => value !== "--"));
const [source, docker] = await Promise.all([
  realpath(values.get("--source")),
  realpath(values.get("--docker")),
]);
for (const [file, expected, label] of [
  [source, VERIFIER_SOURCE_ROOT, "source"],
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

const output = path.resolve(values.get("--output"));
await mkdir(path.dirname(output), { recursive: true, mode: 0o700 });
try {
  await lstat(output);
  throw new Error("verifier output already exists");
} catch (error) {
  if (!(error instanceof Error) || !("code" in error) || error.code !== "ENOENT") {
    throw error;
  }
}
const image = `${VERIFIER_IMAGE}@${VERIFIER_IMAGE_DIGEST}`;
const inspect = await execute([docker, "image", "inspect", image], 30_000);
if (inspect.code !== 0) {
  throw new Error(`exact verifier build image is not installed: ${image}`);
}
const build = await execute([
  docker,
  "run",
  "--rm",
  "--platform",
  "linux/amd64",
  "--network",
  "none",
  "--cap-drop",
  "ALL",
  "--security-opt",
  "no-new-privileges",
  "--memory",
  "2g",
  "--cpus",
  "2",
  "--pids-limit",
  "64",
  "--mount",
  `type=bind,src=${source},dst=/src/verifier.cpp,readonly`,
  "--mount",
  `type=bind,src=${path.dirname(output)},dst=/out`,
  "--entrypoint",
  "/usr/bin/g++",
  image,
  "-O3",
  "-std=c++17",
  "-static",
  "-s",
  `-DCANOPUS_RANGE_START=${RANGE_START}`,
  `-DCANOPUS_RANGE_END=${RANGE_END}`,
  "/src/verifier.cpp",
  "-o",
  `/out/${path.basename(output)}`,
]);
if (build.code !== 0) {
  throw new Error(
    `verifier build failed: exit=${String(build.code)} ` +
    `signal=${String(build.signal)} stderr_sha256=${sha256(build.stderr)}`,
  );
}
const outputRoot = sha256(await readFile(output));
if (outputRoot !== VERIFIER_BINARY_ROOT) {
  throw new Error(
    `built verifier root drifted: expected ${VERIFIER_BINARY_ROOT}, observed ${outputRoot}`,
  );
}
process.stdout.write(`${JSON.stringify({
  ok: true,
  command: "eval:task:build-verifier",
  verifier: output,
  verifier_root: outputRoot,
  capsule_manifest_root: CAPSULE_MANIFEST_ROOT,
  image,
})}\n`);
