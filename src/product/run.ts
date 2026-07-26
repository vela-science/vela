import { lstat, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { CodexToolsNativeEngine } from "../engines/codex-tools-native.js";
import { prepareMission } from "../mission/prepare.js";
import {
  runCanopus,
  type CanopusDiagnosticRunResult,
  type CanopusRunResult,
} from "../run.js";
import { canonicalJson, contentDigest, sha256Bytes } from "../util/canonical.js";
import { isolatedEnvironment, runCommand, type CommandRunner } from "../util/command.js";
import { readBoundedRegularFile } from "../util/files.js";
import { VelaClient } from "../vela/cli.js";
import { doctorProduct, type ProductDoctorResult } from "./doctor.js";
import {
  loadProfileDraft,
  loadProfileResultContract,
  packagedWorkerProfile,
  stageProfileCapsule,
} from "./profile.js";

export interface ProductRunResult {
  run: CanopusRunResult | CanopusDiagnosticRunResult;
  doctor: ProductDoctorResult;
  output_root: string;
  bundle_root: string;
  evidence_manifest: string;
  evidence_root: string;
  source_publication:
    | { state: "committed_local"; commit: string; tree: string }
    | { state: "unchanged_no_land"; commit: string; tree: string };
}

function packageFile(relative: string): string {
  return fileURLToPath(new URL(`../../../${relative}`, import.meta.url));
}

async function assertFreshOutput(outputRoot: string, sourceRoot: string): Promise<void> {
  const output = path.resolve(outputRoot);
  const cloudBackedRoots = [
    path.join(os.homedir(), "Desktop"),
    path.join(os.homedir(), "Library", "Mobile Documents"),
    path.join(os.homedir(), "Library", "CloudStorage"),
  ];
  if (cloudBackedRoots.some((root) => output === root || output.startsWith(`${root}${path.sep}`))) {
    throw new Error(
      "Canopus output must not use a cloud-synced path because Docker verifier bind mounts can stall; use the default ~/.canopus store or another local directory",
    );
  }
  const relative = path.relative(sourceRoot, output);
  if (relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== "..")) {
    throw new Error("Canopus output must be outside the source frontier");
  }
  try {
    await lstat(output);
    throw new Error("Canopus output root already exists");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
  await mkdir(output, { recursive: true, mode: 0o700 });
}

async function gitText(options: {
  argv: readonly string[];
  cwd: string;
  home: string;
  runner: CommandRunner;
}): Promise<string> {
  const result = await options.runner({
    argv: ["git", ...options.argv],
    cwd: options.cwd,
    env: isolatedEnvironment(options.home),
    timeoutMs: 120_000,
    maxOutputBytes: 8 * 1024 * 1024,
  });
  if (result.exitCode !== 0 || result.stderr.length !== 0) {
    throw new Error(
      `git ${options.argv.join(" ")} failed: stdout_sha256=${sha256Bytes(result.stdout)}; ` +
      `stderr_sha256=${sha256Bytes(result.stderr)}`,
    );
  }
  return result.stdout.toString("utf8").trim();
}

async function publishVerifiedLanding(options: {
  source: string;
  landing: string;
  expectedStart: string;
  expectedFinal: string;
  expectedTree: string;
  home: string;
  runner: CommandRunner;
}): Promise<{ state: "committed_local"; commit: string; tree: string }> {
  const [head, status] = await Promise.all([
    gitText({ argv: ["rev-parse", "--verify", "HEAD^{commit}"], cwd: options.source, home: options.home, runner: options.runner }),
    gitText({ argv: ["status", "--porcelain=v1", "--untracked-files=all"], cwd: options.source, home: options.home, runner: options.runner }),
  ]);
  if (head !== options.expectedStart || status !== "") {
    throw new Error("source frontier changed while the bounded run was executing");
  }
  const landingHead = await gitText({
    argv: ["rev-parse", "--verify", "canopus-landing^{commit}"],
    cwd: options.landing,
    home: options.home,
    runner: options.runner,
  });
  if (landingHead !== options.expectedFinal) throw new Error("landing clone final commit drifted");
  await gitText({
    argv: ["fetch", "--quiet", "--no-tags", options.landing, "canopus-landing"],
    cwd: options.source,
    home: options.home,
    runner: options.runner,
  });
  await gitText({
    argv: ["merge", "--ff-only", "--no-edit", "FETCH_HEAD"],
    cwd: options.source,
    home: options.home,
    runner: options.runner,
  });
  const [commit, tree, finalStatus] = await Promise.all([
    gitText({ argv: ["rev-parse", "--verify", "HEAD^{commit}"], cwd: options.source, home: options.home, runner: options.runner }),
    gitText({ argv: ["rev-parse", "--verify", "HEAD^{tree}"], cwd: options.source, home: options.home, runner: options.runner }),
    gitText({ argv: ["status", "--porcelain=v1", "--untracked-files=all"], cwd: options.source, home: options.home, runner: options.runner }),
  ]);
  if (commit !== options.expectedFinal || tree !== options.expectedTree || finalStatus !== "") {
    throw new Error("source publication did not reproduce the verified landing roots");
  }
  return { state: "committed_local", commit, tree };
}

async function writeEvidenceManifest(
  run: CanopusRunResult | CanopusDiagnosticRunResult,
  missionDigest: string,
): Promise<{ file: string; root: string }> {
  const root = run.paths.root;
  const files = {
    activity: "activity.jsonl",
    transcript: "worker-final.json",
    tool_trace: "worker-events.jsonl",
    worker_stderr: "worker-stderr.bin",
    engine_result: "engine-result.json",
    candidate: "candidate.json",
    run: "run.json",
  } as const;
  const digests: Record<string, string> = {};
  for (const [name, relative] of Object.entries(files)) {
    digests[name] = sha256Bytes(await readBoundedRegularFile(path.join(root, relative), 64 * 1024 * 1024));
  }
  const manifest = {
    schema: "canopus.run-evidence.v1",
    authority: "non_authoritative",
    mission_root: missionDigest,
    run_id: run.record.run_id,
    target: run.record.mission.target,
    files: digests,
    artifact_roots: run.record.candidate.artifacts.map((artifact) => artifact.digest).sort(),
    verifier_root: contentDigest(run.record.verifier),
    receipt_root: run.record.landing?.receipt_root ?? null,
    final_roots: "final_roots" in run.record
      ? run.record.final_roots
      : run.record.mission.starting_roots,
  };
  const file = path.join(root, "evidence-manifest.json");
  await writeFile(file, canonicalJson(manifest), { flag: "wx", mode: 0o600 });
  return { file, root: contentDigest(manifest) };
}

export function defaultProductOutput(frontier: string): string {
  const stamp = new Date().toISOString().replaceAll(":", "-").replaceAll(".", "-");
  return path.join(os.homedir(), ".canopus", "runs", path.basename(path.resolve(frontier)), stamp);
}

export function assertToolUsingMissionPlatform(
  platform: NodeJS.Platform = process.platform,
): void {
  if (platform === "win32") {
    throw new Error(
      "tool-using missions do not run in native Windows; open WSL2, enter the frontier through its Linux path, and rerun the same canopus command there",
    );
  }
  if (platform !== "darwin" && platform !== "linux") {
    throw new Error(
      `tool-using missions are unsupported on ${platform}; supported worker hosts are macOS and Linux/WSL2`,
    );
  }
}

export async function runProduct(options: {
  frontier: string;
  profileName?: string;
  requestedTarget?: string;
  outputRoot?: string;
  codexHome?: string;
  noLand?: boolean;
  runner?: CommandRunner;
}): Promise<ProductRunResult> {
  // Refuse unsupported custody before creating an output directory or probing
  // credentials. Native Windows retains doctor/inspect/replay only.
  assertToolUsingMissionPlatform();
  const runner = options.runner ?? runCommand;
  const source = await realpath(options.frontier);
  const outputRoot = path.resolve(options.outputRoot ?? defaultProductOutput(source));
  await assertFreshOutput(outputRoot, source);
  const controlHome = path.join(outputRoot, "control-home");
  await mkdir(controlHome, { mode: 0o700 });
  try {
    const diagnosis = await doctorProduct({
      frontier: source,
      ...(options.profileName === undefined ? {} : { profileName: options.profileName }),
      ...(options.requestedTarget === undefined ? {} : { requestedTarget: options.requestedTarget }),
      runner,
    });
    const codexRuntime = diagnosis.public.runtimes.codex;
    const dockerRuntime = diagnosis.public.runtimes.docker;
    if (!diagnosis.public.worker.mission_ready || codexRuntime === null || dockerRuntime === null) {
      throw new Error(diagnosis.public.next_action);
    }
    const staging = path.join(outputRoot, ".profile-staging");
    await mkdir(staging, { mode: 0o700 });
    await stageProfileCapsule({
      profile: diagnosis.profile,
      stagingRoot: staging,
    });
    const bundleRoot = path.join(outputRoot, "mission");
    const draft = await loadProfileDraft(diagnosis.profile);
    const resultContract = await loadProfileResultContract(diagnosis.profile);
    const prepared = await prepareMission({
      draft: options.requestedTarget === undefined
        ? draft
        : { ...(draft as Record<string, unknown>), target: options.requestedTarget },
      draftRoot: staging,
      sourceRepo: source,
      outputRoot: bundleRoot,
      velaBinary: diagnosis.public.runtimes.vela.binary,
      codexBinary: codexRuntime.binary,
      dockerBinary: dockerRuntime.binary,
      verifierImage: diagnosis.profile.verifier_image,
      verifierPlatform: diagnosis.profile.verifier_platform,
      outputSchema: packageFile("schemas/engine-output.v0.json"),
      permissionProfile: await packagedWorkerProfile(diagnosis.profile),
      targetPacket: {
        target: diagnosis.profile.target,
        schema: diagnosis.profile.target_packet_schema,
      },
      landing: diagnosis.profile.landing,
      profileName: diagnosis.profile.name,
      profileRoot: diagnosis.profile.profile_sha256,
      ...(resultContract === undefined
        ? {}
        : {
            resultContract,
          }),
      runner,
    });
    await rm(staging, { recursive: true, force: true });
    const runRoot = path.join(outputRoot, "run");
    const vela = new VelaClient({
      binary: diagnosis.public.runtimes.vela.binary,
      expectedVersion: prepared.mission.vela_version,
      expectedSha256: prepared.mission.vela_sha256,
      home: path.join(runRoot, "vela-home"),
      ...(process.env.SSH_AUTH_SOCK === undefined
        ? {}
        : { repositoryAuthorityAgentSocket: process.env.SSH_AUTH_SOCK }),
      runner,
    });
    const engine = new CodexToolsNativeEngine({
      binary: codexRuntime.binary,
      authHome: path.resolve(options.codexHome ?? process.env.CODEX_HOME ?? path.join(os.homedir(), ".codex")),
      outputSchema: path.join(bundleRoot, "contract", "engine-output.v0.json"),
      permissionProfile: path.join(bundleRoot, prepared.mission.worker.permission_profile_path),
      runner,
    });
    const commonRun = {
      mission: prepared.mission,
      sourceRepo: source,
      runRoot,
      vela,
      engine,
      bundleRoot,
      dockerBinary: dockerRuntime.binary,
      verifierRunner: runner,
    };
    const run = options.noLand === true
      ? await runCanopus({ ...commonRun, noLand: true })
      : await runCanopus(commonRun);
    const evidence = await writeEvidenceManifest(run, contentDigest(prepared.mission));
    const publication = options.noLand === true
      ? {
          state: "unchanged_no_land" as const,
          commit: prepared.mission.roots.git_commit,
          tree: prepared.mission.roots.git_tree,
        }
      : await publishVerifiedLanding({
          source,
          landing: run.paths.landing,
          expectedStart: prepared.mission.roots.git_commit,
          expectedFinal: (run as CanopusRunResult).record.final_roots.git_commit,
          expectedTree: (run as CanopusRunResult).record.final_roots.git_tree,
          home: controlHome,
          runner,
        });
    await rm(controlHome, { recursive: true, force: true });
    return {
      run,
      doctor: diagnosis.public,
      output_root: outputRoot,
      bundle_root: bundleRoot,
      evidence_manifest: evidence.file,
      evidence_root: evidence.root,
      source_publication: publication,
    };
  } catch (error) {
    // Preserve bounded failure evidence and the exact diagnostic inputs.
    throw error;
  }
}
