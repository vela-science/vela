import assert from "node:assert/strict";
import { mkdir, mkdtemp } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  assertMissionNotCovered,
  findCoveredMission,
} from "../src/product/coverage.js";
import { writeCurrentRunFixture } from "./helpers/current-run-fixture.js";

const root = `sha256:${"a".repeat(64)}`;

test("a verifier-passing retained Run blocks the same exact mission", async () => {
  const home = await mkdtemp(path.join(os.tmpdir(), "canopus-coverage-"));
  const frontier = path.join(home, "frontier");
  const runs = path.join(home, "runs");
  await Promise.all([mkdir(frontier), mkdir(runs)]);
  const fixture = await writeCurrentRunFixture({
    root: path.join(runs, "first"),
    artifact: Buffer.from("{\"value\":42}\n"),
    velaVersion: "0.940.6",
    velaSha256: root,
    gitCommit: "b".repeat(40),
    gitTree: "c".repeat(40),
    roots: {
      git_commit: "b".repeat(40),
      git_tree: "c".repeat(40),
      vela_repository: root,
    },
    missionTarget: "erdos:1056",
    missionObjective:
      "Perform one finite exhaustive search for an Erdős 1056 k=15 witness over every prime p in the exact inclusive range 1..10. This bounded result addresses Erdős 1056 at k=15 only.",
  });
  assert.deepEqual(
    await findCoveredMission({
      missionId: fixture.mission.id,
      coverageKey: null,
      frontier,
      runsRoot: runs,
    }),
    {
      mission_id: fixture.mission.id,
      coverage_key: "erdos:1056:k=15:1..10",
      run_id: "run_export_fixture",
      run_file: fixture.runFile,
    },
  );
  await assert.rejects(
    assertMissionNotCovered({
      draft: fixture.mission,
      frontier,
      runsRoot: runs,
    }),
    /already covered by verifier-passing Run run_export_fixture.+first uncovered bounded range/su,
  );
});

test("a renamed Mission cannot repeat the same exact Erdős range", async () => {
  const home = await mkdtemp(path.join(os.tmpdir(), "canopus-coverage-range-"));
  const frontier = path.join(home, "frontier");
  const runs = path.join(home, "runs");
  await Promise.all([mkdir(frontier), mkdir(runs)]);
  const fixture = await writeCurrentRunFixture({
    root: path.join(runs, "first"),
    artifact: Buffer.from("{\"value\":42}\n"),
    velaVersion: "0.940.6",
    velaSha256: root,
    gitCommit: "b".repeat(40),
    gitTree: "c".repeat(40),
    roots: {
      git_commit: "b".repeat(40),
      git_tree: "c".repeat(40),
      vela_repository: root,
    },
    missionTarget: "erdos:1056",
    missionObjective:
      "Perform one finite exhaustive search for an Erdős 1056 k=15 witness over every prime p in the exact inclusive range 1..10. This bounded result addresses Erdős 1056 at k=15 only.",
  });
  await assert.rejects(
    assertMissionNotCovered({
      draft: {
        ...fixture.mission,
        id: "mission_renamed_but_same_range",
      },
      frontier,
      runsRoot: runs,
    }),
    /erdos:1056:k=15:1\.\.10/u,
  );
});

test("an uncovered mission remains runnable", async () => {
  const home = await mkdtemp(path.join(os.tmpdir(), "canopus-coverage-empty-"));
  const frontier = path.join(home, "frontier");
  const runs = path.join(home, "runs");
  await Promise.all([mkdir(frontier), mkdir(runs)]);
  await assert.doesNotReject(
    assertMissionNotCovered({
      draft: {
        id: "mission_uncovered_range",
        target: "erdos:1056",
        objective:
          "Perform one finite exhaustive search for an Erdős 1056 k=15 witness over every prime p in the exact inclusive range 11..20.",
      },
      frontier,
      runsRoot: runs,
    }),
  );
});
