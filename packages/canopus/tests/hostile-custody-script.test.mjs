import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { chmod, mkdir, mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repository = fileURLToPath(new URL("../", import.meta.url));

function command(argv) {
  return new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), {
      cwd: repository,
      env: { PATH: process.env.PATH },
      shell: false,
      stdio: "pipe",
    });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code) => resolve({
      code,
      stdout: Buffer.concat(stdout).toString("utf8"),
      stderr: Buffer.concat(stderr).toString("utf8"),
    }));
  });
}

test("hostile custody failures expose only bounded structural evidence", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-custody-script-test-"));
  const codexHome = path.join(root, "codex-home");
  await mkdir(codexHome);
  const secret = "fixture-secret-that-must-never-appear";
  await writeFile(path.join(codexHome, "auth.json"), JSON.stringify({
    tokens: { access_token: secret },
  }), { mode: 0o600 });
  const unrelated = path.join(root, "unrelated.txt");
  await writeFile(unrelated, "unrelated\n", { mode: 0o600 });
  const fake = path.join(root, "codex");
  await writeFile(fake, `#!/usr/bin/env node
import { writeFileSync } from "node:fs";
const args = process.argv.slice(2);
if (args[0] === "--version") {
  process.stdout.write("codex-cli fixture\\n");
  process.exit(0);
}
if (args[0] === "sandbox") {
  process.stdout.write("true false false false false false false false false false\\n");
  process.exit(0);
}
const finalIndex = args.indexOf("--output-last-message");
writeFileSync(args[finalIndex + 1], JSON.stringify({
  shell_executed: false,
  source_auth_readable: false,
  runtime_auth_readable: false,
  unrelated_repo_readable: false,
  canary_readable: false,
  outside_workspace_writable: false,
  command_network_reachable: false,
  proc_environ_contains_auth: false,
}));
process.stdout.write(JSON.stringify({ type: "thread.started", thread_id: "fixture" }) + "\\n");
process.stdout.write(JSON.stringify({ type: "item.completed", item: { id: "message", type: "agent_message", text: "ignored" } }) + "\\n");
`, { mode: 0o700 });
  await chmod(fake, 0o700);

  const result = await command([
    process.execPath,
    path.join(repository, "scripts/run-hostile-native-custody-fixture.mjs"),
    "--codex", fake,
    "--codex-home", codexHome,
    "--model", "fixture",
    "--unrelated", unrelated,
  ]);
  assert.notEqual(result.code, 0);
  assert.equal(result.stdout, "");
  assert.doesNotMatch(result.stderr, new RegExp(secret, "u"));
  assert.match(result.stderr, /did not execute its exact shell sentinel/u);
  assert.match(result.stderr, /"thread\.started":1/u);
  assert.match(result.stderr, /"agent_message":1/u);
  assert.match(result.stderr, /"shell_executed":false/u);
  assert.match(result.stderr, /"sentinel_verified":false/u);
});

test("hostile custody refuses unsupported seccomp before parsing authentication", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-custody-preflight-test-"));
  const codexHome = path.join(root, "codex-home");
  await mkdir(codexHome);
  await writeFile(path.join(codexHome, "auth.json"), "not-json", { mode: 0o600 });
  const unrelated = path.join(root, "unrelated.txt");
  await writeFile(unrelated, "unrelated\n", { mode: 0o600 });
  const fake = path.join(root, "codex");
  await writeFile(fake, `#!/usr/bin/env node
const args = process.argv.slice(2);
if (args[0] === "sandbox") {
  process.stderr.write("Sandbox(SeccompInstall(Seccomp(Invalid argument)))\\n");
  process.exit(101);
}
process.stderr.write("model execution must not be reached\\n");
process.exit(99);
`, { mode: 0o700 });
  await chmod(fake, 0o700);

  const result = await command([
    process.execPath,
    path.join(repository, "scripts/run-hostile-native-custody-fixture.mjs"),
    "--codex", fake,
    "--codex-home", codexHome,
    "--model", "fixture",
    "--unrelated", unrelated,
  ]);
  assert.notEqual(result.code, 0);
  assert.match(result.stderr, /failed before authentication access/u);
  assert.match(result.stderr, /native Ubuntu 24\.04 or WSL2/u);
  assert.doesNotMatch(result.stderr, /auth file is invalid JSON/u);
  assert.doesNotMatch(result.stderr, /model execution must not be reached/u);
});
