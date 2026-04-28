// Frontier data loader.
//
// At build time, the site reads the canonical frontier on disk and
// exposes typed records to pages and components. This is the single
// place that knows where findings live; everywhere else just calls
// `loadFrontier()`.
//
// Doctrine: the protocol-side directory stays `bbb-flagship` (stable
// id). The site renders it under the scientific subject defined in
// `config.ts -> FRONTIER`. If the canonical frontier is later
// re-published under a renamed name + new vfr_id, only this file and
// `FRONTIER` need to follow.

import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

import { FRONTIER } from "../config";

// ── Schema ──────────────────────────────────────────────────────────
//
// Mirrors `crates/vela-protocol/src/bundle.rs::FindingBundle` — only
// the fields the site renders. New protocol fields are tolerated
// (we never `deny_unknown_fields` in TS); missing optional fields are
// allowed.

export interface Author {
  name: string;
  orcid?: string | null;
}

export interface EvidenceSpan {
  section: string;
  text: string;
}

// v0.38: causal-typing fields. Added to Assertion in v0.38.0;
// drive the v0.40 identifiability audit. Either may be unset on
// pre-v0.38 findings; renderers must guard for undefined.
export type CausalClaim = "correlation" | "mediation" | "intervention";
export type CausalEvidenceGrade =
  | "rct"
  | "quasi_experimental"
  | "observational"
  | "theoretical";

export interface Assertion {
  text: string;
  type: string;
  entities: Array<{
    name: string;
    type: string;
    aliases?: string[];
  }>;
  relation?: string;
  direction?: string;
  causal_claim?: CausalClaim;
  causal_evidence_grade?: CausalEvidenceGrade;
}

export interface Evidence {
  type: string;
  model_system?: string | null;
  species?: string | null;
  method?: string | null;
  sample_size?: number | null;
  effect_size?: number | null;
  p_value?: number | null;
  replicated: boolean;
  replication_count?: number | null;
  evidence_spans?: EvidenceSpan[];
}

export interface Conditions {
  text?: string | null;
  in_vitro?: boolean;
  in_vivo?: boolean;
  human_data?: boolean;
  clinical_trial?: boolean;
  concentration_range?: string | null;
  duration?: string | null;
  age_group?: string | null;
  cell_type?: string | null;
}

export interface ConfidenceComponents {
  evidence_strength?: number;
  replication_strength?: number;
  sample_strength?: number;
  model_relevance?: number;
  review_penalty?: number;
  calibration_adjustment?: number;
}

export interface Confidence {
  kind: string;
  score: number;
  basis?: string;
  method?: string;
  components?: ConfidenceComponents;
  extraction_confidence?: number;
}

export interface ExtractionMeta {
  method?: string;
  model?: string | null;
  model_version?: string | null;
  extracted_at?: string;
  extractor_version?: string | null;
}

export interface Provenance {
  source_type?: string;
  doi?: string | null;
  pmid?: string | null;
  pmc?: string | null;
  openalex_id?: string | null;
  title?: string | null;
  authors?: Author[];
  year?: number | null;
  journal?: string | null;
  citation_count?: number;
  extraction?: ExtractionMeta;
  review?: unknown;
}

export interface Flags {
  gap?: boolean;
  negative_space?: boolean;
  contested?: boolean;
  retracted?: boolean;
  declining?: boolean;
  gravity_well?: boolean;
}

export interface Link {
  target: string;
  type: string;
  note?: string;
  inferred_by?: string;
  created_at?: string;
  /* v0.45: optional structural causal mechanism on a `depends` /
     `supports` edge. Edges with a mechanism participate in
     counterfactual (Pearl level 3) twin-network propagation. */
  mechanism?: Mechanism;
}

export type MechanismSign = "positive" | "negative";

export type Mechanism =
  | { kind: "linear"; sign: MechanismSign; slope: number }
  | { kind: "monotonic"; sign: MechanismSign }
  | { kind: "threshold"; sign: MechanismSign; threshold: number }
  | { kind: "saturating"; sign: MechanismSign; half_max: number }
  | { kind: "unknown" };

/* Apply a mechanism to a parent perturbation `delta_x` and return the
   implied child perturbation. Mirrors `crate::bundle::Mechanism::apply`. */
export function applyMechanism(m: Mechanism, deltaX: number): number | null {
  const signOf = (s: MechanismSign) => (s === "positive" ? 1 : -1);
  switch (m.kind) {
    case "linear":
      return signOf(m.sign) * m.slope * deltaX;
    case "monotonic": {
      const ax = Math.min(Math.abs(deltaX), 1);
      return signOf(m.sign) * Math.sign(deltaX) * ax;
    }
    case "threshold":
      if (Math.abs(deltaX) >= m.threshold) {
        return signOf(m.sign) * Math.sign(deltaX);
      }
      return 0;
    case "saturating": {
      const denom = Math.abs(deltaX) + Math.max(m.half_max, 1e-9);
      return (signOf(m.sign) * deltaX) / denom;
    }
    case "unknown":
      return null;
  }
}

export interface Annotation {
  id?: string;
  text: string;
  author?: string;
  timestamp?: string;
}

export interface Finding {
  id: string;
  version: number;
  previous_version?: string | null;
  assertion: Assertion;
  evidence: Evidence;
  conditions: Conditions;
  confidence: Confidence;
  provenance: Provenance;
  flags: Flags;
  links: Link[];
  annotations: Annotation[];
  created: string;
  updated?: string | null;
}

// ── Render-side derived view ────────────────────────────────────────
//
// The site doesn't render raw `Finding` everywhere; it renders a
// `ClaimView` that flattens the most-asked-for fields and computes
// site-only properties (slug, target tags, evidence label). Pages
// hold the raw `Finding` for detail views.

export interface ClaimView {
  finding: Finding;
  // Stable site-side slug. Format: `<assertion-slug>-<vf-short>`.
  // Example: `tau-tracks-cognition-vf_3fa8`. Hash is the unique tail.
  slug: string;
  // Short id for compact rendering.
  shortId: string;
  // Drug-target tags inferred from assertion + entities.
  // Drives /targets/[slug] aggregation and homepage filter chips.
  targets: TargetSlug[];
  // Trial tags inferred from text. Drives /trials/[slug] aggregation.
  trials: TrialSlug[];
  // One-word label of evidence strength: experimental / clinical / review / etc.
  evidenceLabel: string;
  // Confidence bucket for the dot indicator: low | mid | high.
  confidenceBucket: "low" | "mid" | "high";
}

// ── Targets and trials ──────────────────────────────────────────────
//
// Hand-curated taxonomy. The protocol doesn't carry domain tags yet;
// the site infers them from assertion text + entity names with a
// simple keyword set. Adding a target is one entry here.

export type TargetSlug =
  | "amyloid-beta"
  | "bace1"
  | "tau"
  | "trem2"
  | "apoe"
  | "bbb-delivery";

export type TrialSlug = "lecanemab" | "donanemab" | "verubecestat" | "atabecestat";

interface TargetDef {
  slug: TargetSlug;
  name: string;
  short: string;
  // Lowercase keyword fragments matched against the assertion + entities.
  keywords: string[];
  blurb: string;
}

interface TrialDef {
  slug: TrialSlug;
  name: string;
  keywords: string[];
  blurb: string;
}

export const TARGETS: TargetDef[] = [
  {
    slug: "amyloid-beta",
    name: "Amyloid-β",
    short: "Aβ",
    keywords: [
      "amyloid",
      "abeta",
      "aβ",
      "ab plaque",
      "amyloid-beta",
      "amyloid beta",
      "rage",
      "app cleavage",
    ],
    blurb:
      "The amyloid cascade hypothesis remains the dominant therapeutic frame, despite repeated clinical failures and a poor correlation between plaque burden and cognitive decline.",
  },
  {
    slug: "bace1",
    name: "BACE1",
    short: "BACE1",
    keywords: ["bace1", "bace-1", "β-secretase", "beta-secretase"],
    blurb:
      "BACE1 inhibition produces robust amyloid-pathology readouts but has failed in trials, with toxicity attributed to non-APP substrates.",
  },
  {
    slug: "tau",
    name: "Tau",
    short: "Tau",
    keywords: ["tau ", "tauopathy", "neurofibrillary", "p-tau", "ptau", "braak"],
    blurb:
      "Tau pathology tracks cognitive decline more closely than amyloid; spread follows predictable Braak staging patterns.",
  },
  {
    slug: "trem2",
    name: "TREM2",
    short: "TREM2",
    keywords: ["trem2", "trem-2"],
    blurb:
      "Microglial receptor whose loss-of-function increases plaque density; activating antibodies are in early clinical evaluation, with timing-dependent benefit/harm an open question.",
  },
  {
    slug: "apoe",
    name: "ApoE",
    short: "ApoE",
    keywords: ["apoe", "apo-e", "apolipoprotein e"],
    blurb:
      "Strongest common genetic risk factor; ApoE4 increases risk 3–12× depending on copy number, while the Christchurch variant may be protective.",
  },
  {
    slug: "bbb-delivery",
    name: "BBB delivery",
    short: "BBB",
    keywords: [
      "blood-brain barrier",
      "blood brain barrier",
      "bbb",
      "transferrin receptor",
      "tfr",
      "antibody transport vehicle",
      "atv",
      "brain shuttle",
    ],
    blurb:
      "The blood-brain barrier rejects most therapeutic biologics; engineered delivery vehicles (transferrin-receptor binders, antibody transport vehicles) reach measurable brain exposure.",
  },
];

