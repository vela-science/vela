import { randomBytes } from "node:crypto";
import { constants } from "node:fs";
import { chmod, copyFile, lstat, mkdir, realpath, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { TextDecoder } from "node:util";

import { assertDraftArtifactsAllowed, parseCandidateDraft } from "../candidate/validate.js";
import type { MissionV1 } from "../contracts/mission.js";
import { canonicalJson, contentDigest, sha256Bytes } from "../util/canonical.js";
import { isolatedEnvironment, runCommand, type CommandRunner } from "../util/command.js";
import { MAX_EXECUTABLE_BYTES, readBoundedRegularFile, sha256RegularFile } from "../util/files.js";
import { parseCodexEvents, summarizeCodexFailure } from "./codex-events.js";
import { prepareIsolatedCodexHome, removeIsolatedCodexHome } from "./codex-home.js";
import type { Engine, EngineContext, EngineResult } from "./engine.js";

export const NATIVE_WORKER_DISABLED_FEATURES = [
  "apps",
  "artifact",
  "auth_elicitation",
  "browser_use",
  "browser_use_external",
  "computer_use",
  "enable_fanout",
  "enable_mcp_apps",
  "goals",
  "hooks",
  "image_generation",
  "in_app_browser",
  "memories",
  "multi_agent",
  "multi_agent_v2",
  "plugin_sharing",
  "plugins",
  "remote_plugin",
  "standalone_web_search",
  "tool_call_mcp_elicitation",
  "tool_suggest",
  "workspace_dependencies",
] as const;

export interface CodexToolsNativeOptions {
  binary: string;
  authHome: string;
  outputSchema: string;
  permissionProfile: string;
  runner?: CommandRunner;
}

function prompt(mission: MissionV1): string {
  const execution = mission.target === "erdos:1056"
    ? [
        "This registered Erdős mission already supplies the complete algorithm, range, artifact grammar, and completion test. The packet identity has already been verified by the harness.",
        "Do not enumerate the workspace, rediscover files, re-hash the packet, inspect unrelated packet fields, or search for the verifier. Start by writing the bounded search program, then compile and run it, inspect only the small result artifact, and return.",
        mission.worker.platform === "darwin"
          ? "The profile exposes the Xcode compiler and SDK directly. Compile inside the workspace with `mkdir -p artifacts tmp && TMPDIR=$PWD/tmp clang++ ...`; do not call `/usr/bin/clang++`, `xcrun`, or `xcodebuild`."
          : "The profile exposes the system C++ compiler directly. Compile inside the workspace with `mkdir -p artifacts tmp && TMPDIR=$PWD/tmp c++ ...`; do not install packages or invoke a container from the worker.",
        "Implement the registered flat table literally: table size 1<<25; slot=(uint64_t(residue)*11400714819323198485ull)>>(64-25); linear probing with mask; uint32 key and generation arrays plus uint8 counts; increment generation once per prime; factorial starts at 1 and multiplies by cut only when cut>0 before counting that cut.",
        "For each prime choose the greatest count and then smallest residue. Across primes replace the best only on a strictly greater count, preserving the earliest prime on ties. After choosing the best prime/residue, recompute its increasing cut list from scratch.",
        "Use at most six shell or patch tool calls. A token-efficient correct computation is part of this mission's product contract.",
      ]
    : mission.target === "formal:erdos-505-test-dim-one"
      ? [
          "This registered formal mission exposes exactly one packet file. Do not enumerate the workspace or use rg, find, Python, package managers, or external tools.",
          `Use one narrow jq command to read only repair_context, source.statement, and output_contract from ${mission.target_packet.path}.`,
          "Then write the single raw Lean term with apply_patch. An optional wc -c check is the only useful shell follow-up; do not print the finished artifact.",
          "Return the artifact path and kind exactly as output_contract specifies. Status success means only that those complete candidate bytes exist; the separate frozen Lean capsule owns elaboration and the axiom audit.",
          "Use at most four shell or patch tool calls. A token-efficient handoff is part of this mission's product contract.",
        ]
      : mission.target === "sidon:a24-improve"
        ? [
            "The shell and apply_patch tools are active in this worker. Begin with a narrow shell call against the one registered packet; do not report a missing-tool failure without attempting that call.",
            `Read only current_state.tracked_unaccepted_seed, output_contract, verification, and negative_contract from ${mission.target_packet.path}. The tracked seed's encoded_points field contains every exact baseline point needed for the search; no other input file exists or is required.`,
            "Write search source and temporary data only inside the current workspace. On macOS compile with the exposed Xcode clang++ and TMPDIR=$PWD/tmp; do not invoke xcrun, xcodebuild, a package manager, a container, Vela, or the frozen verifier.",
            "Spend the bounded runtime on an exact net-positive exchange over the supplied baseline. Independently recheck a positive candidate before returning its compact witness JSON at the exact declared workspace path. Inline it when practical; for a large artifact use the workspace-backed handoff described below. A failed bounded search is null, never a universal maximality claim.",
          ]
      : [];
  return [
    "Execute one bounded Canopus research mission inside a fresh writable workspace containing only the exact hash-verified target packet.",
    "Use shell and apply_patch only when useful. Browser, web search, MCP, apps, memories, computer use, delegation, signing, and human keys are forbidden.",
    "Do not inspect Codex configuration, credentials, process state, unrelated repositories, or paths outside the current workspace.",
    "Command network is denied. Do not invoke Vela or the separately frozen verifier as an authority oracle.",
    `The exact Vela work claim and roots were validated by the harness. The bound target packet is available at ${mission.target_packet.path}; inspect only the fields needed for this bounded computation.`,
    "If the packet has a repair_context object, read that object first. It is a root-bound intervention record and producer strategy, not a verifier or authority decision.",
    "Keep tool output narrow. Do not print or ingest the whole target packet.",
    "Worker status reports producer completion, not verifier or scientific standing. Return status success when you produced all artifact bytes required by the output contract, even though you cannot run the separate verifier. Keep the Claim limited to the bounded result and do not put verifier status in it. State in a caveat that verification remains pending; Canopus will freeze the bytes and run the verifier after you exit.",
    "Return null only when the bounded work produced no candidate. Return failed only when you could not produce a contract-complete candidate or observed disqualifying evidence. Never turn a bounded negative search into universal nonexistence, verifier failure into success, or Git publication into scientific acceptance.",
    "Return only the supplied engine-output JSON shape. Artifact bytes must be UTF-8 at mission.allowed_paths. Inline small artifacts. For a large declared artifact, content may be the empty string only when the complete bytes exist at that exact path inside the current workspace; Canopus will bound, scan, and freeze those bytes before workspace cleanup.",
    ...execution,
    "Mission:",
    canonicalJson(mission),
  ].join("\n");
}

async function prepareTargetPacketWorkspace(
  sourceRoot: string,
  destination: string,
  mission: MissionV1,
): Promise<void> {
  const root = await realpath(sourceRoot);
  const source = await realpath(path.join(root, mission.target_packet.path));
  const relative = path.relative(root, source);
  if (relative === "" || relative === ".." || relative.startsWith(`..${path.sep}`)) {
    throw new Error("native worker target packet escapes the sealed source checkout");
  }
  const stat = await lstat(source);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.nlink !== 1) {
    throw new Error("native worker target packet must be one regular, singly linked file");
  }
  const bytes = await readBoundedRegularFile(source, 64 * 1024 * 1024);
  if (sha256Bytes(bytes) !== mission.target_packet.sha256) {
    throw new Error("native worker target packet drifted from the prepared mission");
  }
  const target = path.join(destination, mission.target_packet.path);
  await mkdir(path.dirname(target), { recursive: true, mode: 0o700 });
  await writeFile(target, bytes, { flag: "wx", mode: 0o600 });
}

