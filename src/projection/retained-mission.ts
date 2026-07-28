import { parseMission, type MissionV1 } from "../contracts/mission.js";
import {
  arrayAt,
  enumAt,
  exactKeys,
  integerAt,
  objectAt,
  sha256At,
  stringAt,
} from "../contracts/validation.js";
import { canonicalJcs, canonicalJson, sha256Bytes } from "../util/canonical.js";
import { normalizeRetainedRunRoots } from "./retained-run.js";

export function parseRetainedMission(value: unknown): {
  mission: MissionV1;
  exactRoot: string;
  exactRoots: string;
} {
  const raw = objectAt(value, "retained Mission");
  const roots = objectAt(raw.roots, "retained Mission.roots");
  if (raw.strict_baseline !== undefined) {
    const baseline = objectAt(raw.strict_baseline, "retained Mission.strict_baseline");
    exactKeys(
      baseline,
      ["status", "blocker_count", "blockers_root", "rule_counts"],
      [],
      "retained Mission.strict_baseline",
    );
    enumAt(
      baseline.status,
      "retained Mission.strict_baseline.status",
      ["pass", "fail"] as const,
    );
    integerAt(
      baseline.blocker_count,
      "retained Mission.strict_baseline.blocker_count",
      0,
      1_000_000,
    );
    sha256At(
      baseline.blockers_root,
      "retained Mission.strict_baseline.blockers_root",
    );
    arrayAt(
      baseline.rule_counts,
      "retained Mission.strict_baseline.rule_counts",
      { min: 0, max: 256 },
      (item, at) => {
        const count = objectAt(item, at);
        exactKeys(count, ["count", "rule"], [], at);
        integerAt(count.count, `${at}.count`, 0, 1_000_000);
        stringAt(count.rule, `${at}.rule`, {
          min: 1,
          max: 128,
          pattern: /^[a-z][a-z0-9_]*$/u,
        });
        return true;
      },
    );
  }
  const parseable = { ...raw };
  delete parseable.strict_baseline;
  const normalized = {
    ...parseable,
    roots: normalizeRetainedRunRoots(roots, "retained Mission.roots"),
  };
  const mission = parseMission(normalized);
  if (mission.schema !== "canopus.mission.v1") {
    throw new Error("current reader requires canopus.mission.v1");
  }
  return {
    mission,
    exactRoot: sha256Bytes(canonicalJson(raw)),
    exactRoots: canonicalJcs(roots),
  };
}