export const TRIALS: TrialDef[] = [
  {
    slug: "lecanemab",
    name: "Lecanemab (Clarity AD)",
    keywords: ["lecanemab", "ban2401", "clarity ad", "clarity-ad"],
    blurb:
      "Anti-Aβ protofibril antibody; Clarity AD showed modest but real cognitive benefit, anchoring the modern amyloid-clearance frame.",
  },
  {
    slug: "donanemab",
    name: "Donanemab (TRAILBLAZER-ALZ)",
    keywords: ["donanemab", "trailblazer", "trailblazer-alz"],
    blurb:
      "Anti-N3pE-Aβ antibody targeting plaque-resident amyloid; demonstrated cognitive benefit in early symptomatic disease.",
  },
  {
    slug: "verubecestat",
    name: "Verubecestat",
    keywords: ["verubecestat"],
    blurb:
      "BACE1 inhibitor; Phase 3 trials terminated for futility despite robust target engagement, raising questions about timing and substrate-mediated toxicity.",
  },
  {
    slug: "atabecestat",
    name: "Atabecestat",
    keywords: ["atabecestat"],
    blurb:
      "BACE1 inhibitor; trials halted for liver toxicity, contributing to the broader failure of the BACE1 mechanism in late-stage disease.",
  },
];

const TARGET_BY_SLUG: Map<TargetSlug, TargetDef> = new Map(
  TARGETS.map((t) => [t.slug, t]),
);
const TRIAL_BY_SLUG: Map<TrialSlug, TrialDef> = new Map(
  TRIALS.map((t) => [t.slug, t]),
);

export function targetDef(slug: TargetSlug): TargetDef | undefined {
  return TARGET_BY_SLUG.get(slug);
}

export function trialDef(slug: TrialSlug): TrialDef | undefined {
  return TRIAL_BY_SLUG.get(slug);
}

// ── Loader ──────────────────────────────────────────────────────────
//
// Resolves the canonical frontier path from `FRONTIER.repoPath` and
// reads every `*.json` under `.vela/findings/`. Memoized per build.

let _cache: ClaimView[] | null = null;

// Resolve the repo root by walking up from cwd looking for the
// canonical `Cargo.toml` (the workspace marker) or the
// `projects/bbb-flagship` directory itself. Robust against being
// invoked from `site/`, the repo root, or a worktree.
function repoRoot(): string {
  let cur = resolve(process.cwd());
  for (let i = 0; i < 6; i++) {
    if (existsSync(join(cur, "Cargo.toml")) && existsSync(join(cur, "projects"))) {
      return cur;
    }
    const parent = resolve(cur, "..");
    if (parent === cur) break;
    cur = parent;
  }
  // Fallback — assume cwd is `site/` (the typical case for `astro build`).
  return resolve(process.cwd(), "..");
}

function frontierFindingsDir(): string {
  const candidate = join(repoRoot(), FRONTIER.repoPath, ".vela", "findings");
  if (existsSync(candidate)) return candidate;
  // Secondary: try cwd-relative (covers the case where cwd is the repo root).
  const alt = join(process.cwd(), FRONTIER.repoPath, ".vela", "findings");
  if (existsSync(alt)) return alt;
  return candidate; // returned even if missing; caller logs a warning
}

// Sanity check used by tests / debug logging.
export function _debugFrontierPaths() {
  return {
    cwd: process.cwd(),
    repoRoot: repoRoot(),
    findingsDir: frontierFindingsDir(),
    findingsExists: existsSync(frontierFindingsDir()),
  };
}

// statSync is only imported for completeness; not currently used.
void statSync;

export function loadFrontier(): ClaimView[] {
  if (_cache) return _cache;
  const dir = frontierFindingsDir();
  if (!existsSync(dir)) {
    console.warn(`[frontier] findings dir missing: ${dir}`);
    _cache = [];
    return _cache;
  }
  const entries = readdirSync(dir).filter((f) => f.endsWith(".json"));
  const claims: ClaimView[] = [];
  for (const file of entries) {
    try {
      const raw = readFileSync(join(dir, file), "utf8");
      const f = JSON.parse(raw) as Finding;
      claims.push(toClaimView(f));
    } catch (err) {
      console.warn(`[frontier] failed to parse ${file}:`, err);
    }
  }
  // Default ordering: newest first.
  claims.sort((a, b) => b.finding.created.localeCompare(a.finding.created));
  _cache = claims;
  return _cache;
}

// ── Derivation ──────────────────────────────────────────────────────

function toClaimView(finding: Finding): ClaimView {
  // Defensively normalize the optional collections. The Notes Compiler
  // and other agents emit findings with `annotations: null` rather than
  // `annotations: []`; older BBB findings always have arrays. The site
  // is the union of both shapes — coerce to arrays once at the entry
  // point so every downstream consumer can safely call `.length` /
  // `.map` without type-narrow guards.
  const f: Finding = {
    ...finding,
    annotations: Array.isArray(finding.annotations) ? finding.annotations : [],
    links: Array.isArray(finding.links) ? finding.links : [],
    flags: finding.flags ?? {},
    provenance: {
      ...(finding.provenance ?? {}),
      authors: Array.isArray(finding.provenance?.authors)
        ? finding.provenance.authors
        : [],
    },
    evidence: {
      ...finding.evidence,
      evidence_spans: Array.isArray(finding.evidence?.evidence_spans)
        ? finding.evidence.evidence_spans
        : [],
    },
    conditions: finding.conditions ?? {},
  };
  return {
    finding: f,
    slug: deriveSlug(f),
    shortId: f.id.replace(/^vf_/, "").slice(0, 4),
    targets: deriveTargets(f),
    trials: deriveTrials(f),
    evidenceLabel: deriveEvidenceLabel(f),
    confidenceBucket: deriveConfidenceBucket(f.confidence?.score ?? 0),
  };
}

// Slug = first ~6 words of the assertion text, kebab-cased, plus
// `vf_<short>` suffix. The hash makes the URL unique without needing
// global slug-collision tracking; the prose makes it readable.
export function deriveSlug(finding: Finding): string {
  const text = finding.assertion.text || "claim";
  const prose = text
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "")
    .split("-")
    .slice(0, 8)
    .join("-");
  const short = finding.id.replace(/^vf_/, "").slice(0, 4);
  return `${prose}-vf_${short}`;
}

function searchHaystack(finding: Finding): string {
  const parts: string[] = [
    finding.assertion.text || "",
    finding.conditions.text || "",
    finding.evidence.method || "",
    finding.evidence.model_system || "",
    finding.provenance.title || "",
  ];
  for (const ent of finding.assertion.entities ?? []) {
    parts.push(ent.name);
    if (ent.aliases) parts.push(...ent.aliases);
  }
  for (const span of finding.evidence.evidence_spans ?? []) {
    parts.push(span.text);
  }
  return parts.join(" ").toLowerCase();
}

function deriveTargets(finding: Finding): TargetSlug[] {
  const hay = searchHaystack(finding);
  const matched: TargetSlug[] = [];
  for (const t of TARGETS) {
    if (t.keywords.some((kw) => hay.includes(kw))) matched.push(t.slug);
  }
  return matched;
}

function deriveTrials(finding: Finding): TrialSlug[] {
  const hay = searchHaystack(finding);
  const matched: TrialSlug[] = [];
  for (const t of TRIALS) {
    if (t.keywords.some((kw) => hay.includes(kw))) matched.push(t.slug);
  }
  return matched;
}

function deriveEvidenceLabel(finding: Finding): string {
  const t = (finding.evidence.type || "").toLowerCase();
  if (t === "experimental") return "experimental";
  if (t === "clinical_trial" || finding.conditions.clinical_trial) return "clinical";
  if (t === "review" || t === "meta-analysis") return "review";
  if (t === "computational") return "computational";
  if (t === "observational") return "observational";
  return t || "evidence";
}

