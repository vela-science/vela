#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import { spawnSync } from "node:child_process";

function fail(message) {
  console.error(message);
  process.exit(1);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    stdio: options.capture ? "pipe" : "inherit",
  });
  if (result.status !== 0) {
    if (options.allowFailure) {
      return null;
    }
    fail(`${command} ${args.join(" ")} exited ${result.status ?? "without status"}`);
  }
  return options.capture ? result.stdout.trim() : "";
}

const [mode = "check", archiveInput] = process.argv.slice(2);
if (!["check", "--execute"].includes(mode) || !archiveInput) {
  fail(
    "usage: node .github/release/publish-protocol.mjs [check|--execute] <protocol.tgz>",
  );
}

const archive = resolve(archiveInput);
const packageJson = JSON.parse(
  readFileSync(resolve("packages/protocol/package.json"), "utf8"),
);
if (packageJson.name !== "@vela-science/protocol") {
  fail(`unexpected Protocol package name ${packageJson.name}`);
}
if (packageJson.private !== false) {
  fail("Protocol package must remain explicitly public");
}
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(packageJson.version)) {
  fail(`invalid Protocol package version ${packageJson.version}`);
}

const expectedArchive = `vela-science-protocol-${packageJson.version}.tgz`;
if (basename(archive) !== expectedArchive) {
  fail(`expected ${expectedArchive}, received ${basename(archive)}`);
}

const bytes = readFileSync(archive);
const integrity = `sha512-${createHash("sha512").update(bytes).digest("base64")}`;
const spec = `${packageJson.name}@${packageJson.version}`;
const publishedIntegrity = run(
  "npm",
  ["view", spec, "dist.integrity", "--json"],
  { allowFailure: true, capture: true },
);

if (publishedIntegrity !== null) {
  let observed;
  try {
    observed = JSON.parse(publishedIntegrity);
  } catch {
    fail(`npm returned malformed integrity for ${spec}`);
  }
  if (observed !== integrity) {
    fail(
      `${spec} already exists with different immutable bytes; bump the Protocol package version`,
    );
  }
  console.log(`${spec} already exists with matching immutable bytes`);
  process.exit(0);
}

if (mode === "check") {
  console.log(`${spec} is publishable from ${basename(archive)} (${integrity})`);
  process.exit(0);
}

run("npm", ["publish", archive, "--provenance", "--access", "public"]);
const observed = JSON.parse(
  run("npm", ["view", spec, "dist.integrity", "--json"], { capture: true }),
);
if (observed !== integrity) {
  fail(`${spec} registry integrity does not match the uploaded archive`);
}
console.log(`${spec} published with matching immutable bytes`);
