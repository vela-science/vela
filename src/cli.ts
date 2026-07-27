#!/usr/bin/env node

import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { mkdir, mkdtemp, readdir, rename, rm, stat, writeFile } from "node:fs/promises";

import { parseMission } from "./contracts/mission.js";
import { CodexExecEngine } from "./engines/codex-exec.js";
import { CodexToolsNativeEngine } from "./engines/codex-tools-native.js";
import { prepareMission, validateMissionBundle } from "./mission/prepare.js";
import { parseDiagnosticRunRecord, projectDiagnosticRun } from "./projection/diagnostic.js";
import { parseFailureRecord, projectFailure } from "./projection/failure.js";
import { parseRunRecord, projectRun } from "./projection/run.js";
import { projectPublicRun } from "./projection/public-run.js";
import { buildPublicationBundle } from "./projection/publication.js";
import { doctorProduct } from "./product/doctor.js";
import { replayProduct } from "./product/replay.js";
import { runProduct } from "./product/run.js";
import { CANOPUS_VERSION } from "./product/version.js";
import {
  listProductProfiles,
  packProductProfile,
  validateProductProfile,
} from "./product/profile-bundle.js";
import { loadProductProfile } from "./product/profile.js";
import { runCanopus } from "./run.js";
import { readBoundedRegularFile } from "./util/files.js";
import { canonicalJson, contentDigest } from "./util/canonical.js";
import { VelaClient } from "./vela/cli.js";

