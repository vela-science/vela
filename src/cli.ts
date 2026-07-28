#!/usr/bin/env node

import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { readdir, stat } from "node:fs/promises";

import { parseMission } from "./contracts/mission.js";
import { prepareMission, validateMissionBundle } from "./mission/prepare.js";
import { parseCurrentRunRecord, projectCurrentRun } from "./projection/current-run.js";
import { parseFailureRecord, projectFailure } from "./projection/failure.js";
import { doctorProduct } from "./product/doctor.js";
import { replayProduct } from "./product/replay.js";
import { runProduct } from "./product/run.js";
import { exportSubmission } from "./product/submission.js";
import { submitBundle } from "./product/submit.js";
import { CANOPUS_VERSION } from "./product/version.js";
import {
  listProductProfiles,
  packProductProfile,
  validateProductProfile,
} from "./product/profile-bundle.js";
import { loadProductProfile } from "./product/profile.js";
import { readBoundedRegularFile } from "./util/files.js";

function usage(): string {
  return `Canopus — bounded Vela research harness

Primary workflow:
  canopus doctor [frontier] [--profile <name>]
  canopus run [frontier] [--first | --target <id>] [--profile <name>] [--output <dir>]
  canopus show [run.json | failure.json | latest]
  canopus replay <run.json>
  canopus export <run.json | latest> [--output <new-directory>] [--as <agent:id>]
  canopus submit <submission-bundle> [frontier] [--vela <binary>]

Mission v1 prepare/validate remains available under advanced help.

Run and export never mutate Vela. submit explicitly registers an authenticated
Submission as pending review. Canopus cannot verify, decide, or change Standing.`;
}

function missionUsage(): string {
  return `Usage:
  canopus mission prepare <draft.json> --source <clean-repo> --output <new-bundle> \\
    --vela <binary> --codex <binary> --verifier-image <image> [--docker <binary>]
  canopus mission validate <bundle/mission.json>

prepare derives exact Git, Vela, packet, native permission-profile,
verifier-image, Codex, capsule, and strict-baseline roots. validate is read-only
and checks the closed contract and portable bundle bytes.`;
}

function profileUsage(): string {
  return `Usage:
  canopus profile list
  canopus profile show <name>
  canopus profile validate <name>
  canopus profile pack <name> --output <new-directory>

Profiles are closed, content-addressed producer contracts. Version 1 profiles
remain replay-only; new tool-using missions require a validated version 2 profile.`;
}

function runUsage(): string {
  return `Usage:
  canopus run [frontier] [--first | --target <id>] [--profile <name>] \\
    [--output <dir>]

Discovers and binds Vela, Codex, Git, Docker, the exact frontier roots, and the
registered verifier profile. Runs the worker and verifier in disposable clones
and always leaves the source frontier unchanged.`;
}

function showUsage(): string {
  return `Usage:
  canopus show [run.json | failure.json | latest]

Shows the newest completed or safely stopped non-authoritative Run without
mutating Vela. Historical formats remain readable through their exact Canopus
release; the current product accepts only Run v2 and failure records.`;
}

function doctorUsage(): string {
  return `Usage:
  canopus doctor [frontier] [--profile <name>]

Checks the compact Vela contract, exact runtimes, first offer, registered
profile, packaged capsule root, real native custody boundary, and verifier
isolation prerequisites.`;
}

function replayUsage(): string {
  return `Usage:
  canopus replay <run.json>

Re-runs the frozen verifier over the content-addressed candidate without a
model call, Vela mutation, network, or authority action.`;
}

function exportUsage(): string {
  return `Usage:
  canopus export <run.json | latest> [--output <new-directory>] [--as <agent:id>]

Creates a signed portable vela.submission.v1 bundle from a successful Run.
Export does not touch a frontier and does not create Verification or Standing.`;
}

function submitUsage(): string {
  return `Usage:
  canopus submit <submission-bundle> [frontier] [--vela <binary>]

Explicitly registers one authenticated Submission through Vela in a disposable
exact-head clone, then fast-forwards the clean source checkout. The result is
pending review with zero accepted-event delta.`;
}

function isHelp(value: string | undefined): boolean {
  return value === "--help" || value === "-h" || value === "help";
}

async function jsonFile(file: string): Promise<unknown> {
  const bytes = await readBoundedRegularFile(path.resolve(file), 8 * 1024 * 1024);
  return JSON.parse(bytes.toString("utf8")) as unknown;
}

