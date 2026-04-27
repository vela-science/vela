// VelaBench data loader.
//
// Reads benchmarks/leaderboard.json and benchmarks/results/*.json at
// build time and exposes the merged leaderboard to the /bench page.
// Same pattern as lib/frontier.ts — single place that knows where the
// score files live.

import { readFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";

interface Metrics {
  precision: number;
  recall: number;
  f1: number;
  matched: number;
  total_gold_findings: number;
  total_frontier_findings: number;
  exact_id_matches: number;
  entity_accuracy: number;
  assertion_type_accuracy: number;
  confidence_calibration: number;
}

export interface SubmissionEntry {
  id: string;
  name: string;
  submitter: string;
  submitted_at: string;
  kind: "gold" | "snapshot" | "manual" | "probe" | "agent";
  method: string;
  is_baseline?: boolean;
  metrics: Metrics;
  composite: number;
}

export interface Leaderboard {
  suite: string;
  gold: {
    frontier: string;
    vfr_id: string;
    claims: number;
    description: string;
  };
  submissions: SubmissionEntry[];
}

function repoRoot(): string {
  let cur = resolve(process.cwd());
  for (let i = 0; i < 6; i++) {
    if (existsSync(join(cur, "Cargo.toml")) && existsSync(join(cur, "benchmarks"))) {
      return cur;
    }
    const parent = resolve(cur, "..");
    if (parent === cur) break;
    cur = parent;
  }
  return resolve(process.cwd(), "..");
}

let _cache: Leaderboard | null = null;

export function loadLeaderboard(): Leaderboard {
  if (_cache) return _cache;
  const root = repoRoot();
  const manifestPath = join(root, "benchmarks", "leaderboard.json");
  if (!existsSync(manifestPath)) {
    console.warn(`[bench] leaderboard manifest missing: ${manifestPath}`);
    return {
      suite: "alzheimers-therapeutics",
      gold: { frontier: "", vfr_id: "", claims: 0, description: "" },
      submissions: [],
    };
  }
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const submissions: SubmissionEntry[] = [];
  for (const s of manifest.submissions ?? []) {
    const resultPath = join(root, "benchmarks", "results", s.result_file);
    if (!existsSync(resultPath)) {
      console.warn(`[bench] result file missing for ${s.id}: ${resultPath}`);
      continue;
    }
    const raw = JSON.parse(readFileSync(resultPath, "utf8"));
    const m: Metrics = (raw.metrics ?? raw) as Metrics;
    // Composite: weighted average of the four normalized scalars in
    // [0,1]. Same weighting as the v0.26 BenchmarkSuite default.
    // Keeps F1 dominant; entity + type + confidence are quality signals.
    const composite =
      m.f1 * 0.5 +
      m.entity_accuracy * 0.2 +
      m.assertion_type_accuracy * 0.2 +
      m.confidence_calibration * 0.1;
    submissions.push({
      id: s.id,
      name: s.name,
      submitter: s.submitter,
      submitted_at: s.submitted_at,
      kind: s.kind,
      method: s.method,
      is_baseline: !!s.is_baseline,
      metrics: m,
      composite: Math.round(composite * 1000) / 1000,
    });
  }
  // Sort: gold first (baseline), then by composite descending.
  submissions.sort((a, b) => {
    if (a.is_baseline !== b.is_baseline) return a.is_baseline ? -1 : 1;
    return b.composite - a.composite;
  });
  _cache = {
    suite: manifest.suite,
    gold: manifest.gold,
    submissions,
  };
  return _cache;
}

export function pct(x: number): string {
  return (x * 100).toFixed(1) + "%";
}

export function fmt3(x: number): string {
  return x.toFixed(3);
}