function deriveConfidenceBucket(score: number): "low" | "mid" | "high" {
  if (score >= 0.6) return "high";
  if (score >= 0.35) return "mid";
  return "low";
}

// ── Replications (v0.32) ────────────────────────────────────────────
//
// Replications live next to findings under `.vela/replications/`. Each
// `vrep_<hash>.json` is a single Replication record content-addressed
// over its target finding, attempting actor, canonical conditions, and
// outcome. The site loads them at build time and indexes by target.

export interface Replication {
  id: string;
  target_finding: string;
  attempted_by: string;
  outcome: "replicated" | "failed" | "partial" | "inconclusive";
  evidence: Evidence;
  conditions: Conditions;
  provenance: Provenance;
  notes: string;
  created: string;
  previous_attempt?: string | null;
}

let _repCache: Replication[] | null = null;

function replicationsDir(): string {
  return join(repoRoot(), FRONTIER.repoPath, ".vela", "replications");
}

// ── Bridges (v0.46) ────────────────────────────────────────────────

export type BridgeStatus = "derived" | "confirmed" | "refuted";

export interface BridgeRef {
  frontier: string;
  finding_id: string;
  assertion_text: string;
  confidence: number;
  direction?: string | null;
}

export interface Bridge {
  id: string;
  schema?: string;
  entity_name: string;
  frontiers: string[];
  frontier_ids?: string[];
  finding_refs: BridgeRef[];
  tension?: string | null;
  derived_at: string;
  status: BridgeStatus;
}

let _bridgeCache: Bridge[] | null = null;

function bridgesDir(): string {
  return join(repoRoot(), FRONTIER.repoPath, ".vela", "bridges");
}

export function loadBridges(): Bridge[] {
  if (_bridgeCache) return _bridgeCache;
  const dir = bridgesDir();
  if (!existsSync(dir)) {
    _bridgeCache = [];
    return _bridgeCache;
  }
  const out: Bridge[] = [];
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
    try {
      const raw = readFileSync(join(dir, file), "utf8");
      out.push(JSON.parse(raw) as Bridge);
    } catch (err) {
      console.warn(`[frontier] failed to parse bridge ${file}:`, err);
    }
  }
  out.sort(
    (a, b) =>
      b.finding_refs.length - a.finding_refs.length ||
      a.entity_name.localeCompare(b.entity_name),
  );
  _bridgeCache = out;
  return _bridgeCache;
}

export function loadReplications(): Replication[] {
  if (_repCache) return _repCache;
  const dir = replicationsDir();
  if (!existsSync(dir)) {
    _repCache = [];
    return _repCache;
  }
  const reps: Replication[] = [];
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
    try {
      const raw = readFileSync(join(dir, file), "utf8");
      reps.push(JSON.parse(raw) as Replication);
    } catch (err) {
      console.warn(`[frontier] failed to parse replication ${file}:`, err);
    }
  }
  reps.sort((a, b) => b.created.localeCompare(a.created));
  _repCache = reps;
  return _repCache;
}

export function replicationsForFinding(vfId: string): Replication[] {
  return loadReplications().filter((r) => r.target_finding === vfId);
}

export interface ReplicationStats {
  total: number;
  replicated: number;
  failed: number;
  partial: number;
  inconclusive: number;
}

export function replicationStats(): ReplicationStats {
  const reps = loadReplications();
  return {
    total: reps.length,
    replicated: reps.filter((r) => r.outcome === "replicated").length,
    failed: reps.filter((r) => r.outcome === "failed").length,
    partial: reps.filter((r) => r.outcome === "partial").length,
    inconclusive: reps.filter((r) => r.outcome === "inconclusive").length,
  };
}

// ── Datasets and code artifacts (v0.33) ─────────────────────────────
//
// Datasets and code artifacts are first-class kernel objects under
// `.vela/datasets/` and `.vela/code-artifacts/`. Each `vd_<hash>` is a
// versioned, content-addressed reference to data; each `vc_<hash>` is
// a content-addressed pointer at a region of source code at a specific
// git commit. Both extend the "Git for science" claim from
// aspirational to operational.

export interface Dataset {
  id: string;
  name: string;
  version?: string | null;
  schema?: Array<[string, string]>;
  row_count?: number | null;
  content_hash: string;
  url?: string | null;
  license?: string | null;
  provenance: Provenance;
  created: string;
}

export interface CodeArtifact {
  id: string;
  language: string;
  repo_url?: string | null;
  git_commit?: string | null;
  path: string;
  line_range?: [number, number] | null;
  content_hash: string;
  entry_point?: string | null;
  created: string;
}

let _dsCache: Dataset[] | null = null;
let _caCache: CodeArtifact[] | null = null;

function datasetsDir(): string {
  return join(repoRoot(), FRONTIER.repoPath, ".vela", "datasets");
}
function codeArtifactsDir(): string {
  return join(repoRoot(), FRONTIER.repoPath, ".vela", "code-artifacts");
}

export function loadDatasets(): Dataset[] {
  if (_dsCache) return _dsCache;
  const dir = datasetsDir();
  if (!existsSync(dir)) {
    _dsCache = [];
    return _dsCache;
  }
  const out: Dataset[] = [];
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
    try {
      out.push(JSON.parse(readFileSync(join(dir, file), "utf8")) as Dataset);
    } catch (err) {
      console.warn(`[frontier] failed to parse dataset ${file}:`, err);
    }
  }
  out.sort((a, b) => a.name.localeCompare(b.name));
  _dsCache = out;
  return _dsCache;
}

export function loadCodeArtifacts(): CodeArtifact[] {
  if (_caCache) return _caCache;
  const dir = codeArtifactsDir();
  if (!existsSync(dir)) {
    _caCache = [];
    return _caCache;
  }
  const out: CodeArtifact[] = [];
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
    try {
      out.push(JSON.parse(readFileSync(join(dir, file), "utf8")) as CodeArtifact);
    } catch (err) {
      console.warn(`[frontier] failed to parse code artifact ${file}:`, err);
    }
  }
  out.sort((a, b) => a.path.localeCompare(b.path));
  _caCache = out;
  return _caCache;
}

export function findDataset(id: string): Dataset | undefined {
  return loadDatasets().find((d) => d.id === id);
}
export function findCodeArtifact(id: string): CodeArtifact | undefined {
  return loadCodeArtifacts().find((c) => c.id === id);
}

// ── Predictions + resolutions (v0.34) ───────────────────────────────
//
// The kernel's epistemic accountability layer. Predictions live under
// `.vela/predictions/<vpred_id>.json`; resolutions under
// `.vela/resolutions/<vres_id>.json`. The site computes per-actor
// calibration (Brier, log score, hit rate) at build time from the
// resolved subset.

export type ExpectedOutcome =
  | { kind: "affirmed" }
  | { kind: "falsified" }
  | { kind: "quantitative"; value: number; tolerance: number; units: string }
  | { kind: "categorical"; value: string };

export interface Prediction {
  id: string;
  claim_text: string;
  target_findings: string[];
  predicted_at: string;
  resolves_by?: string | null;
  resolution_criterion: string;
  expected_outcome: ExpectedOutcome;
  made_by: string;
  confidence: number;
  conditions: Conditions;
}

export interface Resolution {
  id: string;
  prediction_id: string;
  actual_outcome: string;
  matched_expected: boolean;
  resolved_at: string;
  resolved_by: string;
  evidence: Evidence;
  confidence: number;
}

export interface CalibrationRecord {
  actor: string;
  n_predictions: number;
  n_resolved: number;
  n_hit: number;
  hit_rate: number | null;
  brier_score: number | null;
  log_score: number | null;
  reliability_buckets: Array<[number, number, number]>;
}

let _predCache: Prediction[] | null = null;
let _resCache: Resolution[] | null = null;

function predictionsDir(): string {
  return join(repoRoot(), FRONTIER.repoPath, ".vela", "predictions");
}
function resolutionsDir(): string {
  return join(repoRoot(), FRONTIER.repoPath, ".vela", "resolutions");
}

export function loadPredictions(): Prediction[] {
  if (_predCache) return _predCache;
  const dir = predictionsDir();
  if (!existsSync(dir)) {
    _predCache = [];
    return _predCache;
  }
  const out: Prediction[] = [];
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
    try {
      out.push(JSON.parse(readFileSync(join(dir, file), "utf8")) as Prediction);
    } catch (err) {
      console.warn(`[frontier] failed to parse prediction ${file}:`, err);
    }
  }
  // Stable order: by deadline ascending; predictions without deadlines last.
  out.sort((a, b) => {
    const ad = a.resolves_by ?? "9999";
    const bd = b.resolves_by ?? "9999";
    return ad.localeCompare(bd);
  });
  _predCache = out;
  return _predCache;
}

