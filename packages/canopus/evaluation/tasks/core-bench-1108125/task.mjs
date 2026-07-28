import { createHash } from "node:crypto";

import { canonicalJson, digest } from "../../lib/evaluation-plan.mjs";

export const TASK_ID = "core-bench:capsule-1108125";
export const TASK_SCHEMA = "canopus.evaluation-task-packet.v1";
export const ARTIFACT_SCHEMA = "canopus.core-bench-1108125-result.v1";
export const VERIFIER_RESULT_SCHEMA = "canopus.evaluation-verifier-result.v1";
export const SOURCE_ARCHIVE_ROOT =
  "sha256:95240472124f26b33ab40a35dad435b27bc4b42f9b6dbc52d6d02248d72d8371";
export const SOURCE_URL =
  "https://corebench.cs.princeton.edu/capsules/capsule-1108125.tar.gz";
export const SOURCE_DOI = "https://doi.org/10.1016/j.landusepol.2019.05.010";
export const IMAGE =
  "registry.codeocean.com/published/1d48d413-6398-4952-9412-5074b5ebc096";
export const IMAGE_DIGEST =
  "sha256:503117b1e393779705fd34c2dbcabfb04fbd65d755887c13137566205418630a";
export const FIGURE_S5_ROOT =
  "sha256:07304f6bf71d8c2050373a6196cf1adcd7d8a46fcafe66682bed3c0986f60cbc";

export const SOURCE_FILES = [
  "code/LICENSE",
  "code/analysis.R",
  "code/readme.txt",
  "code/run",
  "code/update_from_v3.txt",
  "data/LICENSE",
  "data/exp1.txt",
  "data/exp2.txt",
  "data/payoffs.txt",
  "data/variables exp1.txt",
  "data/variables exp2.txt",
  "data/variables payoffs.txt",
  "data/variables villages.txt",
  "data/villages.txt",
];

function exactKeys(value, expected, at) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${at} must be an object`);
  }
  const keys = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (keys.length !== wanted.length || keys.some((key, index) => key !== wanted[index])) {
    throw new Error(`${at} must contain exactly ${wanted.join(", ")}`);
  }
}

function sha256(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function finite(value, at) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${at} must be a finite number`);
  }
  return value;
}

export function parseArtifact(value) {
  exactKeys(
    value,
    [
      "schema",
      "task_id",
      "forestgroup_mean",
      "gender_mean",
      "income_mean",
      "eigen_trend",
    ],
    "artifact",
  );
  if (value.schema !== ARTIFACT_SCHEMA) throw new Error("artifact.schema is unsupported");
  if (value.task_id !== TASK_ID) throw new Error("artifact.task_id is unsupported");
  if (!["decrease", "increase"].includes(value.eigen_trend)) {
    throw new Error("artifact.eigen_trend is unsupported");
  }
  return {
    schema: ARTIFACT_SCHEMA,
    task_id: TASK_ID,
    forestgroup_mean: finite(value.forestgroup_mean, "artifact.forestgroup_mean"),
    gender_mean: finite(value.gender_mean, "artifact.gender_mean"),
    income_mean: finite(value.income_mean, "artifact.income_mean"),
    eigen_trend: value.eigen_trend,
  };
}

export function assertSafeArchiveEntries(entries) {
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new Error("source archive has no entries");
  }
  for (const entry of entries) {
    if (
      typeof entry !== "string" ||
      entry.length === 0 ||
      entry.startsWith("/") ||
      entry.includes("\\") ||
      entry.split("/").some((part) => part === "..")
    ) {
      throw new Error(`source archive contains an unsafe path: ${String(entry)}`);
    }
    if (!entry.startsWith("capsule-1108125/")) {
      throw new Error(`source archive contains an unexpected root: ${entry}`);
    }
  }
}

