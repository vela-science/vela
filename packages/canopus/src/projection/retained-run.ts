import {
  exactKeys,
  gitObjectAt,
  objectAt,
  sha256At,
} from "../contracts/validation.js";
import { canonicalJcs, canonicalJson, sha256Bytes } from "../util/canonical.js";
import { parseCurrentRunRecord } from "./current-run.js";

export function normalizeRetainedRunRoots(
  value: unknown,
  at: string,
): Record<string, string> {
  const roots = objectAt(value, at);
  if (roots.vela_repository !== undefined) {
    exactKeys(roots, ["git_commit", "git_tree", "vela_repository"], [], at);
    return {
      git_commit: gitObjectAt(roots.git_commit, `${at}.git_commit`),
      git_tree: gitObjectAt(roots.git_tree, `${at}.git_tree`),
      vela_repository: sha256At(roots.vela_repository, `${at}.vela_repository`),
    };
  }
  exactKeys(
    roots,
    ["git_commit", "git_tree", "vela_event_log", "vela_snapshot"],
    [],
    at,
  );
  // Current writers emit vela_repository. Retained Run v2 bytes from the
  // predecessor era instead bind event-log and snapshot roots. Readers verify
  // those immutable roots and normalize only an in-memory copy.
  sha256At(roots.vela_event_log, `${at}.vela_event_log`);
  return {
    git_commit: gitObjectAt(roots.git_commit, `${at}.git_commit`),
    git_tree: gitObjectAt(roots.git_tree, `${at}.git_tree`),
    vela_repository: sha256At(roots.vela_snapshot, `${at}.vela_snapshot`),
  };
}

export function parseRetainedRunRecord(value: unknown): {
  record: ReturnType<typeof parseCurrentRunRecord>;
  exactRoot: string;
  exactStartingRoots: string;
} {
  const raw = objectAt(value, "retained Run");
  const mission = objectAt(raw.mission, "retained Run.mission");
  const reproduction = objectAt(raw.reproduction, "retained Run.reproduction");
  const startingRoots = objectAt(
    mission.starting_roots,
    "retained Run.mission.starting_roots",
  );
  const reproductionRoots = objectAt(
    reproduction.roots,
    "retained Run.reproduction.roots",
  );
  if (canonicalJcs(startingRoots) !== canonicalJcs(reproductionRoots)) {
    throw new Error("retained Run starting and reproduction roots disagree");
  }
  const normalized = {
    ...raw,
    mission: {
      ...mission,
      starting_roots: normalizeRetainedRunRoots(
        startingRoots,
        "retained Run.mission.starting_roots",
      ),
    },
    reproduction: {
      ...reproduction,
      roots: normalizeRetainedRunRoots(
        reproductionRoots,
        "retained Run.reproduction.roots",
      ),
    },
  };
  return {
    record: parseCurrentRunRecord(normalized),
    exactRoot: sha256Bytes(canonicalJson(raw)),
    exactStartingRoots: canonicalJcs(startingRoots),
  };
}
