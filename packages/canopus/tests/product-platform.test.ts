import assert from "node:assert/strict";
import { chmod, mkdtemp, realpath, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  executableNames,
  findExecutable,
  runtimeIdentity,
  runtimeLocator,
} from "../src/product/runtime.js";
import { assertToolUsingMissionPlatform } from "../src/agent/platform.js";
import { sha256Bytes } from "../src/util/canonical.js";
import type { CommandOptions, CommandResult } from "../src/util/command.js";

test("native Windows tool missions fail before work with an exact WSL2 handoff", () => {
  assert.throws(
    () => assertToolUsingMissionPlatform("win32"),
    /require WSL2 on Windows.+rerun the same vela agent command/su,
  );
  assert.doesNotThrow(() => assertToolUsingMissionPlatform("darwin"));
  assert.doesNotThrow(() => assertToolUsingMissionPlatform("linux"));
});

test("Windows executable candidates honor PATHEXT", () => {
  assert.deepEqual(
    executableNames("vela", "win32", ".EXE;.CMD"),
    ["vela", "vela.exe", "vela.cmd"],
  );
  assert.deepEqual(executableNames("vela.exe", "win32", ".EXE;.CMD"), ["vela.exe"]);
  assert.deepEqual(executableNames("vela", "linux", ".EXE;.CMD"), ["vela"]);
});

test("Vela Agent binds Canopus to the invoking Vela binary", () => {
  assert.equal(
    runtimeLocator("vela", { VELA_BIN: "/opt/vela/bin/vela" }),
    "/opt/vela/bin/vela",
  );
  assert.equal(runtimeLocator("git", { VELA_BIN: "/opt/vela/bin/vela" }), "git");
  assert.equal(runtimeLocator("vela", {}), "vela");
  assert.throws(
    () => runtimeLocator("vela", { VELA_BIN: "relative/vela" }),
    /VELA_BIN must be an absolute path/u,
  );
});

test("runtime discovery records exact observed bytes without a global patch matrix", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-runtime-identity-"));
  const binary = path.join(root, "agent-runtime");
  const bytes = Buffer.from("synthetic-runtime\n");
  await writeFile(binary, bytes, { mode: 0o700 });
  await chmod(binary, 0o700);

  const identity = await runtimeIdentity({
    name: binary,
    cwd: root,
    home: root,
    runner: async (options) => commandResult(options, "agent-runtime 99.17.3\n"),
  });
  assert.equal(identity.version, "agent-runtime 99.17.3");
  assert.equal(identity.sha256, sha256Bytes(bytes));
  assert.equal(identity.binary, await realpath(binary));
});

test("active Windows executable discovery resolves a PATHEXT command", {
  skip: process.platform !== "win32",
}, async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-pathext-"));
  const executable = path.join(root, "vela.cmd");
  await writeFile(executable, "@echo off\r\n", { mode: 0o700 });
  await chmod(executable, 0o700);
  assert.equal(
    (await findExecutable("vela", root, { platform: "win32", pathExt: ".EXE;.CMD" })).toLowerCase(),
    (await realpath(executable)).toLowerCase(),
  );
});

function commandResult(
  options: CommandOptions,
  stdout: string,
  exitCode = 0,
): CommandResult {
  return {
    argv: [...options.argv],
    exitCode,
    signal: null,
    stdout: Buffer.from(stdout),
    stderr: Buffer.alloc(0),
    durationMs: 1,
  };
}
