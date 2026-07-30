import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, realpath, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { doctorProduct } from "../src/product/doctor.js";
import { executableNames, findExecutable } from "../src/product/runtime.js";
import { assertToolUsingMissionPlatform } from "../src/product/run.js";
import { SUPPORTED_VELA_VERSION } from "../src/product/version.js";
import type { CommandOptions, CommandResult } from "../src/util/command.js";

test("native Windows tool missions fail before work with an exact WSL2 handoff", () => {
  assert.throws(
    () => assertToolUsingMissionPlatform("win32"),
    /native Windows.+open WSL2.+rerun the same canopus command/su,
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
      profileName: "erdos1056-k15-10429601-10429800",
      runner: async (options) => {
        const executable = path.basename(options.argv[0] ?? "");
        observed.push(`${executable} ${options.argv.slice(1).join(" ")}`);
        if (options.argv[1] === "--version") {
          return commandResult(
            options,
            executable === "vela"
              ? `vela ${SUPPORTED_VELA_VERSION}\n`
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
                sha256: "sha256:8d879e24a537de3b9b13ad7878dc98db8ce4f5273187c7f45d0d49a93e8fe8ad",
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
    assert.match(result.public.next_action, /Open WSL2.+rerun canopus doctor/su);
    assert.equal(observed.some((command) => /codex|docker/u.test(command)), false);
  } finally {
    if (previousPath === undefined) delete process.env.PATH;
    else process.env.PATH = previousPath;
  }
});
