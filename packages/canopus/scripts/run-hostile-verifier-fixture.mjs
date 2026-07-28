#!/usr/bin/env node

import { execFile } from "node:child_process";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const exec = promisify(execFile);
const args = process.argv.slice(2);
const options = new Map();
for (let index = 0; index < args.length; index += 2) {
  const key = args[index];
  if (key !== "--image" && key !== "--docker") throw new Error(`unknown option ${key}`);
  if (args[index + 1] === undefined || options.has(key)) throw new Error(`invalid option ${key}`);
  options.set(key, args[index + 1]);
}
const image = options.get("--image");
if (image === undefined) throw new Error("--image is required");
const docker = options.get("--docker") ?? "docker";
const probe = fileURLToPath(new URL("../tests/fixtures/hostile-verifier/probe.py", import.meta.url));
const runtime = await mkdtemp(path.join(os.tmpdir(), "canopus-hostile-verifier-"));

try {
  const input = path.join(runtime, "input");
  const artifact = path.join(runtime, "artifact");
  await mkdir(input);
  await writeFile(path.join(input, "sealed"), "sealed\n", { mode: 0o444 });
  await writeFile(artifact, "candidate\n", { mode: 0o444 });
  const { stdout, stderr } = await exec(docker, [
    "run", "--rm", "--init", "--read-only", "--network=none",
    "--cap-drop=ALL", "--security-opt=no-new-privileges",
    "--memory=1024m", "--cpus=1", "--pids-limit=64",
    "--env", "HOME=/nonexistent", "--env", "PYTHONDONTWRITEBYTECODE=1",
    "--workdir", "/input/source",
    "--mount", `type=bind,src=${input},dst=/input/source,readonly`,
    "--mount", `type=bind,src=${probe},dst=/capsule/probe.py,readonly`,
    "--mount", `type=bind,src=${artifact},dst=/artifacts/0,readonly`,
    image, "python3", "/capsule/probe.py",
  ], { cwd: runtime, env: { PATH: process.env.PATH, HOME: runtime }, maxBuffer: 1024 * 1024 });
  if (stderr !== "") throw new Error("hostile verifier wrote to stderr");
  const verdict = JSON.parse(stdout);
  const fields = [
    "network_reachable", "root_writable", "input_writable", "artifact_writable",
    "capsule_writable", "host_home_visible",
  ];
  if (Object.keys(verdict).sort().join("\n") !== [...fields].sort().join("\n")) {
    throw new Error("hostile verifier returned an invalid verdict");
  }
  if (fields.some((field) => verdict[field] !== false)) {
    throw new Error("hostile verifier escaped its registered boundary");
  }
  process.stdout.write(`${JSON.stringify({
    ok: true,
    fixture: "hostile-verifier.v1",
    image,
    verdict,
  })}\n`);
} finally {
  await rm(runtime, { recursive: true, force: true });
}
