import { createHash } from "node:crypto";

import { canonicalJson } from "../../lib/evaluation-plan.mjs";

export const TASK_ID = "erdos:1056:10429801-10430000";
export const TASK_SCHEMA = "canopus.evaluation-task-packet.v1";
export const CAPSULE_SCHEMA = "canopus.evaluation-verifier-capsule.v1";

export const FRONTIER_ID = "vfr_0a25edabc16db143";
export const SOURCE_REPOSITORY =
  "https://github.com/vela-science/erdos-frontier.git";
export const SOURCE_CHECKOUT_COMMIT =
  "e4acfab64e5e248dcf3ba029558027fab40579f1";
export const SOURCE_CHECKOUT_TREE =
  "72bae6eba9ad491b7c6c0beca3c28db263709838";
export const SOURCE_REPOSITORY_ROOT =
  "sha256:1964e610500eed9e9b916b0d3433e73401a2b33ae7657dde0f0a949235603caf";
export const REPOSITORY_INDEX_PATH = ".vela/repository.json";

export const TARGET_INDEX_PATH = "targets.json";
export const TARGET_INDEX_FILE_ROOT =
  "sha256:622516ba34407e3c09667ef58f35e34b493c016fb6e602065961de2ab53f8e55";
export const TARGET_INDEX_ROOT =
  "sha256:b321e9ee002040a7ecfd1f54481effd0d64d2599a47892b17df253ada600103f";
export const TARGET_INDEX_SOURCE_COMMIT =
  "3b69575ea353f40d6c57bd375ef30dea4afc47d9";
export const TARGET_INDEX_SOURCE_TREE =
  "14ef138cbce1afda0ee45fdeda73d4b2bceb6403";

export const SOURCE_PACKET_PATH = "targets/erdos-1056.json";
export const SOURCE_PACKET_SCHEMA = "erdos-frontier.problem-work.v2";
export const SOURCE_PACKET_ROOT =
  "sha256:c2b57075a0ec205b4d837382f8b816f31ab20a485e40962a1768e7ed42565344";
export const SOURCE_PACKET_SIZE = 4_408;
export const SOURCE_PACKET_COMMIT =
  "af70d2cc1e9b98ee705e757fa773101ecfc17a01";
export const SOURCE_PACKET_TREE =
  "a0d730fa348b32433bfb3602156e1bea088b9d51";

export const RANGE_START = 10_429_801;
export const RANGE_END = 10_430_000;
export const ACCEPTED_COVERAGE_END = 10_429_600;
export const PENDING_PRODUCER_COVERAGE_END = 10_429_800;
export const ARTIFACT_PATH =
  "artifacts/erdos1056-k15-range-10429801-10430000.txt";

export const VERIFIER_SOURCE_PATH =
  "capsules/erdos1056-k15/verifier.cpp";
export const VERIFIER_SOURCE_ROOT =
  "sha256:adc5482e5809e78aa35eec705cb68a0f9dbcb4c3269ea3e36666ce335b3a1732";
export const VERIFIER_BINARY_ROOT =
  "sha256:c07af92ea296f2e48d97a9aa67b3873090f7c7fc47fde78cb9a774a97df35dca";
export const VERIFIER_IMAGE =
  "registry.codeocean.com/published/1d48d413-6398-4952-9412-5074b5ebc096";
export const VERIFIER_IMAGE_DIGEST =
  "sha256:503117b1e393779705fd34c2dbcabfb04fbd65d755887c13137566205418630a";
export const DOCKER_ROOT =
  "sha256:6f56a151c37ea0e848b3abde7770ad408babef7a56c8f2ec6230fcd582ecdc7e";

export const TASK_PACKET_ROOT =
  "sha256:7ce2feb39a8052dc9ba24cc6e73b308a00cda704f787a637e5190057bc0277fb";
export const CAPSULE_MANIFEST_ROOT =
  "sha256:be368af03ec5a0c4de60777514fa59814d983525dd68c59abc1c2b60fc6d6137";