export function loadResolutions(): Resolution[] {
  if (_resCache) return _resCache;
  const dir = resolutionsDir();
  if (!existsSync(dir)) {
    _resCache = [];
    return _resCache;
  }
  const out: Resolution[] = [];
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
    try {
      out.push(JSON.parse(readFileSync(join(dir, file), "utf8")) as Resolution);
    } catch (err) {
      console.warn(`[frontier] failed to parse resolution ${file}:`, err);
    }
  }
  out.sort((a, b) => b.resolved_at.localeCompare(a.resolved_at));
  _resCache = out;
  return _resCache;
}

export function isResolved(predictionId: string): boolean {
  return loadResolutions().some((r) => r.prediction_id === predictionId);
}

export function resolutionFor(predictionId: string): Resolution | undefined {
  return loadResolutions().find((r) => r.prediction_id === predictionId);
}

export function predictionsForFinding(vfId: string): Prediction[] {
  return loadPredictions().filter((p) => p.target_findings.includes(vfId));
}

/* Compute per-actor calibration records. Mirrors the Rust
   `crates/vela-protocol/src/calibration.rs` logic so site rendering and
   `vela calibration` agree byte-for-byte on the same frontier. */
export function calibrationRecords(): CalibrationRecord[] {
  const preds = loadPredictions();
  const resolutions = loadResolutions();
  const resolutionByPred = new Map<string, Resolution>();
  for (const r of resolutions) resolutionByPred.set(r.prediction_id, r);

  const byActor = new Map<string, Prediction[]>();
  for (const p of preds) {
    if (!byActor.has(p.made_by)) byActor.set(p.made_by, []);
    byActor.get(p.made_by)!.push(p);
  }

  const records: CalibrationRecord[] = [];
  for (const [actor, ps] of byActor) {
    const resolvedPairs = ps
      .map((p) => {
        const r = resolutionByPred.get(p.id);
        return r ? ({ p, r } as { p: Prediction; r: Resolution }) : null;
      })
      .filter((x): x is { p: Prediction; r: Resolution } => x !== null);
    const n_predictions = ps.length;
    const n_resolved = resolvedPairs.length;
    const n_hit = resolvedPairs.filter((x) => x.r.matched_expected).length;
    const hit_rate = n_resolved > 0 ? n_hit / n_resolved : null;
    const brier_score =
      n_resolved > 0
        ? resolvedPairs
            .map(({ p, r }) => Math.pow(p.confidence - (r.matched_expected ? 1 : 0), 2))
            .reduce((a, b) => a + b, 0) / n_resolved
        : null;
    const log_score =
      n_resolved > 0
        ? resolvedPairs
            .map(({ p, r }) => {
              const pa = r.matched_expected ? p.confidence : 1 - p.confidence;
              return Math.log(Math.min(Math.max(pa, 1e-9), 1 - 1e-9));
            })
            .reduce((a, b) => a + b, 0) / n_resolved
        : null;

    const bands: Array<[number, number]> = [
      [0.0, 0.2],
      [0.2, 0.4],
      [0.4, 0.6],
      [0.6, 0.8],
      [0.8, 1.001],
    ];
    const reliability_buckets: Array<[number, number, number]> = [];
    for (const [lo, hi] of bands) {
      const inBand = resolvedPairs.filter(
        ({ p }) => p.confidence >= lo && p.confidence < hi,
      );
      if (inBand.length === 0) continue;
      const hits = inBand.filter((x) => x.r.matched_expected).length;
      reliability_buckets.push([lo, hits / inBand.length, inBand.length]);
    }

    records.push({
      actor,
      n_predictions,
      n_resolved,
      n_hit,
      hit_rate,
      brier_score,
      log_score,
      reliability_buckets,
    });
  }
  records.sort((a, b) => a.actor.localeCompare(b.actor));
  return records;
}

export function calibrationFor(actor: string): CalibrationRecord | undefined {
  return calibrationRecords().find((r) => r.actor === actor);
}

// ── v0.44.1 causal graph (site mirror of crate::causal_graph) ────────

export interface CausalNode {
  vf_id: string;
  slug: string;
  short_id: string;
  assertion_text: string;
  verdict: Identifiability;
  causal_claim?: CausalClaim;
  causal_evidence_grade?: CausalEvidenceGrade;
}

export interface CausalEdge {
  from: string;  // vf_id of the dependent (child)
  to: string;    // vf_id of the parent (cause)
}

export interface CausalGraphData {
  nodes: Map<string, CausalNode>;
  edges: CausalEdge[];
  parents: Map<string, string[]>;
  children: Map<string, string[]>;
}

let _graphCache: CausalGraphData | null = null;

/* Build the substrate's causal-link graph from the loaded findings.
   Mirrors the Rust crate::causal_graph::CausalGraph::from_project
   shape: nodes = findings, directed edges = depends/supports links
   (edge points from dependent → cause). */
export function loadCausalGraph(): CausalGraphData {
  if (_graphCache) return _graphCache;
  const claims = loadFrontier();
  const audit = auditFrontier();
  const auditByFinding = new Map(audit.map((e) => [e.finding_id, e]));

  const nodes = new Map<string, CausalNode>();
  for (const c of claims) {
    const a = auditByFinding.get(c.finding.id);
    nodes.set(c.finding.id, {
      vf_id: c.finding.id,
      slug: c.slug,
      short_id: c.shortId,
      assertion_text: c.finding.assertion.text,
      verdict: a?.verdict ?? "underdetermined",
      causal_claim: c.finding.assertion.causal_claim,
      causal_evidence_grade: c.finding.assertion.causal_evidence_grade,
    });
  }

  const edges: CausalEdge[] = [];
  const parents = new Map<string, string[]>();
  const children = new Map<string, string[]>();
  for (const id of nodes.keys()) {
    parents.set(id, []);
    children.set(id, []);
  }
  for (const c of claims) {
    for (const link of c.finding.links ?? []) {
      if (link.type !== "depends" && link.type !== "supports") continue;
      if (link.target.includes("@")) continue; // cross-frontier, defer
      if (!nodes.has(link.target)) continue;
      edges.push({ from: c.finding.id, to: link.target });
      parents.get(c.finding.id)!.push(link.target);
      children.get(link.target)!.push(c.finding.id);
    }
  }
  _graphCache = { nodes, edges, parents, children };
  return _graphCache;
}

/* Local neighborhood: focal + its parents (1 hop up) + its children
   (1 hop down) + grandparents and grandchildren (2 hops). Limits the
   render to something legible; full graph would be 188 nodes for
   the BBB. */
export function causalNeighborhood(
  vf_id: string,
): { focal: CausalNode; parents: CausalNode[]; grandparents: CausalNode[]; children: CausalNode[]; grandchildren: CausalNode[]; edges: CausalEdge[] } | null {
  const g = loadCausalGraph();
  const focal = g.nodes.get(vf_id);
  if (!focal) return null;

  const parents = (g.parents.get(vf_id) ?? [])
    .map((id) => g.nodes.get(id))
    .filter((n): n is CausalNode => n != null);
  const children = (g.children.get(vf_id) ?? [])
    .map((id) => g.nodes.get(id))
    .filter((n): n is CausalNode => n != null);

  const parentIds = new Set(parents.map((n) => n.vf_id));
  const childIds = new Set(children.map((n) => n.vf_id));

  const grandparents = Array.from(
    new Set(parents.flatMap((p) => g.parents.get(p.vf_id) ?? [])),
  )
    .filter((id) => id !== vf_id && !parentIds.has(id) && !childIds.has(id))
    .map((id) => g.nodes.get(id))
    .filter((n): n is CausalNode => n != null);

  const grandchildren = Array.from(
    new Set(children.flatMap((c) => g.children.get(c.vf_id) ?? [])),
  )
    .filter(
      (id) =>
        id !== vf_id &&
        !parentIds.has(id) &&
        !childIds.has(id) &&
        !grandparents.some((g) => g.vf_id === id),
    )
    .map((id) => g.nodes.get(id))
    .filter((n): n is CausalNode => n != null);

  // Collect edges that connect any visible node to any other.
  const visible = new Set<string>([
    focal.vf_id,
    ...parents.map((n) => n.vf_id),
    ...children.map((n) => n.vf_id),
    ...grandparents.map((n) => n.vf_id),
    ...grandchildren.map((n) => n.vf_id),
  ]);
  const edges = g.edges.filter((e) => visible.has(e.from) && visible.has(e.to));

  return { focal, parents, grandparents, children, grandchildren, edges };
}