function options(args: string[], allowed: readonly string[]): Map<string, string> {
  const result = new Map<string, string>();
  const allow = new Set(allowed);
  for (let index = 0; index < args.length; index += 2) {
    const key = args[index];
    const value = args[index + 1];
    if (key === undefined || value === undefined || !allow.has(key) || result.has(key)) {
      throw new Error(`invalid or duplicate option near ${key ?? "end of arguments"}`);
    }
    result.set(key, value);
  }
  return result;
}

function productOptions(
  args: string[],
  valueOptions: readonly string[],
  booleanOptions: readonly string[],
): { positional: string[]; values: Map<string, string>; flags: Set<string> } {
  const valueAllow = new Set(valueOptions);
  const flagAllow = new Set(booleanOptions);
  const positional: string[] = [];
  const values = new Map<string, string>();
  const flags = new Set<string>();
  for (let index = 0; index < args.length; index += 1) {
    const item = args[index] as string;
    if (!item.startsWith("--")) {
      positional.push(item);
      continue;
    }
    if (flagAllow.has(item)) {
      if (flags.has(item)) throw new Error(`duplicate option ${item}`);
      flags.add(item);
      continue;
    }
    if (!valueAllow.has(item) || values.has(item)) {
      throw new Error(`unknown or duplicate option ${item}`);
    }
    const value = args[index + 1];
    if (value === undefined || value.startsWith("--")) throw new Error(`${item} requires a value`);
    values.set(item, value);
    index += 1;
  }
  return { positional, values, flags };
}

function required(values: Map<string, string>, key: string): string {
  const value = values.get(key);
  if (value === undefined || value === "") throw new Error(`${key} is required`);
  return value;
}

function packagedOutputSchema(): string {
  return fileURLToPath(new URL("../../schemas/engine-output.v0.json", import.meta.url));
}

function packagedNativeWorkerProfile(): string {
  const file = process.platform === "linux" ? "config-linux.toml" : "config.toml";
  return fileURLToPath(new URL(`../../runtime/native-worker/${file}`, import.meta.url));
}

async function missionCommand(args: string[]): Promise<void> {
  const [subcommand, file, ...rest] = args;
  if (subcommand === undefined || isHelp(subcommand)) {
    process.stdout.write(`${missionUsage()}\n`);
    return;
  }
  if (isHelp(file)) {
    process.stdout.write(`${missionUsage()}\n`);
    return;
  }
  if (file === undefined) throw new Error(`mission ${subcommand} requires a JSON file`);

  if (subcommand === "validate") {
    if (rest.length !== 0) throw new Error("mission validate accepts only one mission file");
    const mission = parseMission(await jsonFile(file));
    if (mission.schema === "canopus.mission.v1") {
      await validateMissionBundle(mission, path.dirname(path.resolve(file)));
    }
    process.stdout.write(
      `${JSON.stringify({ ok: true, command: "mission validate", mission_id: mission.id, schema: mission.schema })}\n`,
    );
    return;
  }
  if (subcommand !== "prepare") throw new Error(`unknown mission command ${subcommand}`);
  const values = options(rest, [
    "--source",
    "--output",
    "--vela",
    "--codex",
    "--verifier-image",
    "--docker",
  ]);
  const prepared = await prepareMission({
    draft: await jsonFile(file),
    draftRoot: path.dirname(path.resolve(file)),
    sourceRepo: path.resolve(required(values, "--source")),
    outputRoot: path.resolve(required(values, "--output")),
    velaBinary: path.resolve(required(values, "--vela")),
    codexBinary: path.resolve(required(values, "--codex")),
    dockerBinary: values.get("--docker") ?? "docker",
    verifierImage: required(values, "--verifier-image"),
    outputSchema: packagedOutputSchema(),
    permissionProfile: packagedNativeWorkerProfile(),
  });
  process.stdout.write(
    `${JSON.stringify({
      ok: true,
      command: "mission prepare",
      mission_id: prepared.mission.id,
      target: prepared.mission.target,
      mission: prepared.missionPath,
      manifest: prepared.manifestPath,
    })}\n`,
  );
}

