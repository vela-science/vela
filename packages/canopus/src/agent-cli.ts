import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { parseRetainedRunRecord } from "./projection/retained-run.js";
import { projectCurrentRun } from "./projection/current-run.js";
import { parseFailureRecord, projectFailure } from "./projection/failure.js";
import { observedHelperBuild, runAttemptProduct } from "./product/attempt.js";
import {
  assertNativeOutputPlacement,
  nativeOutputPlacement,
  runNativeCustodyPreflight,
} from "./product/custody.js";
import { replayProduct } from "./product/replay.js";
import { runtimeIdentity } from "./product/runtime.js";
import { exportSubmission } from "./product/submission.js";
import { protocolDigest, sha256Bytes } from "./util/canonical.js";
import { readBoundedRegularFile } from "./util/files.js";

declare const __VELA_AGENT_ENGINE_OUTPUT_SCHEMA__: string;
declare const __VELA_AGENT_MACOS_PERMISSION_PROFILE__: string;
declare const __VELA_AGENT_LINUX_PERMISSION_PROFILE__: string;

async function stdinJson(maxBytes = 2 * 1024 * 1024): Promise<unknown> {
  const chunks: Buffer[] = [];
  let total = 0;
  for await (const chunk of process.stdin) {
    const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    total += bytes.length;
    if (total > maxBytes) throw new Error(`Agent run request exceeds ${maxBytes} bytes`);
    chunks.push(bytes);
  }
  if (total === 0) throw new Error("Agent run request stdin is empty");
  return JSON.parse(Buffer.concat(chunks).toString("utf8")) as unknown;
}

function valueOptions(args: string[], allowed: readonly string[]): Map<string, string> {
  const accepted = new Set(allowed);
  const values = new Map<string, string>();
  for (let index = 0; index < args.length; index += 2) {
    const option = args[index];
    const value = args[index + 1];
    if (
      option === undefined ||
      value === undefined ||
      !accepted.has(option) ||
      values.has(option)
    ) {
      throw new Error(`invalid or duplicate option near ${option ?? "end of arguments"}`);
    }
    values.set(option, value);
  }
  return values;
}

async function show(file: string): Promise<void> {
  const resolved = path.resolve(file);
  const bytes = await readBoundedRegularFile(resolved, 64 * 1024 * 1024);
  const value = JSON.parse(bytes.toString("utf8")) as unknown;
  const schema = typeof value === "object" && value !== null
    ? (value as Record<string, unknown>).schema
    : null;
  const projection = schema === "canopus.failure.v0" || schema === "canopus.failure.v1"
    ? projectFailure(parseFailureRecord(value))
    : projectCurrentRun(parseRetainedRunRecord(value).record);
  process.stdout.write(`${JSON.stringify({
    ok: true,
    command: "show",
    run_file: resolved,
    projection,
  })}\n`);
}

async function run(raw: unknown, helperBinary: string): Promise<void> {
  const result = await runAttemptProduct(raw, {
    helperBinary,
    supportAssets: {
      engineOutputSchema: __VELA_AGENT_ENGINE_OUTPUT_SCHEMA__,
      macosPermissionProfile: __VELA_AGENT_MACOS_PERMISSION_PROFILE__,
      linuxPermissionProfile: __VELA_AGENT_LINUX_PERMISSION_PROFILE__,
    },
  });
  const runFile = path.join(result.run.paths.root, "run.json");
  const runBytes = await readBoundedRegularFile(runFile, 8 * 1024 * 1024);
  const evidenceBytes = await readBoundedRegularFile(
    result.evidence_manifest,
    8 * 1024 * 1024,
  );
  process.stdout.write(`${JSON.stringify({
    schema: "vela.agent-run-result.v1",
    ok: true,
    command: "run",
    effect: "none",
    authority: "none",
    attempt_id: result.attempt_id,
    request_root: result.request_root,
    target: result.target,
    execution_bundle_root: result.execution_bundle_root,
    source_state: result.source_state,
    run: {
      id: result.run.record.run_id,
      path: runFile,
      size: runBytes.length,
      sha256: sha256Bytes(runBytes),
    },
    evidence_manifest: {
      path: result.evidence_manifest,
      size: evidenceBytes.length,
      sha256: sha256Bytes(evidenceBytes),
      root: result.evidence_root,
    },
    candidate: {
      digest: result.run.record.candidate.digest,
      status: result.run.record.candidate.status,
    },
    verifier: {
      status: result.run.record.verifier.status,
    },
    reproduction: {
      matched: result.run.record.reproduction.matched,
    },
    usage: {
      observed_tokens: result.run.record.budget.observed_tokens,
    },
    submission: null,
  })}\n`);
}

