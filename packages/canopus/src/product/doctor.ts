import { mkdir, mkdtemp, realpath, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { objectAt, sha256At, stringAt } from "../contracts/validation.js";
import { isolatedEnvironment, runCommand, type CommandRunner } from "../util/command.js";
import { sha256Bytes } from "../util/canonical.js";
import {
  listProductProfiles,
  loadProductProfile,
  packagedWorkerProfile,
  stageProfileCapsule,
  type ProductProfile,
} from "./profile.js";
import { runNativeCustodyPreflight } from "./custody.js";
import { runtimeIdentity, type RuntimeIdentity } from "./runtime.js";

export interface ProductDoctorResult {
  schema: "canopus.doctor.v1";
  ok: boolean;
  authority: "read_only_diagnostic";
  frontier: {
    path: string;
    git_commit: string;
    git_tree: string;
    repository_root: string;
    clean: boolean;
    strict_blockers: number;
  };
  offer: { target: string; rank: number; profile: string };
  runtimes: {
    vela: RuntimeIdentity;
    git: RuntimeIdentity;
    codex: RuntimeIdentity | null;
    docker: (RuntimeIdentity & {
      daemon: "ready";
      verifier_image: string;
      verifier_image_id: string;
    }) | null;
  };
  capsule: { sha256: string; source: "packaged" };
  worker: {
    platform: string;
    mission_runtime: "native" | "wsl2_required";
    mission_ready: boolean;
    custody_preflight: "passed" | "wsl2_required";
    custody_mode: "deterministic_no_model" | "not_applicable";
    permission_profile_sha256: string;
  };
  next_action: string;
}

async function jsonCommand(options: {
  runner: CommandRunner;
  argv: readonly string[];
  cwd: string;
  home: string;
  label: string;
  acceptedExitCodes?: readonly number[];
}): Promise<Record<string, unknown>> {
  const result = await options.runner({
    argv: options.argv,
    cwd: options.cwd,
    env: isolatedEnvironment(options.home),
    timeoutMs: 120_000,
    maxOutputBytes: 16 * 1024 * 1024,
  });
  if (!(options.acceptedExitCodes ?? [0]).includes(result.exitCode)) {
    throw new Error(
      `${options.label} failed: stdout_sha256=${sha256Bytes(result.stdout)}; ` +
      `stderr_sha256=${sha256Bytes(result.stderr)}`,
    );
  }
  return objectAt(JSON.parse(result.stdout.toString("utf8")) as unknown, options.label);
}

function nonnegative(value: unknown, at: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${at} must be a nonnegative integer`);
  }
  return value;
}

function noAvailableProducerTarget(offer: Record<string, unknown>): Error {
  return new Error("vela next returned no producer target");
}

export function selectProductOffer(
  offer: Record<string, unknown>,
  profile: ProductProfile,
  requestedTarget?: string,
): { target: Record<string, unknown>; targetId: string; rank: number } {
  const targets = offer.targets;
  if (!Array.isArray(targets) || targets.length === 0) {
    throw noAvailableProducerTarget(offer);
  }
  if (requestedTarget !== undefined && requestedTarget !== profile.target) {
    throw new Error(
      `registered profile ${profile.name} targets ${profile.target}, not requested target ${requestedTarget}`,
    );
  }
  const candidates = requestedTarget === undefined
    ? [objectAt(targets[0], "vela next.targets[0]")]
    : targets
        .map((value, index) => objectAt(value, `vela next.targets[${index}]`))
        .filter((value) => value.target_id === requestedTarget);
  if (candidates.length !== 1) {
    throw new Error(
      `requested target ${requestedTarget ?? profile.target} must appear exactly once in the ranked producer offers; ` +
      `observed ${candidates.length}`,
    );
  }
  const target = candidates[0] as Record<string, unknown>;
  const targetId = stringAt(target.target_id, "vela next target_id", { min: 1, max: 256 });
  if (targetId !== profile.target) {
    throw new Error(
      `first ranked target is ${targetId}, but registered profile ${profile.name} targets ` +
      `${profile.target}; Canopus will not skip rank 1, so freeze a verifier profile for ${targetId} before running`,
    );
  }
  const packet = objectAt(target.packet, "vela next target packet");
  const packetSchema = stringAt(packet.schema, "vela next target packet schema", {
    min: 1,
    max: 128,
  });
  const packetRoot = sha256At(packet.sha256, "vela next target packet root");
  if (
    packetSchema !== profile.target_packet_schema ||
    packetRoot !== profile.target_packet_sha256
  ) {
    throw new Error(
      `registered profile ${profile.name} is stale for ${targetId}: expected packet ` +
      `${profile.target_packet_schema} ${profile.target_packet_sha256}, observed ` +
      `${packetSchema} ${packetRoot}; freeze a new exact profile before running`,
    );
  }
  return {
    target,
    targetId,
    rank: nonnegative(target.rank, "vela next target rank"),
  };
}

export async function resolveProductProfile(
  offer: Record<string, unknown>,
  profileName?: string,
  requestedTarget?: string,
): Promise<ProductProfile> {
  if (profileName !== undefined) return loadProductProfile(profileName);
  const targets = offer.targets;
  if (!Array.isArray(targets) || targets.length === 0) {
    throw noAvailableProducerTarget(offer);
  }
  const selectedOffer = requestedTarget === undefined
    ? objectAt(targets[0], "vela next.targets[0]")
    : targets
        .map((value, index) => objectAt(value, `vela next.targets[${index}]`))
        .find((value) => value.target_id === requestedTarget);
  if (selectedOffer === undefined) {
    throw new Error(`requested target ${requestedTarget} is not a current producer offer`);
  }
  const target = stringAt(selectedOffer.target_id, "vela next target_id", { min: 1, max: 256 });
  const matches: ProductProfile[] = [];
  for (const name of await listProductProfiles()) {
    try {
      const candidate = await loadProductProfile(name);
      if (candidate.target === target) matches.push(candidate);
    } catch (error) {
      if (!(error instanceof Error) || !/replay-only/u.test(error.message)) throw error;
    }
  }
  if (matches.length === 0) {
    throw new Error(`no runnable profile is registered for producer target ${target}`);
  }
  if (matches.length !== 1) {
    throw new Error(
      `multiple runnable profiles are registered for producer target ${target}: ` +
      `${matches.map((profile) => profile.name).sort().join(", ")}; select one with --profile`,
    );
  }
  return matches[0]!;
}

export async function doctorProduct(options: {
  frontier: string;
  profileName?: string;
  requestedTarget?: string;
  runner?: CommandRunner;
  platform?: NodeJS.Platform;
}): Promise<{ public: ProductDoctorResult; profile: ProductProfile }> {
  const runner = options.runner ?? runCommand;
  const platform = options.platform ?? process.platform;
  const frontier = await realpath(options.frontier);
  const runtime = await mkdtemp(path.join(os.tmpdir(), "canopus-doctor-"));
  try {
    const [vela, git] = await Promise.all([
      runtimeIdentity({ name: "vela", cwd: frontier, home: runtime, runner }),
      runtimeIdentity({ name: "git", cwd: frontier, home: runtime, runner }),
    ]);
    const [status, offer, gitStatus] = await Promise.all([
      jsonCommand({
        runner,
        argv: [vela.binary, "status", ".", "--json"],
        cwd: frontier,
        home: runtime,
        label: "vela status",
        // Vela deliberately returns domain failure when strict debt exists,
        // while still emitting the complete read-only compact projection.
        acceptedExitCodes: [0, 1],
      }),
      jsonCommand({ runner, argv: [vela.binary, "next", ".", "--limit", "128", "--json"], cwd: frontier, home: runtime, label: "vela next" }),
      runner({ argv: [git.binary, "status", "--porcelain=v1", "--untracked-files=all"], cwd: frontier, env: isolatedEnvironment(runtime), timeoutMs: 30_000, maxOutputBytes: 8 * 1024 * 1024 }),
    ]);
    if (gitStatus.exitCode !== 0 || gitStatus.stderr.length !== 0) throw new Error("git status failed");
    const clean = gitStatus.stdout.length === 0;
    if (!clean) throw new Error("frontier checkout must be clean before Canopus runs");
    if (
      status.schema !== "vela.status.v1" ||
      status.ok !== true ||
      status.command !== "status" ||
      offer.schema !== "vela.offer.v1" ||
      offer.ok !== true ||
      offer.command !== "next"
    ) {
      throw new Error("frontier did not return the current Vela compact contracts");
    }
    const profile = await resolveProductProfile(offer, options.profileName, options.requestedTarget);
    const selected = selectProductOffer(offer, profile, options.requestedTarget);
    const roots = objectAt(status.roots, "vela status.roots");
    const gitState = objectAt(status.git, "vela status.git");
    const integrity = objectAt(status.integrity, "vela status.integrity");
    const repositoryRoot = sha256At(roots.repository, "vela status.roots.repository");
    if (
      integrity.replay !== "verified" ||
      integrity.strict !== "pass" ||
      nonnegative(integrity.blocker_count, "vela status.integrity.blocker_count") !== 0
    ) {
      throw new Error("frontier must replay and pass strict verification before Canopus can run");
    }
    if (sha256At(offer.repository_root, "vela next.repository_root") !== repositoryRoot) {
      throw new Error("vela status and next disagree on the current repository root");
    }
    const staging = path.join(runtime, "capsule-build");
    await mkdir(staging, { mode: 0o700 });
    const capsule = await stageProfileCapsule({ profile, stagingRoot: staging });
    if (platform === "win32") {
      return {
        profile,
        public: {
          schema: "canopus.doctor.v1",
          ok: true,
          authority: "read_only_diagnostic",
          frontier: {
            path: frontier,
            git_commit: stringAt(gitState.commit, "vela status.git.commit", { min: 40, max: 64 }),
            git_tree: stringAt(gitState.tree, "vela status.git.tree", { min: 40, max: 64 }),
            repository_root: repositoryRoot,
            clean,
            strict_blockers: nonnegative(integrity.blocker_count, "vela status.integrity.blocker_count"),
          },
          offer: {
            target: selected.targetId,
            rank: selected.rank,
            profile: profile.name,
          },
          runtimes: { vela, git, codex: null, docker: null },
          capsule: { sha256: profile.capsule_sha256, source: capsule.source },
          worker: {
            platform: `${platform}-${process.arch}`,
            mission_runtime: "wsl2_required",
            mission_ready: false,
            custody_preflight: "wsl2_required",
            custody_mode: "not_applicable",
            permission_profile_sha256: profile.permission_profile_sha256,
          },
          next_action:
            "Open WSL2, enter this frontier through its Linux path, and rerun canopus doctor there; native Windows supports inspect and replay only.",
        },
      };
    }
    if (platform !== "darwin" && platform !== "linux") {
      throw new Error(`Canopus doctor does not support ${platform}`);
    }
    const [codex, docker] = await Promise.all([
      runtimeIdentity({ name: "codex", cwd: frontier, home: runtime, runner }),
      runtimeIdentity({ name: "docker", cwd: frontier, home: runtime, runner }),
    ]);
    const daemon = await runner({
      argv: [docker.binary, "info", "--format={{.ServerVersion}}"],
      cwd: frontier,
      env: isolatedEnvironment(runtime),
      timeoutMs: 30_000,
      maxOutputBytes: 64 * 1024,
    });
    if (daemon.exitCode !== 0 || daemon.stdout.toString("utf8").trim() === "") {
      throw new Error("Docker daemon is not ready");
    }
    const [image, repoDigests] = await Promise.all([
      runner({
        argv: [docker.binary, "image", "inspect", "--format={{.Id}}", profile.verifier_image],
        cwd: frontier,
        env: isolatedEnvironment(runtime),
        timeoutMs: 30_000,
        maxOutputBytes: 64 * 1024,
      }),
      runner({
        argv: [docker.binary, "image", "inspect", "--format={{json .RepoDigests}}", profile.verifier_image],
        cwd: frontier,
        env: isolatedEnvironment(runtime),
        timeoutMs: 30_000,
        maxOutputBytes: 64 * 1024,
      }),
    ]);
    if (image.exitCode !== 0 || repoDigests.exitCode !== 0) {
      throw new Error(
        "registered verifier image is unavailable; run " +
          `docker pull --platform ${profile.verifier_platform} ${profile.verifier_image}`,
      );
    }
    if (image.stderr.length !== 0 || repoDigests.stderr.length !== 0) {
      throw new Error("registered verifier image inspection produced unexpected stderr");
    }
    const imageId = sha256At(image.stdout.toString("utf8").trim(), "Docker image ID");
    const observedDigests = JSON.parse(repoDigests.stdout.toString("utf8")) as unknown;
    if (!Array.isArray(observedDigests) || !observedDigests.includes(profile.verifier_image)) {
      throw new Error("registered verifier image repository digest drifted");
    }
    const permissionProfile = await packagedWorkerProfile(profile);
    const custody = await runNativeCustodyPreflight({
      binary: codex.binary,
      permissionProfile,
      runner,
    });
    if (custody.codex_version !== codex.version || custody.codex_sha256 !== codex.sha256) {
      throw new Error("native custody preflight and discovered Codex identity disagree");
    }
    if (custody.permission_profile_sha256 !== profile.permission_profile_sha256) {
      throw new Error("native custody preflight and registered worker profile disagree");
    }
    return {
      profile,
      public: {
        schema: "canopus.doctor.v1",
        ok: true,
        authority: "read_only_diagnostic",
        frontier: {
          path: frontier,
          git_commit: stringAt(gitState.commit, "vela status.git.commit", { min: 40, max: 64 }),
          git_tree: stringAt(gitState.tree, "vela status.git.tree", { min: 40, max: 64 }),
          repository_root: repositoryRoot,
          clean,
          strict_blockers: nonnegative(integrity.blocker_count, "vela status.integrity.blocker_count"),
        },
        offer: {
          target: selected.targetId,
          rank: selected.rank,
          profile: profile.name,
        },
        runtimes: {
          vela,
          codex,
          git,
          docker: {
            ...docker,
            daemon: "ready",
            verifier_image: profile.verifier_image,
            verifier_image_id: imageId,
          },
        },
        capsule: {
          sha256: profile.capsule_sha256,
          source: capsule.source,
        },
        worker: {
          platform: custody.platform,
          mission_runtime: "native",
          mission_ready: true,
          custody_preflight: "passed",
          custody_mode: custody.mode,
          permission_profile_sha256: custody.permission_profile_sha256,
        },
        next_action: options.requestedTarget === undefined
          ? `canopus run ${frontier} --first`
          : `canopus run ${frontier} --target ${selected.targetId}`,
      },
    };
  } finally {
    await rm(runtime, { recursive: true, force: true });
  }
}
