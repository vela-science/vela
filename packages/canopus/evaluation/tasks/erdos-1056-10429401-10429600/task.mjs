import { createHash } from "node:crypto";

import { canonicalJson } from "../../lib/evaluation-plan.mjs";

export const TASK_ID = "erdos:1056:10429401-10429600";
export const TASK_SCHEMA = "canopus.evaluation-task-packet.v1";
export const SOURCE_PACKET_ROOT =
  "sha256:517c16cc9c59d7f91aeaea4287e0ce49000c7545199e86ea632c0a2e91faf30b";
export const VERIFIER_SOURCE_ROOT =
  "sha256:adc5482e5809e78aa35eec705cb68a0f9dbcb4c3269ea3e36666ce335b3a1732";
export const VERIFIER_COMPILER_ROOT =
  "sha256:b70a2d4da3aa934c1276a038bd69d7041dc5f94e4090e5432500422c59f53b6f";
export const VERIFIER_ROOT =
  "sha256:70baf5baaa44a5a955aea189b0c1393dca11d0731ca32537366460c697e5a255";
export const RANGE_START = 10_429_401;
export const RANGE_END = 10_429_600;
export const ARTIFACT_PATH =
  "artifacts/erdos1056-k15-range-10429401-10429600.txt";

function sha256(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

export function buildPacket(sourceBytes) {
  if (!Buffer.isBuffer(sourceBytes) || sourceBytes.length === 0 || sourceBytes.length > 1_048_576) {
    throw new Error("Erdős source packet violates its byte contract");
  }
  const sourceRoot = sha256(sourceBytes);
  if (sourceRoot !== SOURCE_PACKET_ROOT) {
    throw new Error(
      `Erdős source packet root drifted: expected ${SOURCE_PACKET_ROOT}, observed ${sourceRoot}`,
    );
  }
  let source;
  try {
    source = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(sourceBytes));
  } catch (error) {
    throw new Error(`Erdős source packet is not canonical UTF-8 JSON: ${String(error)}`);
  }
  if (source === null || typeof source !== "object" || Array.isArray(source)) {
    throw new Error("Erdős source packet must be an object");
  }
  return {
    schema: TASK_SCHEMA,
    task_id: TASK_ID,
    source: {
      frontier: "erdos",
      target_id: "erdos:1056",
      packet_schema: "erdos-frontier.problem-work.v1",
      packet_root: SOURCE_PACKET_ROOT,
      packet: source,
    },
    objective: [
      `Exhaustively search every prime in the exact inclusive range ${RANGE_START}..${RANGE_END}.`,
      "For each prime, compute t! modulo p for every cut t in 0..p-1 and find the largest residue fiber.",
      "Stop at the first prime with at least 16 cuts in one fiber; otherwise report the greatest multiplicity across the complete range.",
      "Break ties by the earliest prime and then the smallest residue, and list every increasing cut in the selected fiber.",
    ].join(" "),
    constraints: {
      network: "deny",
      cpu_only: true,
      bounded_result_only: true,
      authority: "none",
      verifier: "not_exposed",
    },
    output: {
      path: ARTIFACT_PATH,
      encoding: "utf8",
      exact_lines: [
        "schema=canopus.erdos1056-k15-search.v1",
        "status=<witness|negative>",
        "problem=1056",
        "k=15",
        `range_start=${RANGE_START}`,
        `range_end=${RANGE_END}`,
        "primes_tested=<nonnegative integer>",
        "max_multiplicity=<nonnegative integer>",
        "best_p=<prime>",
        "best_residue=<residue>",
        "cuts=<comma-separated increasing cuts>",
      ],
      final_newline: true,
    },
    caveat:
      "A bounded negative result applies only to this exact range, algorithm, artifact, and verifier. It does not establish universal nonexistence or resolve Erdős problem 1056.",
  };
}

export function packetBytes(packet) {
  return Buffer.from(canonicalJson(packet));
}

