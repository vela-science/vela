import { randomBytes } from "node:crypto";
import { constants } from "node:fs";
import { chmod, copyFile, mkdir, mkdtemp, realpath, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";

import { assertNativeRuntimeProfile } from "../engines/codex-tools-native.js";
import { sha256Bytes } from "../util/canonical.js";
import { isolatedEnvironment, runCommand, type CommandRunner } from "../util/command.js";
import { MAX_EXECUTABLE_BYTES, readBoundedRegularFile, sha256RegularFile } from "../util/files.js";

export interface NativeCustodyPreflightResult {
  schema: "canopus.native-custody-preflight.v1";
  ok: true;
  mode: "deterministic_no_model";
  platform: string;
  codex_version: string;
  codex_sha256: string;
  permission_profile_sha256: string;
  verdict: {
    curl_available: true;
    source_auth_readable: false;
    runtime_auth_readable: false;
    sealed_input_readable: false;
    unrelated_repo_readable: false;
    canary_readable: false;
    outside_workspace_writable: false;
    command_network_reachable: false;
    environment_contains_auth: false;
    proc_environ_contains_auth: false;
  };
}

export async function runNativeCustodyPreflight(options: {
  binary: string;
  permissionProfile: string;
  runner?: CommandRunner;
}): Promise<NativeCustodyPreflightResult> {
  if (process.platform !== "darwin" && process.platform !== "linux") {
    throw new Error(`native sandbox preflight does not support ${process.platform}`);
  }
  const runner = options.runner ?? runCommand;
  const binary = await realpath(options.binary);
  const profile = await realpath(options.permissionProfile);
  const [binaryDigest, profileBytes] = await Promise.all([
    sha256RegularFile(binary, MAX_EXECUTABLE_BYTES),
    readBoundedRegularFile(profile, 8 * 1024 * 1024),
  ]);
  const runtime = await mkdtemp(path.join(os.homedir(), ".canopus-native-preflight-"));

  try {
    const codexHome = path.join(runtime, "codex-home");
    const home = path.join(runtime, "home");
    const workspace = path.join(runtime, "workspace");
    const sourceHome = path.join(runtime, "source-auth");
    const sealedInput = path.join(runtime, "sealed-input");
    const unrelated = path.join(runtime, "unrelated-repository");
    await Promise.all([
      mkdir(codexHome, { mode: 0o700 }),
      mkdir(home, { mode: 0o700 }),
      mkdir(workspace, { mode: 0o700 }),
      mkdir(sourceHome, { mode: 0o700 }),
      mkdir(sealedInput, { mode: 0o700 }),
      mkdir(unrelated, { mode: 0o700 }),
    ]);

    const authBytes = Buffer.from(JSON.stringify({
      OPENAI_API_KEY: `preflight-${randomBytes(32).toString("hex")}`,
    }) + "\n");
    const sourceAuth = path.join(sourceHome, "auth.json");
    const runtimeAuth = path.join(codexHome, "auth.json");
    const inputFile = path.join(sealedInput, "packet.json");
    const unrelatedFile = path.join(unrelated, "private.txt");
    const canary = path.join(runtime, "host-secret");
    await Promise.all([
      writeFile(sourceAuth, authBytes, { flag: "wx", mode: 0o600 }),
      writeFile(runtimeAuth, authBytes, { flag: "wx", mode: 0o600 }),
      writeFile(inputFile, "sealed input\n", { flag: "wx", mode: 0o400 }),
      writeFile(unrelatedFile, "unrelated repository\n", { flag: "wx", mode: 0o400 }),
      writeFile(canary, `host-secret-${randomBytes(32).toString("hex")}\n`, {
        flag: "wx",
        mode: 0o400,
      }),
    ]);
    await copyFile(profile, path.join(codexHome, "config.toml"));
    await chmod(path.join(codexHome, "config.toml"), 0o600);

    let runtimeBinary = binary;
    if (process.platform === "linux") {
      const runtimeDirectory = path.join(workspace, ".canopus-runtime");
      await mkdir(runtimeDirectory, { mode: 0o700 });
      runtimeBinary = path.join(runtimeDirectory, "codex");
      await copyFile(binary, runtimeBinary, constants.COPYFILE_EXCL);
      await chmod(runtimeBinary, 0o500);
      if (await sha256RegularFile(runtimeBinary, MAX_EXECUTABLE_BYTES) !== binaryDigest) {
        throw new Error("Codex binary changed while it was staged for the Linux sandbox");
      }
    }

    const environment = {
      ...isolatedEnvironment(home),
      CODEX_HOME: codexHome,
      CANOPUS_AUTH: "canopus-preflight-environment-canary",
      NO_COLOR: "1",
    };
    await assertNativeRuntimeProfile({
      binary: runtimeBinary,
      runner,
      environment,
      cwd: workspace,
      sourceAuth,
      runtimeAuth,
      inaccessibleInput: inputFile,
      unrelatedFile,
      canary,
      outsideWrite: path.join(runtime, "outside-write"),
      timeoutMs: 30_000,
      includeSafeDiagnostics: true,
    });
    const version = await runner({
      argv: [runtimeBinary, "--version"],
      cwd: workspace,
      env: environment,
      timeoutMs: 30_000,
      maxOutputBytes: 4096,
    });
    if (version.exitCode !== 0 || version.stderr.length !== 0) {
      throw new Error("Codex version check failed after native sandbox preflight");
    }
    const codexVersion = version.stdout.toString("utf8").trim();
    if (codexVersion === "" || codexVersion.length > 4096) {
      throw new Error("Codex returned an invalid version after native sandbox preflight");
    }
    if (await sha256RegularFile(runtimeBinary, MAX_EXECUTABLE_BYTES) !== binaryDigest) {
      throw new Error("Codex binary changed during native sandbox preflight");
    }
    return {
      schema: "canopus.native-custody-preflight.v1",
      ok: true,
      mode: "deterministic_no_model",
      platform: `${process.platform}-${process.arch}`,
      codex_version: codexVersion,
      codex_sha256: binaryDigest,
      permission_profile_sha256: sha256Bytes(profileBytes),
      verdict: {
        curl_available: true,
        source_auth_readable: false,
        runtime_auth_readable: false,
        sealed_input_readable: false,
        unrelated_repo_readable: false,
        canary_readable: false,
        outside_workspace_writable: false,
        command_network_reachable: false,
        environment_contains_auth: false,
        proc_environ_contains_auth: false,
      },
    };
  } finally {
    await rm(runtime, { recursive: true, force: true });
  }
}
