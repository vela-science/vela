import {
  AGENT_RE,
  ContractError,
  MISSION_ID_RE,
  arrayAt,
  enumAt,
  exactKeys,
  gitObjectAt,
  integerAt,
  objectAt,
  relativePathAt,
  sha256At,
  stringAt,
} from "./validation.js";
import { contentDigest } from "../util/canonical.js";

export const MISSION_SCHEMA = "canopus.mission.v0" as const;
export const MISSION_V1_SCHEMA = "canopus.mission.v1" as const;
export const EXECUTION_BINDING_SCHEMA = "vela.execution-binding.v1" as const;
export const RESULT_CONTRACT_SCHEMA = "canopus.result-contract.v1" as const;
export const ROLES = ["producer", "adversary", "verifier", "fidelity"] as const;
// Vela Deny is deliberately error-shaped and therefore cannot be a successful
// Canopus landing outcome. Missions register only success-shaped routes.
export const LAND_ROUTES = ["permit", "defer"] as const;
export const CLAIM_TYPES = [
  "computational",
  "theoretical",
  "empirical",
  "negative",
  "contradiction",
] as const;
export const REPLAYABILITY = [
  "exact",
  "bounded",
  "approximate",
  "unavailable",
  "unknown",
] as const;
export type MissionRole = (typeof ROLES)[number];
export type LandRoute = (typeof LAND_ROUTES)[number];
export type ClaimType = (typeof CLAIM_TYPES)[number];
export type Replayability = (typeof REPLAYABILITY)[number];

export interface MissionRoots {
  git_commit: string;
  git_tree: string;
  vela_repository: string;
}

export interface MissionBudgets {
  max_research_wall_time_ms: number;
  max_research_processes: number;
  max_research_output_bytes: number;
  max_prompt_bytes: number;
  max_artifact_bytes: number;
  max_attempts: number;
  max_observed_tokens: number;
}

export interface VerifierSpec {
  argv: string[];
  executable_sha256: string;
  cwd: string;
  timeout_ms: number;
  max_output_bytes: number;
  network: "deny";
  writes: "deny";
}

export interface ContainerVerifierSpec extends VerifierSpec {
  capsule_path: string;
  capsule_sha256: string;
  image: string;
  platform?: "linux/amd64" | "linux/arm64";
}

export interface WorkerSpec {
  kind: "codex_tools_native";
  platform: "darwin" | "linux";
  codex_version: string;
  codex_sha256: string;
  permission_profile_path: string;
  permission_profile_sha256: string;
  workspace: "target_packet_only";
  output_schema_sha256: string;
  model: string;
  network: "provider_only";
  tools: ["shell", "apply_patch"];
}

export interface TargetPacketSpec {
  path: string;
  sha256: string;
}

export interface ProducerProfileSpec {
  name: string;
  root: string;
}

export interface LandingSpec {
  expected_routes: LandRoute[];
  max_accepted_delta: number;
}

export interface ExecutionBindingV1 {
  schema: typeof EXECUTION_BINDING_SCHEMA;
  packet_root: string;
  profile_root: string;
  verifier_capsule_root: string;
  result_contract_root: string;
}

export interface PositiveResultContractV1 {
  schema: typeof RESULT_CONTRACT_SCHEMA;
  target: string;
  claim_exact: string;
  claim_type: "computational";
  replayability: "exact";
  candidate_status: "success";
  verifier_status: "passed";
  required_artifact_kinds: string[];
}

export type ScientificChainSpec =
  | { predicted_observable: string; performed_test: string }
  | { not_applicable: true; performed_test: string };

interface MissionBase {
  id: string;
  target: string;
  vela_version: string;
  vela_sha256: string;
  frontier: string;
  actor: string;
  role: MissionRole;
  claim_type: ClaimType;
  replayability: Replayability;
  objective: string;
  completion_condition: string;
  roots: MissionRoots;
  allowed_paths: string[];
  budgets: MissionBudgets;
  scientific_chain: ScientificChainSpec;
  landing: LandingSpec;
  parent_candidate?: string;
  repair_reason?: string;
}

export interface MissionV0 extends MissionBase {
  schema: typeof MISSION_SCHEMA;
  verifier: VerifierSpec;
}

export interface MissionV1 extends MissionBase {
  schema: typeof MISSION_V1_SCHEMA;
  target_packet: TargetPacketSpec;
  profile?: ProducerProfileSpec;
  worker: WorkerSpec;
  verifier: ContainerVerifierSpec;
  execution_binding?: ExecutionBindingV1;
  result_contract?: PositiveResultContractV1;
}