/* Effect identifiability for one (source, target) pair, mirroring
   the Rust crate::causal_graph::identify_effect. Conservative: tries
   the empty adjustment set, then singletons, then bounded pairs.
   The kernel has the canonical implementation; this is a render-side
   helper for site visualizations. */
export type EffectVerdict =
  | { kind: "identified"; adjustment_set: string[] }
  | { kind: "identified_by_front_door"; mediator_set: string[] }
  | { kind: "no_causal_path"; reason: string }
  | { kind: "underidentified"; unblocked: string[][] }
  | { kind: "unknown_node"; which: string };

function pathBlocked(path: string[], z: Set<string>, g: CausalGraphData): boolean {
  if (path.length < 3) return false;
  for (let i = 1; i < path.length - 1; i++) {
    const prev = path[i - 1];
    const node = path[i];
    const next = path[i + 1];
    const prevIsParentOfNode = (g.parents.get(node) ?? []).includes(prev);
    const nextIsParentOfNode = (g.parents.get(node) ?? []).includes(next);
    const prevIsChildOfNode = (g.children.get(node) ?? []).includes(prev);
    const nextIsChildOfNode = (g.children.get(node) ?? []).includes(next);
    const isCollider = prevIsParentOfNode && nextIsParentOfNode;
    const isFork = prevIsChildOfNode && nextIsChildOfNode;
    if (isCollider) {
      // Blocked iff node and all descendants not in Z.
      if (!z.has(node)) {
        const descs = descendantsOf(node, g);
        const anyDescInZ = Array.from(descs).some((d) => z.has(d));
        if (!anyDescInZ) return true;
      }
    } else if (isFork) {
      if (z.has(node)) return true;
    } else {
      // chain
      if (z.has(node)) return true;
    }
  }
  return false;
}

function descendantsOf(node: string, g: CausalGraphData): Set<string> {
  const seen = new Set<string>();
  const queue: string[] = [...(g.children.get(node) ?? [])];
  while (queue.length) {
    const n = queue.shift()!;
    if (seen.has(n)) continue;
    seen.add(n);
    for (const c of g.children.get(n) ?? []) {
      if (!seen.has(c)) queue.push(c);
    }
  }
  return seen;
}

function isBackDoorPath(path: string[], x: string, g: CausalGraphData): boolean {
  if (path.length < 2 || path[0] !== x) return false;
  const second = path[1];
  return (g.parents.get(x) ?? []).includes(second);
}

function pathsBetween(
  start: string,
  end: string,
  g: CausalGraphData,
  maxPaths = 100,
  maxLen = 6,
): string[][] {
  if (start === end || !g.nodes.has(start) || !g.nodes.has(end)) return [];
  const all: string[][] = [];
  const dfs = (cur: string[], visited: Set<string>) => {
    if (all.length >= maxPaths) return;
    if (cur.length > maxLen) return;
    const node = cur[cur.length - 1];
    const neighbors = new Set<string>();
    for (const p of g.parents.get(node) ?? []) neighbors.add(p);
    for (const c of g.children.get(node) ?? []) neighbors.add(c);
    for (const next of neighbors) {
      if (visited.has(next)) continue;
      cur.push(next);
      visited.add(next);
      if (next === end) {
        all.push([...cur]);
      } else {
        dfs(cur, visited);
      }
      visited.delete(next);
      cur.pop();
      if (all.length >= maxPaths) return;
    }
  };
  const visited = new Set<string>([start]);
  dfs([start], visited);
  return all;
}

export function identifyEffect(source: string, target: string): EffectVerdict {
  const g = loadCausalGraph();
  if (!g.nodes.has(source)) return { kind: "unknown_node", which: source };
  if (!g.nodes.has(target)) return { kind: "unknown_node", which: target };
  if (source === target) return { kind: "no_causal_path", reason: "same node" };

  const paths = pathsBetween(source, target, g);
  if (paths.length === 0) return { kind: "no_causal_path", reason: "no path" };

  const backDoor = paths.filter((p) => isBackDoorPath(p, source, g));
  const descSrc = descendantsOf(source, g);
  const candidates = Array.from(g.nodes.keys()).filter(
    (n) => n !== source && n !== target && !descSrc.has(n),
  );

  const blocksAll = (z: Set<string>) =>
    backDoor.every((p) => pathBlocked(p, z, g));

  if (blocksAll(new Set())) {
    return { kind: "identified", adjustment_set: [] };
  }
  for (const c of candidates) {
    const z = new Set([c]);
    if (blocksAll(z)) {
      return { kind: "identified", adjustment_set: [c] };
    }
  }
  // Pair search bounded
  let tried = 0;
  for (let i = 0; i < candidates.length; i++) {
    for (let j = i + 1; j < candidates.length; j++) {
      const z = new Set([candidates[i], candidates[j]]);
      tried++;
      if (blocksAll(z)) {
        return { kind: "identified", adjustment_set: [candidates[i], candidates[j]] };
      }
      if (tried > 1500) break;
    }
    if (tried > 1500) break;
  }

  // v0.44.2: back-door failed. Try front-door (Pearl 1995 §3.3).
  const frontDoorM = findFrontDoorMediator(source, target, paths, g);
  if (frontDoorM != null) {
    return { kind: "identified_by_front_door", mediator_set: [frontDoorM] };
  }

  const unblocked = backDoor.filter((p) => !pathBlocked(p, new Set(), g)).slice(0, 5);
  return { kind: "underidentified", unblocked };
}

/* v0.44.2: site-side front-door criterion. Search for a singleton
   mediator M satisfying Pearl's three conditions:
     1. Every directed path source → target passes through M.
     2. No back-door path source → m is open under the empty set.
     3. Every back-door path m → target is blocked by {source}.
   The Rust kernel has the canonical implementation. */
function isDirectedPath(path: string[], g: CausalGraphData): boolean {
  if (path.length < 2) return false;
  for (let i = 0; i < path.length - 1; i++) {
    const a = path[i];
    const b = path[i + 1];
    const aIsParentOfB = (g.parents.get(b) ?? []).includes(a);
    if (!aIsParentOfB) return false;
  }
  return true;
}

function ancestorsOf(node: string, g: CausalGraphData): Set<string> {
  const seen = new Set<string>();
  const queue: string[] = [...(g.parents.get(node) ?? [])];
  while (queue.length) {
    const n = queue.shift()!;
    if (seen.has(n)) continue;
    seen.add(n);
    for (const p of g.parents.get(n) ?? []) {
      if (!seen.has(p)) queue.push(p);
    }
  }
  return seen;
}

function findFrontDoorMediator(
  source: string,
  target: string,
  allPathsST: string[][],
  g: CausalGraphData,
): string | null {
  const directedST = allPathsST.filter((p) => isDirectedPath(p, g));
  if (directedST.length === 0) return null;

  const descSrc = descendantsOf(source, g);
  const ancTgt = ancestorsOf(target, g);
  const candidates = Array.from(g.nodes.keys()).filter(
    (n) =>
      n !== source &&
      n !== target &&
      descSrc.has(n) &&
      ancTgt.has(n),
  );

  const sourceSet = new Set([source]);

  for (const m of candidates) {
    // Condition 1: M intercepts every directed source → target path.
    const interceptsAll = directedST.every((p) => p.includes(m));
    if (!interceptsAll) continue;

    // Condition 2: no open back-door path source → m.
    const pathsSM = pathsBetween(source, m, g);
    const backDoorSM = pathsSM.filter((p) => isBackDoorPath(p, source, g));
    const anyOpen = backDoorSM.some((p) => !pathBlocked(p, new Set(), g));
    if (anyOpen) continue;

    // Condition 3: every back-door m → target blocked by {source}.
    const pathsMT = pathsBetween(m, target, g);
    const backDoorMT = pathsMT.filter((p) => isBackDoorPath(p, m, g));
    const allBlocked = backDoorMT.every((p) => pathBlocked(p, sourceSet, g));
    if (!allBlocked) continue;

    return m;
  }
  return null;
}

// ── Counterfactual queries (v0.45 — Pearl level 3) ───────────────────
//
// Twin-network propagation over the claim graph using the optional
// `Mechanism` annotation on each `depends`/`supports` link. Mirrors
// `crates/vela-protocol/src/counterfactual.rs`; the Rust kernel is
// canonical, the site renders the same answer.

export interface CounterfactualQuery {
  intervene_on: string;
  set_to: number;
  target: string;
}

export type CounterfactualVerdict =
  | {
      kind: "resolved";
      factual: number;
      counterfactual: number;
      delta: number;
      paths_used: string[][];
    }
  | { kind: "mechanism_unspecified"; unspecified_edges: [string, string][] }
  | { kind: "no_causal_path"; factual: number }
  | { kind: "unknown_node"; which: string }
  | { kind: "invalid_intervention"; reason: string };

