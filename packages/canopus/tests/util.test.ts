import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import test from "node:test";

import { canonicalJson, contentDigest } from "../src/util/canonical.js";
import { CommandFailure, isolatedEnvironment, runCommand } from "../src/util/command.js";

test("canonical JSON sorts object keys and has a stable digest", () => {
  assert.equal(canonicalJson({ z: 1, a: { d: true, c: null } }), '{"a":{"c":null,"d":true},"z":1}\n');
  assert.equal(contentDigest({ b: 2, a: 1 }), contentDigest({ a: 1, b: 2 }));
});

test("canonical JSON rejects undefined and non-finite values", () => {
  assert.throws(() => canonicalJson({ value: undefined }), /is undefined/u);
  assert.throws(() => canonicalJson({ value: Number.NaN }), /non-finite/u);
});

test("isolated environment pins TLS to the system PEM bundle", () => {
  const environment = isolatedEnvironment("/tmp/canopus-home");
  assert.equal(environment.SSL_CERT_FILE, "/etc/ssl/cert.pem");
  assert.equal(environment.SSL_CERT_DIR, undefined);
});

test("isolated Windows environment uses native null and custody paths", () => {
  const environment = isolatedEnvironment("C:\\Canopus\\home", "win32");
  assert.equal(environment.HOME, "C:\\Canopus\\home");
  assert.equal(environment.USERPROFILE, "C:\\Canopus\\home");
  assert.equal(environment.GIT_CONFIG_GLOBAL, "NUL");
  assert.equal(environment.SSL_CERT_FILE, undefined);
  assert.equal(environment.VELA_NO_KEY_ACCESS, "1");
});

test("command runner does not use a shell", async () => {
  const cwd = await mkdtemp(path.join(os.tmpdir(), "canopus-command-"));
  const result = await runCommand({
    argv: [process.execPath, "-e", "process.stdout.write(process.argv[1])", "$(uname)"],
    cwd,
    env: isolatedEnvironment(cwd),
    timeoutMs: 1000,
    maxOutputBytes: 1024,
  });
  assert.equal(result.exitCode, 0);
  assert.equal(result.stdout.toString("utf8"), "$(uname)");
});

test("command runner fails closed on output flood", async () => {
  const cwd = await mkdtemp(path.join(os.tmpdir(), "canopus-output-"));
  await assert.rejects(
    runCommand({
      argv: [process.execPath, "-e", "process.stdout.write('x'.repeat(4096))"],
      cwd,
      env: isolatedEnvironment(cwd),
      timeoutMs: 1000,
      maxOutputBytes: 256,
    }),
    (error: unknown) => error instanceof CommandFailure && error.kind === "output_limit",
  );
});

test("command runner terminates a timed-out process tree", async () => {
  const cwd = await mkdtemp(path.join(os.tmpdir(), "canopus-timeout-"));
  await assert.rejects(
    runCommand({
      argv: [process.execPath, "-e", "setInterval(() => {}, 1000)"],
      cwd,
      env: isolatedEnvironment(cwd),
      timeoutMs: 50,
      maxOutputBytes: 1024,
    }),
    (error: unknown) => error instanceof CommandFailure && error.kind === "timeout",
  );
});

test("command runner kills an orphaned descendant after a successful parent exit", async () => {
  const cwd = await mkdtemp(path.join(os.tmpdir(), "canopus-orphan-"));
  const parent = [
    "const {spawn}=require('node:child_process');",
    "const child=spawn(process.execPath,['-e',\"process.on('SIGTERM',()=>{});setInterval(()=>{},1000)\"],{stdio:'ignore'});child.unref();",
    "process.stdout.write(String(child.pid));",
  ].join("");
  const result = await runCommand({
    argv: [process.execPath, "-e", parent],
    cwd,
    env: isolatedEnvironment(cwd),
    timeoutMs: 1000,
    maxOutputBytes: 1024,
  });
  assert.equal(result.exitCode, 0);
  const pid = Number(result.stdout.toString("utf8"));
  assert.equal(Number.isSafeInteger(pid), true);
  let alive = true;
  for (let attempt = 0; attempt < 20; attempt += 1) {
    try {
      process.kill(pid, 0);
      await new Promise((resolve) => setTimeout(resolve, 25));
    } catch {
      alive = false;
      break;
    }
  }
  if (alive) process.kill(pid, "SIGKILL");
  assert.equal(alive, false);
});