export type Mission = MissionV0 | MissionV1;

function parseRoots(value: unknown): MissionRoots {
  const object = objectAt(value, "mission.roots");
  exactKeys(
    object,
    ["git_commit", "git_tree", "vela_repository"],
    [],
    "mission.roots",
  );
  return {
    git_commit: gitObjectAt(object.git_commit, "mission.roots.git_commit"),
    git_tree: gitObjectAt(object.git_tree, "mission.roots.git_tree"),
    vela_repository: sha256At(object.vela_repository, "mission.roots.vela_repository"),
  };
}

function frontierPathAt(value: unknown): string {
  return value === "." ? "." : relativePathAt(value, "mission.frontier");
}

function parseBudgets(value: unknown): MissionBudgets {
  const object = objectAt(value, "mission.budgets");
  exactKeys(
    object,
    [
      "max_research_wall_time_ms",
      "max_research_processes",
      "max_research_output_bytes",
      "max_prompt_bytes",
      "max_artifact_bytes",
      "max_attempts",
      "max_observed_tokens",
    ],
    [],
    "mission.budgets",
  );
  return {
    max_research_wall_time_ms: integerAt(
      object.max_research_wall_time_ms,
      "mission.budgets.max_research_wall_time_ms",
      100,
      3_600_000,
    ),
    max_research_processes: integerAt(
      object.max_research_processes,
      "mission.budgets.max_research_processes",
      1,
      64,
    ),
    max_research_output_bytes: integerAt(
      object.max_research_output_bytes,
      "mission.budgets.max_research_output_bytes",
      1024,
      67_108_864,
    ),
    max_prompt_bytes: integerAt(
      object.max_prompt_bytes,
      "mission.budgets.max_prompt_bytes",
      1024,
      8_388_608,
    ),
    max_artifact_bytes: integerAt(
      object.max_artifact_bytes,
      "mission.budgets.max_artifact_bytes",
      1,
      1_073_741_824,
    ),
    max_attempts: integerAt(object.max_attempts, "mission.budgets.max_attempts", 1, 8),
    max_observed_tokens: integerAt(
      object.max_observed_tokens,
      "mission.budgets.max_observed_tokens",
      1,
      1_000_000,
    ),
  };
}

function parseVerifier(value: unknown, container: boolean): VerifierSpec | ContainerVerifierSpec {
  const object = objectAt(value, "mission.verifier");
  exactKeys(
    object,
    ["argv", "executable_sha256", "cwd", "timeout_ms", "max_output_bytes", "network", "writes"],
    container ? ["capsule_path", "capsule_sha256", "image", "platform"] : [],
    "mission.verifier",
  );
  const base: VerifierSpec = {
    argv: arrayAt(
      object.argv,
      "mission.verifier.argv",
      { min: 1, max: 64 },
      (item, at) => stringAt(item, at, { max: 4096 }),
    ),
    executable_sha256: sha256At(
      object.executable_sha256,
      "mission.verifier.executable_sha256",
    ),
    cwd: relativePathAt(object.cwd, "mission.verifier.cwd"),
    timeout_ms: integerAt(object.timeout_ms, "mission.verifier.timeout_ms", 100, 3_600_000),
    max_output_bytes: integerAt(
      object.max_output_bytes,
      "mission.verifier.max_output_bytes",
      1024,
      67_108_864,
    ),
    network: enumAt(object.network, "mission.verifier.network", ["deny"] as const),
    writes: enumAt(object.writes, "mission.verifier.writes", ["deny"] as const),
  };
  if (!container) return base;
  return {
    ...base,
    capsule_path: relativePathAt(object.capsule_path, "mission.verifier.capsule_path"),
    capsule_sha256: sha256At(object.capsule_sha256, "mission.verifier.capsule_sha256"),
    image: sha256At(object.image, "mission.verifier.image"),
    ...(object.platform === undefined
      ? {}
      : {
          platform: enumAt(
            object.platform,
            "mission.verifier.platform",
            ["linux/amd64", "linux/arm64"] as const,
          ),
        }),
  };
}

