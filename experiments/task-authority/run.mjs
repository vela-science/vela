import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = dirname(fileURLToPath(import.meta.url));

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value).sort().map((key) => [key, canonical(value[key])]),
    );
  }
  return value;
}

export function contentRoot(value) {
  const bytes = JSON.stringify(canonical(value));
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function allowsPath(patterns, path) {
  if (path === null) return true;
  return patterns.some((pattern) => {
    if (pattern.endsWith("/**")) {
      return path.startsWith(pattern.slice(0, -2));
    }
    return path === pattern;
  });
}

function evaluateBaseline(packet, hostile) {
  const violations = [];
  if (!packet.allowed_tools.includes(hostile.tool)) violations.push("tool_not_allowed");
  if (
    hostile.network_destination !== null &&
    !packet.allowed_network_destinations.includes(hostile.network_destination)
  ) {
    violations.push("network_not_allowed");
  }
  if (!allowsPath(packet.allowed_write_paths, hostile.write_path)) {
    violations.push("write_path_not_allowed");
  }
  if (hostile.worker_root !== packet.worker_root) violations.push("worker_substitution");
  if (hostile.verifier_root !== packet.verifier_root) violations.push("verifier_substitution");
  if (hostile.output_bytes > packet.max_output_bytes) violations.push("output_budget_exceeded");
  return [...new Set(violations)].sort();
}

function evaluateCandidate(packet, hostile, baselineViolations) {
  const violations = [...baselineViolations];
  if (
    !packet.allowed_actions.includes(hostile.action) ||
    packet.forbidden_actions.includes(hostile.action)
  ) {
    violations.push("action_not_allowed");
  }
  if (
    !packet.allowed_source_classes.includes(hostile.source_class) ||
    packet.forbidden_source_classes.includes(hostile.source_class)
  ) {
    violations.push("source_class_not_allowed");
  }
  if (!packet.allowed_host_effects.includes(hostile.host_effect)) {
    violations.push("host_effect_not_allowed");
  }
  if (hostile.principal_id !== packet.principal_id) violations.push("principal_substitution");
  if (hostile.workload_id !== packet.workload_id) violations.push("workload_substitution");
  if (hostile.grant_root !== packet.grant_root || hostile.grant_status !== "active") {
    violations.push("grant_not_current");
  }
  if (hostile.producer_root !== packet.producer_root) violations.push("producer_substitution");
  if (hostile.workbench_root !== packet.workbench_root) {
    violations.push("workbench_substitution");
  }
  if (hostile.approval !== null) {
    if (
      hostile.approval.principal_id !== packet.approval_principal_id ||
      hostile.approval.current_authority !== true
    ) {
      violations.push("approval_principal_not_current");
    }
    if (hostile.approval.evidence_root !== hostile.final_evidence_root) {
      violations.push("reauthorization_required_evidence_changed");
    }
  }
  return [...new Set(violations)].sort();
}

async function load(name) {
  return JSON.parse(await readFile(join(ROOT, name), "utf8"));
}

export async function runExperiment() {
  const registration = await load("registration.json");
  const packets = await load("packets.json");
  const hostileSet = await load("hostile-cases.json");
  const cases = hostileSet.cases.map((hostile) => {
    const baselineViolations = evaluateBaseline(packets.baseline, hostile);
    const candidateViolations = evaluateCandidate(
      packets.candidate,
      hostile,
      baselineViolations,
    );
    return {
      id: hostile.id,
      green_provenance: hostile.provenance_valid,
      green_verifier: hostile.verifier_passed,
      baseline: {
        verdict: baselineViolations.length === 0 ? "permit" : "violation",
        violations: baselineViolations,
      },
      candidate: {
        verdict: candidateViolations.length === 0 ? "permit" : "violation",
        violations: candidateViolations,
      },
      baseline_false_pass:
        baselineViolations.length === 0 && candidateViolations.length > 0,
    };
  });
  const falsePasses = cases.filter((item) => item.baseline_false_pass).map((item) => item.id);
  const candidateDetected = cases.filter(
    (item) => item.candidate.verdict === "violation",
  ).length;
  const reportWithoutRoot = {
    schema: "canopus.task-authority-hostile-report.v0",
    registration_root: contentRoot(registration),
    packet_root: contentRoot(packets),
    hostile_set_root: contentRoot(hostileSet),
    hostile_case_count: cases.length,
    baseline_detected: cases.filter((item) => item.baseline.verdict === "violation").length,
    candidate_detected: candidateDetected,
    baseline_false_passes: falsePasses,
    cases,
    decision: "PIVOT_OPERATIONAL_ONLY",
    rationale:
      "Mission v1 remains the scientific execution contract. Current grant, source-class, host-effect, and reauthorization facts belong in the enforcing workbench or Canopus until live use proves a stable cross-workbench invariant.",
    promotion: "none",
    effects: registration.effects,
  };
  const report = {
    ...reportWithoutRoot,
    report_root: contentRoot(reportWithoutRoot),
  };
  const expected = registration.expected;
  if (
    report.hostile_case_count !== expected.hostile_cases ||
    report.candidate_detected !== expected.candidate_violations_detected ||
    JSON.stringify(report.baseline_false_passes) !==
      JSON.stringify(expected.baseline_false_passes) ||
    report.promotion !== expected.promotion
  ) {
    throw new Error("task-authority experiment differs from its frozen registration");
  }
  return report;
}

if (process.argv[1] !== undefined && fileURLToPath(import.meta.url) === process.argv[1]) {
  process.stdout.write(`${JSON.stringify(await runExperiment(), null, 2)}\n`);
}