/* BFS-enumerate directed parent → child paths from cause to effect. */
function directedPathsFromTo(
  cause: string,
  effect: string,
  g: CausalGraphData,
  maxDepth = 8,
  maxPaths = 32,
): string[][] {
  const out: string[][] = [];
  const queue: string[][] = [[cause]];
  while (queue.length) {
    if (out.length >= maxPaths) break;
    const path = queue.shift()!;
    if (path.length > maxDepth) continue;
    const last = path[path.length - 1];
    if (last === effect && path.length > 1) {
      out.push(path);
      continue;
    }
    for (const child of g.children.get(last) ?? []) {
      if (path.includes(child)) continue;
      queue.push([...path, child]);
    }
  }
  return out;
}

/* Build a (parent, child) → Mechanism index from the loaded findings.
   Convention: a `depends`/`supports` link from finding A to target B
   means A is the dependent (child) and B is the parent. */
function loadMechanismIndex(): Map<string, Mechanism> {
  const claims = loadFrontier();
  const idx = new Map<string, Mechanism>();
  for (const c of claims) {
    for (const link of c.finding.links ?? []) {
      if (link.type !== "depends" && link.type !== "supports") continue;
      if (!link.mechanism) continue;
      const target = link.target.includes("@")
        ? link.target.split("@")[0]
        : link.target;
      idx.set(`${target}|${c.finding.id}`, link.mechanism);
    }
  }
  return idx;
}

function loadConfidenceIndex(): Map<string, number> {
  const claims = loadFrontier();
  const idx = new Map<string, number>();
  for (const c of claims) {
    idx.set(c.finding.id, c.finding.confidence?.score ?? 0);
  }
  return idx;
}

export function answerCounterfactual(q: CounterfactualQuery): CounterfactualVerdict {
  if (!(q.set_to >= 0 && q.set_to <= 1)) {
    return {
      kind: "invalid_intervention",
      reason: `intervention must be on the confidence axis [0,1], got ${q.set_to}`,
    };
  }

  const g = loadCausalGraph();
  if (!g.nodes.has(q.intervene_on)) return { kind: "unknown_node", which: q.intervene_on };
  if (!g.nodes.has(q.target)) return { kind: "unknown_node", which: q.target };

  const conf = loadConfidenceIndex();
  const factualSrc = conf.get(q.intervene_on) ?? 0;
  const factualTgt = conf.get(q.target) ?? 0;

  const paths = directedPathsFromTo(q.intervene_on, q.target, g);
  if (paths.length === 0) {
    return { kind: "no_causal_path", factual: factualTgt };
  }

  const mechIdx = loadMechanismIndex();
  const deltaX = q.set_to - factualSrc;

  const unspecified = new Set<string>();
  const pathDeltas: number[] = [];
  const pathsUsed: string[][] = [];

  for (const path of paths) {
    let delta = deltaX;
    let ok = true;
    for (let i = 0; i < path.length - 1; i++) {
      const parent = path[i];
      const child = path[i + 1];
      const m = mechIdx.get(`${parent}|${child}`);
      if (!m) {
        unspecified.add(`${parent}|${child}`);
        ok = false;
        break;
      }
      const next = applyMechanism(m, delta);
      if (next == null) {
        unspecified.add(`${parent}|${child}`);
        ok = false;
        break;
      }
      delta = next;
    }
    if (ok) {
      pathDeltas.push(delta);
      pathsUsed.push(path);
    }
  }

  if (pathDeltas.length === 0) {
    const edges: [string, string][] = Array.from(unspecified).map((s) => {
      const [a, b] = s.split("|");
      return [a, b];
    });
    edges.sort();
    return { kind: "mechanism_unspecified", unspecified_edges: edges };
  }

  // Max-magnitude aggregation, matching the Rust kernel.
  let agg = 0;
  for (const d of pathDeltas) {
    if (Math.abs(d) > Math.abs(agg)) agg = d;
  }
  const counterfactual = Math.max(0, Math.min(1, factualTgt + agg));
  return {
    kind: "resolved",
    factual: factualTgt,
    counterfactual,
    delta: counterfactual - factualTgt,
    paths_used: pathsUsed,
  };
}

/* Find pairs (cause, effect) in the loaded graph for which at least
   one directed path is fully mechanism-annotated. Used by the site to
   surface the set of "live" counterfactual queries. */
export function liveCounterfactualPairs(): { from: string; to: string }[] {
  const g = loadCausalGraph();
  const mechIdx = loadMechanismIndex();
  const pairs: { from: string; to: string }[] = [];
  for (const cause of g.nodes.keys()) {
    for (const effect of g.nodes.keys()) {
      if (cause === effect) continue;
      const paths = directedPathsFromTo(cause, effect, g, 6, 16);
      if (paths.length === 0) continue;
      const livePath = paths.some((p) =>
        p.slice(0, -1).every((_, i) => mechIdx.has(`${p[i]}|${p[i + 1]}`)),
      );
      if (livePath) pairs.push({ from: cause, to: effect });
    }
  }
  return pairs;
}

// ── Consensus aggregation (v0.35) ───────────────────────────────────
//
// Mirrors `crates/vela-protocol/src/aggregate.rs` byte-for-byte so the
// site renders the same numbers `vela consensus` returns. Doctrine:
// consensus is a derived view, never stored — same input frontier
// produces the same output, every time.

export type WeightingScheme =
  | "unweighted"
  | "replication_weighted"
  | "citation_weighted"
  | "composite";

export interface ConsensusConstituent {
  finding_id: string;
  assertion_text: string;
  raw_score: number;
  adjusted_score: number;
  weight: number;
  n_replications: number;
  n_replicated: number;
  n_failed_replications: number;
}

export interface ConsensusResult {
  target: string;
  target_assertion: string;
  n_findings: number;
  consensus_confidence: number;
  credible_interval_lo: number;
  credible_interval_hi: number;
  constituents: ConsensusConstituent[];
  weighting: WeightingScheme;
}

function tokenSet(text: string, minLen = 5): Set<string> {
  const out = new Set<string>();
  for (const raw of text.toLowerCase().split(/\s+/)) {
    if (raw.length <= minLen - 1) continue;
    const cleaned = raw.replace(/[^a-z0-9]+/g, "").trim();
    if (cleaned.length > 0) out.add(cleaned);
  }
  return out;
}

function entitySet(f: Finding): Set<string> {
  return new Set(
    (f.assertion?.entities ?? []).map((e) => (e?.name ?? "").toLowerCase()).filter(Boolean),
  );
}

function isSimilar(
  candidate: Finding,
  targetEntities: Set<string>,
  targetWords: Set<string>,
  targetType: string,
): boolean {
  const candEntities = entitySet(candidate);
  const sharedEntities = [...candEntities].filter((e) => targetEntities.has(e));
  const entityOverlap = sharedEntities.length >= 1;

  const candWords = tokenSet(candidate.assertion?.text ?? "");
  const sharedWords = [...candWords].filter((w) => targetWords.has(w));
  const textOverlap = sharedWords.length >= 3;

  const typeMatch = candidate.assertion?.type === targetType;

  const signals = [entityOverlap, textOverlap, typeMatch].filter(Boolean).length;
  return signals >= 2 || (entityOverlap && sharedEntities.length >= 2);
}

function replicationTallies(targetId: string): {
  total: number;
  replicated: number;
  failed: number;
} {
  let total = 0;
  let replicated = 0;
  let failed = 0;
  for (const r of loadReplications()) {
    if (r.target_finding === targetId) {
      total++;
      if (r.outcome === "replicated") replicated++;
      else if (r.outcome === "failed") failed++;
    }
  }
  return { total, replicated, failed };
}

function adjustScore(
  raw: number,
  nReplicated: number,
  nFailed: number,
  contested: boolean,
): number {
  let adj = raw + 0.05 * nReplicated - 0.1 * nFailed;
  if (contested) adj *= 0.85;
  return Math.min(1, Math.max(0, adj));
}

function computeWeight(
  scheme: WeightingScheme,
  f: Finding,
  nReplicated: number,
  nFailed: number,
): number {
  const base = 1.0;
  const replicationFactor = 1.0 + 0.5 * nReplicated - 0.5 * nFailed;
  const citation = f.provenance?.citation_count ?? 0;
  const citationFactor = 1.0 + Math.log1p(citation) * 0.10;
  switch (scheme) {
    case "unweighted":
      return base;
    case "replication_weighted":
      return Math.max(0, replicationFactor);
    case "citation_weighted":
      return Math.max(0, citationFactor);
    case "composite":
      return Math.max(
        0,
        0.2 * base + 0.5 * Math.max(0, replicationFactor) + 0.3 * Math.max(0, citationFactor),
      );
  }
}

