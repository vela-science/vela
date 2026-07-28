#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import {
  lstat,
  mkdtemp,
  readFile,
  realpath,
  rm,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";

import { canonicalJson } from "../../lib/evaluation-plan.mjs";
import {
  IMAGE,
  IMAGE_DIGEST,
  SOURCE_ARCHIVE_ROOT,
  assertSafeArchiveEntries,
  verifierRecord,
  verifierRecordRoot,
} from "./task.mjs";

function parseOptions(argv) {
  const options = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (
      !["--archive", "--artifact", "--docker"].includes(key) ||
      value === undefined ||
      options.has(key)
    ) {
      throw new Error(`invalid verifier option near ${key ?? "end"}`);
    }
    options.set(key, value);
  }
  if (!options.has("--archive") || !options.has("--artifact")) {
    throw new Error(
      "usage: verify.mjs --archive <capsule.tar.gz> --artifact <result.json> " +
      "[--docker <binary>]",
    );
  }
  return options;
}

async function execute(argv, options = {}) {
  return await new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), {
      cwd: options.cwd,
      env: options.env ?? {
        PATH: "/usr/local/bin:/usr/bin:/bin",
        LANG: "C",
        LC_ALL: "C",
      },
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
      if (bytes > (options.maxBytes ?? 16 * 1024 * 1024)) {
        child.kill("SIGKILL");
        finish(() => reject(new Error("verifier command exceeded its output bound")));
        return;
      }
      target.push(chunk);
    };
    child.stdout.on("data", (chunk) => collect(stdout, chunk));
    child.stderr.on("data", (chunk) => collect(stderr, chunk));
    child.on("error", (error) => finish(() => reject(error)));
    child.on("close", (code, signal) => finish(() => resolve({
      exitCode: code,
      signal,
      stdout: Buffer.concat(stdout),
      stderr: Buffer.concat(stderr),
    })));
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      finish(() => reject(new Error("verifier command exceeded 60 seconds")));
    }, options.timeoutMs ?? 60_000);
  });
}

function sha256(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

const options = parseOptions(process.argv.slice(2).filter((value) => value !== "--"));
const [archive, artifact, docker] = await Promise.all([
  realpath(options.get("--archive")),
  realpath(options.get("--artifact")),
  realpath(options.get("--docker") ?? "/usr/local/bin/docker"),
]);
for (const [file, at] of [[archive, "archive"], [artifact, "artifact"], [docker, "docker"]]) {
  const metadata = await lstat(file);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`${at} must be a regular non-symbolic file`);
  }
}
const archiveBytes = await readFile(archive);
if (sha256(archiveBytes) !== SOURCE_ARCHIVE_ROOT) {
  throw new Error("verification source archive root drifted");
}
const artifactBytes = await readFile(artifact);
if (artifactBytes.length === 0 || artifactBytes.length > 64 * 1024) {
  throw new Error("verification artifact exceeds its byte contract");
}

const temp = await mkdtemp(path.join(os.tmpdir(), "canopus-eval-1108125-"));
try {
  const listing = await execute(["/usr/bin/tar", "-tzf", archive]);
  const entries = listing.stdout.toString("utf8").split("\n").filter(Boolean);
  assertSafeArchiveEntries(entries);
  const extraction = await execute([
    "/usr/bin/tar",
    "-xzf",
    archive,
    "-C",
    temp,
    "capsule-1108125/code",
    "capsule-1108125/data",
  ]);
  if (extraction.exitCode !== 0) {
    throw new Error(`source extraction failed: ${extraction.stderr.toString("utf8").trim()}`);
  }

  const image = `${IMAGE}@${IMAGE_DIGEST}`;
  const inspect = await execute([docker, "image", "inspect", image], {
    timeoutMs: 30_000,
    maxBytes: 1_048_576,
  });
  if (inspect.exitCode !== 0) {
    throw new Error(
      `exact verifier image is not installed; pull ${image} before verification`,
    );
  }

  const capsule = path.join(temp, "capsule-1108125");
  const replay = await execute([
    docker,
    "run",
    "--platform",
    "linux/amd64",
    "--rm",
    "--network",
    "none",
    "--cpus",
    "4",
    "--memory",
    "8g",
    "--pids-limit",
    "256",
    "--cap-drop",
    "ALL",
    "--security-opt",
    "no-new-privileges",
    "--read-only",
    "--env",
    "HOME=/tmp/home",
    "--env",
    "OPENBLAS_NUM_THREADS=1",
    "--env",
    "OMP_NUM_THREADS=1",
    "--tmpfs",
    "/tmp:rw,noexec,nosuid,size=512m",
    "--tmpfs",
    "/results:rw,noexec,nosuid,size=128m",
    "--workdir",
    "/code",
    "--volume",
    `${path.join(capsule, "code")}:/code:ro`,
    "--volume",
    `${path.join(capsule, "data")}:/data:ro`,
    image,
    "sh",
    "-lc",
    [
      'mkdir -p "$HOME"',
      "bash run",
      "printf 'CANOPUS_FIGURE_S5 '",
      "sha256sum /results/FigureS5.png | cut -d ' ' -f 1 | sed 's/^/sha256:/'",
    ].join(" && "),
  ], {
    timeoutMs: 60_000,
    maxBytes: 16 * 1024 * 1024,
  });
  if (replay.exitCode !== 0) {
    throw new Error(
      `frozen replay failed: exit=${String(replay.exitCode)} ` +
      `signal=${String(replay.signal)} stderr_sha256=${sha256(replay.stderr)}`,
    );
  }
  const record = verifierRecord({
    artifactBytes,
    stdout: replay.stdout,
    stderr: replay.stderr,
  });
  process.stdout.write(canonicalJson({
    ...record,
    verifier_result_root: verifierRecordRoot(record),
  }));
} finally {
  await rm(temp, { recursive: true, force: true });
}
