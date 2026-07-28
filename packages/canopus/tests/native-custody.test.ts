import assert from "node:assert/strict";
import { mkdtemp, readFile, realpath, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import test from "node:test";

import { runNativeCustodyPreflight } from "../src/product/custody.js";
import { sha256Bytes } from "../src/util/canonical.js";
import type { CommandRunner } from "../src/util/command.js";
import { MAX_EXECUTABLE_BYTES } from "../src/util/files.js";

test("native custody preflight reuses the production boundary with generated canaries", async () => {
  if (process.platform !== "darwin" && process.platform !== "linux") return;
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-custody-unit-"));
  const binary = path.join(root, "codex");
  const profile = path.join(root, "config.toml");
  const binaryBytes = Buffer.from("exact-codex-runtime\n");
  const profileBytes = Buffer.from([
    'default_permissions = "canopus-worker"',
    '[permissions.canopus-worker.filesystem]',
    '":minimal" = "read"',
    '[permissions.canopus-worker.filesystem.":workspace_roots"]',
    '"." = "write"',
    '".canopus-runtime" = "read"',
    '[permissions.canopus-worker.network]',
    'enabled = false',
    '',
  ].join("\n"));
  await Promise.all([
    writeFile(binary, binaryBytes, { mode: 0o700 }),
    writeFile(profile, profileBytes, { mode: 0o600 }),
  ]);
  const calls: string[][] = [];
  const runner: CommandRunner = async (options) => {
    calls.push([...options.argv]);
    assert.deepEqual(await readFile(options.argv[0] as string), binaryBytes);
    if (options.argv[1] === "sandbox") {
      return {
        argv: [...options.argv],
        exitCode: 0,
        signal: null,
        stdout: Buffer.from("true false false false false false false false false false\n"),
        stderr: Buffer.alloc(0),
        durationMs: 1,
      };
    }
    assert.equal(options.argv[1], "--version");
    return {
      argv: [...options.argv],
      exitCode: 0,
      signal: null,
      stdout: Buffer.from("codex-cli 0.144.6\n"),
      stderr: Buffer.alloc(0),
      durationMs: 1,
    };
  };

  const result = await runNativeCustodyPreflight({ binary, permissionProfile: profile, runner });
  assert.equal(result.mode, "deterministic_no_model");
  assert.equal(result.codex_sha256, sha256Bytes(binaryBytes));
  assert.equal(result.permission_profile_sha256, sha256Bytes(profileBytes));
  assert.equal(result.verdict.outside_workspace_writable, false);
  assert.equal(result.verdict.command_network_reachable, false);
  assert.equal(calls.length, 2);
  if (process.platform === "linux") {
    assert.match(calls[0]?.[0] ?? "", /\/workspace\/\.canopus-runtime\/codex$/u);
  } else {
    assert.equal(calls[0]?.[0], await realpath(binary));
  }
});

test("native custody executable ceiling covers current large Codex distributions", () => {
  assert.ok(MAX_EXECUTABLE_BYTES > 268_435_456);
});
