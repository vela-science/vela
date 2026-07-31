import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, realpath, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { doctorProduct } from "../src/product/doctor.js";
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

test("native Windows doctor remains read-only and does not probe worker runtimes", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-windows-doctor-"));
  const bin = path.join(root, "bin");
  const frontier = path.join(root, "frontier");
  await mkdir(bin);
  await mkdir(frontier);
  for (const name of ["vela", "git"]) {
    const executable = path.join(bin, name);
    await writeFile(executable, `${name}\n`, { mode: 0o700 });
    await chmod(executable, 0o700);
  }
  const previousPath = process.env.PATH;
  process.env.PATH = previousPath === undefined ? bin : `${bin}${path.delimiter}${previousPath}`;
  const observed: string[] = [];
  try {
    const result = await doctorProduct({
      frontier,
      platform: "win32",
      profileName: "erdos1056-k15-10429801-10430000",
      runner: async (options) => {
        const executable = path.basename(options.argv[0] ?? "");
        observed.push(`${executable} ${options.argv.slice(1).join(" ")}`);
        if (options.argv[1] === "--version") {
          return commandResult(
            options,
            executable === "vela"
              ? "vela 99.1.7\n"
              : "git version 2.50.0\n",
          );
        }
        if (executable === "vela" && options.argv[1] === "status") {
          return commandResult(options, JSON.stringify({
            ok: true,
            schema: "vela.status.v1",
            command: "status",
            roots: {
              repository: `sha256:${"b".repeat(64)}`,
            },
            git: { commit: "c".repeat(40), tree: "d".repeat(40) },
            integrity: { replay: "verified", strict: "pass", blocker_count: 0 },
          }));
        }
        if (executable === "vela" && options.argv[1] === "next") {
          return commandResult(options, JSON.stringify({
            schema: "vela.offer.v1",
            ok: true,
            command: "next",
            repository_root: `sha256:${"b".repeat(64)}`,
            targets: [{
              rank: 1,
              target_id: "erdos:1056",
              packet: {
                schema: "erdos-frontier.problem-work.v2",
                sha256: "sha256:c2b57075a0ec205b4d837382f8b816f31ab20a485e40962a1768e7ed42565344",
              },
            }],
          }));
        }
        if (executable === "git" && options.argv[1] === "status") {
          return commandResult(options, "");
        }
        throw new Error(`unexpected command: ${options.argv.join(" ")}`);
      },
    });
    assert.equal(result.public.ok, true);
    assert.equal(result.public.worker.mission_runtime, "wsl2_required");
    assert.equal(result.public.worker.mission_ready, false);
    assert.equal(
      result.public.frontier.repository_root,
      `sha256:${"b".repeat(64)}`,
    );
    assert.equal(result.public.frontier.strict_blockers, 0);
    assert.equal(result.public.runtimes.codex, null);
    assert.equal(result.public.runtimes.docker, null);
    assert.equal(result.public.runtimes.vela.version, "vela 99.1.7");
    assert.match(result.public.next_action, /Open WSL2.+rerun canopus doctor/su);
    assert.equal(observed.some((command) => /codex|docker/u.test(command)), false);
  } finally {
    if (previousPath === undefined) delete process.env.PATH;
    else process.env.PATH = previousPath;
  }
});
