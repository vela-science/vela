import assert from "node:assert/strict";
import {
  mkdtemp,
  readFile,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import type { CanopusCurrentRunResult } from "../src/run.js";
import { writeEvidenceManifest } from "../src/product/run.js";
import {
  contentDigest,
  protocolDigest,
} from "../src/util/canonical.js";

const vectorName = "Canopus run evidence manifest uses the Vela protocol root";

interface CanonicalVector {
  name: string;
  input: Record<string, unknown>;
  sha256: string;
}

test("evidence manifest root matches the Rust Vela canonical-root vector", async () => {
  const vectorsFile = fileURLToPath(
    new URL("../../../../conformance/canonical-hashing.json", import.meta.url),
  );
  const vectorsDocument = JSON.parse(await readFile(vectorsFile, "utf8")) as {
    vectors: CanonicalVector[];
  };
  const vector = vectorsDocument.vectors.find(({ name }) => name === vectorName);
  assert.notEqual(vector, undefined, `missing conformance vector: ${vectorName}`);

  const root = await mkdtemp(path.join(os.tmpdir(), "canopus-evidence-root-"));
  const files = {
    "activity.jsonl": "activity fixture\n",
    "worker-final.json": "transcript fixture\n",
    "worker-events.jsonl": "tool trace fixture\n",
    "worker-stderr.bin": "",
    "engine-result.json": "engine result fixture\n",
    "candidate.json": "candidate fixture\n",
    "run.json": "run fixture\n",
  };
  await Promise.all(
    Object.entries(files).map(async ([relative, value]) =>
      writeFile(path.join(root, relative), value),
    ),
  );

  const finalRoots =
    vector!.input.final_roots as CanopusCurrentRunResult["record"]["mission"]["starting_roots"];
  const verifier: CanopusCurrentRunResult["record"]["verifier"] = {
    status: "passed",
    sandbox: "macos_sandbox",
    record: {
      argv: ["capsule/verifier", "result.json"],
      executable_digest: `sha256:${"c".repeat(64)}`,
      exit_code: 0,
      stdout_digest: `sha256:${"d".repeat(64)}`,
      stderr_digest: `sha256:${"e".repeat(64)}`,
      duration_ms: 7,
    },
  };
  const run: CanopusCurrentRunResult = {
    record: {
      schema: "canopus.run.v2",
      run_id: "run_evidence_root_interop_fixture",
      status: "completed",
      effect: "none",
      authority: "non_authoritative",
      external_gate_credit: false,
      mission: {
        id: "mission_evidence_root_interop_fixture",
        target: "erdos:1056",
        digest: `sha256:${"a".repeat(64)}`,
        starting_roots: finalRoots,
      },
      candidate: {
        digest: `sha256:${"f".repeat(64)}`,
        status: "success",
        claim: "Fixture claim.",
        artifacts: [
          {
            path: "second.json",
            kind: "witness",
            digest: `sha256:${"2".repeat(64)}`,
            bytes: 2,
          },
          {
            path: "first.json",
            kind: "witness",
            digest: `sha256:${"1".repeat(64)}`,
            bytes: 1,
          },
        ],
        caveats: ["Fixture only."],
      },
      verifier,
      submission: null,
      reproduction: {
        matched: true,
        roots: finalRoots,
        verifier_status: "passed",
        stdout_digest: `sha256:${"d".repeat(64)}`,
        stderr_digest: `sha256:${"e".repeat(64)}`,
      },
      budget: {
        research_elapsed_ms: 1,
        research_processes: 1,
        research_output_bytes: 1,
        prompt_bytes: 1,
        artifact_bytes: 1,
        attempts: 1,
        observed_tokens: 1,
      },
    },
    projection: {
      schema: "canopus.run-projection.v2",
      authority: "read_only_projection",
      run_id: "run_evidence_root_interop_fixture",
      target: "erdos:1056",
      candidate_digest: `sha256:${"f".repeat(64)}`,
      verifier_status: "passed",
      submitted: false,
      clean_clone_reproduced: true,
    },
    paths: {
      root,
      input: path.join(root, "input"),
      frontier: path.join(root, "frontier"),
      work: path.join(root, "work"),
      output: path.join(root, "output"),
      artifacts: path.join(root, "artifacts"),
      home: path.join(root, "home"),
      velaHome: path.join(root, "vela-home"),
      verifierHome: path.join(root, "verifier-home"),
    },
  };

  const result = await writeEvidenceManifest(
    run,
    `sha256:${"a".repeat(64)}`,
  );
  const manifest = JSON.parse(await readFile(result.file, "utf8")) as Record<string, unknown>;

  assert.deepEqual(manifest, vector!.input);
  assert.equal(result.root, `sha256:${vector!.sha256}`);
  assert.equal(result.root, protocolDigest(manifest));
  assert.notEqual(result.root, contentDigest(manifest));
});