async function profileCommand(args: string[]): Promise<void> {
  const [subcommand, name, ...rest] = args;
  if (subcommand === undefined || isHelp(subcommand) || isHelp(name)) {
    process.stdout.write(`${profileUsage()}\n`);
    return;
  }
  if (subcommand === "list") {
    if (name !== undefined || rest.length !== 0) throw new Error("profile list accepts no arguments");
    process.stdout.write(`${JSON.stringify({
      ok: true,
      command: "profile list",
      profiles: await listProductProfiles(),
    })}\n`);
    return;
  }
  if (name === undefined) throw new Error(`profile ${subcommand} requires a profile name`);
  if (subcommand === "show") {
    if (rest.length !== 0) throw new Error("profile show accepts exactly one profile name");
    const profile = await loadProductProfile(name);
    process.stdout.write(`${JSON.stringify({ ok: true, command: "profile show", profile })}\n`);
    return;
  }
  if (subcommand === "validate") {
    if (rest.length !== 0) throw new Error("profile validate accepts exactly one profile name");
    process.stdout.write(`${JSON.stringify({
      ok: true,
      command: "profile validate",
      validation: await validateProductProfile(name),
    })}\n`);
    return;
  }
  if (subcommand === "pack") {
    const values = options(rest, ["--output"]);
    const result = await packProductProfile(name, path.resolve(required(values, "--output")));
    process.stdout.write(`${JSON.stringify({ ok: true, command: "profile pack", ...result })}\n`);
    return;
  }
  throw new Error(`unknown profile command ${subcommand}`);
}

async function doctorCommand(args: string[]): Promise<void> {
  const parsed = productOptions(args, ["--profile"], []);
  if (parsed.positional.length > 1) throw new Error("doctor accepts at most one frontier");
  const profileName = parsed.values.get("--profile");
  const result = await doctorProduct({
    frontier: path.resolve(parsed.positional[0] ?? "."),
    ...(profileName === undefined ? {} : { profileName }),
  });
  process.stdout.write(`${JSON.stringify(result.public)}\n`);
}

async function productRunCommand(args: string[]): Promise<void> {
  const parsed = productOptions(
    args,
    ["--target", "--profile", "--output", "--codex-home"],
    ["--first"],
  );
  if (parsed.positional.length > 1) throw new Error("run accepts at most one frontier");
  if (parsed.flags.has("--first") && parsed.values.has("--target")) {
    throw new Error("--first and --target are mutually exclusive");
  }
  const frontier = path.resolve(parsed.positional[0] ?? ".");
  const requested = parsed.values.get("--target");
  const profileName = parsed.values.get("--profile");
  const outputRoot = parsed.values.get("--output");
  const codexHome = parsed.values.get("--codex-home");
  const result = await runProduct({
    frontier,
    ...(profileName === undefined ? {} : { profileName }),
    ...(requested === undefined ? {} : { requestedTarget: requested }),
    ...(outputRoot === undefined ? {} : { outputRoot: path.resolve(outputRoot) }),
    ...(codexHome === undefined ? {} : { codexHome: path.resolve(codexHome) }),
  });
  process.stdout.write(`${JSON.stringify({
    ok: true,
    command: "run",
    effect: "none",
    run_id: result.run.record.run_id,
    target: result.run.record.mission.target,
    candidate_digest: result.run.record.candidate.digest,
    verifier_status: result.run.record.verifier.status,
    observed_tokens: result.run.record.budget.observed_tokens,
    submission: null,
    clean_clone_reproduced: result.run.record.reproduction.matched,
    evidence_root: result.evidence_root,
    source_state: result.source_state,
    run_file: path.join(result.run.paths.root, "run.json"),
  })}\n`);
}

async function latestRunFile(): Promise<string> {
  const root = path.join(os.homedir(), ".canopus", "runs");
  const entries = await readdir(root, { recursive: true });
  const candidates = entries
    .filter((entry) => entry.endsWith(`${path.sep}run${path.sep}run.json`) || entry === path.join("run", "run.json"))
    .map((entry) => path.join(root, entry));
  if (candidates.length === 0) throw new Error("no completed Canopus product run was found");
  const ranked = await Promise.all(candidates.map(async (file) => ({
    file,
    modified: (await stat(file)).mtimeMs,
  })));
  ranked.sort((left, right) => right.modified - left.modified || left.file.localeCompare(right.file));
  return ranked[0]?.file ?? (() => { throw new Error("no completed Canopus product run was found"); })();
}

async function latestInspectionFile(): Promise<string> {
  const root = path.join(os.homedir(), ".canopus", "runs");
  const entries = await readdir(root, { recursive: true });
  const candidates = entries
    .filter((entry) =>
      entry.endsWith(`${path.sep}run${path.sep}run.json`) ||
      entry === path.join("run", "run.json") ||
      entry.endsWith(`${path.sep}run${path.sep}failure.json`) ||
      entry === path.join("run", "failure.json"),
    )
    .map((entry) => path.join(root, entry));
  if (candidates.length === 0) throw new Error("no inspectable Canopus product run was found");
  const ranked = await Promise.all(candidates.map(async (file) => ({
    file,
    modified: (await stat(file)).mtimeMs,
  })));
  ranked.sort((left, right) => right.modified - left.modified || left.file.localeCompare(right.file));
  return ranked[0]?.file ?? (() => { throw new Error("no inspectable Canopus product run was found"); })();
}