export function consensusFor(
  targetId: string,
  scheme: WeightingScheme = "composite",
): ConsensusResult | null {
  const all = loadFrontier();
  const target = all.find((c) => c.finding.id === targetId)?.finding;
  if (!target) return null;
  const targetEntities = entitySet(target);
  const targetWords = tokenSet(target.assertion?.text ?? "");
  const targetType = target.assertion?.type ?? "";

  const candidates: Finding[] = [];
  for (const c of all) {
    const f = c.finding;
    if (f.id === targetId) {
      candidates.push(f);
      continue;
    }
    if (isSimilar(f, targetEntities, targetWords, targetType)) {
      candidates.push(f);
    }
  }

  const constituents: ConsensusConstituent[] = candidates.map((f) => {
    const t = replicationTallies(f.id);
    const raw_score = f.confidence?.score ?? 0;
    const adjusted_score = adjustScore(raw_score, t.replicated, t.failed, !!f.flags?.contested);
    const weight = computeWeight(scheme, f, t.replicated, t.failed);
    return {
      finding_id: f.id,
      assertion_text: f.assertion?.text ?? "",
      raw_score,
      adjusted_score,
      weight,
      n_replications: t.total,
      n_replicated: t.replicated,
      n_failed_replications: t.failed,
    };
  });

  const totalWeight = constituents.reduce((s, c) => s + c.weight, 0);
  const consensus =
    totalWeight > 0
      ? constituents.reduce((s, c) => s + c.adjusted_score * c.weight, 0) / totalWeight
      : constituents.length > 0
        ? constituents.reduce((s, c) => s + c.adjusted_score, 0) / constituents.length
        : 0;

  let lo = consensus;
  let hi = consensus;
  if (constituents.length > 0 && totalWeight > 0) {
    const variance =
      constituents.reduce((s, c) => s + c.weight * Math.pow(c.adjusted_score - consensus, 2), 0) /
      totalWeight;
    const sd = Math.sqrt(variance);
    lo = Math.max(0, consensus - 1.96 * sd);
    hi = Math.min(1, consensus + 1.96 * sd);
  }

  return {
    target: target.id,
    target_assertion: target.assertion?.text ?? "",
    n_findings: constituents.length,
    consensus_confidence: Math.round(consensus * 1000) / 1000,
    credible_interval_lo: Math.round(lo * 1000) / 1000,
    credible_interval_hi: Math.round(hi * 1000) / 1000,
    constituents,
    weighting: scheme,
  };
}

// ── Aggregations used by pages ──────────────────────────────────────

export function findBySlug(slug: string): ClaimView | undefined {
  return loadFrontier().find((c) => c.slug === slug);
}

export function findById(vfId: string): ClaimView | undefined {
  return loadFrontier().find((c) => c.finding.id === vfId);
}

export function claimsForTarget(slug: TargetSlug): ClaimView[] {
  return loadFrontier().filter((c) => c.targets.includes(slug));
}

export function claimsForTrial(slug: TrialSlug): ClaimView[] {
  return loadFrontier().filter((c) => c.trials.includes(slug));
}

export function contradictions(): ClaimView[] {
  return loadFrontier().filter(
    (c) => c.finding.flags.contested || c.finding.assertion.type === "tension",
  );
}

export interface FrontierStats {
  claims: number;
  contradictions: number;
  papers: number;
  lastUpdated: string | null;
}

export function frontierStats(): FrontierStats {
  const claims = loadFrontier();
  const dois = new Set<string>();
  let last = "";
  for (const c of claims) {
    if (c.finding.provenance.doi) dois.add(c.finding.provenance.doi);
    else if (c.finding.provenance.pmid) dois.add(`pmid:${c.finding.provenance.pmid}`);
    else if (c.finding.provenance.title) dois.add(c.finding.provenance.title);
    if (c.finding.created > last) last = c.finding.created;
    if (c.finding.updated && c.finding.updated > last) last = c.finding.updated;
  }
  return {
    claims: claims.length,
    contradictions: contradictions().length,
    papers: dois.size,
    lastUpdated: last || null,
  };
}

// ── Citation rendering ──────────────────────────────────────────────

// ── Weekly diffs ────────────────────────────────────────────────────
//
// "What changed in the frontier this week?" The diff is a derived
// view over `created` and `updated` timestamps, plus a summary of
// contradictions resolved. v0.32 will replace this with a signed
// `weekly_diff` event read from `.vela/events/`; for now the site
// computes the diff at build time so the rhythm starts immediately.

export interface WeekKey {
  // ISO 8601 year + week, e.g. "2026-W18".
  key: string;
  // Inclusive start (Monday 00:00 UTC), exclusive end.
  start: string; // ISO date "YYYY-MM-DD"
  end: string;   // ISO date "YYYY-MM-DD" (next Monday)
  year: number;
  week: number;
}

export interface WeeklyDiff {
  week: WeekKey;
  added: ClaimView[];
  updated: ClaimView[];
  // Contradictions whose `created` lies in this week.
  newContradictions: ClaimView[];
  // Total claims as of end-of-week (computed against `created`).
  cumulativeClaims: number;
}

/* ISO week date math. Returns the Monday-based week index for a date. */
function isoWeek(date: Date): { year: number; week: number } {
  // Source: ISO 8601 — week starts Monday, week 1 contains the year's
  // first Thursday.
  const d = new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate()));
  const day = d.getUTCDay() || 7;
  d.setUTCDate(d.getUTCDate() + 4 - day);
  const yearStart = new Date(Date.UTC(d.getUTCFullYear(), 0, 1));
  const week = Math.ceil(((d.getTime() - yearStart.getTime()) / 86_400_000 + 1) / 7);
  return { year: d.getUTCFullYear(), week };
}

function isoWeekStart(year: number, week: number): Date {
  // The Monday of ISO week `week` of `year`.
  const jan4 = new Date(Date.UTC(year, 0, 4));
  const jan4Day = jan4.getUTCDay() || 7;
  const week1Mon = new Date(jan4);
  week1Mon.setUTCDate(jan4.getUTCDate() - (jan4Day - 1));
  const target = new Date(week1Mon);
  target.setUTCDate(week1Mon.getUTCDate() + (week - 1) * 7);
  return target;
}