export function buildPacket(files) {
  if (!(files instanceof Map)) throw new Error("task source files must be a Map");
  const missing = SOURCE_FILES.filter((file) => !files.has(file));
  const extras = [...files.keys()].filter((file) => !SOURCE_FILES.includes(file));
  if (missing.length > 0 || extras.length > 0) {
    throw new Error(
      `task source allowlist mismatch: missing=${missing.join(",") || "none"}; ` +
      `extra=${extras.join(",") || "none"}`,
    );
  }
  const projectedFiles = SOURCE_FILES.map((file) => {
    const bytes = files.get(file);
    if (!Buffer.isBuffer(bytes) || bytes.length === 0) {
      throw new Error(`task source ${file} must be nonempty bytes`);
    }
    let content;
    try {
      content = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    } catch (error) {
      throw new Error(`task source ${file} is not UTF-8: ${String(error)}`);
    }
    return {
      path: file,
      encoding: "utf8",
      sha256: sha256(bytes),
      content,
    };
  });
  return {
    schema: TASK_SCHEMA,
    task_id: TASK_ID,
    source: {
      corpus: "CORE-Bench",
      capsule_id: "capsule-1108125",
      archive_sha256: SOURCE_ARCHIVE_ROOT,
      source_url: SOURCE_URL,
      doi: SOURCE_DOI,
      code_license: "MIT",
      data_license: "CC0-1.0",
    },
    objective: [
      "Reproduce the registered analysis from the supplied code and data.",
      "Report the means of forestgroup, gender, and income.",
      "Report whether the eigenvalues of factors and components in the scree result decrease as their number increases.",
    ].join(" "),
    constraints: {
      network: "deny",
      cpu_only: true,
      precomputed_results: "not_provided",
      verifier: "not_exposed",
      authority: "none",
    },
    output: {
      path: "artifacts/result.json",
      schema: ARTIFACT_SCHEMA,
      fields: [
        "forestgroup_mean",
        "gender_mean",
        "income_mean",
        "eigen_trend",
      ],
    },
    files: projectedFiles,
  };
}

export function packetBytes(packet) {
  return Buffer.from(canonicalJson(packet));
}

function row(stdout, name, count) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
  const match = stdout.match(
    new RegExp(`^${escaped}\\s+${count}\\s+(-?\\d+(?:\\.\\d+)?)\\s`, "mu"),
  );
  if (match?.[1] === undefined) {
    throw new Error(`replay stdout is missing the ${name} summary row`);
  }
  return Number(match[1]);
}

export function parseReplayEvidence(stdout) {
  if (typeof stdout !== "string" || stdout.length === 0 || stdout.length > 2_000_000) {
    throw new Error("replay stdout is invalid");
  }
  const marker = stdout.match(/^CANOPUS_FIGURE_S5 (sha256:[0-9a-f]{64})$/mu);
  if (marker?.[1] === undefined) {
    throw new Error("replay stdout is missing the FigureS5 root marker");
  }
  return {
    forestgroup_mean: row(stdout, "forestgroup", 173),
    gender_mean: row(stdout, "gender", 173),
    income_mean: row(stdout, "income", 172),
    figure_s5_root: marker[1],
  };
}

export function verifyArtifactAgainstReplay(artifactValue, replay) {
  const artifact = parseArtifact(artifactValue);
  const expected = {
    forestgroup_mean: 0.34,
    gender_mean: 0.46,
    income_mean: 1,
    eigen_trend: "decrease",
  };
  for (const key of ["forestgroup_mean", "gender_mean", "income_mean"]) {
    if (replay[key] !== expected[key]) {
      throw new Error(`replay ${key} drifted: expected ${expected[key]}, observed ${replay[key]}`);
    }
    if (artifact[key] !== replay[key]) {
      throw new Error(`artifact ${key} does not match the replay`);
    }
  }
  if (replay.figure_s5_root !== FIGURE_S5_ROOT) {
    throw new Error(
      `replay FigureS5 root drifted: expected ${FIGURE_S5_ROOT}, ` +
      `observed ${replay.figure_s5_root}`,
    );
  }
  if (artifact.eigen_trend !== expected.eigen_trend) {
    throw new Error("artifact eigen_trend does not match the registered scree result");
  }
  return {
    artifact,
    expected,
  };
}

export function verifierRecord({ artifactBytes, stdout, stderr }) {
  const artifact = JSON.parse(artifactBytes.toString("utf8"));
  const replay = parseReplayEvidence(stdout.toString("utf8"));
  verifyArtifactAgainstReplay(artifact, replay);
  return {
    schema: VERIFIER_RESULT_SCHEMA,
    task_id: TASK_ID,
    verdict: "pass",
    authority: "none",
    source_root: SOURCE_ARCHIVE_ROOT,
    image_digest: IMAGE_DIGEST,
    artifact_root: sha256(artifactBytes),
    replay: {
      stdout_root: sha256(stdout),
      stderr_root: sha256(stderr),
      result: replay,
    },
  };
}

export function verifierRecordRoot(record) {
  return digest(record);
}