async function showCommand(value: string | undefined, rest: string[]): Promise<void> {
  if (rest.length !== 0) throw new Error("show accepts at most one run file");
  const file = value === undefined || value === "latest"
    ? await latestInspectionFile()
    : path.resolve(value);
  const raw = await jsonFile(file);
  const schema = typeof raw === "object" && raw !== null && !Array.isArray(raw)
    ? (raw as Record<string, unknown>).schema
    : undefined;
  const projection = schema === "canopus.run.v2"
    ? projectCurrentRun(parseCurrentRunRecord(raw))
    : schema === "canopus.failure.v0" || schema === "canopus.failure.v1"
      ? projectFailure(parseFailureRecord(raw))
      : (() => {
          throw new Error(
            "current Canopus accepts only canopus.run.v2 and retained failure records; use the exact historical release for older Run schemas",
          );
        })();
  process.stdout.write(`${JSON.stringify({ ok: true, command: "show", run_file: file, projection })}\n`);
}

async function replayCommand(file: string | undefined, rest: string[]): Promise<void> {
  if (file === undefined || rest.length !== 0) throw new Error("replay requires exactly one run file");
  process.stdout.write(`${JSON.stringify(await replayProduct(path.resolve(file)))}\n`);
}

async function exportCommand(file: string | undefined, rest: string[]): Promise<void> {
  if (file === undefined) throw new Error("export requires a Run file or latest");
  const parsed = productOptions(rest, ["--output", "--as"], []);
  if (parsed.positional.length !== 0) throw new Error("export accepts one Run file");
  const runFile = file === "latest" ? await latestRunFile() : path.resolve(file);
  const output = parsed.values.get("--output") ??
    path.join(path.dirname(path.dirname(runFile)), `submission-${Date.now()}`);
  const actor = parsed.values.get("--as");
  process.stdout.write(`${JSON.stringify(await exportSubmission({
    runFile,
    outputRoot: path.resolve(output),
    ...(actor === undefined ? {} : { actor }),
  }))}\n`);
}

async function submitCommand(bundle: string | undefined, rest: string[]): Promise<void> {
  if (bundle === undefined) throw new Error("submit requires a Submission bundle");
  const parsed = productOptions(rest, ["--vela"], []);
  if (parsed.positional.length > 1) throw new Error("submit accepts at most one frontier");
  const velaBinary = parsed.values.get("--vela");
  process.stdout.write(`${JSON.stringify(await submitBundle({
    bundle: path.resolve(bundle),
    frontier: path.resolve(parsed.positional[0] ?? "."),
    ...(velaBinary === undefined ? {} : { velaBinary }),
  }))}\n`);
}

async function main(argv: string[]): Promise<void> {
  const [command, file, ...rest] = argv;
  if (command === undefined || isHelp(command)) {
    process.stdout.write(`${usage()}\n`);
    return;
  }
  if (command === "--version" || command === "-V") {
    if (file !== undefined) throw new Error("--version accepts no arguments");
    process.stdout.write(`canopus ${CANOPUS_VERSION}\n`);
    return;
  }
  if (command === "mission") {
    await missionCommand(argv.slice(1));
    return;
  }
  if (command === "profile") {
    await profileCommand(argv.slice(1));
    return;
  }
  if (isHelp(file)) {
    process.stdout.write(`${
      command === "run" ? runUsage()
      : command === "show" ? showUsage()
      : command === "doctor" ? doctorUsage()
      : command === "replay" ? replayUsage()
      : command === "export" ? exportUsage()
      : command === "submit" ? submitUsage()
      : usage()
    }\n`);
    return;
  }
  if (command === "doctor") {
    await doctorCommand(argv.slice(1));
    return;
  }
  if (command === "replay") {
    await replayCommand(file, rest);
    return;
  }
  if (command === "validate") {
    if (file === undefined) throw new Error("validate requires a mission file");
    await missionCommand(["validate", file, ...rest]);
    return;
  }
  if (command === "show") {
    await showCommand(file, rest);
    return;
  }
  if (command === "run") {
    await productRunCommand(argv.slice(1));
    return;
  }
  if (command === "export") {
    await exportCommand(file, rest);
    return;
  }
  if (command === "submit") {
    await submitCommand(file, rest);
    return;
  }
  throw new Error(`unknown command ${command}`);
}

main(process.argv.slice(2)).catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${JSON.stringify({ ok: false, error: message })}\n`);
  process.exitCode = 1;
});
