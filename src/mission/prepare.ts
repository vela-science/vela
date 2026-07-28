import { constants } from "node:fs";
import {
  access,
  chmod,
  copyFile,
  lstat,
  mkdir,
  mkdtemp,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";

import {
  EXECUTION_BINDING_SCHEMA,
  parseMission,
  type LandingSpec,
  type MissionV1,
  type PositiveResultContractV1,
} from "../contracts/mission.js";
import {
  objectAt,
  relativePathAt,
  sha256At,
  stringAt,
} from "../contracts/validation.js";
import { canonicalJson, contentDigest, sha256Bytes } from "../util/canonical.js";
import { isolatedEnvironment, runCommand, type CommandRunner } from "../util/command.js";
import { MAX_EXECUTABLE_BYTES, readBoundedRegularFile, sha256RegularFile } from "../util/files.js";
import { VelaClient } from "../vela/cli.js";

export interface PrepareMissionOptions {
  draft: unknown;
  draftRoot: string;
  sourceRepo: string;
  outputRoot: string;
  velaBinary: string;
  codexBinary: string;
  dockerBinary: string;
  verifierImage: string;
  verifierPlatform?: "linux/amd64" | "linux/arm64";
  outputSchema: string;
  permissionProfile: string;
  targetPacket?: { target: string; schema: string };
  landing?: LandingSpec;
  profileName?: string;
  profileRoot?: string;
  resultContract?: PositiveResultContractV1;
  runner?: CommandRunner;
}

export interface PreparedMission {
  mission: MissionV1;
  bundleRoot: string;
  missionPath: string;
  manifestPath: string;
}

export function parseVelaVersionOutput(versionText: string): string {
  const match = /^vela (0\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?)$/u.exec(
    versionText,
  );
  if (match?.[1] === undefined) throw new Error("Vela returned an invalid version");
  return match[1];
}

export async function assertVerifierWorkingDirectory(
  sourceRepo: string,
  declaredCwd: string,
): Promise<void> {
  const root = await realpath(sourceRepo);
  const relative = relativePathAt(declaredCwd, "mission.verifier.cwd");
  const candidate = path.resolve(root, relative);
  let resolved: string;
  let stat: Awaited<ReturnType<typeof lstat>>;
  try {
    [resolved, stat] = await Promise.all([realpath(candidate), lstat(candidate)]);
  } catch {
    throw new Error(`mission verifier cwd does not exist in the sealed source checkout: ${relative}`);
  }
  if (!resolved.startsWith(`${root}${path.sep}`) || !stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error(`mission verifier cwd is not a real directory below the sealed source checkout: ${relative}`);
  }
}

export async function validateMissionBundle(
  mission: MissionV1,
  bundleRoot: string,
): Promise<void> {
  const root = await realpath(bundleRoot);
  const capsule = await sourceFile(root, mission.verifier.capsule_path, "verifier capsule");
  await access(capsule, constants.X_OK);
  const capsuleDigest = sha256Bytes(await readBoundedRegularFile(capsule, 268_435_456));
  if (
    capsuleDigest !== mission.verifier.capsule_sha256 ||
    capsuleDigest !== mission.verifier.executable_sha256
  ) {
    throw new Error("mission bundle verifier capsule drifted");
  }
  const packet = await sourceFile(root, "packet/target.json", "bundled target packet");
  if (sha256Bytes(await readBoundedRegularFile(packet, 64 * 1024 * 1024)) !== mission.target_packet.sha256) {
    throw new Error("mission bundle target packet drifted");
  }
  const outputSchema = await sourceFile(
    root,
    "contract/engine-output.v0.json",
    "bundled engine output schema",
  );
  if (
    sha256Bytes(await readBoundedRegularFile(outputSchema, 8 * 1024 * 1024)) !==
    mission.worker.output_schema_sha256
  ) {
    throw new Error("mission bundle engine output schema drifted");
  }
  const permissionProfile = await sourceFile(
    root,
    mission.worker.permission_profile_path,
    "bundled worker permission profile",
  );
  if (
    sha256Bytes(await readBoundedRegularFile(permissionProfile, 8 * 1024 * 1024)) !==
    mission.worker.permission_profile_sha256
  ) {
    throw new Error("mission bundle worker permission profile drifted");
  }
  const manifestBytes = await readBoundedRegularFile(
    path.join(root, "bundle-manifest.json"),
    8 * 1024 * 1024,
  );
  let manifestValue: unknown;
  try {
    manifestValue = JSON.parse(manifestBytes.toString("utf8")) as unknown;
  } catch (error) {
    throw new Error(`mission bundle manifest is invalid JSON: ${String(error)}`);
  }
  const manifest = objectAt(manifestValue, "mission bundle manifest");
  if (
    manifest.schema !== "canopus.mission-bundle.v1" ||
    manifest.authority !== "non_authoritative" ||
    manifest.mission_sha256 !== contentDigest(mission)
  ) {
    throw new Error("mission bundle manifest does not bind the mission");
  }
}

async function commandText(
  runner: CommandRunner,
  argv: readonly string[],
  cwd: string,
  home: string,
  allowStderr = false,
): Promise<string> {
  const result = await runner({
    argv,
    cwd,
    env: isolatedEnvironment(home),
    timeoutMs: 120_000,
    maxOutputBytes: 8 * 1024 * 1024,
  });
  if (result.exitCode !== 0 || (!allowStderr && result.stderr.length !== 0)) {
    throw new Error(
      `${argv[0]} ${argv.slice(1).join(" ")} failed with exit ${result.exitCode}; ` +
      `stdout_sha256=${sha256Bytes(result.stdout)}; stderr_sha256=${sha256Bytes(result.stderr)}`,
    );
  }
  return result.stdout.toString("utf8").trim();
}

async function assertFreshOutput(outputRoot: string): Promise<void> {
  try {
    await lstat(outputRoot);
    throw new Error("mission output root already exists");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
}

function below(child: string, parent: string): boolean {
  const relative = path.relative(parent, child);
  return relative !== "" && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

async function sourceFile(root: string, relative: string, label: string): Promise<string> {
  const parent = await realpath(root);
  const resolved = await realpath(path.resolve(parent, relative));
  if (!below(resolved, parent)) throw new Error(`${label} escapes its registered root`);
  const stat = await lstat(resolved);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.nlink !== 1) {
    throw new Error(`${label} must be one regular, singly linked file`);
  }
  return resolved;
}

function boundedObjective(value: unknown): string {
  const objective = stringAt(value, "mission.objective", { min: 1, max: 8192 });
  const universal = /\b(?:settle|solve|prove|establish)\b[^.]{0,160}\b(?:for all|for every|unbounded|universally)\b/iu;
  const finite = /\b(?:bounded|exact|finite|range|through|up to|k\s*=\s*\d+|witness|counterexample|exhaust)\b/iu;
  if (universal.test(objective) || !finite.test(objective)) {
    throw new Error("mission objective must name a finite or exactly checkable obligation");
  }
  return objective;
}

export function selectedProducerTarget(
  offer: Record<string, unknown>,
  requested?: string,
): Record<string, unknown> {
  if (!Array.isArray(offer.targets)) throw new Error("vela next omitted targets");
  const producers: Record<string, unknown>[] = [];
  for (const [index, raw] of offer.targets.entries()) {
    const target = objectAt(raw, `vela next.targets[${index}]`);
    if (target.lane === "produce") producers.push(target);
  }
  if (producers.length === 0) {
    throw new Error("vela next returned no producer target in the first 128 offers");
  }
  if (requested === undefined) return producers[0] as Record<string, unknown>;
  const matches = producers.filter((target) => (target.target_id ?? target.id) === requested);
  if (matches.length !== 1) {
    throw new Error(
      `explicit mission target ${requested} must appear exactly once among the first 128 producer offers; ` +
      `observed ${matches.length}`,
    );
  }
  return matches[0] as Record<string, unknown>;
}

export function packetFromTarget(
  target: Record<string, unknown>,
  expected?: { target: string; schema: string },
): { path: string; sha256: string } {
  const packet = objectAt(target.packet, "vela next producer packet");
  const targetId = target.target_id ?? target.id;
  if (expected !== undefined && targetId !== expected.target) {
    throw new Error(`selected producer target ${String(targetId)} does not match profile target ${expected.target}`);
  }
  const schema = stringAt(packet.schema, "vela next producer packet.schema", { min: 1, max: 128 });
  if (expected !== undefined && schema !== expected.schema) {
    throw new Error(`selected packet schema ${schema} does not match profile schema ${expected.schema}`);
  }
  if (expected === undefined && schema !== "erdos-frontier.problem-work.v1") {
    throw new Error("selected producer target has no registered exact problem packet schema");
  }
  return {
    path: relativePathAt(packet.path, "vela next producer packet.path"),
    sha256: sha256At(packet.sha256, "vela next producer packet.sha256"),
  };
}

async function imageId(
  runner: CommandRunner,
  dockerBinary: string,
  image: string,
  cwd: string,
  home: string,
): Promise<string> {
  const observed = await commandText(
    runner,
    [dockerBinary, "image", "inspect", "--format={{.Id}}", image],
    cwd,
    home,
  );
  return sha256At(observed, "Docker image ID");
}

async function workerIdentity(
  runner: CommandRunner,
  binaryPath: string,
  cwd: string,
  home: string,
): Promise<{ codex_version: string; codex_sha256: string }> {
  if (process.platform !== "darwin" && process.platform !== "linux") {
    throw new Error("canopus.mission.v1 tool workers require macOS, Linux, or WSL2");
  }
  const binary = await realpath(binaryPath);
  const output = await commandText(runner, [binary, "--version"], cwd, home, true);
  const version = stringAt(output, "worker Codex version", {
    min: 10,
    max: 64,
    pattern: /^codex-cli [0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?$/u,
  });
  return {
    codex_version: version,
    codex_sha256: await sha256RegularFile(binary, MAX_EXECUTABLE_BYTES),
  };
}

export async function prepareMission(options: PrepareMissionOptions): Promise<PreparedMission> {
  const runner = options.runner ?? runCommand;
  const sourceRepo = await realpath(options.sourceRepo);
  const draftRoot = await realpath(options.draftRoot);
  const outputRoot = path.resolve(options.outputRoot);
  await assertFreshOutput(outputRoot);
  const runtimeHome = await mkdtemp(path.join(os.tmpdir(), "canopus-prepare-"));
  try {
    const status = await commandText(
      runner,
      ["git", "status", "--porcelain=v1", "--untracked-files=all"],
      sourceRepo,
      runtimeHome,
    );
    if (status !== "") throw new Error("mission source checkout must be clean");

    const raw = objectAt(options.draft, "mission draft");
    const frontier = raw.frontier === "."
      ? "."
      : relativePathAt(raw.frontier, "mission.frontier");
    const velaBinary = await realpath(options.velaBinary);
    const velaSha256 = await sha256RegularFile(velaBinary, MAX_EXECUTABLE_BYTES);
    const versionText = await commandText(
      runner,
      [velaBinary, "--version"],
      sourceRepo,
      runtimeHome,
    );
    const velaVersion = parseVelaVersionOutput(versionText);
    const vela = new VelaClient({
      binary: velaBinary,
      expectedVersion: velaVersion,
      expectedSha256: velaSha256,
      home: path.join(runtimeHome, "vela"),
      runner,
    });
    const inspection = await vela.inspect(sourceRepo, frontier);
    const offer = await vela.offer(
      sourceRepo,
      frontier,
      inspection.roots,
      128,
    );
    const requestedTarget = raw.target === undefined || raw.target === "auto"
      ? undefined
      : stringAt(raw.target, "mission.target", { min: 1, max: 256 });
    const target = selectedProducerTarget(offer.value, requestedTarget);
    const targetId = stringAt(
      target.target_id ?? target.id,
      "vela next producer target_id",
      { min: 1, max: 256 },
    );
    const packet = packetFromTarget(target, options.targetPacket);
    const packetSource = await sourceFile(sourceRepo, packet.path, "target packet");
    const packetBytes = await readBoundedRegularFile(packetSource, 64 * 1024 * 1024);
    if (sha256Bytes(packetBytes) !== packet.sha256) {
      throw new Error("selected target packet digest does not match vela next");
    }
    const tracked = await commandText(
      runner,
      ["git", "ls-files", "--error-unmatch", "--", packet.path],
      sourceRepo,
      runtimeHome,
    );
    if (tracked !== packet.path) throw new Error("target packet is not an exact tracked source file");

    const verifierDraft = objectAt(raw.verifier, "mission.verifier");
    const sourceCapsuleRelative = relativePathAt(
      verifierDraft.capsule_path,
      "mission.verifier.capsule_path",
    );
    const sourceCapsule = await sourceFile(draftRoot, sourceCapsuleRelative, "verifier capsule");
    await access(sourceCapsule, constants.X_OK);
    const capsuleBytes = await readBoundedRegularFile(sourceCapsule, 268_435_456);
    const capsuleDigest = sha256Bytes(capsuleBytes);

    const workerDraft = objectAt(raw.worker, "mission.worker");
    const outputSchemaSource = await sourceFile(
      path.dirname(await realpath(options.outputSchema)),
      path.basename(options.outputSchema),
      "engine output schema",
    );
    const outputSchemaBytes = await readBoundedRegularFile(outputSchemaSource, 8 * 1024 * 1024);
    const outputSchemaDigest = sha256Bytes(outputSchemaBytes);
    const permissionProfileSource = await sourceFile(
      path.dirname(await realpath(options.permissionProfile)),
      path.basename(options.permissionProfile),
      "worker permission profile",
    );
    const permissionProfileBytes = await readBoundedRegularFile(
      permissionProfileSource,
      8 * 1024 * 1024,
    );
    const permissionProfileDigest = sha256Bytes(permissionProfileBytes);
    const verifierImage = await imageId(
      runner,
      options.dockerBinary,
      options.verifierImage,
      sourceRepo,
      runtimeHome,
    );
    const identity = await workerIdentity(
      runner,
      options.codexBinary,
      sourceRepo,
      runtimeHome,
    );
    const verifierArgv = Array.isArray(verifierDraft.argv)
      ? [...verifierDraft.argv]
      : verifierDraft.argv;
    if (!Array.isArray(verifierArgv) || verifierArgv.length === 0) {
      throw new Error("mission verifier argv is required");
    }
    verifierArgv[0] = "capsule/verifier";

    const prepared = parseMission({
      ...raw,
      schema: "canopus.mission.v1",
      target: targetId,
      vela_version: velaVersion,
      vela_sha256: velaSha256,
      objective: boundedObjective(raw.objective),
      roots: inspection.roots,
      target_packet: packet,
      worker: {
        ...workerDraft,
        kind: "codex_tools_native",
        platform: process.platform,
        ...identity,
        permission_profile_path: "contract/native-worker.config.toml",
        permission_profile_sha256: permissionProfileDigest,
        workspace: "target_packet_only",
        output_schema_sha256: outputSchemaDigest,
        network: "provider_only",
        tools: ["shell", "apply_patch"],
      },
      verifier: {
        ...verifierDraft,
        argv: verifierArgv,
        executable_sha256: capsuleDigest,
        capsule_path: "capsule/verifier",
        capsule_sha256: capsuleDigest,
        image: verifierImage,
        ...(options.verifierPlatform === undefined
          ? {}
          : { platform: options.verifierPlatform }),
        network: "deny",
        writes: "deny",
      },
      landing: options.landing ?? { expected_routes: ["defer"], max_accepted_delta: 0 },
      ...(options.profileName === undefined || options.profileRoot === undefined
        ? {}
        : {
            profile: {
              name: options.profileName,
              root: sha256At(options.profileRoot, "profile root"),
            },
          }),
      ...(options.profileRoot === undefined || options.resultContract === undefined
        ? {}
        : {
            execution_binding: {
              schema: EXECUTION_BINDING_SCHEMA,
              packet_root: packet.sha256,
              profile_root: sha256At(options.profileRoot, "profile root"),
              verifier_capsule_root: capsuleDigest,
              result_contract_root: contentDigest(options.resultContract),
            },
            result_contract: options.resultContract,
          }),
    });
    if ((options.profileName === undefined) !== (options.profileRoot === undefined)) {
      throw new Error("mission preparation requires profile name and root together");
    }
    if (options.resultContract !== undefined && options.profileRoot === undefined) {
      throw new Error("mission result contract requires a profile root");
    }
    if (prepared.schema !== "canopus.mission.v1") {
      throw new Error("prepared mission did not produce mission v1");
    }
    await assertVerifierWorkingDirectory(sourceRepo, prepared.verifier.cwd);

    const capsuleTarget = path.join(outputRoot, "capsule", "verifier");
    const packetTarget = path.join(outputRoot, "packet", "target.json");
    const outputSchemaTarget = path.join(outputRoot, "contract", "engine-output.v0.json");
    const permissionProfileTarget = path.join(
      outputRoot,
      "contract",
      "native-worker.config.toml",
    );
    await Promise.all([
      mkdir(path.dirname(capsuleTarget), { recursive: true, mode: 0o700 }),
      mkdir(path.dirname(packetTarget), { recursive: true, mode: 0o700 }),
      mkdir(path.dirname(outputSchemaTarget), { recursive: true, mode: 0o700 }),
    ]);
    await Promise.all([
      copyFile(sourceCapsule, capsuleTarget),
      copyFile(packetSource, packetTarget),
      copyFile(outputSchemaSource, outputSchemaTarget),
      copyFile(permissionProfileSource, permissionProfileTarget),
    ]);
    await chmod(capsuleTarget, 0o555);
    const missionPath = path.join(outputRoot, "mission.json");
    const manifestPath = path.join(outputRoot, "bundle-manifest.json");
    const manifest = {
      schema: "canopus.mission-bundle.v1",
      authority: "non_authoritative",
      mission_sha256: contentDigest(prepared),
      source: {
        git_commit: prepared.roots.git_commit,
        git_tree: prepared.roots.git_tree,
      },
      packet: {
        source_path: packet.path,
        bundle_path: "packet/target.json",
        sha256: packet.sha256,
      },
      verifier: {
        bundle_path: "capsule/verifier",
        sha256: capsuleDigest,
        image: verifierImage,
        ...(options.verifierPlatform === undefined
          ? {}
          : { platform: options.verifierPlatform }),
      },
      worker: {
        platform: process.platform,
        codex_version: identity.codex_version,
        codex_sha256: identity.codex_sha256,
        permission_profile_path: "contract/native-worker.config.toml",
        permission_profile_sha256: permissionProfileDigest,
        workspace: "target_packet_only",
        output_schema_sha256: outputSchemaDigest,
      },
      ...(prepared.execution_binding === undefined
        ? {}
        : { execution_binding: prepared.execution_binding }),
    };
    await Promise.all([
      writeFile(missionPath, canonicalJson(prepared), { flag: "wx", mode: 0o600 }),
      writeFile(manifestPath, canonicalJson(manifest), { flag: "wx", mode: 0o600 }),
    ]);
    return { mission: prepared, bundleRoot: outputRoot, missionPath, manifestPath };
  } catch (error) {
    await rm(outputRoot, { recursive: true, force: true });
    throw error;
  } finally {
    await rm(runtimeHome, { recursive: true, force: true });
  }
}