async function doctor(helperBinary: string): Promise<void> {
  const build = await observedHelperBuild(helperBinary);
  const defaultOutput = path.join(os.homedir(), ".vela", "agent", "runs");
  assertNativeOutputPlacement(defaultOutput);
  const placement = {
    default_output: "local_user_home",
    suitable: nativeOutputPlacement(defaultOutput).suitable,
    system_temporary_output: "rejected",
  } as const;
  if (process.platform === "win32") {
    process.stdout.write(`${JSON.stringify({
      ok: true,
      command: "doctor",
      authority: "none",
      effect: "none",
      build,
      build_root: protocolDigest(build),
      custody: {
        preflight: "wsl2_required",
        mode: "not_applicable",
        placement,
      },
    })}\n`);
    return;
  }
  if (process.platform !== "darwin" && process.platform !== "linux") {
    throw new Error(`Vela Agent doctor does not support ${process.platform}`);
  }
  const diagnosticRoot = await mkdtemp(path.join(os.homedir(), ".vela-agent-doctor-"));
  try {
    const profile = path.join(diagnosticRoot, "native-worker.toml");
    const profileBytes = process.platform === "linux"
      ? __VELA_AGENT_LINUX_PERMISSION_PROFILE__
      : __VELA_AGENT_MACOS_PERMISSION_PROFILE__;
    await writeFile(profile, profileBytes, { flag: "wx", mode: 0o400 });
    const codex = await runtimeIdentity({
      name: "codex",
      cwd: diagnosticRoot,
      home: path.join(diagnosticRoot, "home"),
    });
    const custody = await runNativeCustodyPreflight({
      binary: codex.binary,
      permissionProfile: profile,
    });
    if (custody.codex_sha256 !== codex.sha256 || custody.codex_version !== codex.version) {
      throw new Error("Vela Agent doctor and custody preflight disagree on Codex identity");
    }
    process.stdout.write(`${JSON.stringify({
      ok: true,
      command: "doctor",
      authority: "none",
      effect: "none",
      build,
      build_root: protocolDigest(build),
      codex,
      custody: {
        preflight: "passed",
        mode: custody.mode,
        placement,
        permission_profile_sha256: custody.permission_profile_sha256,
        verdict: custody.verdict,
      },
    })}\n`);
  } finally {
    await rm(diagnosticRoot, { recursive: true, force: true });
  }
}

async function main(argv: string[]): Promise<void> {
  const helperBinary = fileURLToPath(import.meta.url);
  const [command, file, ...rest] = argv;
  if (command === "doctor" && (file === undefined || file === "--json") && rest.length === 0) {
    await doctor(helperBinary);
    return;
  }
  if (command === "run" && file === "--request-stdin" && rest.length === 0) {
    await run(await stdinJson(), helperBinary);
    return;
  }
  if (command === "show" && file !== undefined && rest.length === 0) {
    await show(file);
    return;
  }
  if (command === "replay" && file !== undefined && rest.length === 0) {
    process.stdout.write(`${JSON.stringify(await replayProduct(path.resolve(file)))}\n`);
    return;
  }
  if (command === "export" && file !== undefined) {
    const options = valueOptions(
      rest,
      ["--output", "--as", "--attempt", "--claim", "--scope-limit"],
    );
    const output = options.get("--output") ??
      path.join(path.dirname(path.dirname(path.resolve(file))), `submission-${Date.now()}`);
    const actor = options.get("--as");
    const attempt = options.get("--attempt");
    const correctedClaim = options.get("--claim");
    const scopeLimit = options.get("--scope-limit");
    process.stdout.write(`${JSON.stringify(await exportSubmission({
      runFile: path.resolve(file),
      outputRoot: path.resolve(output),
      ...(actor === undefined ? {} : { actor }),
      ...(attempt === undefined ? {} : { attempt }),
      ...(correctedClaim === undefined ? {} : { correctedClaim }),
      ...(scopeLimit === undefined ? {} : { scopeLimit }),
    }))}\n`);
    return;
  }
  throw new Error("the private Vela Agent helper accepts doctor, run, show, replay, or export");
}

main(process.argv.slice(2)).catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${JSON.stringify({ ok: false, error: message })}\n`);
  process.exitCode = 1;
});