async function stageLinuxSandboxBinary(
  workspace: string,
  sourceBinary: string,
  expectedDigest: string,
): Promise<string> {
  if (process.platform !== "linux") return sourceBinary;
  const runtimeDirectory = path.join(workspace, ".canopus-runtime");
  await mkdir(runtimeDirectory, { recursive: false, mode: 0o700 });
  const runtimeBinary = path.join(runtimeDirectory, "codex");
  await copyFile(sourceBinary, runtimeBinary, constants.COPYFILE_EXCL);
  await chmod(runtimeBinary, 0o500);
  if (await sha256RegularFile(runtimeBinary, MAX_EXECUTABLE_BYTES) !== expectedDigest) {
    throw new Error("native Codex binary changed while it was staged for the Linux sandbox");
  }
  return runtimeBinary;
}

function authSecrets(bytes: Buffer): Buffer[] {
  let value: unknown;
  try {
    value = JSON.parse(bytes.toString("utf8")) as unknown;
  } catch (error) {
    throw new Error(`Codex auth file is invalid JSON: ${String(error)}`);
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("Codex auth file must be an object");
  }
  const object = value as Record<string, unknown>;
  const tokens = typeof object.tokens === "object" && object.tokens !== null && !Array.isArray(object.tokens)
    ? object.tokens as Record<string, unknown>
    : {};
  return [
    object.OPENAI_API_KEY,
    tokens.access_token,
    tokens.id_token,
    tokens.refresh_token,
  ]
    .filter((item): item is string => typeof item === "string" && item.length >= 16)
    .map((item) => Buffer.from(item));
}