function pad(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function weekKey(year: number, week: number): WeekKey {
  const start = isoWeekStart(year, week);
  const end = new Date(start);
  end.setUTCDate(end.getUTCDate() + 7);
  return {
    key: `${year}-W${pad(week)}`,
    start: start.toISOString().slice(0, 10),
    end: end.toISOString().slice(0, 10),
    year,
    week,
  };
}

/* All ISO weeks that contain at least one finding event. Sorted
   newest-first for surfacing in nav. */
export function activeWeeks(): WeekKey[] {
  const seen = new Map<string, WeekKey>();
  for (const c of loadFrontier()) {
    const ts = c.finding.updated || c.finding.created;
    const { year, week } = isoWeek(new Date(ts));
    const k = weekKey(year, week);
    if (!seen.has(k.key)) seen.set(k.key, k);
  }
  return [...seen.values()].sort((a, b) => b.key.localeCompare(a.key));
}

export function diffForWeek(weekKeyStr: string): WeeklyDiff | null {
  const m = weekKeyStr.match(/^(\d{4})-W(\d{2})$/);
  if (!m) return null;
  const year = Number(m[1]);
  const week = Number(m[2]);
  const wk = weekKey(year, week);
  const startMs = Date.parse(wk.start + "T00:00:00Z");
  const endMs = Date.parse(wk.end + "T00:00:00Z");

  const added: ClaimView[] = [];
  const updated: ClaimView[] = [];
  let cumulative = 0;

  for (const c of loadFrontier()) {
    const createdMs = Date.parse(c.finding.created);
    if (createdMs < endMs) cumulative++;
    if (createdMs >= startMs && createdMs < endMs) {
      added.push(c);
      continue;
    }
    if (c.finding.updated) {
      const upd = Date.parse(c.finding.updated);
      if (upd >= startMs && upd < endMs) updated.push(c);
    }
  }

  const newContradictions = added.filter(
    (c) => c.finding.flags.contested || c.finding.assertion.type === "tension",
  );

  return {
    week: wk,
    added,
    updated,
    newContradictions,
    cumulativeClaims: cumulative,
  };
}

export function currentWeek(): WeekKey {
  const { year, week } = isoWeek(new Date());
  return weekKey(year, week);
}

// ── Citation rendering ──────────────────────────────────────────────

export function shortCitation(p: Provenance): string {
  if (!p.authors?.length) return p.title || "Untitled source";
  const first = p.authors[0]?.name || "Unknown";
  const etAl = p.authors.length > 1 ? " et al." : "";
  const year = p.year ? ` ${p.year}` : "";
  const journal = p.journal ? `, ${p.journal}` : "";
  return `${first}${etAl}${year}${journal}`;
}

export function doiUrl(p: Provenance): string | null {
  if (p.doi) return `https://doi.org/${p.doi}`;
  if (p.pmid) return `https://pubmed.ncbi.nlm.nih.gov/${p.pmid}/`;
  return null;
}

// ── v0.40 causal reasoning (site mirror of crate::causal_reasoning) ─

export type Identifiability =
  | "identified"
  | "conditional"
  | "underidentified"
  | "underdetermined";

export interface AuditEntry {
  finding_id: string;
  slug: string;
  short_id: string;
  assertion_text: string;
  causal_claim?: CausalClaim;
  causal_evidence_grade?: CausalEvidenceGrade;
  verdict: Identifiability;
  rationale: string;
  remediation: string;
}

export interface AuditSummary {
  total: number;
  identified: number;
  conditional: number;
  underidentified: number;
  underdetermined: number;
}

// Decision matrix from crates/vela-protocol/src/causal_reasoning.rs.
// Kept in sync by hand — there's no codegen, but the matrix is small,
// stable, and the Rust kernel has unit tests that pin it.
function isIdentifiable(
  claim: CausalClaim | undefined,
  grade: CausalEvidenceGrade | undefined,
): Identifiability {
  if (!claim || !grade) return "underdetermined";
  if (claim === "correlation") return "identified";
  if (grade === "rct") return "identified";
  // Mediation
  if (claim === "mediation" && grade === "quasi_experimental") return "conditional";
  if (claim === "mediation") return "underidentified";
  // Intervention
  if (claim === "intervention" && grade === "quasi_experimental") return "conditional";
  return "underidentified";
}

function rationaleFor(
  claim: CausalClaim,
  grade: CausalEvidenceGrade,
): string {
  if (claim === "correlation") {
    return "Correlation claims are admitted by any reasonable design.";
  }
  if (grade === "rct") {
    return claim === "intervention"
      ? "RCT design identifies intervention effects directly."
      : "RCT design identifies mediation pathways.";
  }
  if (claim === "mediation") {
    if (grade === "quasi_experimental") {
      return "Quasi-experimental design identifies mediation only when the instrument is valid and confounders are addressed.";
    }
    if (grade === "observational") {
      return "Observational data leaves the back-door problem open: confounders may explain the apparent mediation.";
    }
    return "Theoretical models propose mediation; they do not identify it from data.";
  }
  // intervention
  if (grade === "quasi_experimental") {
    return "Quasi-experimental design identifies intervention effects only under instrument validity.";
  }
  if (grade === "observational") {
    return "Observational data does not identify intervention effects (Rubin/Pearl: do(X=x) is unobserved).";
  }
  return "Theoretical analysis cannot identify intervention effects from real-world data alone.";
}

function remediationFor(
  verdict: Identifiability,
  claim: CausalClaim | undefined,
): string {
  if (verdict === "identified") return "No action; design supports the claim.";
  if (verdict === "conditional") {
    return "Document the additional assumptions (instrument validity, ignorability of confounders) on the finding as a caveat or evidence_span.";
  }
  if (verdict === "underdetermined") {
    return "Set causal_claim and causal_evidence_grade via `vela finding causal-set`.";
  }
  // underidentified
  if (claim === "intervention") {
    return "Either downgrade the claim from `intervention` to `correlation`, or attach RCT/QE-grade evidence that identifies the effect.";
  }
  if (claim === "mediation") {
    return "Either downgrade to `correlation`, or attach RCT/QE-grade evidence that closes the back-door pathways.";
  }
  return "Downgrade the claim or supply stronger evidence.";
}

export function auditFrontier(): AuditEntry[] {
  const claims = loadFrontier();
  const entries: AuditEntry[] = claims.map((c) => {
    const claim = c.finding.assertion.causal_claim;
    const grade = c.finding.assertion.causal_evidence_grade;
    const verdict = isIdentifiable(claim, grade);
    const rationale =
      claim && grade
        ? rationaleFor(claim, grade)
        : "Causal type or evidence grade unset.";
    return {
      finding_id: c.finding.id,
      slug: c.slug,
      short_id: c.shortId,
      assertion_text: c.finding.assertion.text,
      causal_claim: claim,
      causal_evidence_grade: grade,
      verdict,
      rationale,
      remediation: remediationFor(verdict, claim),
    };
  });
  // Reviewer-attention items first.
  const order: Record<Identifiability, number> = {
    underidentified: 0,
    conditional: 1,
    underdetermined: 2,
    identified: 3,
  };
  entries.sort((a, b) => order[a.verdict] - order[b.verdict]);
  return entries;
}

export function summarizeAudit(entries: AuditEntry[]): AuditSummary {
  const s: AuditSummary = {
    total: entries.length,
    identified: 0,
    conditional: 0,
    underidentified: 0,
    underdetermined: 0,
  };
  for (const e of entries) s[e.verdict]++;
  return s;
}

// ── v0.39 federation (site mirror of crate::federation) ──────────────

export interface PeerHub {
  id: string;
  url: string;
  public_key: string;
  added_at: string;
  note?: string;
}

export type ConflictKind =
  | "missing_in_peer"
  | "missing_locally"
  | "confidence_diverged"
  | "retracted_diverged"
  | "review_state_diverged"
  | "superseded_diverged"
  | "assertion_text_diverged"
  | "broken_locator"
  | "unverified_peer_entry";

export interface SyncRecord {
  timestamp: string;
  peer_id: string;
  our_snapshot_hash: string;
  peer_snapshot_hash: string;
  divergence_count: number;
}

export interface ConflictRecord {
  timestamp: string;
  peer_id: string;
  finding_id: string;
  kind: ConflictKind;
  detail: string;
}

function frontierEventsDir(): string {
  return join(repoRoot(), FRONTIER.repoPath, ".vela", "events");
}

function frontierPeersFile(): string {
  return join(repoRoot(), FRONTIER.repoPath, ".vela", "peers.json");
}

let _peersCache: PeerHub[] | null = null;
export function loadPeers(): PeerHub[] {
  if (_peersCache) return _peersCache;
  const path = frontierPeersFile();
  if (!existsSync(path)) {
    _peersCache = [];
    return _peersCache;
  }
  try {
    const raw = readFileSync(path, "utf8");
    _peersCache = JSON.parse(raw) as PeerHub[];
  } catch (err) {
    console.warn(`[frontier] failed to parse peers.json:`, err);
    _peersCache = [];
  }
  return _peersCache;
}

interface RawEvent {
  id: string;
  kind: string;
  target?: { type?: string; id?: string };
  timestamp?: string;
  payload?: Record<string, unknown>;
}

let _federationCache: { syncs: SyncRecord[]; conflicts: ConflictRecord[] } | null = null;

export function loadFederationEvents(): {
  syncs: SyncRecord[];
  conflicts: ConflictRecord[];
} {
  if (_federationCache) return _federationCache;
  const dir = frontierEventsDir();
  const syncs: SyncRecord[] = [];
  const conflicts: ConflictRecord[] = [];
  if (!existsSync(dir)) {
    _federationCache = { syncs, conflicts };
    return _federationCache;
  }
  for (const file of readdirSync(dir)) {
    if (!file.endsWith(".json")) continue;
    let ev: RawEvent;
    try {
      ev = JSON.parse(readFileSync(join(dir, file), "utf8")) as RawEvent;
    } catch {
      continue;
    }
    const ts = ev.timestamp ?? "";
    const p = ev.payload ?? {};
    if (ev.kind === "frontier.synced_with_peer") {
      syncs.push({
        timestamp: ts,
        peer_id: String(p.peer_id ?? ""),
        our_snapshot_hash: String(p.our_snapshot_hash ?? ""),
        peer_snapshot_hash: String(p.peer_snapshot_hash ?? ""),
        divergence_count: Number(p.divergence_count ?? 0),
      });
    } else if (ev.kind === "frontier.conflict_detected") {
      conflicts.push({
        timestamp: ts,
        peer_id: String(p.peer_id ?? ""),
        finding_id: String(p.finding_id ?? ""),
        kind: String(p.kind ?? "missing_in_peer") as ConflictKind,
        detail: String(p.detail ?? ""),
      });
    }
  }
  // Newest first for readable defaults.
  syncs.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
  conflicts.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
  _federationCache = { syncs, conflicts };
  return _federationCache;
}