function usage(): string {
  return `Canopus — bounded Vela research harness

Primary workflow:
  canopus --version
  canopus doctor [frontier] [--profile <name>]
  canopus run [frontier] [--first | --target <id>] [--profile <name>] \\
    [--output <dir>] [--no-land]
  canopus inspect [run.json | latest]
  canopus public-run <run.json> --mission <mission.json> \
    --repository <public-url> --output <file>
  canopus publish-run <run.json> --mission <mission.json> \
    --repository <public-url> --output <new-directory>
  canopus replay <run.json>

Mission v1 prepare/validate remains available under advanced help.

Canopus may land a Receipt v1 as an agent after verifier success. It cannot
sign, accept, or make a human scientific decision.`;
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
    [--output <dir>] [--no-land]

Discovers and binds Vela, Codex, Git, Docker, the exact frontier roots, and the
registered verifier profile. --no-land runs the worker and verifier in disposable
clones and leaves the source frontier unchanged.`;
}

function inspectUsage(): string {
  return `Usage:
  canopus inspect [run.json | failure.json | latest]

Projects the newest completed or safely stopped non-authoritative run record
without mutating Vela. A failed run never implies that landing was unchanged;
inspect reports whether retained landing-recovery evidence is required.`;
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

function publicRunUsage(): string {
  return `Usage:
  canopus public-run <run.json> --mission <mission.json> \\
    --repository <public-url> --output <file>

Generates one canonical, sanitized canopus.public-run.v1 projection from a
submission-ready completed run. Raw worker logs, homes, credentials, private
paths, and unrestricted transcripts are never copied.`;
}

function publishRunUsage(): string {
  return `Usage:
  canopus publish-run <run.json> --mission <mission.json> \\
    --repository <public-url> --output <new-directory>

Creates a sanitized public projection, root manifest, proposal-scoped pending
commands, and a read-only Vela Observatory import descriptor. It never lands,
signs, accepts, pushes, or deploys.`;
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

function authHome(values: Map<string, string>): string {
  return path.resolve(
    values.get("--codex-home") ?? process.env.CODEX_HOME ?? path.join(os.homedir(), ".codex"),
  );
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

async function runMission(file: string, rest: string[]): Promise<void> {
  const values = options(rest, [
    "--source",
    "--run-root",
    "--vela",
    "--docker",
    "--codex",
    "--codex-version",
    "--codex-sha256",
    "--model",
    "--codex-home",
  ]);
  const mission = parseMission(await jsonFile(file));
  const sourceRepo = path.resolve(required(values, "--source"));
  const runRoot = path.resolve(required(values, "--run-root"));
  const velaBinary = path.resolve(required(values, "--vela"));
  const outputSchema = mission.schema === "canopus.mission.v1"
    ? path.join(path.dirname(path.resolve(file)), "contract", "engine-output.v0.json")
    : packagedOutputSchema();
  const vela = new VelaClient({
    binary: velaBinary,
    expectedVersion: mission.vela_version,
    expectedSha256: mission.vela_sha256,
    home: path.join(runRoot, "vela-home"),
  });
  const engine = mission.schema === "canopus.mission.v1"
    ? new CodexToolsNativeEngine({
        binary: path.resolve(required(values, "--codex")),
        authHome: authHome(values),
        outputSchema,
        permissionProfile: path.join(
          path.dirname(path.resolve(file)),
          mission.worker.permission_profile_path,
        ),
      })
    : new CodexExecEngine({
        binary: path.resolve(required(values, "--codex")),
        expectedSha256: required(values, "--codex-sha256"),
        expectedVersion: required(values, "--codex-version"),
        model: required(values, "--model"),
        authHome: authHome(values),
        outputSchema,
      });
  const result = await runCanopus({
    mission,
    sourceRepo,
    runRoot,
    vela,
    engine,
    bundleRoot: path.dirname(path.resolve(file)),
    dockerBinary: values.get("--docker") ?? "docker",
  });
  process.stdout.write(
    `${JSON.stringify({
      ok: true,
      command: "run",
      run_id: result.record.run_id,
      candidate_digest: result.record.candidate.digest,
      receipt_root: result.record.landing.receipt_root,
      route: result.record.landing.route,
      accepted_event_delta: result.record.landing.accepted_event_delta,
      clean_clone_reproduced: result.record.reproduction.matched,
      run_file: path.join(result.paths.root, "run.json"),
    })}\n`,
  );
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
    ["--first", "--no-land"],
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
    noLand: parsed.flags.has("--no-land"),
  });
  const landing = result.run.record.landing;
  process.stdout.write(`${JSON.stringify({
    ok: true,
    command: "run",
    mode: landing === null ? "no_land" : "land",
    run_id: result.run.record.run_id,
    target: result.run.record.mission.target,
    candidate_digest: result.run.record.candidate.digest,
    verifier_status: result.run.record.verifier.status,
    observed_tokens: result.run.record.budget.observed_tokens,
    receipt_root: landing?.receipt_root ?? null,
    proposal_id: landing?.proposal_id ?? null,
    route: landing?.route ?? null,
    accepted_event_delta: landing?.accepted_event_delta ?? null,
    clean_clone_reproduced: result.run.record.reproduction.matched,
    evidence_root: result.evidence_root,
    source_state: result.source_state,
    landing_candidate: result.landing_candidate,
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

async function inspectCommand(value: string | undefined, rest: string[]): Promise<void> {
  if (rest.length !== 0) throw new Error("inspect accepts at most one run file");
  const file = value === undefined || value === "latest"
    ? await latestInspectionFile()
    : path.resolve(value);
  const raw = await jsonFile(file);
  const schema = typeof raw === "object" && raw !== null && !Array.isArray(raw)
    ? (raw as Record<string, unknown>).schema
    : undefined;
  const projection = schema === "canopus.diagnostic-run.v1"
    ? projectDiagnosticRun(parseDiagnosticRunRecord(raw))
    : schema === "canopus.failure.v0"
      ? projectFailure(parseFailureRecord(raw))
      : projectRun(parseRunRecord(raw));
  process.stdout.write(`${JSON.stringify({ ok: true, command: "inspect", run_file: file, projection })}\n`);
}

async function replayCommand(file: string | undefined, rest: string[]): Promise<void> {
  if (file === undefined || rest.length !== 0) throw new Error("replay requires exactly one run file");
  process.stdout.write(`${JSON.stringify(await replayProduct(path.resolve(file)))}\n`);
}

async function publicRunCommand(file: string | undefined, rest: string[]): Promise<void> {
  if (file === undefined) throw new Error("public-run requires one completed run file");
  const values = options(rest, ["--mission", "--repository", "--output"]);
  const record = parseRunRecord(await jsonFile(file));
  const mission = parseMission(await jsonFile(required(values, "--mission")));
  const projection = projectPublicRun({
    record,
    mission,
    repository: required(values, "--repository"),
  });
  const output = path.resolve(required(values, "--output"));
  await writeFile(output, canonicalJson(projection), { flag: "wx", mode: 0o644 });
  process.stdout.write(`${JSON.stringify({
    ok: true,
    command: "public-run",
    run_id: projection.run_id,
    output,
    projection_root: contentDigest(projection),
  })}\n`);
}

async function publishRunCommand(file: string | undefined, rest: string[]): Promise<void> {
  if (file === undefined) throw new Error("publish-run requires one completed run file");
  const values = options(rest, ["--mission", "--repository", "--output"]);
  const record = parseRunRecord(await jsonFile(file));
  const mission = parseMission(await jsonFile(required(values, "--mission")));
  const bundle = buildPublicationBundle({
    record,
    mission,
    repository: required(values, "--repository"),
  });
  const output = path.resolve(required(values, "--output"));
  const parent = path.dirname(output);
  await mkdir(parent, { recursive: true, mode: 0o755 });
  const temporary = await mkdtemp(path.join(parent, `.${path.basename(output)}.tmp-`));
  try {
    await Promise.all([
      writeFile(path.join(temporary, "public-run.json"), canonicalJson(bundle.projection), { flag: "wx", mode: 0o644 }),
      writeFile(path.join(temporary, "pending-commands.json"), canonicalJson(bundle.pendingCommands), { flag: "wx", mode: 0o644 }),
      writeFile(path.join(temporary, "web-import.json"), canonicalJson(bundle.webImport), { flag: "wx", mode: 0o644 }),
      writeFile(path.join(temporary, "root-manifest.json"), canonicalJson(bundle.manifest), { flag: "wx", mode: 0o644 }),
    ]);
    await rename(temporary, output);
  } catch (error) {
    await rm(temporary, { recursive: true, force: true });
    throw error;
  }
  process.stdout.write(`${JSON.stringify({
    ok: true,
    command: "publish-run",
    run_id: bundle.projection.run_id,
    output,
    projection_root: contentDigest(bundle.projection),
    bundle_root: contentDigest(bundle.manifest),
    authority_effect: "none",
  })}\n`);
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
      : command === "inspect" ? inspectUsage()
      : command === "doctor" ? doctorUsage()
      : command === "replay" ? replayUsage()
      : command === "public-run" ? publicRunUsage()
      : command === "publish-run" ? publishRunUsage()
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
  if (command === "public-run") {
    await publicRunCommand(file, rest);
    return;
  }
  if (command === "publish-run") {
    await publishRunCommand(file, rest);
    return;
  }
  if (command === "validate") {
    if (file === undefined) throw new Error("validate requires a mission file");
    await missionCommand(["validate", file, ...rest]);
    return;
  }
  if (command === "inspect") {
    await inspectCommand(file, rest);
    return;
  }
  if (command === "run") {
    if (file !== undefined && (file.endsWith(".json") || rest.includes("--source"))) {
      await runMission(file, rest);
    } else {
      await productRunCommand(argv.slice(1));
    }
    return;
  }
  throw new Error(`unknown command ${command}`);
}

main(process.argv.slice(2)).catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${JSON.stringify({ ok: false, error: message })}\n`);
  process.exitCode = 1;
});