function assertNoSecrets(buffers: readonly Buffer[], secrets: readonly Buffer[]): void {
  for (const buffer of buffers) {
    for (const secret of secrets) {
      if (buffer.includes(secret)) throw new Error("native Codex worker exposed authentication material");
    }
  }
}

export async function hydrateWorkspaceArtifacts(options: {
  draft: ReturnType<typeof parseCandidateDraft>;
  workspace: string;
  maxArtifactBytes: number;
  secrets: readonly Buffer[];
}): Promise<ReturnType<typeof parseCandidateDraft>> {
  const workspace = await realpath(options.workspace);
  const artifacts = await Promise.all(options.draft.artifacts.map(async (artifact) => {
    if (artifact.content !== "") return artifact;

    const candidate = path.join(workspace, artifact.path);
    const relative = path.relative(workspace, candidate);
    if (relative === "" || relative === ".." || relative.startsWith(`..${path.sep}`)) {
      throw new Error(`workspace-backed artifact escapes the native worker workspace: ${artifact.path}`);
    }

    let resolved: string;
    try {
      resolved = await realpath(candidate);
    } catch (error) {
      throw new Error(
        `workspace-backed artifact is missing at ${artifact.path}: ${String(error)}`,
      );
    }
    if (resolved !== candidate) {
      throw new Error(`workspace-backed artifact must not traverse a symbolic link: ${artifact.path}`);
    }

    const bytes = await readBoundedRegularFile(resolved, options.maxArtifactBytes);
    if (bytes.length === 0) {
      throw new Error(`workspace-backed artifact is empty at ${artifact.path}`);
    }
    assertNoSecrets([bytes], options.secrets);
    let content: string;
    try {
      content = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    } catch (error) {
      throw new Error(`workspace-backed artifact is not valid UTF-8 at ${artifact.path}: ${String(error)}`);
    }
    return { ...artifact, content };
  }));
  return { ...options.draft, artifacts };
}

function workerArgv(options: {
  binary: string;
  mission: MissionV1;
  outputSchema: string;
  finalPath: string;
  cwd: string;
}): string[] {
  return [
    options.binary,
    "exec",
    "--ephemeral",
    "--strict-config",
    "--ignore-rules",
    "--skip-git-repo-check",
    "--model",
    options.mission.worker.model,
    "--output-schema",
    options.outputSchema,
    "--output-last-message",
    options.finalPath,
    "--json",
    "--color",
    "never",
    "--cd",
    options.cwd,
    "--config",
    'web_search="disabled"',
    "--config",
    'model_reasoning_effort="low"',
    ...NATIVE_WORKER_DISABLED_FEATURES.flatMap((feature) => ["--disable", feature]),
    "-",
  ];
}

