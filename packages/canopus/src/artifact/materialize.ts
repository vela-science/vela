import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

import type { CandidateDraft } from "../engines/engine.js";

function below(root: string, candidate: string): boolean {
  const relative = path.relative(root, candidate);
  return (
    relative !== "" &&
    relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative)
  );
}

export async function materializeDraftArtifacts(options: {
  draft: CandidateDraft;
  outputRoot: string;
  maxTotalBytes: number;
}): Promise<void> {
  const root = path.resolve(options.outputRoot);
  const seen = new Set<string>();
  let total = 0;
  for (const artifact of options.draft.artifacts) {
    if (seen.has(artifact.path)) throw new Error(`duplicate engine artifact ${artifact.path}`);
    seen.add(artifact.path);
    const target = path.resolve(root, artifact.path);
    if (!below(root, target)) throw new Error(`engine artifact escapes output root: ${artifact.path}`);
    const bytes = Buffer.from(artifact.content, "utf8");
    total += bytes.length;
    if (total > options.maxTotalBytes) {
      throw new Error(`inline engine artifacts exceed ${options.maxTotalBytes} bytes`);
    }
    await mkdir(path.dirname(target), { recursive: true, mode: 0o700 });
    await writeFile(target, bytes, { flag: "wx", mode: 0o600 });
  }
}