export function sha256(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function object(value, at) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${at} must be an object`);
  }
  return value;
}

function equal(observed, expected, at) {
  if (observed !== expected) {
    throw new Error(`${at} drifted: expected ${String(expected)}, observed ${String(observed)}`);
  }
}

function parseRootedJson(bytes, expectedRoot, maxBytes, at) {
  if (!Buffer.isBuffer(bytes) || bytes.length === 0 || bytes.length > maxBytes) {
    throw new Error(`${at} violates its byte contract`);
  }
  const observedRoot = sha256(bytes);
  if (observedRoot !== expectedRoot) {
    throw new Error(
      `${at} root drifted: expected ${expectedRoot}, observed ${observedRoot}`,
    );
  }
  try {
    return object(
      JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)),
      at,
    );
  } catch (error) {
    throw new Error(`${at} is not canonical UTF-8 JSON: ${String(error)}`);
  }
}

export function buildPacket({
  repositoryIndexBytes,
  targetIndexBytes,
  targetPacketBytes,
}) {
  const repositoryIndex = parseRootedJson(
    repositoryIndexBytes,
    SOURCE_REPOSITORY_ROOT,
    2 * 1024 * 1024,
    "Erdős repository index",
  );
  const targetIndex = parseRootedJson(
    targetIndexBytes,
    TARGET_INDEX_FILE_ROOT,
    64 * 1024,
    "Erdős Target Index",
  );
  const targetPacket = parseRootedJson(
    targetPacketBytes,
    SOURCE_PACKET_ROOT,
    64 * 1024,
    "Erdős source packet",
  );

  equal(repositoryIndex.schema, "vela.repository.v3", "repository index schema");
  equal(repositoryIndex.frontier_id, FRONTIER_ID, "repository frontier");

  equal(targetIndex.schema, "vela.target-index.v4", "Target Index schema");
  equal(targetIndex.index_root, TARGET_INDEX_ROOT, "Target Index root");
  equal(
    object(targetIndex.source, "Target Index source").git_commit,
    TARGET_INDEX_SOURCE_COMMIT,
    "Target Index source commit",
  );
  equal(
    targetIndex.source.git_tree,
    TARGET_INDEX_SOURCE_TREE,
    "Target Index source tree",
  );
  equal(
    object(targetIndex.repository, "Target Index repository").repository_root,
    SOURCE_REPOSITORY_ROOT,
    "Target Index repository root",
  );
  if (!Array.isArray(targetIndex.targets) || targetIndex.targets.length !== 1) {
    throw new Error("Target Index must contain exactly one current target");
  }
  const indexedTarget = object(targetIndex.targets[0], "Target Index target");
  equal(indexedTarget.id, "erdos:1056", "Target Index target id");
  equal(indexedTarget.rank, 1, "Target Index target rank");
  equal(indexedTarget.state, "open", "Target Index target state");
  const indexedPacket = object(indexedTarget.packet, "Target Index target packet");
  equal(indexedPacket.path, SOURCE_PACKET_PATH, "Target Index packet path");
  equal(indexedPacket.schema, SOURCE_PACKET_SCHEMA, "Target Index packet schema");
  equal(indexedPacket.sha256, SOURCE_PACKET_ROOT, "Target Index packet root");
  equal(indexedPacket.size, SOURCE_PACKET_SIZE, "Target Index packet size");

  equal(targetPacket.schema, SOURCE_PACKET_SCHEMA, "source packet schema");
  equal(targetPacket.frontier_id, FRONTIER_ID, "source packet frontier");
  const packetRepository = object(
    targetPacket.repository,
    "source packet repository",
  );
  equal(packetRepository.commit, SOURCE_PACKET_COMMIT, "source packet commit");
  equal(packetRepository.tree, SOURCE_PACKET_TREE, "source packet tree");
  equal(packetRepository.root, SOURCE_REPOSITORY_ROOT, "source packet repository root");
  const packetTarget = object(targetPacket.target, "source packet target");
  equal(packetTarget.id, "erdos:1056", "source packet target id");
  equal(packetTarget.problem, 1056, "source packet problem");
  equal(packetTarget.state, "open", "source packet target state");
  const nextRange = object(
    packetTarget.next_bounded_range,
    "source packet next bounded range",
  );
  equal(nextRange.first, RANGE_START, "source packet range start");
  equal(nextRange.last, RANGE_END, "source packet range end");
  equal(nextRange.inclusive, true, "source packet range inclusivity");

  const acceptedRange = object(
    object(
      object(targetPacket.accepted_state, "source packet accepted state")
        .latest_bounded_negative,
      "source packet latest accepted bounded result",
    ).range,
    "source packet latest accepted range",
  );
  equal(acceptedRange.last, ACCEPTED_COVERAGE_END, "accepted coverage end");
  const pending = object(
    object(
      targetPacket.producer_completion,
      "source packet producer completion",
    ).latest_registered_submission,
    "source packet latest producer completion",
  );
  equal(
    object(pending.range, "source packet pending range").last,
    PENDING_PRODUCER_COVERAGE_END,
    "pending producer coverage end",
  );
  equal(pending.registration_route, "pending_review", "producer completion route");
  equal(
    pending.registration_accepted_state_changed,
    false,
    "producer completion accepted-state delta",
  );
  const completion = object(
    targetPacket.completion_contract,
    "source packet completion contract",
  );
  equal(completion.duplicate_range_forbidden, true, "duplicate-range policy");
  equal(
    completion.accepted_state_change,
    "none until a separate authorized Decision",
    "accepted-state boundary",
  );

  return {
    schema: TASK_SCHEMA,
    task_id: TASK_ID,
    source: {
      frontier: "erdos",
      frontier_id: FRONTIER_ID,
      repository: {
        origin: SOURCE_REPOSITORY,
        checkout_commit: SOURCE_CHECKOUT_COMMIT,
        checkout_tree: SOURCE_CHECKOUT_TREE,
        repository_root: SOURCE_REPOSITORY_ROOT,
      },
      target_index: {
        path: TARGET_INDEX_PATH,
        file_root: TARGET_INDEX_FILE_ROOT,
        index_root: TARGET_INDEX_ROOT,
        generated_from_commit: TARGET_INDEX_SOURCE_COMMIT,
        generated_from_tree: TARGET_INDEX_SOURCE_TREE,
      },
      target: {
        id: "erdos:1056",
        packet_path: SOURCE_PACKET_PATH,
        packet_schema: SOURCE_PACKET_SCHEMA,
        packet_root: SOURCE_PACKET_ROOT,
        packet_size: SOURCE_PACKET_SIZE,
        packet_commit: SOURCE_PACKET_COMMIT,
        packet_tree: SOURCE_PACKET_TREE,
      },
      coverage: {
        accepted_through: ACCEPTED_COVERAGE_END,
        producer_complete_pending_review_through:
          PENDING_PRODUCER_COVERAGE_END,
      },
    },
    objective: [
      `Exhaustively search every prime in the exact inclusive range ${RANGE_START}..${RANGE_END}.`,
      "For each prime, compute t! modulo p for every cut t in 0..p-1 and find the largest residue fiber.",
      "Stop at the first prime with at least 16 cuts in one fiber; otherwise report the greatest multiplicity across the complete range.",
      "Break ties by the earliest prime and then the smallest residue, and list every increasing cut in the selected fiber.",
    ].join(" "),
    constraints: {
      answer_access: "held_out",
      precomputed_result: "not_provided",
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

export function buildCapsuleManifest() {
  return {
    schema: CAPSULE_SCHEMA,
    task_id: TASK_ID,
    source: {
      path: VERIFIER_SOURCE_PATH,
      root: VERIFIER_SOURCE_ROOT,
    },
    build: {
      platform: "linux/amd64",
      image: VERIFIER_IMAGE,
      image_digest: VERIFIER_IMAGE_DIGEST,
      docker_root: DOCKER_ROOT,
      compiler: "/usr/bin/g++",
      argv: [
        "-O3",
        "-std=c++17",
        "-static",
        "-s",
        `-DCANOPUS_RANGE_START=${RANGE_START}`,
        `-DCANOPUS_RANGE_END=${RANGE_END}`,
        "/src/verifier.cpp",
        "-o",
        "/out/verifier",
      ],
    },
    executable: {
      format: "elf-static",
      architecture: "x86_64",
      root: VERIFIER_BINARY_ROOT,
    },
    execution: {
      network: "deny",
      root_filesystem: "read_only",
      capabilities: "drop_all",
      no_new_privileges: true,
      artifact_mount: "read_only",
      authority: "none",
    },
  };
}

export function capsuleManifestBytes(manifest = buildCapsuleManifest()) {
  return Buffer.from(canonicalJson(manifest));
}