function requireV1(context: EngineContext): MissionV1 {
  if (context.mission.schema !== "canopus.mission.v1") {
    throw new Error("the native tool worker accepts only canopus.mission.v1");
  }
  return context.mission;
}

export async function assertNativeRuntimeProfile(options: {
  binary: string;
  runner: CommandRunner;
  environment: NodeJS.ProcessEnv;
  cwd: string;
  sourceAuth: string;
  runtimeAuth: string;
  inaccessibleInput: string;
  unrelatedFile: string;
  canary: string;
  outsideWrite: string;
  timeoutMs: number;
  includeSafeDiagnostics?: boolean;
}): Promise<void> {
  const script = [
    "curl=false; source=false; runtime=false; input=false; unrelated=false; canary=false; writable=false; network=false; environ=false; proc=false;",
    'if /usr/bin/curl --version >/dev/null 2>&1; then curl=true; fi;',
    'if /bin/dd if="$1" of=/dev/null bs=1 count=1 2>/dev/null; then source=true; fi;',
    'if /bin/dd if="$2" of=/dev/null bs=1 count=1 2>/dev/null; then runtime=true; fi;',
    'if /bin/dd if="$3" of=/dev/null bs=1 count=1 2>/dev/null; then input=true; fi;',
    'if /bin/dd if="$4" of=/dev/null bs=1 count=1 2>/dev/null; then unrelated=true; fi;',
    'if /bin/dd if="$5" of=/dev/null bs=1 count=1 2>/dev/null; then canary=true; fi;',
    'if { printf "probe\\n" > "$6"; } 2>/dev/null; then writable=true; fi;',
    'if /usr/bin/curl --fail --silent --show-error --max-time 3 --output /dev/null https://example.com/ 2>/dev/null; then network=true; fi;',
    "if /usr/bin/env | /usr/bin/grep -Eq '^(OPENAI_API_KEY|CODEX_API_KEY|CANOPUS_AUTH)='; then environ=true; fi;",
    "if [ -r /proc/1/environ ] && /usr/bin/grep -aEq '(OPENAI_API_KEY|CODEX_API_KEY|CANOPUS_AUTH)=' /proc/1/environ; then proc=true; fi;",
    'printf "%s %s %s %s %s %s %s %s %s %s\\n" "$curl" "$source" "$runtime" "$input" "$unrelated" "$canary" "$writable" "$network" "$environ" "$proc";',
  ].join(" ");
  const result = await options.runner({
    argv: [
      options.binary,
      "sandbox",
      "-P",
      "canopus-worker",
      "-C",
      options.cwd,
      "--sandbox-state-readable-root",
      options.binary,
      "--",
      "/bin/sh",
      "-c",
      script,
      "sh",
      options.sourceAuth,
      options.runtimeAuth,
      options.inaccessibleInput,
      options.unrelatedFile,
      options.canary,
      options.outsideWrite,
    ],
    cwd: options.cwd,
    env: options.environment,
    timeoutMs: Math.min(30_000, options.timeoutMs),
    maxOutputBytes: 4096,
  });
  if (
    result.exitCode !== 0 ||
    result.stderr.length !== 0 ||
    result.stdout.toString("utf8").trim() !==
      "true false false false false false false false false false"
  ) {
    const stderr = result.stderr.toString("utf8");
    if (stderr.includes("bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted")) {
      throw new Error(
        "native Codex sandbox is blocked by Ubuntu AppArmor user-namespace policy; " +
        "install and load the targeted bwrap-userns-restrict profile documented at " +
        "https://developers.openai.com/codex/concepts/sandboxing#prerequisites, then retry",
      );
    }
    const stdout = result.stdout.toString("utf8").trim();
    const boundedStdout = /^(?:true|false)(?: (?:true|false)){9}$/u.test(stdout)
      ? stdout
      : `sha256=${sha256Bytes(result.stdout)}`;
    const boundedStderr =
      options.includeSafeDiagnostics === true &&
      result.stderr.length <= 4096 &&
      /^[\t\n\r\x20-\x7e]*$/u.test(stderr)
        ? JSON.stringify(stderr.trim())
        : `sha256=${sha256Bytes(result.stderr)}`;
    throw new Error(
      `native Codex permission profile failed its deterministic custody preflight: ` +
      `exit=${result.exitCode}; stdout=${boundedStdout}; stderr=${boundedStderr}`,
    );
  }
}

