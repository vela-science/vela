import { CANDIDATE_STATUSES } from "../contracts/candidate.js";
import {
  arrayAt,
  enumAt,
  exactKeys,
  objectAt,
  relativePathAt,
  stringAt,
} from "../contracts/validation.js";
import {
  ENGINE_OUTPUT_SCHEMA,
  MAX_ENGINE_ARTIFACTS,
  MAX_ENGINE_CAVEATS,
  MAX_ENGINE_OBSERVATIONS,
  type CandidateDraft,
  type DraftArtifact,
} from "../engines/engine.js";

function parseDraftArtifact(value: unknown, at: string): DraftArtifact {
  const object = objectAt(value, at);
  exactKeys(object, ["path", "kind", "encoding", "content"], [], at);
  return {
    path: relativePathAt(object.path, `${at}.path`),
    kind: stringAt(object.kind, `${at}.kind`, { min: 1, max: 128 }),
    encoding: enumAt(object.encoding, `${at}.encoding`, ["utf8"] as const),
    content: stringAt(object.content, `${at}.content`, { max: 1_048_576 }),
  };
}

export function parseCandidateDraft(value: unknown): CandidateDraft {
  const object = objectAt(value, "engine_output");
  exactKeys(
    object,
    ["schema", "status", "claim", "artifacts", "observations", "caveats"],
    [],
    "engine_output",
  );
  return {
    schema: enumAt(object.schema, "engine_output.schema", [ENGINE_OUTPUT_SCHEMA] as const),
    status: enumAt(object.status, "engine_output.status", CANDIDATE_STATUSES),
    claim: stringAt(object.claim, "engine_output.claim", { min: 1, max: 8192 }),
    artifacts: arrayAt(
      object.artifacts,
      "engine_output.artifacts",
      { max: MAX_ENGINE_ARTIFACTS },
      parseDraftArtifact,
    ),
    observations: arrayAt(
      object.observations,
      "engine_output.observations",
      { max: MAX_ENGINE_OBSERVATIONS },
      (item, at) => stringAt(item, at, { min: 1, max: 4096 }),
    ),
    caveats: arrayAt(object.caveats, "engine_output.caveats", { max: MAX_ENGINE_CAVEATS }, (item, at) =>
      stringAt(item, at, { min: 1, max: 4096 }),
    ),
  };
}

export function assertDraftArtifactsAllowed(
  draft: CandidateDraft,
  allowedPaths: readonly string[],
): void {
  for (const artifact of draft.artifacts) {
    const allowed = allowedPaths.some(
      (entry) => artifact.path === entry || artifact.path.startsWith(`${entry}/`),
    );
    if (!allowed) throw new Error(`engine declared non-allowlisted artifact ${artifact.path}`);
  }
}
