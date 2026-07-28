import { lstat, readdir } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { objectAt, stringAt } from "../contracts/validation.js";
import { parseRetainedRunRecord } from "../projection/retained-run.js";
import { contentDigest } from "../util/canonical.js";
import { readBoundedRegularFile } from "../util/files.js";

const MAX_RUN_DIRECTORIES = 4096;

export interface CoveredMission {
  mission_id: string;
  coverage_key?: string;
  run_id: string;
  run_file: string;
}

function consistentMatch(value: string, expression: RegExp): RegExpMatchArray | null {
  const matches = [...value.matchAll(expression)];
  if (matches.length === 0) return null;
  const expected = matches[0]!.slice(1);
  return matches.every((match) =>
    match.slice(1).length === expected.length &&
    match.slice(1).every((entry, index) => entry === expected[index]))
    ? matches[0]!
    : null;
}

function erdosCoverageKey(value: Record<string, unknown>, context: string): string | null {
  const target = typeof value.target === "string" ? value.target : "";
  const objective = typeof value.objective === "string" ? value.objective : "";
  const targetMatch = target.match(/^erdos:(\d+)$/u);
  if (targetMatch === null) return null;
  const problemMatch = consistentMatch(objective, /Erdős\s+(\d+)/gu);
  const kMatch = consistentMatch(objective, /\bk=(\d+)\b/gu);
  const rangeMatch = consistentMatch(objective, /\binclusive range\s+(\d+)\.\.(\d+)\b/gu);
  if (problemMatch === null || kMatch === null || rangeMatch === null) {
    throw new Error(`${context} does not expose one exact Erdős problem, k, and inclusive range`);
  }
  if (problemMatch[1] !== targetMatch[1]) {
    throw new Error(`${context} target and objective disagree on the Erdős problem`);
  }
  const start = BigInt(rangeMatch[1]!);
  const end = BigInt(rangeMatch[2]!);
  if (start > end) throw new Error(`${context} inclusive range is reversed`);
  return `erdos:${targetMatch[1]}:k=${kMatch[1]}:${start}..${end}`;
}

async function retainedMissionForRun(
  runFile: string,
  expected: { id: string; target: string; digest: string },
): Promise<Record<string, unknown>> {
  const missionFile = path.join(path.dirname(runFile), "..", "mission", "mission.json");
  const mission = objectAt(
    JSON.parse(
      (await readBoundedRegularFile(missionFile, 8 * 1024 * 1024)).toString("utf8"),
    ) as unknown,
    `retained Mission ${missionFile}`,
  );
  if (contentDigest(mission) !== expected.digest) {
    throw new Error(`retained Run ${runFile} and Mission ${missionFile} roots disagree`);
  }
  if (
    stringAt(mission.id, `retained Mission ${missionFile}.id`, { min: 1, max: 134 }) !==
      expected.id ||
    stringAt(mission.target, `retained Mission ${missionFile}.target`, { min: 1, max: 256 }) !==
      expected.target
  ) {
    throw new Error(`retained Run ${runFile} and Mission ${missionFile} identities disagree`);
  }
  return mission;
}

async function regularFileIfPresent(candidate: string): Promise<string | null> {
  try {
    const metadata = await lstat(candidate);
    return metadata.isFile() && !metadata.isSymbolicLink() ? candidate : null;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw error;
  }
}

async function currentRunFiles(root: string): Promise<string[]> {
  let entries;
  try {
    entries = await readdir(root, { withFileTypes: true });
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw error;
  }
  if (entries.length > MAX_RUN_DIRECTORIES) {
    throw new Error(
      `retained Run coverage index exceeds ${MAX_RUN_DIRECTORIES} entries; ` +
      "archive obsolete generated output before running another mission",
    );
  }
  const files: string[] = [];
  for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
    if (!entry.isDirectory() || entry.isSymbolicLink()) continue;
    const runRoot = path.join(root, entry.name);
    const candidates = [
      path.join(runRoot, "run.json"),
      path.join(runRoot, "run", "run.json"),
    ];
    for (const candidate of candidates) {
      const runFile = await regularFileIfPresent(candidate);
      if (runFile !== null) files.push(runFile);
    }
  }
  return files;
}

export async function findCoveredMission(options: {
  missionId: string;
  coverageKey?: string | null;
  frontier: string;
  runsRoot?: string;
}): Promise<CoveredMission | null> {
  const root = options.runsRoot ??
    path.join(os.homedir(), ".canopus", "runs", path.basename(path.resolve(options.frontier)));
  for (const runFile of await currentRunFiles(root)) {
    const value = JSON.parse(
      (await readBoundedRegularFile(runFile, 8 * 1024 * 1024)).toString("utf8"),
    ) as unknown;
    const object = objectAt(value, `retained Run ${runFile}`);
    if (object.schema !== "canopus.run.v2") continue;
    const run = parseRetainedRunRecord(value).record;
    const mission = await retainedMissionForRun(runFile, run.mission);
    const coverageKey = erdosCoverageKey(
      mission,
      `retained Run ${runFile} Mission`,
    );
    if (
      run.mission.id === options.missionId ||
      (options.coverageKey !== null &&
        options.coverageKey !== undefined &&
        coverageKey === options.coverageKey)
    ) {
      return {
        mission_id: run.mission.id,
        ...(coverageKey === null ? {} : { coverage_key: coverageKey }),
        run_id: run.run_id,
        run_file: runFile,
      };
    }
  }
  return null;
}

export async function assertMissionNotCovered(options: {
  draft: unknown;
  frontier: string;
  runsRoot?: string;
}): Promise<void> {
  const draft = objectAt(options.draft, "profile draft");
  const missionId = stringAt(draft.id, "profile draft.id", { min: 1, max: 134 });
  const coverageKey = erdosCoverageKey(draft, "profile draft");
  const covered = await findCoveredMission({
    missionId,
    coverageKey,
    frontier: options.frontier,
    ...(options.runsRoot === undefined ? {} : { runsRoot: options.runsRoot }),
  });
  if (covered !== null) {
    throw new Error(
      `mission ${covered.mission_id} is already covered by verifier-passing Run ` +
      `${covered.run_id} at ${covered.run_file}` +
      `${covered.coverage_key === undefined ? "" : ` (${covered.coverage_key})`}; ` +
      "freeze the first uncovered bounded range instead of repeating it",
    );
  }
}