function parseWorker(value: unknown): WorkerSpec {
  const object = objectAt(value, "mission.worker");
  exactKeys(
    object,
    [
      "kind", "platform", "codex_version", "codex_sha256", "permission_profile_path",
      "permission_profile_sha256", "workspace", "output_schema_sha256", "model", "network", "tools",
    ],
    [],
    "mission.worker",
  );
  const tools = arrayAt(
    object.tools,
    "mission.worker.tools",
    { min: 2, max: 2, unique: true },
    (item, at) => enumAt(item, at, ["shell", "apply_patch"] as const),
  );
  if (tools[0] !== "shell" || tools[1] !== "apply_patch") {
    throw new ContractError("mission.worker.tools must be [shell, apply_patch]");
  }
  return {
    kind: enumAt(object.kind, "mission.worker.kind", ["codex_tools_native"] as const),
    platform: enumAt(object.platform, "mission.worker.platform", ["darwin", "linux"] as const),
    codex_version: stringAt(object.codex_version, "mission.worker.codex_version", {
      min: 10,
      max: 64,
      pattern: /^codex-cli [0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?$/u,
    }),
    codex_sha256: sha256At(object.codex_sha256, "mission.worker.codex_sha256"),
    permission_profile_path: relativePathAt(
      object.permission_profile_path,
      "mission.worker.permission_profile_path",
    ),
    permission_profile_sha256: sha256At(
      object.permission_profile_sha256,
      "mission.worker.permission_profile_sha256",
    ),
    workspace: enumAt(
      object.workspace,
      "mission.worker.workspace",
      ["target_packet_only"] as const,
    ),
    output_schema_sha256: sha256At(
      object.output_schema_sha256,
      "mission.worker.output_schema_sha256",
    ),
    model: stringAt(object.model, "mission.worker.model", { min: 1, max: 128 }),
    network: enumAt(object.network, "mission.worker.network", ["provider_only"] as const),
    tools: ["shell", "apply_patch"],
  };
}

function parseTargetPacket(value: unknown): TargetPacketSpec {
  const object = objectAt(value, "mission.target_packet");
  exactKeys(object, ["path", "sha256"], [], "mission.target_packet");
  return {
    path: relativePathAt(object.path, "mission.target_packet.path"),
    sha256: sha256At(object.sha256, "mission.target_packet.sha256"),
  };
}

function parseLanding(value: unknown): LandingSpec {
  const object = objectAt(value, "mission.landing");
  exactKeys(object, ["expected_routes", "max_accepted_delta"], [], "mission.landing");
  return {
    expected_routes: arrayAt(
      object.expected_routes,
      "mission.landing.expected_routes",
      { min: 1, max: 3, unique: true },
      (item, at) => enumAt(item, at, LAND_ROUTES),
    ),
    max_accepted_delta: integerAt(
      object.max_accepted_delta,
      "mission.landing.max_accepted_delta",
      0,
      1,
    ),
  };
}

function parseProducerProfile(value: unknown): ProducerProfileSpec {
  const object = objectAt(value, "mission.profile");
  exactKeys(object, ["name", "root"], [], "mission.profile");
  return {
    name: stringAt(object.name, "mission.profile.name", {
      min: 1,
      max: 128,
      pattern: /^[a-z0-9][a-z0-9._-]*$/u,
    }),
    root: sha256At(object.root, "mission.profile.root"),
  };
}

function parseExecutionBinding(value: unknown): ExecutionBindingV1 {
  const object = objectAt(value, "mission.execution_binding");
  exactKeys(
    object,
    ["schema", "packet_root", "profile_root", "verifier_capsule_root", "result_contract_root"],
    [],
    "mission.execution_binding",
  );
  if (object.schema !== EXECUTION_BINDING_SCHEMA) {
    throw new ContractError(`mission.execution_binding.schema must be ${EXECUTION_BINDING_SCHEMA}`);
  }
  return {
    schema: EXECUTION_BINDING_SCHEMA,
    packet_root: sha256At(object.packet_root, "mission.execution_binding.packet_root"),
    profile_root: sha256At(object.profile_root, "mission.execution_binding.profile_root"),
    verifier_capsule_root: sha256At(
      object.verifier_capsule_root,
      "mission.execution_binding.verifier_capsule_root",
    ),
    result_contract_root: sha256At(
      object.result_contract_root,
      "mission.execution_binding.result_contract_root",
    ),
  };
}

export function parsePositiveResultContract(value: unknown): PositiveResultContractV1 {
  const object = objectAt(value, "mission.result_contract");
  exactKeys(
    object,
    [
      "schema",
      "target",
      "claim_exact",
      "claim_type",
      "replayability",
      "candidate_status",
      "verifier_status",
      "required_artifact_kinds",
    ],
    [],
    "mission.result_contract",
  );
  if (object.schema !== RESULT_CONTRACT_SCHEMA) {
    throw new ContractError(`mission.result_contract.schema must be ${RESULT_CONTRACT_SCHEMA}`);
  }
  if (object.claim_type !== "computational" || object.replayability !== "exact") {
    throw new ContractError("mission.result_contract is limited to exact computational results");
  }
  if (object.candidate_status !== "success" || object.verifier_status !== "passed") {
    throw new ContractError("mission.result_contract must require a successful verified result");
  }
  return {
    schema: RESULT_CONTRACT_SCHEMA,
    target: stringAt(object.target, "mission.result_contract.target", { min: 1, max: 256 }),
    claim_exact: stringAt(object.claim_exact, "mission.result_contract.claim_exact", {
      min: 1,
      max: 8192,
    }),
    claim_type: "computational",
    replayability: "exact",
    candidate_status: "success",
    verifier_status: "passed",
    required_artifact_kinds: arrayAt(
      object.required_artifact_kinds,
      "mission.result_contract.required_artifact_kinds",
      { min: 1, max: 16, unique: true },
      (item, at) => stringAt(item, at, {
        min: 1,
        max: 64,
        pattern: /^[a-z][a-z0-9._-]*$/u,
      }),
    ).sort(),
  };
}

function parseScientificChain(value: unknown): ScientificChainSpec {
  const object = objectAt(value, "mission.scientific_chain");
  exactKeys(
    object,
    ["performed_test"],
    ["predicted_observable", "not_applicable"],
    "mission.scientific_chain",
  );
  const performed_test = stringAt(
    object.performed_test,
    "mission.scientific_chain.performed_test",
    { min: 1, max: 16_384 },
  );
  const hasPrediction = object.predicted_observable !== undefined;
  const hasNotApplicable = object.not_applicable !== undefined;
  if (hasPrediction === hasNotApplicable) {
    throw new ContractError(
      "mission.scientific_chain requires exactly one of predicted_observable or not_applicable",
    );
  }
  if (hasPrediction) {
    return {
      predicted_observable: stringAt(
        object.predicted_observable,
        "mission.scientific_chain.predicted_observable",
        { min: 1, max: 16_384 },
      ),
      performed_test,
    };
  }
  if (object.not_applicable !== true) {
    throw new ContractError("mission.scientific_chain.not_applicable must be true");
  }
  return { not_applicable: true, performed_test };
}

export function parseMission(value: unknown): Mission {
  const object = objectAt(value, "mission");
  const schema = enumAt(
    object.schema,
    "mission.schema",
    [MISSION_SCHEMA, MISSION_V1_SCHEMA] as const,
  );
  const isV1 = schema === MISSION_V1_SCHEMA;
  exactKeys(
    object,
    [
      "schema",
      "id",
      "target",
      "vela_version",
      "vela_sha256",
      "frontier",
      "actor",
      "role",
      "claim_type",
      "replayability",
      "objective",
      "completion_condition",
      "roots",
      "allowed_paths",
      "budgets",
      "verifier",
      "scientific_chain",
      "landing",
    ],
    isV1
      ? [
          "parent_candidate",
          "repair_reason",
          "target_packet",
          "profile",
          "worker",
          "execution_binding",
          "result_contract",
        ]
      : ["parent_candidate", "repair_reason"],
    "mission",
  );

  if (isV1) {
    for (const required of ["target_packet", "worker"] as const) {
      if (object[required] === undefined) {
        throw new ContractError(`mission.${required} is required for mission v1`);
      }
    }
  }

  const replayability = enumAt(object.replayability, "mission.replayability", REPLAYABILITY);
  if (replayability !== "exact") {
    throw new ContractError("Canopus v0 requires replayability exact");
  }
  const verifier = parseVerifier(object.verifier, isV1);
  const verifierExecutable = relativePathAt(verifier.argv[0], "mission.verifier.argv[0]");
  if (!/^(?:[A-Za-z0-9_+.-]+\/)*[A-Za-z0-9_+.-]+$/u.test(verifierExecutable)) {
    throw new ContractError(
      "mission.verifier.argv[0] must name one relative verifier capsule without shell syntax",
    );
  }
  verifier.argv[0] = verifierExecutable;
  const landing = parseLanding(object.landing);
  const requiredDelta = landing.expected_routes.includes("permit") ? 1 : 0;
  if (landing.max_accepted_delta !== requiredDelta) {
    throw new ContractError(
      `mission.landing.max_accepted_delta must be ${requiredDelta} for the registered routes`,
    );
  }
  if (landing.expected_routes.length !== 1) {
    throw new ContractError("mission landing must register exactly one expected route");
  }

  const base = {
    schema,
    id: stringAt(object.id, "mission.id", { max: 134, pattern: MISSION_ID_RE }),
    target: stringAt(object.target, "mission.target", { min: 1, max: 256 }),
    vela_version: stringAt(object.vela_version, "mission.vela_version", {
      min: 5,
      max: 32,
      pattern:
        /^0\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/u,
    }),
    vela_sha256: sha256At(object.vela_sha256, "mission.vela_sha256"),
    frontier: frontierPathAt(object.frontier),
    actor: stringAt(object.actor, "mission.actor", { max: 69, pattern: AGENT_RE }),
    role: enumAt(object.role, "mission.role", ROLES),
    claim_type: enumAt(object.claim_type, "mission.claim_type", CLAIM_TYPES),
    replayability,
    objective: stringAt(object.objective, "mission.objective", { min: 1, max: 8192 }),
    completion_condition: stringAt(object.completion_condition, "mission.completion_condition", {
      min: 1,
      max: 4096,
    }),
    roots: parseRoots(object.roots),
    allowed_paths: arrayAt(
      object.allowed_paths,
      "mission.allowed_paths",
      { min: 1, max: 64, unique: true },
      (item, at) => {
        const allowed = relativePathAt(item, at);
        if (allowed === "canopus" || allowed.startsWith("canopus/")) {
          throw new ContractError(`${at} uses the reserved Canopus evidence namespace`);
        }
        return allowed;
      },
    ),
    budgets: parseBudgets(object.budgets),
    verifier,
    scientific_chain: parseScientificChain(object.scientific_chain),
    landing,
  };

  const versioned = isV1
    ? (() => {
        const targetPacket = parseTargetPacket(object.target_packet);
        const profile = object.profile === undefined
          ? undefined
          : parseProducerProfile(object.profile);
        const executionBinding = object.execution_binding === undefined
          ? undefined
          : parseExecutionBinding(object.execution_binding);
        const resultContract = object.result_contract === undefined
          ? undefined
          : parsePositiveResultContract(object.result_contract);
        if ((executionBinding === undefined) !== (resultContract === undefined)) {
          throw new ContractError(
            "mission v1 exact Permit requires both execution_binding and result_contract",
          );
        }
        if (landing.expected_routes[0] === "permit" && executionBinding === undefined) {
          throw new ContractError("mission v1 Permit requires an exact execution binding");
        }
        if (executionBinding !== undefined && resultContract !== undefined) {
          if (base.claim_type !== "computational" || base.replayability !== "exact") {
            throw new ContractError("exact Permit bindings require an exact computational mission");
          }
          if (
            executionBinding.packet_root !== targetPacket.sha256 ||
            executionBinding.verifier_capsule_root !== (verifier as ContainerVerifierSpec).capsule_sha256 ||
            executionBinding.result_contract_root !== contentDigest(resultContract) ||
            resultContract.target !== base.target
          ) {
            throw new ContractError(
              "mission execution binding does not match its retained packet, capsule, result contract, or target",
            );
          }
          if (profile !== undefined && executionBinding.profile_root !== profile.root) {
            throw new ContractError(
              "mission execution binding does not match its retained producer profile",
            );
          }
        }
        return {
        ...base,
        schema: MISSION_V1_SCHEMA,
        target_packet: targetPacket,
        ...(profile === undefined ? {} : { profile }),
        worker: parseWorker(object.worker),
        verifier: verifier as ContainerVerifierSpec,
        ...(executionBinding === undefined ? {} : { execution_binding: executionBinding }),
        ...(resultContract === undefined ? {} : { result_contract: resultContract }),
      };
      })()
    : { ...base, schema: MISSION_SCHEMA, verifier: verifier as VerifierSpec };

  if ((object.parent_candidate === undefined) !== (object.repair_reason === undefined)) {
    throw new ContractError("mission repair requires both parent_candidate and repair_reason");
  }
  if (object.parent_candidate === undefined || object.repair_reason === undefined) {
    return versioned;
  }
  return {
    ...versioned,
    parent_candidate: sha256At(object.parent_candidate, "mission.parent_candidate"),
    repair_reason: stringAt(object.repair_reason, "mission.repair_reason", {
      min: 1,
      max: 4096,
    }),
  };
}