export class CodexToolsNativeEngine implements Engine {
  public readonly name = "codex-tools-native";
  readonly #options: CodexToolsNativeOptions;
  readonly #runner: CommandRunner;

  public constructor(options: CodexToolsNativeOptions) {
    this.#options = options;
    this.#runner = options.runner ?? runCommand;
  }

  public async run(context: EngineContext): Promise<EngineResult> {
    const mission = requireV1(context);
    if (process.platform !== mission.worker.platform) {
      throw new Error(`native worker requires ${mission.worker.platform}, observed ${process.platform}`);
    }
    context.budget.beginAttempt();
    const binary = await realpath(this.#options.binary);
    const [binaryDigest, profileBytes, schemaBytes, sourceAuthBytes] = await Promise.all([
      sha256RegularFile(binary, MAX_EXECUTABLE_BYTES),
      readBoundedRegularFile(this.#options.permissionProfile, 8 * 1024 * 1024),
      readBoundedRegularFile(this.#options.outputSchema, 8 * 1024 * 1024),
      readBoundedRegularFile(path.join(this.#options.authHome, "auth.json"), 2 * 1024 * 1024),
    ]);
    const profileDigest = sha256Bytes(profileBytes);
    const schemaDigest = sha256Bytes(schemaBytes);
    if (binaryDigest !== mission.worker.codex_sha256) throw new Error("native Codex binary drifted");
    if (profileDigest !== mission.worker.permission_profile_sha256) {
      throw new Error("native Codex permission profile drifted");
    }
    if (schemaDigest !== mission.worker.output_schema_sha256) {
      throw new Error("native Codex output schema drifted");
    }

    const workspace = path.join(context.paths.work, "native-worker");
    await prepareTargetPacketWorkspace(context.paths.input, workspace, mission);
    const runtimeBinary = await stageLinuxSandboxBinary(workspace, binary, binaryDigest);
    const canaryDirectory = path.join(context.paths.work, "custody-canary");
    await mkdir(canaryDirectory, { mode: 0o700 });
    const canaryBytes = Buffer.from(`canopus-host-secret-${randomBytes(32).toString("hex")}\n`);
    const canary = path.join(canaryDirectory, "host-secret");
    await writeFile(canary, canaryBytes, { flag: "wx", mode: 0o400 });
    const runtimeCodexHome = await prepareIsolatedCodexHome(
      this.#options.authHome,
      context.paths.home,
      profileBytes,
    );
    const finalPath = path.join(workspace, ".canopus-final.json");
    const environment = {
      ...isolatedEnvironment(context.paths.home),
      CODEX_HOME: runtimeCodexHome,
      NO_COLOR: "1",
    };
    const secrets = [...authSecrets(sourceAuthBytes), canaryBytes.subarray(0, canaryBytes.length - 1)];
    try {
      context.budget.beginProcess();
      const versionResult = await this.#runner({
        argv: [runtimeBinary, "--version"],
        cwd: workspace,
        env: environment,
        timeoutMs: Math.min(30_000, context.budget.remainingTimeMs()),
        maxOutputBytes: 4096,
      });
      const version = versionResult.stdout.toString("utf8").trim();
      if (
        versionResult.exitCode !== 0 ||
        versionResult.stderr.length !== 0 ||
        version !== mission.worker.codex_version
      ) {
        throw new Error(`expected ${mission.worker.codex_version}, observed ${JSON.stringify(version)}`);
      }

      context.budget.beginProcess();
      await assertNativeRuntimeProfile({
        binary: runtimeBinary,
        runner: this.#runner,
        environment: {
          ...environment,
          CANOPUS_AUTH: "canopus-preflight-environment-canary",
        },
        cwd: workspace,
        sourceAuth: path.join(this.#options.authHome, "auth.json"),
        runtimeAuth: path.join(runtimeCodexHome, "auth.json"),
        inaccessibleInput: path.join(context.paths.input, mission.target_packet.path),
        unrelatedFile: path.join(context.paths.frontier, ".git", "HEAD"),
        canary,
        outsideWrite: path.join(context.paths.work, "outside-write"),
        timeoutMs: context.budget.remainingTimeMs(),
      });

      const workerPrompt = prompt(mission);
      context.budget.addPrompt(Buffer.byteLength(workerPrompt));
      context.budget.beginProcess();
      const started = performance.now();
      const result = await this.#runner({
        argv: workerArgv({
          binary: runtimeBinary,
          mission,
          outputSchema: this.#options.outputSchema,
          finalPath,
          cwd: workspace,
        }),
        cwd: workspace,
        env: environment,
        timeoutMs: context.budget.remainingTimeMs(),
        maxOutputBytes: context.budget.remainingOutputBytes(),
        stdin: workerPrompt,
      });
      context.budget.addOutput(result.stdout.length + result.stderr.length);
      assertNoSecrets([result.stdout, result.stderr], secrets);
      if (result.exitCode !== 0) {
        throw new Error(
          `native codex exec exited ${result.exitCode}: ` +
          `${summarizeCodexFailure(result.stdout.toString("utf8"))}; ` +
          `stdout_sha256=${sha256Bytes(result.stdout)}; stderr_sha256=${sha256Bytes(result.stderr)}`,
        );
      }
      await Promise.all([
        writeFile(path.join(context.paths.root, "worker-events.jsonl"), result.stdout, {
          flag: "wx",
          mode: 0o600,
        }),
        writeFile(path.join(context.paths.root, "worker-stderr.bin"), result.stderr, {
          flag: "wx",
          mode: 0o600,
        }),
      ]);
      const events = parseCodexEvents(result.stdout.toString("utf8"));
      try {
        context.budget.addTokens(events.usage.input_tokens + events.usage.output_tokens);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new Error(
          `${message}; event_stream_sha256=${sha256Bytes(result.stdout)}; ` +
          `stderr_sha256=${sha256Bytes(result.stderr)}`,
        );
      }
      const finalBytes = await readBoundedRegularFile(finalPath, 1_048_576);
      context.budget.addOutput(finalBytes.length);
      assertNoSecrets([finalBytes], secrets);
      await writeFile(path.join(context.paths.root, "worker-final.json"), finalBytes, {
        flag: "wx",
        mode: 0o600,
      });
      let raw: unknown;
      try {
        raw = JSON.parse(finalBytes.toString("utf8")) as unknown;
      } catch (error) {
        throw new Error(`native Codex final response is not JSON: ${String(error)}`);
      }
      const parsedDraft = parseCandidateDraft(raw);
      assertDraftArtifactsAllowed(parsedDraft, mission.allowed_paths);
      const draft = await hydrateWorkspaceArtifacts({
        draft: parsedDraft,
        workspace,
        maxArtifactBytes: mission.budgets.max_artifact_bytes,
        secrets,
      });
      return {
        draft,
        engine: {
          name: this.name,
          version,
          binary_sha256: binaryDigest,
          model: mission.worker.model,
          configuration_sha256: contentDigest({
            worker: mission.worker,
            permission_profile_sha256: profileDigest,
            output_schema_sha256: schemaDigest,
            disabled_features: NATIVE_WORKER_DISABLED_FEATURES,
            source_projection: "exact_target_packet_only",
            command_network: "denied",
            credentials: "outside_command_read_boundary",
          }),
        },
        usage: events.usage,
        wallTimeMs: Math.max(0, Math.round(performance.now() - started)),
        eventTypes: events.eventTypes,
        actionTypes: events.actionTypes,
        eventsDigest: sha256Bytes(result.stdout),
        stderrDigest: sha256Bytes(result.stderr),
      };
    } finally {
      await removeIsolatedCodexHome(runtimeCodexHome);
      await rm(canaryDirectory, { recursive: true, force: true });
      await rm(workspace, { recursive: true, force: true });
    }
  }
}
