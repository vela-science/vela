import { mkdtemp, realpath, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { BudgetTracker } from "../budget/enforce.js";
import { parseMission } from "../contracts/mission.js";
import { validateMissionBundle } from "../mission/prepare.js";
import { parseDiagnosticRunRecord } from "../projection/diagnostic.js";
import { parseCurrentRunRecord } from "../projection/current-run.js";
import { parseRunRecord } from "../projection/run.js";
import { contentDigest } from "../util/canonical.js";
import { readBoundedRegularFile } from "../util/files.js";
import { runVerifier } from "../verifier/run.js";
import { cleanupWorkspace, prepareWorkspace } from "../workspace/prepare.js";
import type { FrozenArtifactLocation } from "../artifact/freeze.js";
import type { FrozenArtifact } from "../contracts/candidate.js";

export async function replayProduct(runFile: string, dockerBinary = "docker"): Promise<{
  schema: "canopus.replay.v1";
  ok: true;
  run_id: string;
  mission_root: string;
  verifier_status: "passed" | "failed" | "error";
  stdout_digest: string;
  stderr_digest: string;
  matched: true;
}> {
  const absoluteRun = await realpath(runFile);
  const runRoot = path.dirname(absoluteRun);
  const raw = JSON.parse((await readBoundedRegularFile(absoluteRun, 8 * 1024 * 1024)).toString("utf8")) as unknown;
  const schema = typeof raw === "object" && raw !== null && !Array.isArray(raw)
    ? (raw as Record<string, unknown>).schema
    : undefined;
  const record = schema === "canopus.run.v2"
    ? parseCurrentRunRecord(raw)
    : schema === "canopus.diagnostic-run.v1"
      ? parseDiagnosticRunRecord(raw)
      : parseRunRecord(raw);
  const bundleRoot = await realpath(path.join(runRoot, "..", "mission"));
  const mission = parseMission(JSON.parse(
    (await readBoundedRegularFile(path.join(bundleRoot, "mission.json"), 8 * 1024 * 1024)).toString("utf8"),
  ) as unknown);
  if (mission.schema !== "canopus.mission.v1") throw new Error("product replay requires Mission v1");
  await validateMissionBundle(mission, bundleRoot);
  if (contentDigest(mission) !== record.mission.digest) throw new Error("run and mission roots disagree");
  const artifacts: FrozenArtifactLocation[] = record.candidate.artifacts.map((artifact: FrozenArtifact) => ({
    artifact,
    frozenPath: path.join(runRoot, "artifacts", artifact.digest.slice("sha256:".length)),
  }));
  const replayRoot = await mkdtemp(path.join(os.tmpdir(), "canopus-replay-root-"));
  await rm(replayRoot, { recursive: true, force: true });
  const source = path.join(runRoot, "input");
  const paths = await prepareWorkspace({
    sourceRepo: source,
    runRoot: replayRoot,
    gitCommit: mission.roots.git_commit,
    gitTree: mission.roots.git_tree,
  });
  try {
    const verifier = await runVerifier({
      mission,
      paths,
      artifacts,
      budget: new BudgetTracker(mission.budgets),
      bundleRoot,
      dockerBinary,
    });
    if (
      verifier.status !== record.verifier.status ||
      verifier.record.stdout_digest !== record.verifier.record.stdout_digest ||
      verifier.record.stderr_digest !== record.verifier.record.stderr_digest
    ) {
      throw new Error("verifier replay does not match the frozen run record");
    }
    return {
      schema: "canopus.replay.v1",
      ok: true,
      run_id: record.run_id,
      mission_root: record.mission.digest,
      verifier_status: verifier.status,
      stdout_digest: verifier.record.stdout_digest,
      stderr_digest: verifier.record.stderr_digest,
      matched: true,
    };
  } finally {
    await cleanupWorkspace(paths);
  }
}
