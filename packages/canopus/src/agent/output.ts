import { lstat, mkdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { assertNativeOutputPlacement } from "../product/custody.js";
import type { CanopusCurrentRunResult } from "../run.js";
import {
  canonicalJson,
  contentDigest,
  protocolDigest,
  sha256Bytes,
} from "../util/canonical.js";
import { readBoundedRegularFile } from "../util/files.js";

export async function assertFreshOutput(outputRoot: string, sourceRoot: string): Promise<void> {
  const output = path.resolve(outputRoot);
  assertNativeOutputPlacement(output);
  const cloudBackedRoots = [
    path.join(os.homedir(), "Desktop"),
    path.join(os.homedir(), "Library", "Mobile Documents"),
    path.join(os.homedir(), "Library", "CloudStorage"),
  ];
  if (cloudBackedRoots.some((root) => output === root || output.startsWith(`${root}${path.sep}`))) {
    throw new Error(
      "Vela Agent output must use a local path; cloud-synced paths can stall Docker verifier bind mounts. Use ~/.vela/agent or another local directory.",
    );
  }
  const relative = path.relative(sourceRoot, output);
  if (relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== "..")) {
    throw new Error("Vela Agent output must be outside the source frontier");
  }
  try {
    await lstat(output);
    throw new Error("Vela Agent output root already exists");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
  await mkdir(output, { recursive: true, mode: 0o700 });
}

export async function writeEvidenceManifest(
  run: CanopusCurrentRunResult,
  missionDigest: string,
  execution?: {
    attempt_id: string;
    attempt_authorization_root: string;
    task_contract_root: string;
    target_binding_root: string;
    target_packet_root: string;
    execution_bundle_root: string;
    runner_build_root: string;
    request_root: string;
  },
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
    digests[name] = sha256Bytes(
      await readBoundedRegularFile(path.join(root, relative), 64 * 1024 * 1024),
    );
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
    submission_root: null,
    final_roots: run.record.mission.starting_roots,
    ...(execution === undefined ? {} : { execution }),
  };
  const file = path.join(root, "evidence-manifest.json");
  await writeFile(file, canonicalJson(manifest), { flag: "wx", mode: 0o600 });
  return { file, root: protocolDigest(manifest) };
}
