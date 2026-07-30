import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import test from "node:test";
import { CANOPUS_VERSION } from "../src/product/version.js";

const execute = promisify(execFile);
const cli = fileURLToPath(new URL("../src/cli.js", import.meta.url));

async function help(...args: string[]): Promise<string> {
  const result = await execute(process.execPath, [cli, ...args], {
    encoding: "utf8",
    timeout: 30_000,
    maxBuffer: 1024 * 1024,
  });
  assert.equal(result.stderr, "");
  return result.stdout;
}

test("primary help presents only the compact product workflow", async () => {
  const output = await help("--help");
  for (const command of ["doctor", "run", "show", "replay", "export", "submit"]) {
    assert.match(output, new RegExp(`canopus ${command}\\b`, "u"));
  }
  assert.doesNotMatch(output, /canopus inspect\b/u);
  assert.doesNotMatch(output, /--no-land/u);
  assert.doesNotMatch(output, /canopus (?:public-run|publish-run)\b/u);
  assert.doesNotMatch(output, /canopus withdraw\b/u);
  assert.doesNotMatch(output, /^\s*canopus (?:benchmark|benchmark-composition|validate)\b/mu);
  assert.match(output, /Mission v1 prepare\/validate.+advanced help/su);
  assert.match(output, /cannot verify, decide, or change Standing/su);
});

test("version is a stable single-line product identity", async () => {
  assert.equal(await help("--version"), `canopus ${CANOPUS_VERSION}\n`);
  assert.equal(await help("-V"), `canopus ${CANOPUS_VERSION}\n`);
});

test("every compact product subcommand has focused help", async () => {
  for (const command of ["doctor", "run", "show", "replay", "export", "submit"]) {
    const output = await help(command, "--help");
    assert.match(output, new RegExp(`canopus ${command}\\b`, "u"));
    assert.doesNotMatch(output, /Primary workflow:/u);
  }
});

test("mission help retains the advanced portable interface", async () => {
  const output = await help("mission", "--help");
  assert.match(output, /canopus mission prepare/u);
  assert.match(output, /canopus mission validate/u);
});

test("profile help and validation retain the advanced closed interface", async () => {
  const output = await help("profile", "--help");
  assert.match(output, /canopus profile list/u);
  assert.match(output, /canopus profile validate/u);
  assert.match(output, /canopus profile pack/u);

  const list = JSON.parse(await help("profile", "list")) as { profiles: string[] };
  assert.deepEqual(list.profiles, [
    "erdos1056-k15-10429401-10429600",
    "formal-erdos-505-test-dim-one",
  ]);
  const validation = JSON.parse(
    await help("profile", "validate", "erdos1056-k15-10429401-10429600"),
  ) as { validation: { schema: string } };
  assert.equal(validation.validation.schema, "canopus.profile-validation.v1");
});

test("show latest reports the newest safely stopped run", async () => {
  const home = await mkdtemp(path.join(os.tmpdir(), "canopus-show-home-"));
  const run = path.join(home, ".canopus", "runs", "formal", "latest", "run");
  await mkdir(run, { recursive: true });
  const failure = {
    schema: "canopus.failure.v0",
    run_id: "run_stopped",
    error: "worker returned null; verifier and landing were not run",
    phase: "engine_non_success",
    landing_observed: false,
    landing_recovery: null,
    activity_tip: `sha256:${"a".repeat(64)}`,
    authority: "non_authoritative",
  };
  await writeFile(path.join(run, "failure.json"), `${JSON.stringify(failure)}\n`);
  const result = await execute(process.execPath, [cli, "show", "latest"], {
    encoding: "utf8",
    timeout: 30_000,
    maxBuffer: 1024 * 1024,
    env: { ...process.env, HOME: home, USERPROFILE: home },
  });
  assert.equal(result.stderr, "");
  const output = JSON.parse(result.stdout) as {
    run_file: string;
    projection: { schema: string; run_id: string; effect: string };
    withdrawal?: unknown;
  };
  assert.equal(output.run_file, path.join(run, "failure.json"));
  assert.equal(output.projection.schema, "canopus.failure-projection.v1");
  assert.equal(output.projection.run_id, "run_stopped");
  assert.equal(output.projection.effect, "none");
  assert.equal("withdrawal" in output, false);
});
