use crate::{
    benchmark, bridge, bundle, conformance, diff, events, export, lint, normalize,
    packet, project, propagate, proposals, repo, review, search, serve, sign, signals, sources,
    state, tensions, validate,
};

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::OnceLock;

use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::cli_style as style;
use reqwest::Client;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Parser)]
#[command(name = "vela", version = "0.40.0")]
#[command(about = "Portable frontier state for science")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// v0.22 Agent Inbox: run Literature Scout against a folder of
    /// PDFs. Each candidate finding becomes a `finding.add`
    /// `StateProposal` tagged with the scout's `AgentRun`, written
    /// to the frontier's `proposals` array. Reviewers accept or
    /// reject in the Workbench Inbox; nothing becomes a canonical
    /// finding without a signed accept.
    Scout {
        /// Folder of PDFs to read.
        folder: PathBuf,
        /// Frontier file the proposals are appended to.
        #[arg(long)]
        frontier: PathBuf,
        /// LLM backend override (matches `vela ingest --backend`).
        #[arg(short, long)]
        backend: Option<String>,
        /// Preview without writing to the frontier file.
        #[arg(long)]
        dry_run: bool,
        /// Output stable JSON for programmatic callers.
        #[arg(long)]
        json: bool,
    },
    /// v0.23 Agent Inbox: run Notes Compiler against a folder of
    /// Markdown / Obsidian notes. Each open question, hypothesis,
    /// candidate finding, or tension becomes a `finding.add`
    /// `StateProposal` tagged with the compiler's `AgentRun`,
    /// written to the frontier's `proposals` array. Same review
    /// loop as Literature Scout.
    CompileNotes {
        /// Vault or folder of Markdown notes to read.
        vault: PathBuf,
        /// Frontier file the proposals are appended to.
        #[arg(long)]
        frontier: PathBuf,
        /// Optional model alias (`sonnet`, `opus`, …).
        #[arg(short, long)]
        backend: Option<String>,
        /// Cap on files processed (default 50).
        #[arg(long)]
        max_files: Option<usize>,
        /// Per-note cap on items emitted in *each* category
        /// (open_questions / hypotheses / candidate_findings /
        /// tensions). Default 4. Trims the strongest items the model
        /// returns so dense notes don't drown the Inbox.
        #[arg(long)]
        max_items_per_category: Option<usize>,
        /// Preview without writing to the frontier file.
        #[arg(long)]
        dry_run: bool,
        /// Output stable JSON for programmatic callers.
        #[arg(long)]
        json: bool,
    },
    /// v0.24 Agent Inbox: run Code & Notebook Analyst against a
    /// research repo (Jupyter `.ipynb`, Python / R / Julia / Quarto
    /// / Rmd scripts). Each analysis, code-derived finding, or
    /// experiment intent becomes a `finding.add` `StateProposal`
    /// tagged with the analyst's `AgentRun`. Same review loop.
    CompileCode {
        /// Repo / folder root to walk.
        root: PathBuf,
        /// Frontier file the proposals are appended to.
        #[arg(long)]
        frontier: PathBuf,
        /// Optional model alias (`sonnet`, `opus`, …).
        #[arg(short, long)]
        backend: Option<String>,
        /// Cap on files processed (default 30).
        #[arg(long)]
        max_files: Option<usize>,
        /// Preview without writing to the frontier file.
        #[arg(long)]
        dry_run: bool,
        /// Output stable JSON for programmatic callers.
        #[arg(long)]
        json: bool,
    },
    /// v0.28 Agent Inbox: run Reviewer Agent against a frontier's
    /// pending proposals. Each scored proposal gets a
    /// `finding.note` proposal attached with plausibility +
    /// evidence + scope + duplicate-risk scores so reviewers can
    /// triage faster.
    ReviewPending {
        #[arg(long)]
        frontier: PathBuf,
        #[arg(short, long)]
        backend: Option<String>,
        #[arg(long)]
        max_proposals: Option<usize>,
        /// Number of proposals scored per `claude -p` call.
        /// 1 = per-proposal mode (full transcript). 5–10 = ~5×
        /// faster wall-clock, single response covers the batch.
        /// Capped at 12 internally.
        #[arg(long, default_value = "1")]
        batch_size: usize,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// v0.28 Agent Inbox: run Contradiction Finder against a
    /// frontier's findings. Pairs that contradict get emitted as
    /// `tension`-typed `finding.add` proposals.
    FindTensions {
        #[arg(long)]
        frontier: PathBuf,
        #[arg(short, long)]
        backend: Option<String>,
        #[arg(long)]
        max_findings: Option<usize>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// v0.28 Agent Inbox: run Experiment Planner against a
    /// frontier's open questions and hypotheses. Each gets 1–3
    /// `experiment_intent`-typed `finding.add` proposals.
    PlanExperiments {
        #[arg(long)]
        frontier: PathBuf,
        #[arg(short, long)]
        backend: Option<String>,
        #[arg(long)]
        max_findings: Option<usize>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// v0.25 Agent Inbox: run Datasets agent against a folder of
    /// CSV / TSV / Parquet files. Each dataset gets a summary
    /// proposal + optional supported-claim proposals tagged with
    /// the agent's `AgentRun`. Same review loop.
    CompileData {
        /// Folder root to walk (top level only in v0.25).
        root: PathBuf,
        /// Frontier file the proposals are appended to.
        #[arg(long)]
        frontier: PathBuf,
        /// Optional model alias (`sonnet`, `opus`, …).
        #[arg(short, long)]
        backend: Option<String>,
        /// Sample rows sent to the model per dataset (default 50).
        #[arg(long)]
        sample_rows: Option<usize>,
        /// Preview without writing to the frontier file.
        #[arg(long)]
        dry_run: bool,
        /// Output stable JSON for programmatic callers.
        #[arg(long)]
        json: bool,
    },
    /// Check frontier quality and proof readiness
    Check {
        /// Frontier JSON file, Vela repo, or proof packet
        source: Option<PathBuf>,
        /// Run schema validation
        #[arg(long)]
        schema: bool,
        /// Run frontier lint checks
        #[arg(long)]
        stats: bool,
        /// Run conformance vectors
        #[arg(long)]
        conformance: bool,
        /// Conformance test directory
        #[arg(long, default_value = "tests/conformance")]
        conformance_dir: PathBuf,
        /// Run all checks
        #[arg(long)]
        all: bool,
        /// Run only structural schema validation
        #[arg(long)]
        schema_only: bool,
        /// Treat warnings and blocking signals as failures
        #[arg(long)]
        strict: bool,
        /// Show fix suggestions
        #[arg(long)]
        fix: bool,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
    },
    /// Normalize deterministic frontier state without changing claims
    Normalize {
        /// Frontier JSON file or Vela repo
        source: PathBuf,
        /// Output normalized frontier copy
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Write changes back to the input
        #[arg(long)]
        write: bool,
        /// Force dry-run
        #[arg(long)]
        dry_run: bool,
        /// Rewrite finding IDs to content addresses and update links
        #[arg(long)]
        rewrite_ids: bool,
        /// Write old-to-new ID map when rewriting IDs
        #[arg(long)]
        id_map: Option<PathBuf>,
        /// Phase N: regenerate finding.provenance fields (title, year,
        /// journal, authors, license, publisher, funders) from the
        /// canonical SourceRecord matched by DOI / PMID / title.
        #[arg(long)]
        resync_provenance: bool,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
    },
    /// Export and validate a proof packet
    Proof {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Output proof packet directory
        #[arg(long, short = 'o', default_value = "proof-packet")]
        out: PathBuf,
        /// Proof packet template
        #[arg(long, default_value = "bbb-alzheimer")]
        template: String,
        /// Optional benchmark suite to include
        #[arg(long)]
        gold: Option<PathBuf>,
        /// Record latest proof packet state back into the input frontier
        #[arg(long)]
        record_proof_state: bool,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
    },
    /// Serve a read-only frontier over MCP stdio or HTTP
    Serve {
        /// Frontier JSON file or Vela repo
        #[arg(required_unless_present_any = ["frontiers", "setup"])]
        frontier: Option<PathBuf>,
        /// Directory of frontier files
        #[arg(long)]
        frontiers: Option<PathBuf>,
        /// LLM backend reserved for future optional tools
        #[arg(short, long)]
        backend: Option<String>,
        /// Run an HTTP server on this port instead of MCP stdio
        #[arg(long)]
        http: Option<u16>,
        /// Print MCP setup instructions
        #[arg(long)]
        setup: bool,
        /// Validate public tool contracts and exit
        #[arg(long)]
        check_tools: bool,
        /// Output stable JSON for --check-tools
        #[arg(long)]
        json: bool,
        /// Serve the local Workbench web UI (`web/`) alongside the
        /// HTTP API. Implies `--http` if no port is specified
        /// (default 3848). Phase R, v0.5.
        #[arg(long)]
        workbench: bool,
    },
    /// Show frontier statistics
    Stats {
        /// Frontier JSON file, Vela repo, or packet
        frontier: PathBuf,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
    },
    /// Search findings
    Search {
        /// Search query
        query: String,
        /// Frontier JSON file, Vela repo, or packet
        #[arg(long)]
        source: Option<PathBuf>,
        /// Filter by entity
        #[arg(long)]
        entity: Option<String>,
        /// Filter by assertion type
        #[arg(long)]
        r#type: Option<String>,
        /// Search every frontier in a directory
        #[arg(long)]
        all: Option<PathBuf>,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
    },
    /// List candidate contradictions and tensions
    Tensions {
        source: PathBuf,
        #[arg(long)]
        both_high: bool,
        #[arg(long)]
        cross_domain: bool,
        #[arg(long, default_value = "20")]
        top: usize,
        #[arg(long)]
        json: bool,
    },
    /// Inspect and rank candidate gap review leads
    Gaps {
        #[command(subcommand)]
        action: GapsAction,
    },
    /// Find candidate cross-domain connections
    Bridge {
        /// Input frontier JSON files or Vela repos
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Run rough PubMed prior-art checks for top bridges
        #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
        novelty: bool,
        /// Max bridges to check
        #[arg(long, default_value = "30")]
        top: usize,
    },
    /// Export frontier artifacts
    Export {
        frontier: PathBuf,
        #[arg(short, long, default_value = "csv")]
        format: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Inspect or validate proof packets
    Packet {
        #[command(subcommand)]
        action: PacketAction,
    },
    /// Run deterministic benchmark gates.
    ///
    /// Two modes:
    ///   - **legacy** (extraction quality): `--gold <gold.json>`
    ///     against an extracted-findings frontier. Pre-v0.26
    ///     behaviour, unchanged.
    ///   - **v0.26 VelaBench** (agent state-update scoring): pass
    ///     `--candidate <frontier.json>` together with `--gold`
    ///     to compare a candidate frontier (typically agent-
    ///     generated) against a curator-validated gold. Composite
    ///     score with optional `--threshold` for CI gating.
    Bench {
        /// Frontier file for single-task benchmark (legacy mode).
        frontier: Option<PathBuf>,
        /// Gold frontier (used by both modes).
        #[arg(long)]
        gold: Option<PathBuf>,
        /// v0.26: Candidate frontier to score against `--gold`.
        /// Presence of this flag selects VelaBench (agent state-
        /// update scoring) instead of the legacy extraction harness.
        #[arg(long)]
        candidate: Option<PathBuf>,
        /// v0.26: Optional source-files directory for
        /// `evidence_fidelity` checks. Without it, that metric is
        /// dropped from the composite (weights rebalanced).
        #[arg(long)]
        sources: Option<PathBuf>,
        /// v0.26: Composite-score threshold; non-zero exit if
        /// composite < threshold. Default 0.0 (report only).
        #[arg(long)]
        threshold: Option<f64>,
        /// v0.26: Write the JSON report to this path in addition
        /// to printing.
        #[arg(long)]
        report: Option<PathBuf>,
        #[arg(long)]
        entity_gold: Option<PathBuf>,
        #[arg(long)]
        link_gold: Option<PathBuf>,
        #[arg(long)]
        suite: Option<PathBuf>,
        #[arg(long)]
        suite_ready: bool,
        #[arg(long)]
        min_f1: Option<f64>,
        #[arg(long)]
        min_precision: Option<f64>,
        #[arg(long)]
        min_recall: Option<f64>,
        #[arg(long)]
        no_thresholds: bool,
        #[arg(long)]
        json: bool,
    },
    /// Run protocol conformance vectors
    Conformance {
        #[arg(default_value = "tests/conformance")]
        dir: PathBuf,
    },
    /// Show version information
    Version,
    /// Optional signing and signature verification
    Sign {
        #[command(subcommand)]
        action: SignAction,
    },
    /// Manage the frontier's registered actor identities (Phase M, v0.4)
    Actor {
        #[command(subcommand)]
        action: ActorAction,
    },
    /// v0.39: Manage the frontier's federation peer registry. A peer
    /// is another hub this frontier knows about — id, HTTPS URL, and
    /// the Ed25519 pubkey they sign manifests with. Adding a peer
    /// declares awareness; the actual sync runtime ships in v0.39.1+.
    Federation {
        #[command(subcommand)]
        action: FederationAction,
    },
    /// v0.40: Causal reasoning over the schema landed in v0.38. Audits
    /// every finding for identifiability: does the declared
    /// study-design grade actually support the causal claim being
    /// made? Surfaces underidentified findings (intervention from
    /// observational) and conditional ones (intervention from
    /// quasi-experimental designs that need explicit assumptions).
    Causal {
        #[command(subcommand)]
        action: CausalAction,
    },
    /// Manage frontier-level metadata: cross-frontier dependencies (v0.8).
    /// Use `vela frontier add-dep` to declare a remote frontier this
    /// frontier links into via `vf_…@vfr_…` references.
    Frontier {
        #[command(subcommand)]
        action: FrontierAction,
    },
    /// Walk the local Workbench draft queue (Phase R, v0.5):
    /// list, sign-and-apply, or clear queued review actions
    Queue {
        #[command(subcommand)]
        action: QueueAction,
    },
    /// Publish, list, or pull frontiers through a registry
    /// (Phase S, v0.5: verifiable distribution)
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },
    /// Initialize a .vela frontier repo
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "unnamed")]
        name: String,
    },
    /// Import frontier JSON into a .vela repo
    Import {
        frontier: PathBuf,
        #[arg(long)]
        into: Option<PathBuf>,
    },
    /// Compare two frontiers
    Diff {
        frontier_a: PathBuf,
        frontier_b: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        quiet: bool,
    },
    /// Inspect or apply proposal-first frontier writes
    Proposals {
        #[command(subcommand)]
        action: ProposalAction,
    },
    /// Manage finding bundles as the core frontier primitive
    Finding {
        #[command(subcommand)]
        command: FindingCommands,
    },
    /// Add typed links between findings — including cross-frontier
    /// references of the form `vf_<id>@vfr_<id>` (v0.8). Until v0.9
    /// link state lived only in JSON; `vela link add` is the CLI on-ramp.
    Link {
        #[command(subcommand)]
        action: LinkAction,
    },
    /// v0.19: resolve unresolved entities against a bundled common-entity
    /// table (UniProt for proteins, MeSH for diseases, ChEBI/DrugBank for
    /// compounds, etc.). Lowers `needs_review` for matched entities and
    /// populates `canonical_id`. Idempotent unless `--force` is passed.
    Entity {
        #[command(subcommand)]
        action: EntityAction,
    },
    /// Create or apply one proposal-backed finding review
    Review {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Finding ID to review
        finding_id: String,
        /// accepted, contested, needs_revision, or rejected
        #[arg(long)]
        status: Option<String>,
        /// Reason for the review
        #[arg(long)]
        reason: Option<String>,
        /// Reviewer identifier
        #[arg(long)]
        reviewer: String,
        /// Immediately accept and apply the proposal locally
        #[arg(long)]
        apply: bool,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
    },
    /// Add a lightweight note to a finding
    Note {
        frontier: PathBuf,
        finding_id: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        author: String,
        /// Immediately accept and apply the proposal locally
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Add an explicit caveat to a finding
    Caveat {
        frontier: PathBuf,
        finding_id: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        author: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Revise an interpretation field while preserving history
    Revise {
        frontier: PathBuf,
        finding_id: String,
        /// New confidence score from 0.0 to 1.0
        #[arg(long)]
        confidence: f64,
        /// Reason for the revision
        #[arg(long)]
        reason: String,
        /// Reviewer identifier
        #[arg(long)]
        reviewer: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Mark a finding as rejected without deleting it
    Reject {
        frontier: PathBuf,
        finding_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        reviewer: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show state-transition history for one finding
    History {
        frontier: PathBuf,
        finding_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Import review/state events from a packet or JSON file into a frontier
    ImportEvents {
        source: PathBuf,
        #[arg(long)]
        into: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Retract a finding
    Retract {
        source: PathBuf,
        finding_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        reviewer: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Simulate correction impact over declared dependency links
    Propagate {
        frontier: PathBuf,
        #[arg(long)]
        retract: Option<String>,
        #[arg(long)]
        reduce_confidence: Option<String>,
        #[arg(long)]
        to: Option<f64>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// v0.32: Record an independent replication attempt against a
    /// canonical finding. Each attempt becomes a `vrep_<hash>` object
    /// in `.vela/replications/`, content-addressed by target +
    /// attempting actor + canonical conditions + outcome. Replication
    /// is the empirical bedrock of science; making it kernel-level
    /// means downstream tools (site, bench, agents) can reason about
    /// "this lab tried in human iPSC, that lab failed in mouse OPCs"
    /// as distinct epistemic facts.
    Replicate {
        /// Path to the frontier (project dir, `.vela/` repo, or `.json`).
        frontier: PathBuf,
        /// Target finding id (`vf_<hash>`) being replicated.
        target: String,
        /// Outcome label: `replicated` | `failed` | `partial` | `inconclusive`.
        #[arg(long)]
        outcome: String,
        /// Stable actor id of the lab/curator/agent attempting.
        #[arg(long)]
        by: String,
        /// One-paragraph description of conditions (model system,
        /// species, sample size, in_vivo / in_vitro / human_data).
        /// Goes into the content-address preimage.
        #[arg(long)]
        conditions: String,
        /// Source paper title for the replicating work.
        #[arg(long)]
        source_title: String,
        /// Optional DOI for the replicating paper.
        #[arg(long)]
        doi: Option<String>,
        /// Optional PMID for the replicating paper.
        #[arg(long)]
        pmid: Option<String>,
        /// Sample size description (e.g. "n=42").
        #[arg(long)]
        sample_size: Option<String>,
        /// Free-text reviewer note. Especially important for
        /// `partial` and `inconclusive` outcomes.
        #[arg(long, default_value = "")]
        note: String,
        /// `vrep_<id>` of a previous attempt this one extends/refines.
        #[arg(long)]
        previous_attempt: Option<String>,
        /// v0.36.2: skip the propagation cascade. By default,
        /// recording a replication recomputes the target finding's
        /// confidence from the live `Project.replications` collection
        /// and flags downstream dependents linked via `supports` /
        /// `depends`. Use this flag to stage replications without
        /// immediate review-queue churn.
        #[arg(long, default_value_t = false)]
        no_cascade: bool,
        /// Emit JSON to stdout.
        #[arg(long)]
        json: bool,
    },
    /// v0.32: List replication attempts in a frontier, optionally
    /// filtered by target finding id.
    Replications {
        /// Path to the frontier (project dir, `.vela/` repo, or `.json`).
        frontier: PathBuf,
        /// Optional target finding id to filter by.
        #[arg(long)]
        target: Option<String>,
        /// Emit JSON to stdout.
        #[arg(long)]
        json: bool,
    },
    /// v0.33: Register a Dataset as a first-class kernel object
    /// (`vd_<hash>`). Datasets anchor empirical claims that rest on
    /// data — the canonical Alzheimer's frontier should know that
    /// "ATV:TREM2 reduces plaque density" rests on a specific cohort
    /// of n=24 iPSC-derived microglia, not on "the iPSC dataset" in
    /// the abstract.
    DatasetAdd {
        /// Path to the frontier (project dir, `.vela/` repo, or `.json`).
        frontier: PathBuf,
        /// Human-readable dataset name (e.g. `ADNI`, `TRAILBLAZER-ALZ`).
        #[arg(long)]
        name: String,
        /// Semantic version or release tag (e.g. `ADNI-3`, `v2.2`).
        #[arg(long)]
        version: Option<String>,
        /// SHA-256 of canonical contents. For remote datasets, the
        /// publisher's declared content hash; integrity verification
        /// is the puller's responsibility.
        #[arg(long)]
        content_hash: String,
        /// Where the dataset is reachable (https / file / s3 URL).
        #[arg(long)]
        url: Option<String>,
        /// License identifier or URL.
        #[arg(long)]
        license: Option<String>,
        /// Source paper title or release name (for provenance).
        #[arg(long)]
        source_title: String,
        /// Optional DOI for the source publication.
        #[arg(long)]
        doi: Option<String>,
        /// Optional row count.
        #[arg(long)]
        row_count: Option<u64>,
        /// Emit JSON to stdout.
        #[arg(long)]
        json: bool,
    },
    /// v0.33: List datasets in a frontier.
    Datasets {
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// v0.33: Register a CodeArtifact as a first-class kernel object
    /// (`vc_<hash>`). Code artifacts make "Git for science" mean
    /// something operational — claims literally reference the code
    /// that produced them, pinned to a specific git commit and a
    /// specific path.
    CodeAdd {
        /// Path to the frontier.
        frontier: PathBuf,
        /// Source language: `python`, `r`, `julia`, `rust`, `bash`, etc.
        #[arg(long)]
        language: String,
        /// Repository URL (e.g. `https://github.com/vela-science/vela`).
        #[arg(long)]
        repo_url: Option<String>,
        /// Specific git commit SHA. Required for reproducibility;
        /// `None` means "unpinned" and weakens the substrate claim.
        #[arg(long)]
        commit: Option<String>,
        /// Path within the repository.
        #[arg(long)]
        path: String,
        /// SHA-256 of the snippet body.
        #[arg(long)]
        content_hash: String,
        /// Optional starting line.
        #[arg(long)]
        line_start: Option<u32>,
        /// Optional ending line.
        #[arg(long)]
        line_end: Option<u32>,
        /// Optional entry point: function name, notebook cell id.
        #[arg(long)]
        entry_point: Option<String>,
        /// Emit JSON to stdout.
        #[arg(long)]
        json: bool,
    },
    /// v0.33: List code artifacts in a frontier.
    CodeArtifacts {
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// v0.34: Make a falsifiable Prediction (`vpred_<hash>`) about a
    /// future observation. Predictions are scoped to one or more
    /// existing findings, carry an explicit resolution criterion,
    /// and live in the kernel's epistemic accountability ledger.
    /// When a Resolution arrives later, the prediction's confidence
    /// flows into the predictor's Brier score and log score.
    Predict {
        /// Path to the frontier (project dir, `.vela/` repo, or `.json`).
        frontier: PathBuf,
        /// Stable actor id of the predictor.
        #[arg(long)]
        by: String,
        /// Plain-prose prediction (e.g. "lecanemab Phase 4 will show
        /// >0.4 SD CDR-SB effect").
        #[arg(long)]
        claim: String,
        /// Unambiguous criterion describing how to recognize resolution.
        #[arg(long)]
        criterion: String,
        /// RFC 3339 deadline for resolution.
        #[arg(long)]
        resolves_by: Option<String>,
        /// Confidence on [0, 1] in the expected outcome.
        #[arg(long)]
        confidence: f64,
        /// Comma-separated `vf_*` finding ids this prediction depends on.
        #[arg(long, default_value = "")]
        target: String,
        /// Outcome shape: `affirmed` | `falsified` | `quant:V±T units` | `cat:value`.
        #[arg(long, default_value = "affirmed")]
        outcome: String,
        /// Free-text scope/conditions of the prediction.
        #[arg(long, default_value = "")]
        conditions: String,
        /// Emit JSON to stdout.
        #[arg(long)]
        json: bool,
    },
    /// v0.34: Resolve an open Prediction. Records what actually
    /// happened, who observed it, and whether it matched the
    /// prediction. Drives Brier / log-score / hit-rate calibration
    /// over the resolved subset.
    Resolve {
        /// Path to the frontier.
        frontier: PathBuf,
        /// `vpred_<id>` of the prediction being resolved.
        prediction: String,
        /// Free-text description of what actually happened.
        #[arg(long)]
        outcome: String,
        /// Whether the actual outcome matched the predicted one.
        #[arg(long)]
        matched: bool,
        /// Stable actor id of the resolver. Independent resolvers
        /// (different from the predictor) produce stronger signal.
        #[arg(long)]
        by: String,
        /// Resolver's confidence in the match judgment, on [0, 1].
        #[arg(long, default_value = "1.0")]
        confidence: f64,
        /// Source paper / trial readout title for the resolution.
        #[arg(long, default_value = "")]
        source_title: String,
        /// Optional DOI for the resolving source.
        #[arg(long)]
        doi: Option<String>,
        /// Emit JSON to stdout.
        #[arg(long)]
        json: bool,
    },
    /// v0.34: List predictions in a frontier with their resolution state.
    Predictions {
        frontier: PathBuf,
        /// Optional actor filter.
        #[arg(long)]
        by: Option<String>,
        /// Show only unresolved predictions.
        #[arg(long)]
        open: bool,
        /// Emit JSON to stdout.
        #[arg(long)]
        json: bool,
    },
    /// v0.34: Compute calibration scores (Brier, log score, hit rate)
    /// for one or all actors with predictions in the frontier.
    Calibration {
        frontier: PathBuf,
        /// Optional actor filter (e.g. `reviewer:will-blair`).
        #[arg(long)]
        actor: Option<String>,
        /// Emit JSON to stdout.
        #[arg(long)]
        json: bool,
    },
    /// v0.35: Compute consensus over claim-similar findings, weighted
    /// by evidence quality. Takes a target `vf_<id>` and finds other
    /// findings making a similar assertion (shared entities + text
    /// overlap), weighs them by replication count + citation count
    /// + review state, and returns a consensus confidence with a
    /// credible interval. The substrate move that turns "what does
    /// the field hold about X?" from a manual graph walk into a
    /// queryable result.
    Consensus {
        /// Path to the frontier (project dir, `.vela/` repo, or `.json`).
        frontier: PathBuf,
        /// Target finding id (`vf_<hash>`).
        target: String,
        /// Weighting scheme: `unweighted` | `replication` | `citation` |
        /// `composite`. Default is `composite`.
        #[arg(long, default_value = "composite")]
        weighting: String,
        /// v0.38.2: restrict neighbor findings to a specific causal
        /// claim type: `correlation` | `mediation` | `intervention`.
        /// Useful for asking "what does the field hold *as
        /// causation*?" — distinct from the global blend.
        #[arg(long)]
        causal_claim: Option<String>,
        /// v0.38.2: restrict neighbor findings to study designs at or
        /// above the given grade: `theoretical` | `observational` |
        /// `quasi_experimental` | `rct`. Findings with no grade are
        /// excluded when this is set.
        #[arg(long)]
        causal_grade_min: Option<String>,
        /// Emit JSON to stdout.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum PacketAction {
    /// Inspect a proof packet manifest
    Inspect {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Validate a proof packet
    Validate {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum SignAction {
    /// Generate an Ed25519 keypair
    GenerateKeypair {
        #[arg(long, default_value = ".vela/keys")]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Sign unsigned findings in a frontier
    Apply {
        frontier: PathBuf,
        #[arg(long)]
        private_key: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Verify frontier signatures
    Verify {
        frontier: PathBuf,
        #[arg(long)]
        public_key: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// v0.37: Attach a multi-signature threshold to a finding. Once
    /// `k` distinct registered actors have each signed the finding, it
    /// is marked `jointly_accepted`. Setting `--to 1` is equivalent to
    /// the default single-sig regime.
    ThresholdSet {
        frontier: PathBuf,
        /// Target finding id (`vf_<hash>`).
        finding_id: String,
        /// Number of unique valid signatures required (>= 1).
        #[arg(long)]
        to: u32,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ActorAction {
    /// Register an Ed25519 public key for a stable actor identity
    Add {
        frontier: PathBuf,
        /// Stable actor id (e.g. "reviewer:will-blair")
        id: String,
        /// Hex-encoded Ed25519 public key (64 hex chars)
        #[arg(long)]
        pubkey: String,
        /// Optional trust tier (Phase α, v0.6). Currently recognized:
        /// "auto-notes" — permits one-call propose_and_apply_note.
        /// Unknown tier strings load fine but never grant auto-apply.
        #[arg(long)]
        tier: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// List registered actors in a frontier
    List {
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum CausalAction {
    /// v0.40: Audit every finding's (causal_claim, causal_evidence_grade)
    /// for identifiability. Reports underidentified, conditional,
    /// and underdetermined findings with rationale + remediation.
    Audit {
        frontier: PathBuf,
        /// Restrict the report to entries needing reviewer attention
        /// (Underidentified or Conditional). Useful for triage.
        #[arg(long)]
        problems_only: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum FederationAction {
    /// v0.39: Register a peer hub in this frontier. Adding a peer
    /// declares awareness — it does not trust their state. Sync /
    /// merge runtime ships in v0.39.1+.
    PeerAdd {
        frontier: PathBuf,
        /// Stable peer id (e.g. `hub:vela-mirror-eu`).
        id: String,
        /// HTTPS URL where the peer publishes signed manifests.
        #[arg(long)]
        url: String,
        /// Hex-encoded Ed25519 public key (64 hex chars).
        #[arg(long)]
        pubkey: String,
        /// Optional human-readable note (e.g. "EU mirror, run by lab Z").
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        json: bool,
    },
    /// List federation peers registered in a frontier.
    PeerList {
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Remove a peer from the registry. Does not retroactively
    /// invalidate events that referenced the peer; just stops further
    /// sync attempts.
    PeerRemove {
        frontier: PathBuf,
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// v0.39.1: Sync our frontier against a peer's published view.
    /// Fetches the peer's frontier JSON over HTTP, diffs it against
    /// ours, appends one `frontier.synced_with_peer` event recording
    /// the pass + one `frontier.conflict_detected` event per
    /// disagreement. Read-only with respect to findings — no
    /// peer-state is merged in. Conflict resolution ships in
    /// v0.39.2+ via proposals.
    Sync {
        frontier: PathBuf,
        /// Peer id (must already be in the registry).
        peer_id: String,
        /// Override the peer's manifest URL. Default: `<peer.url>/manifest/<vfr_id>.json`.
        /// Useful for testing or when the peer publishes at a non-standard path.
        #[arg(long)]
        url: Option<String>,
        /// Run the diff but don't append events. Reports what *would*
        /// have been recorded. Useful for surfacing conflicts before
        /// committing them to the canonical event log.
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum FrontierAction {
    /// Scaffold a fresh, publishable `frontier.json` stub. The result
    /// passes `vela check --strict` immediately and is ready to accept
    /// findings via `vela finding add` and a publish via `vela registry
    /// publish`. Use this instead of `vela init` when you intend to
    /// publish to a hub — `init` creates a `.vela/` repo, which is not
    /// directly publishable in v0.
    New {
        /// Path to write the new frontier file (e.g. `./frontier.json`).
        path: PathBuf,
        /// Human-readable frontier name.
        #[arg(long)]
        name: String,
        /// Optional one-paragraph description of the bounded question.
        #[arg(long, default_value = "")]
        description: String,
        /// Overwrite if the file already exists.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Declare a cross-frontier dependency. Subsequent links of the
    /// form `vf_<id>@vfr_<id>` resolve through this entry; strict
    /// validation refuses cross-frontier targets without one.
    AddDep {
        /// Path to the frontier file
        frontier: PathBuf,
        /// The remote frontier's content-addressed id (`vfr_…`)
        vfr_id: String,
        /// Where to fetch the remote frontier file from. Typically
        /// an `https://…` URL pointing at raw JSON.
        #[arg(long)]
        locator: String,
        /// SHA-256 of the remote's canonical snapshot. Strict pull
        /// verifies the fetched dependency's snapshot matches this.
        #[arg(long)]
        snapshot: String,
        /// Optional human-readable name for the dependency.
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// List the frontier's declared dependencies.
    ListDeps {
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Remove a previously-declared cross-frontier dependency by `vfr_id`.
    /// Refuses if any link target still references it.
    RemoveDep {
        frontier: PathBuf,
        vfr_id: String,
        #[arg(long)]
        json: bool,
    },
    /// v0.11: re-pin every declared cross-frontier dependency to the
    /// hub's current snapshot for that `vfr_id`. Useful when a dep
    /// (e.g. BBB) republishes weekly and your local pin goes stale.
    /// Reports per-dep status: unchanged, refreshed (with old → new
    /// snapshot), missing (vfr_id not on hub), or unreachable. Does
    /// nothing destructive if --dry-run is passed.
    RefreshDeps {
        frontier: PathBuf,
        /// Hub URL to query. Defaults to https://vela-hub.fly.dev.
        #[arg(long, default_value = "https://vela-hub.fly.dev")]
        from: String,
        /// Show what would change without writing.
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// v0.32: emit a structured diff of findings added, updated, and
    /// contradicted in a time window. The canonical replacement for the
    /// `scripts/weekly-diff.sh` Python fallback shipped in v0.31.
    ///
    /// Default window is the current ISO week (Monday 00:00 UTC →
    /// next Monday 00:00 UTC). Use `--since <RFC3339>` for an arbitrary
    /// start, or `--week YYYY-Www` for a specific ISO week.
    ///
    /// Output is JSON if `--json` is set; otherwise a human summary.
    /// The diff is read-only over the canonical state — it does not
    /// modify the frontier and does not require a signing key.
    Diff {
        /// Path to the frontier (project dir, `.vela/` repo, or `.json` file).
        frontier: PathBuf,
        /// Compute diff since this RFC 3339 timestamp.
        /// Mutually exclusive with `--week`.
        #[arg(long)]
        since: Option<String>,
        /// Compute diff for a specific ISO week (e.g. `2026-W18`).
        /// If absent and no `--since`, defaults to the current ISO week.
        #[arg(long)]
        week: Option<String>,
        /// Emit JSON to stdout.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum QueueAction {
    /// List queued draft actions (no signing)
    List {
        #[arg(long)]
        queue_file: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Sign each queued draft with the actor's Ed25519 key and apply
    /// it locally. Removes signed entries from the queue on success.
    Sign {
        /// Stable actor id matching a registered entry in the frontier
        #[arg(long)]
        actor: String,
        /// Path to the actor's Ed25519 private key (hex-encoded)
        #[arg(long)]
        key: PathBuf,
        /// Override the queue file location
        #[arg(long)]
        queue_file: Option<PathBuf>,
        /// Skip per-action confirmation prompts and sign every queued
        /// draft. Required in non-interactive contexts. The `--all`
        /// alias is accepted for muscle-memory convenience (the v0.28
        /// sim-user docs and an early friction report both wrote it
        /// that way; cheaper to accept the alias than to retrain).
        #[arg(long, alias = "all")]
        yes_to_all: bool,
        #[arg(long)]
        json: bool,
    },
    /// Drop all queued draft actions
    Clear {
        #[arg(long)]
        queue_file: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum RegistryAction {
    /// List all entries in a local registry
    List {
        /// Path or file:// URL of the registry; defaults to ~/.vela/registry/entries.json
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Publish a frontier's current snapshot+event_log hashes to a registry
    Publish {
        /// Path to the frontier file
        frontier: PathBuf,
        /// Stable owner actor id (must be registered in the frontier)
        #[arg(long)]
        owner: String,
        /// Path to the owner's Ed25519 private key (hex-encoded)
        #[arg(long)]
        key: PathBuf,
        /// Network locator under which the frontier is reachable
        /// (file:// path or HTTP URL the publisher serves)
        #[arg(long)]
        locator: String,
        /// Registry to publish to (path/URL); default ~/.vela/registry/entries.json
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// v0.15: list registry entries whose frontier declares a
    /// cross-frontier dependency on the given `vfr_id`. Surfaces
    /// "who is referencing my frontier" — the bidirectional view
    /// of cross-frontier composition. Hub-only (no local-registry
    /// equivalent yet); requires the hub to support
    /// `GET /entries/{vfr_id}/depends-on`.
    DependsOn {
        /// Frontier address (`vfr_…`) to look up dependents of.
        vfr_id: String,
        /// Hub URL. Required for v0.15 (no local file walk yet).
        #[arg(long, default_value = "https://vela-hub.fly.dev")]
        from: String,
        #[arg(long)]
        json: bool,
    },
    /// v0.20: federation primitive. Pull a signed manifest from one hub
    /// (`--from`) and POST it verbatim to another (`--to`). Both hubs
    /// validate the signature against the manifest's embedded
    /// `owner_pubkey`; mirroring is a no-op for authenticity. Use this
    /// to replicate a frontier across hubs (resilience), seed a fresh
    /// hub from an established one, or test a hub deployment with real
    /// signed bytes.
    Mirror {
        /// Frontier address (`vfr_…`) to mirror.
        vfr_id: String,
        /// Source hub URL.
        #[arg(long)]
        from: String,
        /// Destination hub URL.
        #[arg(long)]
        to: String,
        #[arg(long)]
        json: bool,
    },
    /// Pull and verify a frontier from a registry by `vfr_id`
    Pull {
        /// Frontier address (`vfr_…`)
        vfr_id: String,
        /// Registry to pull from
        #[arg(long)]
        from: Option<String>,
        /// Output path for the pulled frontier. With --transitive, this
        /// is the directory dependencies are also written into; without
        /// it, this is the file path the primary lands at.
        #[arg(long)]
        out: PathBuf,
        /// v0.8: also pull every cross-frontier dependency the primary
        /// declares, recursively, verifying each pinned snapshot.
        #[arg(long)]
        transitive: bool,
        /// v0.8: maximum recursion depth when --transitive is set.
        /// Primary is depth 0; its direct deps are depth 1.
        #[arg(long, default_value = "4")]
        depth: usize,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum GapsAction {
    /// Rank candidate gap review leads
    Rank {
        frontier: PathBuf,
        #[arg(long, default_value = "10")]
        top: usize,
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum LinkAction {
    /// Append a typed link from one finding to another. The target
    /// may be a local `vf_<hex>` or a cross-frontier `vf_<hex>@vfr_<hex>`
    /// (v0.8). Cross-frontier targets require a matching declared dep —
    /// run `vela frontier add-dep` first or strict validation will refuse.
    Add {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Source finding id (`vf_<hex>`)
        #[arg(long)]
        from: String,
        /// Target. Either `vf_<hex>` (local) or `vf_<hex>@vfr_<hex>` (cross).
        #[arg(long)]
        to: String,
        /// Link type. One of: supports, contradicts, extends, depends, replicates, supersedes, synthesized_from
        #[arg(long, default_value = "supports")]
        r#type: String,
        /// Optional human-readable note
        #[arg(long, default_value = "")]
        note: String,
        /// Who inferred the link. One of: compiler, reviewer, author
        #[arg(long, default_value = "reviewer")]
        inferred_by: String,
        /// v0.16: skip the cross-frontier target-status check. By
        /// default, when adding a cross-frontier link, the substrate
        /// fetches the dep's frontier from its declared locator and
        /// warns if the target finding has `flags.superseded = true`
        /// (you'd be linking to an outdated wording). The link is
        /// still recorded — this is a best-effort review hint, not a
        /// hard refusal. Set this flag to skip the network fetch
        /// (useful in CI or when offline).
        #[arg(long)]
        no_check_target: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum EntityAction {
    /// Walk every finding's entities and try to resolve each against
    /// the bundled common-entity table. Matched entities get
    /// `canonical_id` populated, `resolution_method = manual`,
    /// `resolution_confidence = 0.95`, `needs_review = false`. Already-
    /// resolved entities are skipped unless `--force` is passed. The
    /// frontier file is written back atomically.
    Resolve {
        frontier: PathBuf,
        /// Re-resolve entities that already have a canonical_id.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// List the bundled lookup table.
    List {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum FindingCommands {
    /// Add a manual finding bundle with an assertion field
    Add {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Assertion text inside the finding bundle
        #[arg(long)]
        assertion: String,
        /// Assertion type. One of: mechanism, therapeutic, diagnostic, epidemiological, observational, review, methodological, computational, theoretical, negative
        #[arg(long, default_value = "mechanism")]
        r#type: String,
        /// Source label for the finding
        #[arg(long, default_value = "manual finding")]
        source: String,
        /// Source type. One of: published_paper, preprint, clinical_trial, lab_notebook, model_output, expert_assertion, database_record
        #[arg(long, default_value = "expert_assertion")]
        source_type: String,
        /// Author/reviewer identifier
        #[arg(long)]
        author: String,
        /// Initial confidence score from 0.0 to 1.0
        #[arg(long, default_value = "0.3")]
        confidence: f64,
        /// Evidence type. One of: experimental, observational, computational, theoretical, meta_analysis, systematic_review, case_report
        #[arg(long, default_value = "theoretical")]
        evidence_type: String,
        /// Entities as comma-separated name:type pairs. Entity types: gene, protein, compound, disease, cell_type, organism, pathway, assay, anatomical_structure, particle, instrument, dataset, quantity, other
        #[arg(long, default_value = "")]
        entities: String,
        /// v0.11: DOI of the source artifact (e.g. "10.1038/s41586-024-...")
        #[arg(long)]
        doi: Option<String>,
        /// v0.11: PubMed ID
        #[arg(long)]
        pmid: Option<String>,
        /// v0.11: Publication year
        #[arg(long)]
        year: Option<i32>,
        /// v0.11: Journal name
        #[arg(long)]
        journal: Option<String>,
        /// v0.11: Generic source URL when none of the structured identifiers fit
        #[arg(long)]
        url: Option<String>,
        /// v0.11: Source-paper authors as semicolon-separated list (distinct from --author which is the curating Vela actor)
        #[arg(long)]
        source_authors: Option<String>,
        /// v0.11: Conditions/scope text. Replaces the placeholder otherwise written. Should describe scope boundaries (species, dosing, age range, model, etc.)
        #[arg(long)]
        conditions_text: Option<String>,
        /// v0.11: Verified species as semicolon-separated list (e.g. "Mus musculus;Homo sapiens")
        #[arg(long)]
        species: Option<String>,
        /// v0.11: Mark the finding as in vivo
        #[arg(long)]
        in_vivo: bool,
        /// v0.11: Mark the finding as in vitro
        #[arg(long)]
        in_vitro: bool,
        /// v0.11: Mark the finding as having human data
        #[arg(long)]
        human_data: bool,
        /// v0.11: Mark the finding as a clinical trial
        #[arg(long)]
        clinical_trial: bool,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
        /// Immediately accept and apply the proposal locally
        #[arg(long)]
        apply: bool,
    },
    /// v0.14: Supersede an existing finding with a new content-addressed
    /// claim. The new finding gets its own `vf_…` id; an auto-injected
    /// `supersedes` link points back at the old id; the old finding is
    /// flagged `superseded`. Both remain queryable. Real corrections
    /// (Phase 4 follow-up data, retraction, refined wording) belong here
    /// rather than as caveats stacked on top of an immutable claim.
    Supersede {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// `vf_…` id of the finding to supersede
        old_id: String,
        /// New assertion text (drives the new finding's content address)
        #[arg(long)]
        assertion: String,
        /// New assertion type
        #[arg(long, default_value = "mechanism")]
        r#type: String,
        /// Source label
        #[arg(long, default_value = "manual finding")]
        source: String,
        /// Source type
        #[arg(long, default_value = "expert_assertion")]
        source_type: String,
        /// Curating Vela actor id
        #[arg(long)]
        author: String,
        /// Reason for the supersede (becomes the proposal/event reason)
        #[arg(long)]
        reason: String,
        /// New confidence score 0.0..=1.0
        #[arg(long, default_value = "0.5")]
        confidence: f64,
        /// New evidence type
        #[arg(long, default_value = "experimental")]
        evidence_type: String,
        /// New entities (`name:type` pairs, comma-separated)
        #[arg(long, default_value = "")]
        entities: String,
        /// DOI of the source artifact
        #[arg(long)]
        doi: Option<String>,
        /// PubMed ID
        #[arg(long)]
        pmid: Option<String>,
        /// Publication year
        #[arg(long)]
        year: Option<i32>,
        /// Journal name
        #[arg(long)]
        journal: Option<String>,
        /// Generic source URL
        #[arg(long)]
        url: Option<String>,
        /// Source-paper authors (semicolon-separated)
        #[arg(long)]
        source_authors: Option<String>,
        /// Conditions/scope text
        #[arg(long)]
        conditions_text: Option<String>,
        /// Verified species (semicolon-separated)
        #[arg(long)]
        species: Option<String>,
        #[arg(long)]
        in_vivo: bool,
        #[arg(long)]
        in_vitro: bool,
        #[arg(long)]
        human_data: bool,
        #[arg(long)]
        clinical_trial: bool,
        #[arg(long)]
        json: bool,
        /// Immediately accept and apply the proposal locally
        #[arg(long)]
        apply: bool,
    },
    /// v0.38: Set or revise the Pearlian causal type and study-design
    /// grade for a finding. Appends an `assertion.reinterpreted_causal`
    /// event capturing the prior reading, the new reading, and the
    /// reviewer who re-graded. Pre-v0.38 findings carry no causal
    /// metadata; the first call materializes both fields.
    CausalSet {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// `vf_<id>` of the finding to re-grade.
        finding_id: String,
        /// Causal claim kind: correlation | mediation | intervention.
        #[arg(long)]
        claim: String,
        /// Optional study-design grade: rct | quasi_experimental |
        /// observational | theoretical.
        #[arg(long)]
        grade: Option<String>,
        /// Reviewer/curator id (must match a registered actor under
        /// `--strict`). Recorded on the appended event.
        #[arg(long)]
        actor: String,
        /// One-paragraph reason. Becomes the event's `reason` field
        /// and ships with the proposal.
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ProposalAction {
    /// List proposals in a frontier
    List {
        frontier: PathBuf,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show one proposal
    Show {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Import proposal files into a frontier
    Import {
        frontier: PathBuf,
        source: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Validate standalone proposal files or directories
    Validate {
        source: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Export proposal records from a frontier
    Export {
        frontier: PathBuf,
        output: PathBuf,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Accept and apply one proposal
    Accept {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long)]
        reviewer: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    /// Reject one proposal
    Reject {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long)]
        reviewer: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
}

pub async fn run_command() {
    dotenvy::dotenv().ok();

    match Cli::parse().command {
        Commands::Scout {
            folder,
            frontier,
            backend,
            dry_run,
            json,
        } => {
            cmd_scout(&folder, &frontier, backend.as_deref(), dry_run, json).await;
        }
        Commands::CompileNotes {
            vault,
            frontier,
            backend,
            max_files,
            max_items_per_category,
            dry_run,
            json,
        } => {
            cmd_compile_notes(
                &vault,
                &frontier,
                backend.as_deref(),
                max_files,
                max_items_per_category,
                dry_run,
                json,
            )
            .await;
        }
        Commands::CompileCode {
            root,
            frontier,
            backend,
            max_files,
            dry_run,
            json,
        } => {
            cmd_compile_code(
                &root,
                &frontier,
                backend.as_deref(),
                max_files,
                dry_run,
                json,
            )
            .await;
        }
        Commands::CompileData {
            root,
            frontier,
            backend,
            sample_rows,
            dry_run,
            json,
        } => {
            cmd_compile_data(
                &root,
                &frontier,
                backend.as_deref(),
                sample_rows,
                dry_run,
                json,
            )
            .await;
        }
        Commands::ReviewPending {
            frontier,
            backend,
            max_proposals,
            batch_size,
            dry_run,
            json,
        } => {
            cmd_review_pending(
                &frontier,
                backend.as_deref(),
                max_proposals,
                batch_size,
                dry_run,
                json,
            )
            .await;
        }
        Commands::FindTensions {
            frontier,
            backend,
            max_findings,
            dry_run,
            json,
        } => {
            cmd_find_tensions(
                &frontier,
                backend.as_deref(),
                max_findings,
                dry_run,
                json,
            )
            .await;
        }
        Commands::PlanExperiments {
            frontier,
            backend,
            max_findings,
            dry_run,
            json,
        } => {
            cmd_plan_experiments(
                &frontier,
                backend.as_deref(),
                max_findings,
                dry_run,
                json,
            )
            .await;
        }
        Commands::Check {
            source,
            schema,
            stats,
            conformance,
            conformance_dir,
            all,
            schema_only,
            strict,
            fix,
            json,
        } => cmd_check(
            source.as_deref(),
            schema,
            stats,
            conformance,
            &conformance_dir,
            all,
            schema_only,
            strict,
            fix,
            json,
        ),
        Commands::Normalize {
            source,
            out,
            write,
            dry_run,
            rewrite_ids,
            id_map,
            resync_provenance,
            json,
        } => cmd_normalize(
            &source,
            out.as_deref(),
            write,
            dry_run,
            rewrite_ids,
            id_map.as_deref(),
            resync_provenance,
            json,
        ),
        Commands::Proof {
            frontier,
            out,
            template,
            gold,
            record_proof_state,
            json,
        } => cmd_proof(
            &frontier,
            &out,
            &template,
            gold.as_deref(),
            record_proof_state,
            json,
        ),
        Commands::Serve {
            frontier,
            frontiers,
            backend,
            http,
            setup,
            check_tools,
            json,
            workbench,
        } => {
            if setup {
                cmd_mcp_setup(frontier.as_deref(), frontiers.as_deref());
            } else if check_tools {
                let source =
                    serve::ProjectSource::from_args(frontier.as_deref(), frontiers.as_deref());
                match serve::check_tools(source) {
                    Ok(report) => {
                        if json {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&report)
                                    .expect("failed to serialize tool check report")
                            );
                        } else {
                            print_tool_check_report(&report);
                        }
                    }
                    Err(e) => fail(&format!("Tool check failed: {e}")),
                }
            } else {
                let source =
                    serve::ProjectSource::from_args(frontier.as_deref(), frontiers.as_deref());
                // Phase R: --workbench implies HTTP and serves web/.
                let resolved_port = if workbench {
                    Some(http.unwrap_or(3848))
                } else {
                    http
                };
                if let Some(port) = resolved_port {
                    serve::run_http(source, backend.as_deref(), port, workbench).await;
                } else {
                    serve::run(source, backend.as_deref()).await;
                }
            }
        }
        Commands::Stats { frontier, json } => {
            if json {
                print_stats_json(&frontier);
            } else {
                cmd_stats(&frontier);
            }
        }
        Commands::Search {
            source,
            query,
            entity,
            r#type,
            all,
            limit,
            json,
        } => cmd_search(
            source.as_deref(),
            &query,
            entity.as_deref(),
            r#type.as_deref(),
            all.as_deref(),
            limit,
            json,
        ),
        Commands::Tensions {
            source,
            both_high,
            cross_domain,
            top,
            json,
        } => cmd_tensions(&source, both_high, cross_domain, top, json),
        Commands::Gaps { action } => cmd_gaps(action),
        Commands::Bridge {
            inputs,
            novelty,
            top,
        } => cmd_bridge(&inputs, novelty, top).await,
        Commands::Export {
            frontier,
            format,
            output,
        } => export::run(&frontier, &format, output.as_deref()),
        Commands::Packet { action } => cmd_packet(action),
        Commands::Bench {
            frontier,
            gold,
            candidate,
            sources,
            threshold,
            report,
            entity_gold,
            link_gold,
            suite,
            suite_ready,
            min_f1,
            min_precision,
            min_recall,
            no_thresholds,
            json,
        } => {
            // v0.26 VelaBench routing: presence of `--candidate`
            // selects the agent state-update scorer. The legacy
            // extraction harness keeps every other invocation
            // unchanged.
            if let Some(cand) = candidate.clone() {
                let Some(g) = gold.clone() else {
                    eprintln!(
                        "{} `vela bench --candidate <…>` requires `--gold <…>`",
                        style::err_prefix()
                    );
                    std::process::exit(2);
                };
                cmd_agent_bench(&g, &cand, sources.as_deref(), threshold, report.as_deref(), json);
            } else {
                cmd_bench(BenchArgs {
                    frontier,
                    gold,
                    entity_gold,
                    link_gold,
                    suite,
                    suite_ready,
                    min_f1,
                    min_precision,
                    min_recall,
                    no_thresholds,
                    json,
                });
            }
        }
        Commands::Conformance { dir } => {
            let _ = conformance::run(&dir);
        }
        Commands::Version => println!("vela 0.36.0"),
        Commands::Sign { action } => cmd_sign(action),
        Commands::Actor { action } => cmd_actor(action),
        Commands::Federation { action } => cmd_federation(action),
        Commands::Causal { action } => cmd_causal(action),
        Commands::Frontier { action } => cmd_frontier(action),
        Commands::Queue { action } => cmd_queue(action),
        Commands::Registry { action } => cmd_registry(action),
        Commands::Init { path, name } => cmd_init(&path, &name),
        Commands::Import { frontier, into } => cmd_import(&frontier, into.as_deref()),
        Commands::Diff {
            frontier_a,
            frontier_b,
            json,
            quiet,
        } => diff::run(&frontier_a, &frontier_b, json, quiet),
        Commands::Proposals { action } => cmd_proposals(action),
        Commands::Link { action } => cmd_link(action),
        Commands::Entity { action } => cmd_entity(action),
        Commands::Finding { command } => match command {
            FindingCommands::Add {
                frontier,
                assertion,
                r#type,
                source,
                source_type,
                author,
                confidence,
                evidence_type,
                entities,
                doi,
                pmid,
                year,
                journal,
                url,
                source_authors,
                conditions_text,
                species,
                in_vivo,
                in_vitro,
                human_data,
                clinical_trial,
                json,
                apply,
            } => {
                validate_enum_arg("--type", &r#type, bundle::VALID_ASSERTION_TYPES);
                validate_enum_arg(
                    "--evidence-type",
                    &evidence_type,
                    bundle::VALID_EVIDENCE_TYPES,
                );
                validate_enum_arg(
                    "--source-type",
                    &source_type,
                    bundle::VALID_PROVENANCE_SOURCE_TYPES,
                );
                let parsed_entities = parse_entities(&entities);
                for (name, etype) in &parsed_entities {
                    if !bundle::VALID_ENTITY_TYPES.contains(&etype.as_str()) {
                        fail(&format!(
                            "invalid entity type '{}' for '{}'. Valid: {}",
                            etype,
                            name,
                            bundle::VALID_ENTITY_TYPES.join(", "),
                        ));
                    }
                }
                let parsed_source_authors = source_authors
                    .map(|s| {
                        s.split(';')
                            .map(|a| a.trim().to_string())
                            .filter(|a| !a.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                let parsed_species = species
                    .map(|s| {
                        s.split(';')
                            .map(|a| a.trim().to_string())
                            .filter(|a| !a.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                let report = state::add_finding(
                    &frontier,
                    state::FindingDraftOptions {
                        text: assertion,
                        assertion_type: r#type,
                        source,
                        source_type,
                        author,
                        confidence,
                        evidence_type,
                        entities: parsed_entities,
                        doi,
                        pmid,
                        year,
                        journal,
                        url,
                        source_authors: parsed_source_authors,
                        conditions_text,
                        species: parsed_species,
                        in_vivo,
                        in_vitro,
                        human_data,
                        clinical_trial,
                    },
                    apply,
                )
                .unwrap_or_else(|e| fail_return(&e));
                print_state_report(&report, json);
            }
            FindingCommands::Supersede {
                frontier,
                old_id,
                assertion,
                r#type,
                source,
                source_type,
                author,
                reason,
                confidence,
                evidence_type,
                entities,
                doi,
                pmid,
                year,
                journal,
                url,
                source_authors,
                conditions_text,
                species,
                in_vivo,
                in_vitro,
                human_data,
                clinical_trial,
                json,
                apply,
            } => {
                validate_enum_arg("--type", &r#type, bundle::VALID_ASSERTION_TYPES);
                validate_enum_arg(
                    "--evidence-type",
                    &evidence_type,
                    bundle::VALID_EVIDENCE_TYPES,
                );
                validate_enum_arg(
                    "--source-type",
                    &source_type,
                    bundle::VALID_PROVENANCE_SOURCE_TYPES,
                );
                let parsed_entities = parse_entities(&entities);
                for (name, etype) in &parsed_entities {
                    if !bundle::VALID_ENTITY_TYPES.contains(&etype.as_str()) {
                        fail(&format!(
                            "invalid entity type '{}' for '{}'. Valid: {}",
                            etype,
                            name,
                            bundle::VALID_ENTITY_TYPES.join(", "),
                        ));
                    }
                }
                let parsed_source_authors = source_authors
                    .map(|s| {
                        s.split(';')
                            .map(|a| a.trim().to_string())
                            .filter(|a| !a.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                let parsed_species = species
                    .map(|s| {
                        s.split(';')
                            .map(|a| a.trim().to_string())
                            .filter(|a| !a.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                let report = state::supersede_finding(
                    &frontier,
                    &old_id,
                    &reason,
                    state::FindingDraftOptions {
                        text: assertion,
                        assertion_type: r#type,
                        source,
                        source_type,
                        author,
                        confidence,
                        evidence_type,
                        entities: parsed_entities,
                        doi,
                        pmid,
                        year,
                        journal,
                        url,
                        source_authors: parsed_source_authors,
                        conditions_text,
                        species: parsed_species,
                        in_vivo,
                        in_vitro,
                        human_data,
                        clinical_trial,
                    },
                    apply,
                )
                .unwrap_or_else(|e| fail_return(&e));
                print_state_report(&report, json);
            }
            FindingCommands::CausalSet {
                frontier,
                finding_id,
                claim,
                grade,
                actor,
                reason,
                json,
            } => {
                if !bundle::VALID_CAUSAL_CLAIMS.contains(&claim.as_str()) {
                    fail(&format!(
                        "invalid --claim '{claim}'; valid: {:?}",
                        bundle::VALID_CAUSAL_CLAIMS
                    ));
                }
                if let Some(g) = grade.as_deref()
                    && !bundle::VALID_CAUSAL_EVIDENCE_GRADES.contains(&g)
                {
                    fail(&format!(
                        "invalid --grade '{g}'; valid: {:?}",
                        bundle::VALID_CAUSAL_EVIDENCE_GRADES
                    ));
                }
                let report =
                    state::set_causal(&frontier, &finding_id, &claim, grade.as_deref(), &actor, &reason)
                        .unwrap_or_else(|e| fail_return(&e));
                print_state_report(&report, json);
            }
        },
        Commands::Review {
            frontier,
            finding_id,
            status,
            reason,
            reviewer,
            apply,
            json,
        } => {
            let status = status.unwrap_or_else(|| fail_return("--status is required for review"));
            let reason = reason.unwrap_or_else(|| fail_return("--reason is required for review"));
            let report = state::review_finding(
                &frontier,
                &finding_id,
                state::ReviewOptions {
                    status,
                    reason,
                    reviewer,
                },
                apply,
            )
            .unwrap_or_else(|e| fail_return(&e));
            print_state_report(&report, json);
        }
        Commands::Note {
            frontier,
            finding_id,
            text,
            author,
            apply,
            json,
        } => {
            let report = state::add_note(&frontier, &finding_id, &text, &author, apply)
                .unwrap_or_else(|e| fail_return(&e));
            print_state_report(&report, json);
        }
        Commands::Caveat {
            frontier,
            finding_id,
            text,
            author,
            apply,
            json,
        } => {
            let report = state::caveat_finding(&frontier, &finding_id, &text, &author, apply)
                .unwrap_or_else(|e| fail_return(&e));
            print_state_report(&report, json);
        }
        Commands::Revise {
            frontier,
            finding_id,
            confidence,
            reason,
            reviewer,
            apply,
            json,
        } => {
            let report = state::revise_confidence(
                &frontier,
                &finding_id,
                state::ReviseOptions {
                    confidence,
                    reason,
                    reviewer,
                },
                apply,
            )
            .unwrap_or_else(|e| fail_return(&e));
            print_state_report(&report, json);
        }
        Commands::Reject {
            frontier,
            finding_id,
            reason,
            reviewer,
            apply,
            json,
        } => {
            let report = state::reject_finding(&frontier, &finding_id, &reviewer, &reason, apply)
                .unwrap_or_else(|e| fail_return(&e));
            print_state_report(&report, json);
        }
        Commands::History {
            frontier,
            finding_id,
            json,
        } => {
            let payload =
                state::history(&frontier, &finding_id).unwrap_or_else(|e| fail_return(&e));
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize history response")
                );
            } else {
                print_history(&payload);
            }
        }
        Commands::ImportEvents { source, into, json } => {
            let report =
                review::import_review_events(&source, &into).unwrap_or_else(|e| fail_return(&e));
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "command": "import-events",
                        "source": report.source,
                        "target": into.display().to_string(),
                        "summary": {
                            "imported": report.imported,
                            "new": report.new,
                            "duplicate": report.duplicate,
                            "canonical_events_imported": report.events_imported,
                            "canonical_events_new": report.events_new,
                            "canonical_events_duplicate": report.events_duplicate,
                        }
                    }))
                    .expect("failed to serialize import-events response")
                );
            } else {
                println!("{report}");
            }
        }
        Commands::Retract {
            source,
            finding_id,
            reason,
            reviewer,
            apply,
            json,
        } => {
            let report = state::retract_finding(&source, &finding_id, &reviewer, &reason, apply)
                .unwrap_or_else(|e| fail_return(&e));
            print_state_report(&report, json);
        }
        Commands::Propagate {
            frontier,
            retract,
            reduce_confidence,
            to,
            output,
        } => cmd_propagate(&frontier, retract, reduce_confidence, to, output.as_deref()),
        Commands::Replicate {
            frontier,
            target,
            outcome,
            by,
            conditions,
            source_title,
            doi,
            pmid,
            sample_size,
            note,
            previous_attempt,
            no_cascade,
            json,
        } => cmd_replicate(
            &frontier,
            &target,
            &outcome,
            &by,
            &conditions,
            &source_title,
            doi.as_deref(),
            pmid.as_deref(),
            sample_size.as_deref(),
            &note,
            previous_attempt.as_deref(),
            no_cascade,
            json,
        ),
        Commands::Replications {
            frontier,
            target,
            json,
        } => cmd_replications(&frontier, target.as_deref(), json),
        Commands::DatasetAdd {
            frontier,
            name,
            version,
            content_hash,
            url,
            license,
            source_title,
            doi,
            row_count,
            json,
        } => cmd_dataset_add(
            &frontier,
            &name,
            version.as_deref(),
            &content_hash,
            url.as_deref(),
            license.as_deref(),
            &source_title,
            doi.as_deref(),
            row_count,
            json,
        ),
        Commands::Datasets { frontier, json } => cmd_datasets(&frontier, json),
        Commands::CodeAdd {
            frontier,
            language,
            repo_url,
            commit,
            path,
            content_hash,
            line_start,
            line_end,
            entry_point,
            json,
        } => cmd_code_add(
            &frontier,
            &language,
            repo_url.as_deref(),
            commit.as_deref(),
            &path,
            &content_hash,
            line_start,
            line_end,
            entry_point.as_deref(),
            json,
        ),
        Commands::CodeArtifacts { frontier, json } => cmd_code_artifacts(&frontier, json),
        Commands::Predict {
            frontier,
            by,
            claim,
            criterion,
            resolves_by,
            confidence,
            target,
            outcome,
            conditions,
            json,
        } => cmd_predict(
            &frontier,
            &by,
            &claim,
            &criterion,
            resolves_by.as_deref(),
            confidence,
            &target,
            &outcome,
            &conditions,
            json,
        ),
        Commands::Resolve {
            frontier,
            prediction,
            outcome,
            matched,
            by,
            confidence,
            source_title,
            doi,
            json,
        } => cmd_resolve(
            &frontier,
            &prediction,
            &outcome,
            matched,
            &by,
            confidence,
            &source_title,
            doi.as_deref(),
            json,
        ),
        Commands::Predictions {
            frontier,
            by,
            open,
            json,
        } => cmd_predictions(&frontier, by.as_deref(), open, json),
        Commands::Calibration {
            frontier,
            actor,
            json,
        } => cmd_calibration(&frontier, actor.as_deref(), json),
        Commands::Consensus {
            frontier,
            target,
            weighting,
            causal_claim,
            causal_grade_min,
            json,
        } => cmd_consensus(
            &frontier,
            &target,
            &weighting,
            causal_claim.as_deref(),
            causal_grade_min.as_deref(),
            json,
        ),
    }
}

/// v0.35 / v0.38.2: print consensus over claim-similar findings,
/// optionally filtered by causal claim type / minimum study grade.
fn cmd_consensus(
    frontier: &Path,
    target: &str,
    weighting_str: &str,
    causal_claim: Option<&str>,
    causal_grade_min: Option<&str>,
    json: bool,
) {
    use crate::bundle::{CausalClaim, CausalEvidenceGrade};

    if !target.starts_with("vf_") {
        fail(&format!("target `{target}` is not a vf_ finding id"));
    }
    let scheme = crate::aggregate::WeightingScheme::parse(weighting_str)
        .unwrap_or_else(|e| fail_return(&e));

    let parsed_claim = match causal_claim {
        None => None,
        Some("correlation") => Some(CausalClaim::Correlation),
        Some("mediation") => Some(CausalClaim::Mediation),
        Some("intervention") => Some(CausalClaim::Intervention),
        Some(other) => fail_return(&format!(
            "invalid --causal-claim '{other}'; valid: correlation | mediation | intervention"
        )),
    };
    let parsed_grade = match causal_grade_min {
        None => None,
        Some("theoretical") => Some(CausalEvidenceGrade::Theoretical),
        Some("observational") => Some(CausalEvidenceGrade::Observational),
        Some("quasi_experimental") => Some(CausalEvidenceGrade::QuasiExperimental),
        Some("rct") => Some(CausalEvidenceGrade::Rct),
        Some(other) => fail_return(&format!(
            "invalid --causal-grade-min '{other}'; valid: theoretical | observational | quasi_experimental | rct"
        )),
    };
    let filter = crate::aggregate::AggregateFilter {
        causal_claim: parsed_claim,
        causal_grade_min: parsed_grade,
    };
    let project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));

    let result = crate::aggregate::consensus_for_with_filter(&project, target, scheme, &filter)
        .unwrap_or_else(|| fail_return(&format!("target `{target}` not in frontier")));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).expect("serialize consensus")
        );
        return;
    }

    println!();
    println!(
        "  {}",
        format!("VELA · CONSENSUS · {} ({})", result.target, result.weighting)
            .to_uppercase()
            .dimmed()
    );
    println!("  {}", style::tick_row(60));
    println!("  target:           {}", truncate(&result.target_assertion, 80));
    println!("  similar findings: {}", result.n_findings);
    println!(
        "  consensus:        {:.3}  ({:.3} – {:.3} 95% credible)",
        result.consensus_confidence,
        result.credible_interval_lo,
        result.credible_interval_hi
    );
    println!();
    println!("  constituents (sorted by weight):");
    let mut sorted = result.constituents.clone();
    sorted.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
    for c in sorted.iter().take(10) {
        let repls = if c.n_replications > 0 {
            format!(
                "  ({}r {}f)",
                c.n_replicated, c.n_failed_replications
            )
        } else {
            String::new()
        };
        println!(
            "    · w={:.2}  raw={:.2}  adj={:.2}{}",
            c.weight, c.raw_score, c.adjusted_score, repls
        );
        println!("        {}", truncate(&c.assertion_text, 88));
    }
    if result.constituents.len() > 10 {
        println!("    ... ({} more)", result.constituents.len() - 10);
    }
}

/// v0.34: parse the `--outcome` CLI string into a structured
/// `ExpectedOutcome`. Accepted forms:
///   - `affirmed` / `falsified`
///   - `quant:VALUE±TOL UNITS`  (e.g. `quant:0.4±0.1 SD`)
///   - `cat:LABEL`              (e.g. `cat:full_approval`)
fn parse_expected_outcome(s: &str) -> Result<crate::bundle::ExpectedOutcome, String> {
    let trimmed = s.trim();
    if trimmed.eq_ignore_ascii_case("affirmed") {
        return Ok(crate::bundle::ExpectedOutcome::Affirmed);
    }
    if trimmed.eq_ignore_ascii_case("falsified") {
        return Ok(crate::bundle::ExpectedOutcome::Falsified);
    }
    if let Some(rest) = trimmed.strip_prefix("cat:") {
        return Ok(crate::bundle::ExpectedOutcome::Categorical {
            value: rest.to_string(),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("quant:") {
        let (vt, units) = rest.split_once(' ').unwrap_or((rest, ""));
        let (val_s, tol_s) = vt
            .split_once('±')
            .or_else(|| vt.split_once("+/-"))
            .ok_or_else(|| {
                format!("expected `quant:VALUE±TOL UNITS`, got `quant:{rest}`")
            })?;
        let value: f64 = val_s
            .parse()
            .map_err(|e| format!("bad quant value `{val_s}`: {e}"))?;
        let tolerance: f64 = tol_s
            .parse()
            .map_err(|e| format!("bad quant tolerance `{tol_s}`: {e}"))?;
        return Ok(crate::bundle::ExpectedOutcome::Quantitative {
            value,
            tolerance,
            units: units.to_string(),
        });
    }
    Err(format!(
        "unknown outcome `{s}`; expected one of: affirmed | falsified | quant:V±T units | cat:label"
    ))
}

/// v0.34: append a Prediction to a frontier and persist it.
#[allow(clippy::too_many_arguments)]
fn cmd_predict(
    frontier: &Path,
    by: &str,
    claim: &str,
    criterion: &str,
    resolves_by: Option<&str>,
    confidence: f64,
    target_csv: &str,
    outcome: &str,
    conditions_text: &str,
    json: bool,
) {
    if !(0.0..=1.0).contains(&confidence) {
        fail(&format!(
            "confidence must be in [0, 1]; got {confidence}"
        ));
    }
    let expected =
        parse_expected_outcome(outcome).unwrap_or_else(|e| fail_return(&e));

    let mut project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));

    let targets: Vec<String> = target_csv
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for t in &targets {
        if !t.starts_with("vf_") {
            fail(&format!("target `{t}` is not a vf_ id"));
        }
        if !project.findings.iter().any(|f| f.id == *t) {
            fail(&format!("target `{t}` not present in frontier"));
        }
    }

    let lower = conditions_text.to_lowercase();
    let conditions = crate::bundle::Conditions {
        text: conditions_text.to_string(),
        species_verified: Vec::new(),
        species_unverified: Vec::new(),
        in_vitro: lower.contains("in vitro"),
        in_vivo: lower.contains("in vivo"),
        human_data: lower.contains("human") || lower.contains("clinical"),
        clinical_trial: lower.contains("clinical trial") || lower.contains("phase "),
        concentration_range: None,
        duration: None,
        age_group: None,
        cell_type: None,
    };

    let prediction = crate::bundle::Prediction::new(
        claim.to_string(),
        targets,
        None,
        resolves_by.map(|s| s.to_string()),
        criterion.to_string(),
        expected,
        by.to_string(),
        confidence,
        conditions,
    );

    if project.predictions.iter().any(|p| p.id == prediction.id) {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": false,
                    "command": "predict",
                    "reason": "prediction_already_exists",
                    "id": prediction.id,
                }))
                .expect("serialize")
            );
        } else {
            println!(
                "{} prediction {} already exists in {}; skipping.",
                style::warn("predict"),
                prediction.id,
                frontier.display()
            );
        }
        return;
    }

    let new_id = prediction.id.clone();
    project.predictions.push(prediction);
    repo::save_to_path(frontier, &project).unwrap_or_else(|e| fail_return(&e));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "predict",
                "id": new_id,
                "made_by": by,
                "confidence": confidence,
                "frontier": frontier.display().to_string(),
            }))
            .expect("serialize predict result")
        );
    } else {
        println!();
        println!(
            "  {}",
            format!("VELA · PREDICT · {}", new_id).to_uppercase().dimmed()
        );
        println!("  {}", style::tick_row(60));
        println!("  by:           {by}");
        println!("  confidence:   {confidence:.3}");
        if let Some(d) = resolves_by {
            println!("  resolves by:  {d}");
        }
        println!("  outcome:      {outcome}");
        println!("  claim:        {}", truncate(claim, 88));
        println!();
        println!(
            "  {} prediction recorded in {}",
            style::ok("ok"),
            frontier.display()
        );
    }
}

/// v0.34: append a Resolution that closes out a Prediction.
#[allow(clippy::too_many_arguments)]
fn cmd_resolve(
    frontier: &Path,
    prediction_id: &str,
    actual_outcome: &str,
    matched: bool,
    by: &str,
    confidence: f64,
    source_title: &str,
    doi: Option<&str>,
    json: bool,
) {
    if !prediction_id.starts_with("vpred_") {
        fail(&format!("prediction `{prediction_id}` is not a vpred_ id"));
    }
    if !(0.0..=1.0).contains(&confidence) {
        fail(&format!("confidence must be in [0, 1]; got {confidence}"));
    }
    let mut project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));
    if !project.predictions.iter().any(|p| p.id == prediction_id) {
        fail(&format!(
            "prediction `{prediction_id}` not present in frontier"
        ));
    }

    let evidence = crate::bundle::Evidence {
        evidence_type: "experimental".to_string(),
        model_system: String::new(),
        species: None,
        method: "prediction_resolution".to_string(),
        sample_size: None,
        effect_size: None,
        p_value: None,
        replicated: false,
        replication_count: None,
        evidence_spans: if source_title.is_empty() {
            Vec::new()
        } else {
            vec![serde_json::json!({"text": source_title})]
        },
    };

    // If the resolver provided source provenance, embed it via the
    // evidence span (the Resolution carries Evidence; for v0.34 we
    // keep the structure minimal). DOI flows through evidence_spans
    // commentary; richer linking lands in v0.34.x.
    let _ = doi; // currently unused — placeholder for v0.34.x.

    let resolution = crate::bundle::Resolution::new(
        prediction_id.to_string(),
        actual_outcome.to_string(),
        matched,
        by.to_string(),
        evidence,
        confidence,
    );

    if project.resolutions.iter().any(|r| r.id == resolution.id) {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": false,
                    "command": "resolve",
                    "reason": "resolution_already_exists",
                    "id": resolution.id,
                }))
                .expect("serialize")
            );
        } else {
            println!(
                "{} resolution {} already exists in {}; skipping.",
                style::warn("resolve"),
                resolution.id,
                frontier.display()
            );
        }
        return;
    }

    let new_id = resolution.id.clone();
    project.resolutions.push(resolution);
    repo::save_to_path(frontier, &project).unwrap_or_else(|e| fail_return(&e));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "resolve",
                "id": new_id,
                "prediction": prediction_id,
                "matched": matched,
                "frontier": frontier.display().to_string(),
            }))
            .expect("serialize resolve result")
        );
    } else {
        println!();
        println!(
            "  {}",
            format!("VELA · RESOLVE · {}", new_id).to_uppercase().dimmed()
        );
        println!("  {}", style::tick_row(60));
        println!("  prediction:   {prediction_id}");
        println!(
            "  matched:      {}",
            if matched { style::ok("yes") } else { style::lost("no") }
        );
        println!("  by:           {by}");
        println!("  outcome:      {}", truncate(actual_outcome, 80));
        println!();
        println!(
            "  {} resolution recorded in {}",
            style::ok("ok"),
            frontier.display()
        );
    }
}

/// v0.34: list predictions, with resolution state.
fn cmd_predictions(frontier: &Path, by: Option<&str>, open: bool, json: bool) {
    let project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));

    let resolved_ids: std::collections::HashSet<&str> = project
        .resolutions
        .iter()
        .map(|r| r.prediction_id.as_str())
        .collect();

    let mut filtered: Vec<&crate::bundle::Prediction> = project
        .predictions
        .iter()
        .filter(|p| by.is_none_or(|b| p.made_by == b))
        .filter(|p| !open || !resolved_ids.contains(p.id.as_str()))
        .collect();
    filtered.sort_by(|a, b| {
        a.resolves_by
            .as_deref()
            .unwrap_or("9999")
            .cmp(b.resolves_by.as_deref().unwrap_or("9999"))
    });

    if json {
        let payload: Vec<serde_json::Value> = filtered
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "claim_text": p.claim_text,
                    "made_by": p.made_by,
                    "confidence": p.confidence,
                    "predicted_at": p.predicted_at,
                    "resolves_by": p.resolves_by,
                    "expected_outcome": p.expected_outcome,
                    "resolved": resolved_ids.contains(p.id.as_str()),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "predictions",
                "frontier": frontier.display().to_string(),
                "count": payload.len(),
                "predictions": payload,
            }))
            .expect("serialize predictions")
        );
        return;
    }

    println!();
    println!(
        "  {}",
        format!("VELA · PREDICTIONS · {}", frontier.display())
            .to_uppercase()
            .dimmed()
    );
    println!("  {}", style::tick_row(60));
    if filtered.is_empty() {
        println!("  (no predictions matching filters)");
        return;
    }
    for p in &filtered {
        let resolved = resolved_ids.contains(p.id.as_str());
        let chip = if resolved {
            style::ok("resolved")
        } else {
            style::warn("open")
        };
        let deadline = p
            .resolves_by
            .as_deref()
            .unwrap_or("(no deadline)");
        println!(
            "  · {}  {}  by {}  → {}",
            p.id.dimmed(),
            chip,
            p.made_by,
            deadline,
        );
        println!("      claim:      {}", truncate(&p.claim_text, 90));
        println!("      confidence: {:.2}", p.confidence);
    }
}

/// v0.34: print calibration scores per actor.
fn cmd_calibration(frontier: &Path, actor: Option<&str>, json: bool) {
    let project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));
    let records = match actor {
        Some(a) => crate::calibration::calibration_for_actor(
            a,
            &project.predictions,
            &project.resolutions,
        )
        .map(|r| vec![r])
        .unwrap_or_default(),
        None => crate::calibration::calibration_records(
            &project.predictions,
            &project.resolutions,
        ),
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "calibration",
                "frontier": frontier.display().to_string(),
                "filter_actor": actor,
                "records": records,
            }))
            .expect("serialize calibration")
        );
        return;
    }

    println!();
    println!(
        "  {}",
        format!("VELA · CALIBRATION · {}", frontier.display())
            .to_uppercase()
            .dimmed()
    );
    println!("  {}", style::tick_row(60));
    if records.is_empty() {
        println!("  (no calibration records)");
        return;
    }
    for r in &records {
        println!("  · {}", r.actor);
        println!(
            "      predictions: {}  resolved: {}  hits: {}",
            r.n_predictions, r.n_resolved, r.n_hit
        );
        match r.hit_rate {
            Some(h) => println!("      hit rate:    {:.1}%", h * 100.0),
            None => println!("      hit rate:    n/a"),
        }
        match r.brier_score {
            Some(b) => println!("      brier:       {:.4}  (lower is better; 0.25 = chance)", b),
            None => println!("      brier:       n/a"),
        }
        match r.log_score {
            Some(l) => println!("      log score:   {:.4}  (higher is better; 0 = perfect)", l),
            None => println!("      log score:   n/a"),
        }
    }
}

/// v0.33: append a Dataset record to a frontier and persist it.
#[allow(clippy::too_many_arguments)]
fn cmd_dataset_add(
    frontier: &Path,
    name: &str,
    version: Option<&str>,
    content_hash: &str,
    url: Option<&str>,
    license: Option<&str>,
    source_title: &str,
    doi: Option<&str>,
    row_count: Option<u64>,
    json: bool,
) {
    let mut project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));

    let provenance = crate::bundle::Provenance {
        source_type: "data_release".to_string(),
        doi: doi.map(|s| s.to_string()),
        pmid: None,
        pmc: None,
        openalex_id: None,
        url: url.map(|s| s.to_string()),
        title: source_title.to_string(),
        authors: Vec::new(),
        year: None,
        journal: None,
        license: license.map(|s| s.to_string()),
        publisher: None,
        funders: Vec::new(),
        extraction: crate::bundle::Extraction {
            method: "manual_curation".to_string(),
            model: None,
            model_version: None,
            extracted_at: chrono::Utc::now().to_rfc3339(),
            extractor_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        review: None,
        citation_count: None,
    };

    let mut dataset = crate::bundle::Dataset::new(
        name.to_string(),
        version.map(|s| s.to_string()),
        content_hash.to_string(),
        url.map(|s| s.to_string()),
        license.map(|s| s.to_string()),
        provenance,
    );
    dataset.row_count = row_count;

    if project.datasets.iter().any(|d| d.id == dataset.id) {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": false,
                    "command": "dataset.add",
                    "reason": "dataset_already_exists",
                    "id": dataset.id,
                }))
                .expect("serialize")
            );
        } else {
            println!(
                "{} dataset {} already exists in {}; skipping.",
                style::warn("dataset"),
                dataset.id,
                frontier.display()
            );
        }
        return;
    }

    let new_id = dataset.id.clone();
    project.datasets.push(dataset);
    repo::save_to_path(frontier, &project).unwrap_or_else(|e| fail_return(&e));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "dataset.add",
                "id": new_id,
                "name": name,
                "version": version,
                "frontier": frontier.display().to_string(),
            }))
            .expect("failed to serialize dataset.add result")
        );
    } else {
        println!();
        println!(
            "  {}",
            format!("VELA · DATASET · {}", new_id).to_uppercase().dimmed()
        );
        println!("  {}", style::tick_row(60));
        println!("  name:          {name}");
        if let Some(v) = version {
            println!("  version:       {v}");
        }
        println!("  content_hash:  {content_hash}");
        if let Some(u) = url {
            println!("  url:           {u}");
        }
        println!("  source:        {source_title}");
        println!();
        println!(
            "  {} dataset recorded in {}",
            style::ok("ok"),
            frontier.display()
        );
    }
}

/// v0.33: list datasets in a frontier.
fn cmd_datasets(frontier: &Path, json: bool) {
    let project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "datasets",
                "frontier": frontier.display().to_string(),
                "count": project.datasets.len(),
                "datasets": project.datasets,
            }))
            .expect("serialize datasets")
        );
        return;
    }
    println!();
    println!(
        "  {}",
        format!("VELA · DATASETS · {}", frontier.display())
            .to_uppercase()
            .dimmed()
    );
    println!("  {}", style::tick_row(60));
    if project.datasets.is_empty() {
        println!("  (no datasets registered)");
        return;
    }
    for ds in &project.datasets {
        let v = ds
            .version
            .as_deref()
            .map(|s| format!("@{s}"))
            .unwrap_or_default();
        println!("  · {}  {}{}", ds.id.dimmed(), ds.name, v);
        if let Some(u) = &ds.url {
            println!("      url:    {}", truncate(u, 80));
        }
        println!("      hash:   {}", truncate(&ds.content_hash, 80));
    }
}

/// v0.33: append a CodeArtifact record to a frontier and persist it.
#[allow(clippy::too_many_arguments)]
fn cmd_code_add(
    frontier: &Path,
    language: &str,
    repo_url: Option<&str>,
    commit: Option<&str>,
    path: &str,
    content_hash: &str,
    line_start: Option<u32>,
    line_end: Option<u32>,
    entry_point: Option<&str>,
    json: bool,
) {
    let mut project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));

    let line_range = match (line_start, line_end) {
        (Some(a), Some(b)) => Some((a, b)),
        (Some(a), None) => Some((a, a)),
        _ => None,
    };

    let artifact = crate::bundle::CodeArtifact::new(
        language.to_string(),
        repo_url.map(|s| s.to_string()),
        commit.map(|s| s.to_string()),
        path.to_string(),
        line_range,
        content_hash.to_string(),
        entry_point.map(|s| s.to_string()),
    );

    if project.code_artifacts.iter().any(|c| c.id == artifact.id) {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": false,
                    "command": "code.add",
                    "reason": "artifact_already_exists",
                    "id": artifact.id,
                }))
                .expect("serialize")
            );
        } else {
            println!(
                "{} code artifact {} already exists in {}; skipping.",
                style::warn("code"),
                artifact.id,
                frontier.display()
            );
        }
        return;
    }

    let new_id = artifact.id.clone();
    project.code_artifacts.push(artifact);
    repo::save_to_path(frontier, &project).unwrap_or_else(|e| fail_return(&e));

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "code.add",
                "id": new_id,
                "language": language,
                "path": path,
                "frontier": frontier.display().to_string(),
            }))
            .expect("failed to serialize code.add result")
        );
    } else {
        println!();
        println!(
            "  {}",
            format!("VELA · CODE · {}", new_id).to_uppercase().dimmed()
        );
        println!("  {}", style::tick_row(60));
        println!("  language:      {language}");
        if let Some(r) = repo_url {
            println!("  repo:          {r}");
        }
        if let Some(c) = commit {
            println!("  commit:        {c}");
        }
        println!("  path:          {path}");
        if let Some((a, b)) = line_range {
            println!("  lines:         {a}-{b}");
        }
        println!("  content_hash:  {content_hash}");
        println!();
        println!(
            "  {} code artifact recorded in {}",
            style::ok("ok"),
            frontier.display()
        );
    }
}

/// v0.33: list code artifacts in a frontier.
fn cmd_code_artifacts(frontier: &Path, json: bool) {
    let project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "code-artifacts",
                "frontier": frontier.display().to_string(),
                "count": project.code_artifacts.len(),
                "code_artifacts": project.code_artifacts,
            }))
            .expect("serialize code-artifacts")
        );
        return;
    }
    println!();
    println!(
        "  {}",
        format!("VELA · CODE · {}", frontier.display())
            .to_uppercase()
            .dimmed()
    );
    println!("  {}", style::tick_row(60));
    if project.code_artifacts.is_empty() {
        println!("  (no code artifacts registered)");
        return;
    }
    for c in &project.code_artifacts {
        let lr = c
            .line_range
            .map(|(a, b)| format!(":{a}-{b}"))
            .unwrap_or_default();
        println!(
            "  · {}  {} {}{}",
            c.id.dimmed(),
            c.language,
            c.path,
            lr
        );
        if let Some(r) = &c.repo_url {
            println!("      repo:   {}", truncate(r, 80));
        }
        if let Some(g) = &c.git_commit {
            println!("      commit: {g}");
        }
    }
}

/// v0.32: append a Replication attempt to a frontier.
///
/// Validates the outcome label, builds a `Replication` with a fresh
/// content-addressed id, persists it, and prints either a structured
/// JSON receipt or a human summary. Refuses to write if the target
/// finding is not present in the frontier.
#[allow(clippy::too_many_arguments)]
fn cmd_replicate(
    frontier: &Path,
    target: &str,
    outcome: &str,
    attempted_by: &str,
    conditions_text: &str,
    source_title: &str,
    doi: Option<&str>,
    pmid: Option<&str>,
    sample_size: Option<&str>,
    note: &str,
    previous_attempt: Option<&str>,
    no_cascade: bool,
    json: bool,
) {
    if !crate::bundle::VALID_REPLICATION_OUTCOMES.contains(&outcome) {
        fail(&format!(
            "invalid outcome '{outcome}'; valid: {:?}",
            crate::bundle::VALID_REPLICATION_OUTCOMES
        ));
    }
    if !target.starts_with("vf_") {
        fail(&format!("target '{target}' is not a vf_ finding id"));
    }

    let mut project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));

    if !project.findings.iter().any(|f| f.id == target) {
        fail(&format!(
            "target finding '{target}' not present in frontier '{}'",
            frontier.display()
        ));
    }

    // Build the conditions, evidence, provenance for the replication.
    // Conditions text is what enters the content-address preimage; we
    // also lift in_vivo/in_vitro/human_data flags from common keywords
    // so confidence math behaves sensibly downstream.
    let lower = conditions_text.to_lowercase();
    let conditions = crate::bundle::Conditions {
        text: conditions_text.to_string(),
        species_verified: Vec::new(),
        species_unverified: Vec::new(),
        in_vitro: lower.contains("in vitro") || lower.contains("ipsc"),
        in_vivo: lower.contains("in vivo") || lower.contains("mouse") || lower.contains("rat"),
        human_data: lower.contains("human")
            || lower.contains("clinical")
            || lower.contains("patient"),
        clinical_trial: lower.contains("clinical trial") || lower.contains("phase "),
        concentration_range: None,
        duration: None,
        age_group: None,
        cell_type: None,
    };

    let evidence = crate::bundle::Evidence {
        evidence_type: "experimental".to_string(),
        model_system: String::new(),
        species: None,
        method: "replication_attempt".to_string(),
        sample_size: sample_size.map(|s| s.to_string()),
        effect_size: None,
        p_value: None,
        replicated: outcome == "replicated",
        replication_count: None,
        evidence_spans: Vec::new(),
    };

    let provenance = crate::bundle::Provenance {
        source_type: "published_paper".to_string(),
        doi: doi.map(|s| s.to_string()),
        pmid: pmid.map(|s| s.to_string()),
        pmc: None,
        openalex_id: None,
        url: None,
        title: source_title.to_string(),
        authors: Vec::new(),
        year: None,
        journal: None,
        license: None,
        publisher: None,
        funders: Vec::new(),
        extraction: crate::bundle::Extraction {
            method: "manual_curation".to_string(),
            model: None,
            model_version: None,
            extracted_at: chrono::Utc::now().to_rfc3339(),
            extractor_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        review: None,
        citation_count: None,
    };

    let mut rep = crate::bundle::Replication::new(
        target.to_string(),
        attempted_by.to_string(),
        outcome.to_string(),
        evidence,
        conditions,
        provenance,
        note.to_string(),
    );
    rep.previous_attempt = previous_attempt.map(|s| s.to_string());

    // Refuse to write if the same vrep_id already exists (idempotent
    // re-runs are safe; conflicts surface here).
    if project.replications.iter().any(|r| r.id == rep.id) {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": false,
                    "command": "replicate",
                    "reason": "replication_already_exists",
                    "id": rep.id,
                }))
                .expect("serialize")
            );
        } else {
            println!(
                "{} replication {} already exists in {}; skipping.",
                style::warn("replicate"),
                rep.id,
                frontier.display()
            );
        }
        return;
    }

    let new_id = rep.id.clone();
    project.replications.push(rep);

    // v0.36.2: trigger the replication-aware propagation cascade. The
    // target's confidence is recomputed from the now-updated
    // `project.replications` collection (closes the A.1 loop) and
    // dependents are flagged for review with `upstream_replication_*`.
    // `inconclusive` outcomes do not cascade; we still call propagate
    // so the source-side recompute always runs.
    let cascade_result = if no_cascade {
        None
    } else {
        let result = propagate::propagate_correction(
            &mut project,
            target,
            propagate::PropagationAction::ReplicationOutcome {
                outcome: outcome.to_string(),
                vrep_id: new_id.clone(),
            },
        );
        // Persist propagation events into the canonical review log.
        // Without this, the events are emitted to stdout and lost.
        project.review_events.extend(result.events.clone());
        project::recompute_stats(&mut project);
        Some(result)
    };

    repo::save_to_path(frontier, &project).unwrap_or_else(|e| fail_return(&e));

    if json {
        let cascade_json = cascade_result.as_ref().map(|r| {
            json!({
                "affected": r.affected,
                "events": r.events.len(),
            })
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "replicate",
                "id": new_id,
                "target": target,
                "outcome": outcome,
                "attempted_by": attempted_by,
                "cascade": cascade_json,
                "frontier": frontier.display().to_string(),
            }))
            .expect("failed to serialize replicate result")
        );
    } else {
        println!();
        println!(
            "  {}",
            format!("VELA · REPLICATE · {}", new_id)
                .to_uppercase()
                .dimmed()
        );
        println!("  {}", style::tick_row(60));
        println!("  target:        {target}");
        println!("  outcome:       {outcome}");
        println!("  attempted by:  {attempted_by}");
        println!("  conditions:    {conditions_text}");
        println!("  source:        {source_title}");
        if let Some(d) = doi {
            println!("  doi:           {d}");
        }
        println!();
        println!(
            "  {} replication recorded in {}",
            style::ok("ok"),
            frontier.display()
        );
        if let Some(result) = cascade_result {
            println!(
                "  {} cascade: {} dependent(s) flagged, {} review event(s) recorded",
                style::ok("ok"),
                result.affected,
                result.events.len()
            );
        } else {
            println!(
                "  {} cascade skipped (--no-cascade)",
                style::warn("info")
            );
        }
    }
}

/// v0.32: list replications in a frontier, optionally filtered by target.
fn cmd_replications(frontier: &Path, target: Option<&str>, json: bool) {
    let project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));
    let filtered: Vec<&crate::bundle::Replication> = project
        .replications
        .iter()
        .filter(|r| target.is_none_or(|t| r.target_finding == t))
        .collect();

    if json {
        let payload = json!({
            "ok": true,
            "command": "replications",
            "frontier": frontier.display().to_string(),
            "filter_target": target,
            "count": filtered.len(),
            "replications": filtered,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .expect("failed to serialize replications list")
        );
        return;
    }

    println!();
    let header = match target {
        Some(t) => format!("VELA · REPLICATIONS · {t}"),
        None => format!("VELA · REPLICATIONS · {}", frontier.display()),
    };
    println!("  {}", header.to_uppercase().dimmed());
    println!("  {}", style::tick_row(60));
    if filtered.is_empty() {
        println!("  (no replications recorded)");
        return;
    }
    for rep in &filtered {
        let outcome_chip = match rep.outcome.as_str() {
            "replicated" => style::ok(&rep.outcome),
            "failed" => style::lost(&rep.outcome),
            "partial" => style::warn(&rep.outcome),
            _ => rep.outcome.clone().normal().to_string(),
        };
        println!(
            "  · {}  {}  by {}",
            rep.id.dimmed(),
            outcome_chip,
            rep.attempted_by
        );
        println!("      target:     {}", rep.target_finding);
        if !rep.conditions.text.is_empty() {
            println!("      conditions: {}", truncate(&rep.conditions.text, 80));
        }
        if !rep.provenance.title.is_empty() {
            println!("      source:     {}", truncate(&rep.provenance.title, 80));
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// v0.25 Agent Inbox: dispatches the registered datasets handler.
async fn cmd_compile_data(
    root: &Path,
    frontier: &Path,
    backend: Option<&str>,
    sample_rows: Option<usize>,
    dry_run: bool,
    json_out: bool,
) {
    match DATASETS_HANDLER.get() {
        Some(handler) => {
            handler(
                root.to_path_buf(),
                frontier.to_path_buf(),
                backend.map(String::from),
                sample_rows,
                dry_run,
                json_out,
            )
            .await;
        }
        None => {
            eprintln!(
                "{} `vela compile-data` requires the vela CLI binary; the library is unwired without a registered datasets handler.",
                style::err_prefix()
            );
            std::process::exit(1);
        }
    }
}

/// v0.28 Agent Inbox: dispatches the registered reviewer-agent
/// handler.
async fn cmd_review_pending(
    frontier: &Path,
    backend: Option<&str>,
    max_proposals: Option<usize>,
    batch_size: usize,
    dry_run: bool,
    json_out: bool,
) {
    match REVIEWER_HANDLER.get() {
        Some(handler) => {
            handler(
                frontier.to_path_buf(),
                backend.map(String::from),
                max_proposals,
                batch_size,
                dry_run,
                json_out,
            )
            .await;
        }
        None => {
            eprintln!(
                "{} `vela review-pending` requires the vela CLI binary; the library is unwired without a registered reviewer handler.",
                style::err_prefix()
            );
            std::process::exit(1);
        }
    }
}

/// v0.28 Agent Inbox: dispatches the registered contradiction-finder
/// handler.
async fn cmd_find_tensions(
    frontier: &Path,
    backend: Option<&str>,
    max_findings: Option<usize>,
    dry_run: bool,
    json_out: bool,
) {
    match TENSIONS_HANDLER.get() {
        Some(handler) => {
            handler(
                frontier.to_path_buf(),
                backend.map(String::from),
                max_findings,
                dry_run,
                json_out,
            )
            .await;
        }
        None => {
            eprintln!(
                "{} `vela find-tensions` requires the vela CLI binary; the library is unwired without a registered tensions handler.",
                style::err_prefix()
            );
            std::process::exit(1);
        }
    }
}

/// v0.28 Agent Inbox: dispatches the registered experiment-planner
/// handler.
async fn cmd_plan_experiments(
    frontier: &Path,
    backend: Option<&str>,
    max_findings: Option<usize>,
    dry_run: bool,
    json_out: bool,
) {
    match EXPERIMENTS_HANDLER.get() {
        Some(handler) => {
            handler(
                frontier.to_path_buf(),
                backend.map(String::from),
                max_findings,
                dry_run,
                json_out,
            )
            .await;
        }
        None => {
            eprintln!(
                "{} `vela plan-experiments` requires the vela CLI binary; the library is unwired without a registered experiments handler.",
                style::err_prefix()
            );
            std::process::exit(1);
        }
    }
}

/// v0.24 Agent Inbox: dispatches the registered code-analyst
/// handler.
async fn cmd_compile_code(
    root: &Path,
    frontier: &Path,
    backend: Option<&str>,
    max_files: Option<usize>,
    dry_run: bool,
    json_out: bool,
) {
    match CODE_HANDLER.get() {
        Some(handler) => {
            handler(
                root.to_path_buf(),
                frontier.to_path_buf(),
                backend.map(String::from),
                max_files,
                dry_run,
                json_out,
            )
            .await;
        }
        None => {
            eprintln!(
                "{} `vela compile-code` requires the vela CLI binary; the library is unwired without a registered code handler.",
                style::err_prefix()
            );
            std::process::exit(1);
        }
    }
}

/// v0.23 Agent Inbox: dispatches the registered notes-compiler
/// handler. Same rationale as `cmd_scout` — the substrate stays
/// agent-free; the `vela` CLI binary registers the handler at
/// startup.
async fn cmd_compile_notes(
    vault: &Path,
    frontier: &Path,
    backend: Option<&str>,
    max_files: Option<usize>,
    max_items_per_category: Option<usize>,
    dry_run: bool,
    json_out: bool,
) {
    match NOTES_HANDLER.get() {
        Some(handler) => {
            handler(
                vault.to_path_buf(),
                frontier.to_path_buf(),
                backend.map(String::from),
                max_files,
                max_items_per_category,
                dry_run,
                json_out,
            )
            .await;
        }
        None => {
            eprintln!(
                "{} `vela compile-notes` requires the vela CLI binary; the library is unwired without a registered notes handler.",
                style::err_prefix()
            );
            std::process::exit(1);
        }
    }
}

/// v0.22 Agent Inbox: dispatches the registered scout handler. The
/// substrate library does not import `vela-scientist` (it would induce
/// a Cargo cycle); the `vela` CLI binary in `crates/vela-cli`
/// registers a handler at startup that calls into the scientist
/// crate. Running the lib directly without that registration prints
/// a clear error.
async fn cmd_scout(
    folder: &Path,
    frontier: &Path,
    backend: Option<&str>,
    dry_run: bool,
    json_out: bool,
) {
    match SCOUT_HANDLER.get() {
        Some(handler) => {
            handler(
                folder.to_path_buf(),
                frontier.to_path_buf(),
                backend.map(String::from),
                dry_run,
                json_out,
            )
            .await;
        }
        None => {
            eprintln!(
                "{} `vela scout` requires the vela CLI binary; the library is unwired without a registered scout handler.",
                style::err_prefix()
            );
            std::process::exit(1);
        }
    }
}


#[allow(clippy::too_many_arguments)]
fn cmd_check(
    source: Option<&Path>,
    schema: bool,
    stats: bool,
    conformance_flag: bool,
    conformance_dir: &Path,
    all: bool,
    schema_only: bool,
    strict: bool,
    fix: bool,
    json_output: bool,
) {
    if json_output {
        let Some(src) = source else {
            fail("--json requires a frontier source");
        };
        let payload = check_json_payload(src, schema_only, strict);
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("failed to serialize check report")
        );
        if payload.get("ok").and_then(Value::as_bool) != Some(true) {
            std::process::exit(1);
        }
        return;
    }

    let run_all = all || (!schema && !stats && !conformance_flag && !schema_only);
    if run_all || schema || schema_only {
        let Some(src) = source else {
            fail("check requires a frontier source");
        };
        validate::run(src);
    }
    if !schema_only && (run_all || stats) {
        let Some(src) = source else {
            fail("--stats requires a frontier source");
        };
        let frontier = repo::load_from_path(src).expect("Failed to load frontier");
        let report = lint::lint(&frontier, None, None);
        lint::print_report(&report);
        let replay_report = events::replay_report(&frontier);
        println!("event replay: {}", replay_report.status);
        if !replay_report.conflicts.is_empty() {
            for conflict in &replay_report.conflicts {
                println!("  - {conflict}");
            }
        }
        if let Ok(signature_report) = sign::verify_frontier_data(&frontier, None)
            && signature_report.signed > 0
        {
            println!(
                "Signatures: {} valid / {} invalid / {} unsigned",
                signature_report.valid, signature_report.invalid, signature_report.unsigned
            );
        }
        let signal_report = signals::analyze(&frontier, &[]);
        print_signal_summary(&signal_report, strict);
        if !replay_report.ok
            || (strict
                && (!signal_report.review_queue.is_empty()
                    || signal_report.proof_readiness.status != "ready"))
        {
            std::process::exit(1);
        }
    }
    if run_all || conformance_flag {
        conformance::run(conformance_dir);
    }
    let _ = fix;
}

fn check_json_payload(src: &Path, schema_only: bool, strict: bool) -> Value {
    let report = validate::validate(src);
    let loaded = repo::load_from_path(src).ok();
    let (method_report, graph_report) = if schema_only {
        (None, None)
    } else if let Some(frontier) = loaded.as_ref() {
        (
            Some(lint::lint(frontier, None, None)),
            Some(lint::lint_frontier(frontier)),
        )
    } else {
        (None, None)
    };
    let source_hash = hash_path(src).unwrap_or_else(|_| "unavailable".to_string());
    let mut diagnostics = Vec::new();
    diagnostics.extend(report.errors.iter().map(|e| {
        json!({
            "severity": "error",
            "rule_id": "schema",
            "finding_id": null,
            "file": &e.file,
            "field_path": null,
            "message": &e.error,
            "suggestion": schema_error_suggestion(&e.error),
            "fixable": schema_error_fix(&e.error),
            "normalize_action": schema_error_action(&e.error),
        })
    }));
    for (check_id, lint_report) in [
        ("methodology", method_report.as_ref()),
        ("frontier_graph", graph_report.as_ref()),
    ] {
        if let Some(lint_report) = lint_report {
            diagnostics.extend(lint_report.diagnostics.iter().map(|d| {
                json!({
                    "severity": d.severity.to_string(),
                    "rule_id": &d.rule_id,
                    "check": check_id,
                    "finding_id": &d.finding_id,
                    "field_path": null,
                    "message": &d.message,
                    "suggestion": &d.suggestion,
                    "fixable": false,
                    "normalize_action": null,
                })
            }));
        }
    }
    let method_errors = method_report.as_ref().map_or(0, |r| r.errors);
    let method_warnings = method_report.as_ref().map_or(0, |r| r.warnings);
    let method_infos = method_report.as_ref().map_or(0, |r| r.infos);
    let graph_errors = graph_report.as_ref().map_or(0, |r| r.errors);
    let graph_warnings = graph_report.as_ref().map_or(0, |r| r.warnings);
    let graph_infos = graph_report.as_ref().map_or(0, |r| r.infos);
    let replay_report = loaded.as_ref().map(events::replay_report);
    if let Some(replay) = replay_report.as_ref()
        && !replay.ok
    {
        diagnostics.extend(replay.conflicts.iter().map(|conflict| {
            json!({
                "severity": "error",
                "rule_id": "event_replay",
                "check": "events",
                "finding_id": null,
                "field_path": null,
                "message": conflict,
                "suggestion": "Inspect canonical state events and repair the frontier event log before proof export.",
                "fixable": false,
                "normalize_action": null,
            })
        }));
    }
    let event_errors = replay_report
        .as_ref()
        .map_or(0, |replay| usize::from(!replay.ok));
    let (source_registry, evidence_atoms, conditions, proposal_summary, proof_state) = loaded
        .as_ref()
        .map(|frontier| {
            (
                sources::source_summary(frontier),
                sources::evidence_summary(frontier),
                sources::condition_summary(frontier),
                proposals::summary(frontier),
                proposals::proof_state_json(&frontier.proof_state),
            )
        })
        .unwrap_or_else(|| {
            (
                sources::SourceRegistrySummary::default(),
                sources::EvidenceAtomSummary::default(),
                sources::ConditionSummary::default(),
                proposals::ProposalSummary::default(),
                Value::Null,
            )
        });
    let signature_report = loaded
        .as_ref()
        .and_then(|frontier| sign::verify_frontier_data(frontier, None).ok());
    if let Some(frontier) = loaded.as_ref()
        && !schema_only
    {
        let projection = sources::derive_projection(frontier);
        let existing_sources = frontier
            .sources
            .iter()
            .map(|source| source.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let existing_atoms = frontier
            .evidence_atoms
            .iter()
            .map(|atom| atom.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let existing_conditions = frontier
            .condition_records
            .iter()
            .map(|record| record.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for source in projection
            .sources
            .iter()
            .filter(|source| !existing_sources.contains(source.id.as_str()))
        {
            diagnostics.push(json!({
                "severity": "warning",
                "rule_id": "missing_source_record",
                "check": "source_registry",
                "finding_id": source.finding_ids.first(),
                "field_path": "sources",
                "message": format!("Source record {} is derivable but not materialized in frontier state.", source.id),
                "suggestion": "Run `vela normalize` to materialize source records before proof export.",
                "fixable": true,
                "normalize_action": "materialize_source_record",
            }));
        }
        for atom in projection
            .evidence_atoms
            .iter()
            .filter(|atom| !existing_atoms.contains(atom.id.as_str()))
        {
            diagnostics.push(json!({
                "severity": "warning",
                "rule_id": "missing_evidence_atom",
                "check": "evidence_atoms",
                "finding_id": atom.finding_id,
                "field_path": "evidence_atoms",
                "message": format!("Evidence atom {} is derivable but not materialized in frontier state.", atom.id),
                "suggestion": "Run `vela normalize` to materialize evidence atoms before proof export.",
                "fixable": true,
                "normalize_action": "materialize_evidence_atom",
            }));
        }
        for atom in projection
            .evidence_atoms
            .iter()
            .filter(|atom| atom.locator.is_none())
        {
            diagnostics.push(json!({
                "severity": "warning",
                "rule_id": "missing_evidence_locator",
                "check": "evidence_atoms",
                "finding_id": atom.finding_id,
                "field_path": "evidence_atoms[].locator",
                "message": format!("Evidence atom {} has no source locator.", atom.id),
                "suggestion": "Add or verify evidence spans, table rows, pages, sections, or run locators.",
                "fixable": false,
                "normalize_action": null,
            }));
        }
        for condition in projection
            .condition_records
            .iter()
            .filter(|condition| !existing_conditions.contains(condition.id.as_str()))
        {
            diagnostics.push(json!({
                "severity": "warning",
                "rule_id": "condition_record_missing",
                "check": "conditions",
                "finding_id": condition.finding_id,
                "field_path": "condition_records",
                "message": format!("Condition record {} is derivable but not materialized in frontier state.", condition.id),
                "suggestion": "Run `vela normalize` to materialize condition boundaries before proof export.",
                "fixable": true,
                "normalize_action": "materialize_condition_record",
            }));
        }
        for proposal in frontier.proposals.iter().filter(|proposal| {
            matches!(proposal.status.as_str(), "accepted" | "applied")
                && proposal
                    .reviewed_by
                    .as_deref()
                    .is_none_or(proposals::is_placeholder_reviewer)
        }) {
            diagnostics.push(json!({
                "severity": "error",
                "rule_id": "reviewer_identity_missing",
                "check": "proposals",
                "finding_id": proposal.target.id,
                "field_path": "proposals[].reviewed_by",
                "message": format!("Accepted or applied proposal {} uses a missing or placeholder reviewer identity.", proposal.id),
                "suggestion": "Accept the proposal with a stable named reviewer id before strict proof use.",
                "fixable": false,
                "normalize_action": null,
            }));
        }
    }
    let signal_report = loaded
        .as_ref()
        .map(|frontier| signals::analyze(frontier, &diagnostics))
        .unwrap_or_else(empty_signal_report);
    let errors = report.errors.len() + method_errors + graph_errors + event_errors;
    let warnings = method_warnings + graph_warnings + signal_report.proof_readiness.warnings;
    let infos = method_infos + graph_infos;
    let strict_blockers = signal_report
        .signals
        .iter()
        .filter(|signal| signal.blocks.iter().any(|block| block == "strict_check"))
        .count();
    let fixable = diagnostics
        .iter()
        .filter(|d| d.get("fixable").and_then(Value::as_bool).unwrap_or(false))
        .count();
    let ok = errors == 0 && (!strict || (warnings == 0 && strict_blockers == 0));

    json!({
        "ok": ok,
        "command": "check",
        "schema_version": project::VELA_SCHEMA_VERSION,
        "source": {
            "path": src.display().to_string(),
            "hash": format!("sha256:{source_hash}"),
        },
        "summary": {
            "status": if ok { "pass" } else { "fail" },
            "checked_findings": report.total_files,
            "valid_findings": report.valid,
            "invalid_findings": report.invalid,
            "errors": errors,
            "warnings": warnings,
            "info": infos,
            "fixable": fixable,
            "strict": strict,
            "schema_only": schema_only,
        },
        "checks": [
            {
                "id": "schema",
                "status": if report.invalid == 0 { "pass" } else { "fail" },
                "checked": report.total_files,
                "failed": report.invalid,
                "errors": report.errors.iter().map(|e| json!({
                    "file": e.file,
                    "message": e.error,
                })).collect::<Vec<_>>(),
            },
            {
                "id": "methodology",
                "status": if method_errors == 0 { "pass" } else { "fail" },
                "checked": method_report.as_ref().map_or(0, |r| r.findings_checked),
                "failed": method_errors,
                "warnings": method_warnings,
                "info": method_infos,
                "skipped": schema_only,
            },
            {
                "id": "frontier_graph",
                "status": if graph_errors == 0 { "pass" } else { "fail" },
                "checked": graph_report.as_ref().map_or(0, |r| r.findings_checked),
                "failed": graph_errors,
                "warnings": graph_warnings,
                "info": graph_infos,
                "skipped": schema_only,
            },
            {
                "id": "signals",
                "status": if strict_blockers == 0 { "pass" } else { "fail" },
                "checked": signal_report.signals.len(),
                "failed": strict_blockers,
                "warnings": signal_report.proof_readiness.warnings,
                "skipped": loaded.is_none(),
                "blockers": signal_report.signals.iter()
                    .filter(|s| s.blocks.iter().any(|b| b == "strict_check"))
                    .map(|s| json!({
                        "id": s.id,
                        "kind": s.kind,
                        "severity": s.severity,
                        "reason": s.reason,
                    }))
                    .collect::<Vec<_>>(),
            },
            {
                "id": "events",
                "status": if replay_report.as_ref().is_none_or(|replay| replay.ok) { "pass" } else { "fail" },
                "checked": replay_report.as_ref().map_or(0, |replay| replay.event_log.count),
                "failed": event_errors,
                "skipped": schema_only || loaded.is_none(),
            }
        ],
        "event_log": replay_report.as_ref().map(|replay| &replay.event_log),
        "replay": replay_report,
        "source_registry": source_registry,
        "evidence_atoms": evidence_atoms,
        "conditions": conditions,
        "proposals": proposal_summary,
        "proof_state": proof_state,
        "signatures": signature_report,
        "diagnostics": diagnostics,
        "signals": signal_report.signals,
        "review_queue": signal_report.review_queue,
        "proof_readiness": signal_report.proof_readiness,
        "repair_plan": build_repair_plan(&diagnostics),
    })
}

#[allow(clippy::too_many_arguments)]
fn cmd_normalize(
    source: &Path,
    out: Option<&Path>,
    write: bool,
    dry_run: bool,
    rewrite_ids: bool,
    id_map: Option<&Path>,
    resync_provenance: bool,
    json_output: bool,
) {
    if write && out.is_some() {
        fail("Use either --write or --out, not both.");
    }
    if dry_run && (write || out.is_some()) {
        fail("--dry-run cannot be combined with --write or --out.");
    }
    if id_map.is_some() && !rewrite_ids {
        fail("--id-map requires --rewrite-ids.");
    }

    let detected = repo::detect(source).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    if matches!(detected, repo::VelaSource::PacketDir(_)) {
        fail(
            "Cannot normalize a proof packet directory. Export a new packet from frontier state instead.",
        );
    }
    let mut frontier = repo::load(&detected).unwrap_or_else(|e| fail_return(&e));
    // Phase J: every v0.4 frontier carries a `frontier.created` genesis
    // event in events[0]. That's identity metadata, not a substantive
    // mutation, so it doesn't disqualify normalization. Any non-genesis
    // canonical event still blocks normalize.
    let has_substantive_events = frontier
        .events
        .iter()
        .any(|event| event.kind != "frontier.created");
    if has_substantive_events && (write || out.is_some()) {
        fail(
            "Refusing to normalize a frontier with canonical events. Normalize before proposal-backed writes, or create a new reviewed transition for the intended change.",
        );
    }
    let source_hash = hash_path(source).unwrap_or_else(|_| "unavailable".to_string());
    let before_stats = serde_json::to_value(&frontier.stats).unwrap_or(Value::Null);
    let (entity_type_fixes, entity_name_fixes) =
        normalize::normalize_findings(&mut frontier.findings);
    let confidence_updates =
        bundle::recompute_all_confidence(&mut frontier.findings, &frontier.replications);
    // Phase N: optionally rewrite finding.provenance from the canonical
    // SourceRecord. The source registry is the authority; provenance is
    // the denormalized cache.
    let provenance_resync_count = if resync_provenance {
        sources::resync_provenance_from_sources(&mut frontier)
    } else {
        0
    };
    let before_source_count = frontier.sources.len();
    let before_evidence_atom_count = frontier.evidence_atoms.len();
    let before_condition_record_count = frontier.condition_records.len();

    let mut id_rewrites = Vec::new();
    if rewrite_ids {
        let mut id_map_values = std::collections::BTreeMap::<String, String>::new();
        for finding in &frontier.findings {
            let expected =
                bundle::FindingBundle::content_address(&finding.assertion, &finding.provenance);
            if expected != finding.id {
                id_map_values.insert(finding.id.clone(), expected);
            }
        }
        let new_ids = id_map_values
            .values()
            .map(String::as_str)
            .collect::<std::collections::HashSet<_>>();
        if new_ids.len() != id_map_values.len() {
            fail("Refusing to rewrite IDs because two findings map to the same content address.");
        }
        for finding in &mut frontier.findings {
            if let Some(new_id) = id_map_values.get(&finding.id) {
                id_rewrites.push(json!({"old": finding.id, "new": new_id}));
                finding.previous_version = Some(finding.id.clone());
                finding.id = new_id.clone();
            }
        }
        for finding in &mut frontier.findings {
            for link in &mut finding.links {
                if let Some(new_target) = id_map_values.get(&link.target) {
                    link.target = new_target.clone();
                }
            }
        }
        if let Some(path) = id_map {
            std::fs::write(
                path,
                serde_json::to_string_pretty(&id_map_values)
                    .expect("failed to serialize normalize id map"),
            )
            .unwrap_or_else(|e| fail(&format!("Failed to write {}: {e}", path.display())));
        }
    }

    sources::materialize_project(&mut frontier);
    let source_records_materialized = frontier.sources.len().saturating_sub(before_source_count);
    let evidence_atoms_materialized = frontier
        .evidence_atoms
        .len()
        .saturating_sub(before_evidence_atom_count);
    let condition_records_materialized = frontier
        .condition_records
        .len()
        .saturating_sub(before_condition_record_count);
    let after_stats = serde_json::to_value(&frontier.stats).unwrap_or(Value::Null);
    let id_rewrite_count = id_rewrites.len();
    let wrote_to = if write {
        repo::save(&detected, &frontier).unwrap_or_else(|e| fail(&e));
        Some(source.display().to_string())
    } else if let Some(out_path) = out {
        repo::save_to_path(out_path, &frontier).unwrap_or_else(|e| fail(&e));
        Some(out_path.display().to_string())
    } else {
        None
    };
    let wrote = wrote_to.is_some();
    let planned_changes = entity_type_fixes
        + entity_name_fixes
        + confidence_updates
        + id_rewrite_count
        + source_records_materialized
        + evidence_atoms_materialized
        + condition_records_materialized
        + provenance_resync_count;
    let payload = json!({
        "ok": true,
        "command": "normalize",
        "schema_version": project::VELA_SCHEMA_VERSION,
        "source": {
            "path": source.display().to_string(),
            "hash": format!("sha256:{source_hash}"),
        },
        "dry_run": wrote_to.is_none(),
        "wrote_to": wrote_to,
        "summary": {
            "planned": planned_changes,
            "safe": planned_changes,
            "unsafe": 0,
            "applied": if wrote { planned_changes } else { 0 },
        },
        "changes": {
            "entity_type_fixes": entity_type_fixes,
            "entity_name_fixes": entity_name_fixes,
            "confidence_updates": confidence_updates,
            "id_rewrites": id_rewrite_count,
            "source_records_materialized": source_records_materialized,
            "evidence_atoms_materialized": evidence_atoms_materialized,
            "condition_records_materialized": condition_records_materialized,
            "provenance_resyncs": provenance_resync_count,
            "stats_changed": before_stats != after_stats,
        },
        "id_rewrites": id_rewrites,
        "repair_plan": if wrote { Vec::<Value>::new() } else {
            vec![json!({
                "action": "apply_normalization",
                "command": "vela normalize <frontier> --out frontier.normalized.json"
            })]
        },
    });
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("failed to serialize normalize report")
        );
    } else if let Some(path) = payload.get("wrote_to").and_then(Value::as_str) {
        println!("{} normalized frontier written to {path}", style::ok("ok"));
        println!(
            "  entity type fixes: {}, entity name fixes: {}, confidence updates: {}, id rewrites: {}",
            entity_type_fixes, entity_name_fixes, confidence_updates, id_rewrite_count
        );
    } else {
        println!("normalize dry run for {}", source.display());
        println!(
            "  would apply entity type fixes: {}, entity name fixes: {}, confidence updates: {}, id rewrites: {}",
            entity_type_fixes, entity_name_fixes, confidence_updates, id_rewrite_count
        );
    }
}

fn cmd_proof(
    frontier: &Path,
    out: &Path,
    template: &str,
    gold: Option<&Path>,
    record_proof_state: bool,
    json_output: bool,
) {
    if template != "bbb-alzheimer" {
        fail(&format!(
            "Unsupported proof template '{template}'. Supported: bbb-alzheimer"
        ));
    }
    let mut loaded = repo::load_from_path(frontier).expect("Failed to load frontier");
    let source_hash = hash_path(frontier).expect("failed to hash frontier");
    let export_record = export::export_packet(&loaded, out).unwrap_or_else(|e| fail(&e));
    let benchmark_summary = gold.map(|gold_path| {
        let summary = benchmark::run_suite(gold_path).unwrap_or_else(|e| {
            fail(&format!(
                "Failed to run proof benchmark '{}': {e}",
                gold_path.display()
            ))
        });
        append_packet_json_file(out, "benchmark-summary.json", &summary).unwrap_or_else(|e| {
            fail(&format!("Failed to write benchmark summary: {e}"));
        });
        if summary.get("ok").and_then(Value::as_bool) != Some(true) {
            fail(&format!(
                "Proof benchmark failed for {}",
                gold_path.display()
            ));
        }
        summary
    });
    let validation_summary = packet::validate(out).unwrap_or_else(|e| {
        fail(&format!("Proof packet validation failed: {e}"));
    });
    proposals::record_proof_export(
        &mut loaded,
        proposals::ProofPacketRecord {
            generated_at: export_record.generated_at.clone(),
            snapshot_hash: export_record.snapshot_hash.clone(),
            event_log_hash: export_record.event_log_hash.clone(),
            packet_manifest_hash: export_record.packet_manifest_hash.clone(),
        },
    );
    project::recompute_stats(&mut loaded);
    if record_proof_state {
        repo::save_to_path(frontier, &loaded).unwrap_or_else(|e| fail(&e));
    }
    let signal_report = signals::analyze(&loaded, &[]);
    if json_output {
        let payload = json!({
            "ok": true,
            "command": "proof",
            "schema_version": project::VELA_SCHEMA_VERSION,
            "recorded_proof_state": record_proof_state,
            "frontier": {
                "name": &loaded.project.name,
                "source": frontier.display().to_string(),
                "hash": format!("sha256:{source_hash}"),
            },
            "template": template,
            "gold": gold.map(|p| p.display().to_string()),
            "benchmark": benchmark_summary,
            "output": out.display().to_string(),
            "packet": {
                "manifest_path": out.join("manifest.json").display().to_string(),
            },
            "validation": {
                "status": "ok",
                "summary": validation_summary,
            },
            "proposals": proposals::summary(&loaded),
            "proof_state": loaded.proof_state,
            "signals": signal_report.signals,
            "review_queue": signal_report.review_queue,
            "proof_readiness": signal_report.proof_readiness,
            "trace_path": out.join("proof-trace.json").display().to_string(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("failed to serialize proof response")
        );
    } else {
        println!("vela proof");
        println!("  source:   {}", frontier.display());
        println!("  template: {template}");
        println!("  output:   {}", out.display());
        println!("  trace:    {}", out.join("proof-trace.json").display());
        println!(
            "  proof state: {}",
            if record_proof_state {
                "recorded"
            } else {
                "not recorded"
            }
        );
        println!();
        println!("{validation_summary}");
    }
}

fn cmd_stats(path: &Path) {
    let frontier = repo::load_from_path(path).expect("Failed to load frontier");
    let s = &frontier.stats;
    println!();
    println!("  {}", "FRONTIER · V0.36.0".dimmed());
    println!("  {}", frontier.project.name.bold());
    println!("  {}", style::tick_row(60));
    println!("  id:             {}", frontier.frontier_id());
    println!("  compiled:       {}", frontier.project.compiled_at);
    println!("  papers:         {}", frontier.project.papers_processed);
    println!("  findings:       {}", s.findings);
    println!("  links:          {}", s.links);
    println!("  replicated:     {}", s.replicated);
    println!("  avg confidence: {}", s.avg_confidence);
    println!("  gaps:           {}", s.gaps);
    println!("  contested:      {}", s.contested);
    println!("  reviewed:       {}", s.human_reviewed);
    println!("  proposals:      {}", s.proposal_count);
    println!(
        "  recorded proof: {}",
        frontier.proof_state.latest_packet.status
    );
    if frontier.proof_state.latest_packet.status != "never_exported" {
        println!(
            "  proof note:     recorded frontier metadata; packet files are checked by `vela packet validate`"
        );
    }
    if !s.categories.is_empty() {
        println!();
        println!("  {}", "categories".dimmed());
        let mut categories = s.categories.iter().collect::<Vec<_>>();
        categories.sort_by(|a, b| b.1.cmp(a.1));
        for (category, count) in categories {
            println!("    {category}: {}", count);
        }
    }
    println!();
    println!("  {}", style::tick_row(60));
    println!();
}

fn cmd_proposals(action: ProposalAction) {
    match action {
        ProposalAction::List {
            frontier,
            status,
            json,
        } => {
            let frontier_state =
                repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let proposals_list = proposals::list(&frontier_state, status.as_deref());
            let payload = json!({
                "ok": true,
                "command": "proposals.list",
                "frontier": frontier_state.project.name,
                "status_filter": status,
                "summary": proposals::summary(&frontier_state),
                "proposals": proposals_list,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize proposals list")
                );
            } else {
                println!("vela proposals list");
                println!("  frontier: {}", frontier_state.project.name);
                println!(
                    "  proposals: {}",
                    payload["proposals"].as_array().map_or(0, Vec::len)
                );
            }
        }
        ProposalAction::Show {
            frontier,
            proposal_id,
            json,
        } => {
            let frontier_state =
                repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let proposal =
                proposals::show(&frontier_state, &proposal_id).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "proposals.show",
                "frontier": frontier_state.project.name,
                "proposal": proposal,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize proposal show")
                );
            } else {
                println!("vela proposals show");
                println!("  frontier: {}", frontier_state.project.name);
                println!("  proposal: {}", proposal_id);
                println!("  kind: {}", proposal.kind);
                println!("  status: {}", proposal.status);
            }
        }
        ProposalAction::Import {
            frontier,
            source,
            json,
        } => {
            let report =
                proposals::import_from_path(&frontier, &source).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "proposals.import",
                "frontier": frontier.display().to_string(),
                "source": source.display().to_string(),
                "summary": {
                    "imported": report.imported,
                    "applied": report.applied,
                    "rejected": report.rejected,
                    "duplicates": report.duplicates,
                },
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize proposal import")
                );
            } else {
                println!(
                    "Imported {} proposals into {}",
                    report.imported, report.wrote_to
                );
            }
        }
        ProposalAction::Validate { source, json } => {
            let report = proposals::validate_source(&source).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": report.ok,
                "command": "proposals.validate",
                "source": source.display().to_string(),
                "summary": {
                    "checked": report.checked,
                    "valid": report.valid,
                    "invalid": report.invalid,
                },
                "proposal_ids": report.proposal_ids,
                "errors": report.errors,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize proposal validation")
                );
            } else if report.ok {
                println!("{} validated {} proposals", style::ok("ok"), report.valid);
            } else {
                println!(
                    "{} validated {} proposals, {} invalid",
                    style::lost("lost"),
                    report.valid,
                    report.invalid
                );
                for error in &report.errors {
                    println!("  · {error}");
                }
                std::process::exit(1);
            }
        }
        ProposalAction::Export {
            frontier,
            output,
            status,
            json,
        } => {
            let count = proposals::export_to_path(&frontier, &output, status.as_deref())
                .unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "proposals.export",
                "frontier": frontier.display().to_string(),
                "output": output.display().to_string(),
                "status": status,
                "exported": count,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize proposal export")
                );
            } else {
                println!("sealed · {count} proposals · {}", output.display());
            }
        }
        ProposalAction::Accept {
            frontier,
            proposal_id,
            reviewer,
            reason,
            json,
        } => {
            let event_id = proposals::accept_at_path(&frontier, &proposal_id, &reviewer, &reason)
                .unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "proposals.accept",
                "frontier": frontier.display().to_string(),
                "proposal_id": proposal_id,
                "reviewer": reviewer,
                "applied_event_id": event_id,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize proposal accept")
                );
            } else {
                println!(
                    "{} accepted and applied proposal {}",
                    style::ok("ok"),
                    proposal_id
                );
                println!("  event: {}", event_id);
            }
        }
        ProposalAction::Reject {
            frontier,
            proposal_id,
            reviewer,
            reason,
            json,
        } => {
            proposals::reject_at_path(&frontier, &proposal_id, &reviewer, &reason)
                .unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "proposals.reject",
                "frontier": frontier.display().to_string(),
                "proposal_id": proposal_id,
                "reviewer": reviewer,
                "status": "rejected",
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize proposal reject")
                );
            } else {
                println!(
                    "{} rejected proposal {}",
                    style::warn("rejected"),
                    proposal_id
                );
            }
        }
    }
}

fn cmd_sign(action: SignAction) {
    match action {
        SignAction::GenerateKeypair { out, json } => {
            let public_key = sign::generate_keypair(&out).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "sign.generate-keypair",
                "output_dir": out.display().to_string(),
                "public_key": public_key,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize sign.generate-keypair")
                );
            } else {
                println!("{} keypair · {}", style::ok("generated"), out.display());
                println!("  public key: {public_key}");
            }
        }
        SignAction::Apply {
            frontier,
            private_key,
            json,
        } => {
            let count =
                sign::sign_frontier(&frontier, &private_key).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "sign.apply",
                "frontier": frontier.display().to_string(),
                "private_key": private_key.display().to_string(),
                "signed": count,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).expect("failed to serialize sign.apply")
                );
            } else {
                println!(
                    "{} {count} findings in {}",
                    style::ok("signed"),
                    frontier.display()
                );
            }
        }
        SignAction::Verify {
            frontier,
            public_key,
            json,
        } => {
            let report = sign::verify_frontier(&frontier, public_key.as_deref())
                .unwrap_or_else(|e| fail_return(&e));
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("failed to serialize sign.verify")
                );
            } else {
                println!();
                println!(
                    "  {}",
                    format!("VELA · SIGN · VERIFY · {}", frontier.display())
                        .to_uppercase()
                        .dimmed()
                );
                println!("  {}", style::tick_row(60));
                println!("  total findings:   {}", report.total_findings);
                println!("  signed:           {}", report.signed);
                println!("  unsigned:         {}", report.unsigned);
                println!("  valid:            {}", report.valid);
                println!("  invalid:          {}", report.invalid);
                if report.findings_with_threshold > 0 {
                    println!("  with threshold:   {}", report.findings_with_threshold);
                    println!("  jointly accepted: {}", report.jointly_accepted);
                }
            }
        }
        SignAction::ThresholdSet {
            frontier,
            finding_id,
            to,
            json,
        } => {
            if to == 0 {
                fail("--to must be >= 1");
            }
            let mut project =
                repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let Some(idx) = project.findings.iter().position(|f| f.id == finding_id) else {
                fail(&format!("finding '{finding_id}' not present in frontier"));
            };
            project.findings[idx].flags.signature_threshold = Some(to);
            // Re-derive the joint-accept flag immediately; if the
            // existing signature pool already meets the threshold, the
            // finding becomes jointly_accepted on the same write.
            sign::refresh_jointly_accepted(&mut project);
            let met = project.findings[idx].flags.jointly_accepted;
            repo::save_to_path(&frontier, &project).unwrap_or_else(|e| fail_return(&e));

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "command": "sign.threshold-set",
                        "finding_id": finding_id,
                        "threshold": to,
                        "jointly_accepted": met,
                        "frontier": frontier.display().to_string(),
                    }))
                    .expect("failed to serialize sign.threshold-set")
                );
            } else {
                println!(
                    "{} signature_threshold={to} on {finding_id} ({})",
                    style::ok("set"),
                    if met {
                        "jointly accepted"
                    } else {
                        "awaiting signatures"
                    }
                );
            }
        }
    }
}

fn cmd_actor(action: ActorAction) {
    match action {
        ActorAction::Add {
            frontier,
            id,
            pubkey,
            tier,
            json,
        } => {
            // Validate the pubkey shape before mutating the frontier.
            let trimmed = pubkey.trim();
            if trimmed.len() != 64 || hex::decode(trimmed).is_err() {
                fail("Public key must be 64 hex characters (32-byte Ed25519 pubkey).");
            }
            let mut project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            if project.actors.iter().any(|actor| actor.id == id) {
                fail(&format!(
                    "Actor '{id}' already registered in this frontier."
                ));
            }
            project.actors.push(sign::ActorRecord {
                id: id.clone(),
                public_key: trimmed.to_string(),
                algorithm: "ed25519".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tier: tier.clone(),
            });
            repo::save_to_path(&frontier, &project).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "actor.add",
                "frontier": frontier.display().to_string(),
                "actor_id": id,
                "public_key": trimmed,
                "tier": tier,
                "registered_count": project.actors.len(),
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).expect("failed to serialize actor.add")
                );
            } else {
                let tier_suffix = tier
                    .as_deref()
                    .map_or_else(String::new, |t| format!(" tier={t}"));
                println!(
                    "{} actor {} (pubkey {}{tier_suffix})",
                    style::ok("registered"),
                    id,
                    &trimmed[..16]
                );
            }
        }
        ActorAction::List { frontier, json } => {
            let project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            if json {
                let payload = json!({
                    "ok": true,
                    "command": "actor.list",
                    "frontier": frontier.display().to_string(),
                    "actors": project.actors,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).expect("failed to serialize actor.list")
                );
            } else {
                println!();
                println!(
                    "  {}",
                    format!("VELA · ACTOR · LIST · {}", frontier.display())
                        .to_uppercase()
                        .dimmed()
                );
                println!("  {}", style::tick_row(60));
                if project.actors.is_empty() {
                    println!("  (no actors registered)");
                } else {
                    for actor in &project.actors {
                        println!(
                            "  {:<28} {}…  registered {}",
                            actor.id,
                            &actor.public_key[..16],
                            actor.created_at
                        );
                    }
                }
            }
        }
    }
}

/// v0.40: Causal-typing audit over a frontier.
fn cmd_causal(action: CausalAction) {
    use crate::causal_reasoning;

    match action {
        CausalAction::Audit {
            frontier,
            problems_only,
            json,
        } => {
            let project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let mut entries = causal_reasoning::audit_frontier(&project);
            if problems_only {
                entries.retain(|e| e.verdict.needs_reviewer_attention());
            }
            let summary = causal_reasoning::summarize_audit(&entries);

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "command": "causal.audit",
                        "frontier": frontier.display().to_string(),
                        "summary": summary,
                        "entries": entries,
                    }))
                    .expect("serialize causal.audit")
                );
                return;
            }

            println!();
            println!(
                "  {}",
                format!("VELA · CAUSAL · AUDIT · {}", frontier.display())
                    .to_uppercase()
                    .dimmed()
            );
            println!("  {}", style::tick_row(60));
            println!(
                "  total: {}  identified: {}  conditional: {}  underidentified: {}  underdetermined: {}",
                summary.total,
                summary.identified,
                summary.conditional,
                summary.underidentified,
                summary.underdetermined,
            );
            if entries.is_empty() {
                println!("  (no entries to report)");
                return;
            }
            for e in &entries {
                let chip = match e.verdict {
                    crate::causal_reasoning::Identifiability::Identified => style::ok("identified"),
                    crate::causal_reasoning::Identifiability::Conditional => style::warn("conditional"),
                    crate::causal_reasoning::Identifiability::Underidentified => {
                        style::lost("underidentified")
                    }
                    crate::causal_reasoning::Identifiability::Underdetermined => {
                        style::warn("underdetermined")
                    }
                };
                let claim = e.causal_claim.map_or("none".to_string(), |c| {
                    format!("{c:?}").to_lowercase()
                });
                let grade = e.causal_evidence_grade.map_or("none".to_string(), |g| {
                    format!("{g:?}").to_lowercase()
                });
                println!();
                println!("  {chip}  {}  ({}/{})", e.finding_id, claim, grade);
                let assertion_short: String = e.assertion_text.chars().take(78).collect();
                println!("    {assertion_short}");
                println!("    {} {}", style::ok("why:"), e.rationale);
                if e.verdict.needs_reviewer_attention()
                    || matches!(
                        e.verdict,
                        crate::causal_reasoning::Identifiability::Underdetermined
                    )
                {
                    println!("    {} {}", style::ok("fix:"), e.remediation);
                }
            }
        }
    }
}

/// v0.39: Manage the federation peer registry.
fn cmd_federation(action: FederationAction) {
    use crate::federation::PeerHub;

    match action {
        FederationAction::PeerAdd {
            frontier,
            id,
            url,
            pubkey,
            note,
            json,
        } => {
            let peer = PeerHub {
                id: id.clone(),
                url: url.clone(),
                public_key: pubkey.trim().to_string(),
                added_at: chrono::Utc::now().to_rfc3339(),
                note: note.clone(),
            };
            peer.validate().unwrap_or_else(|e| fail_return(&e));

            let mut project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            if project.peers.iter().any(|p| p.id == id) {
                fail(&format!("peer '{id}' already in registry"));
            }
            project.peers.push(peer.clone());
            repo::save_to_path(&frontier, &project).unwrap_or_else(|e| fail_return(&e));

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "command": "federation.peer-add",
                        "frontier": frontier.display().to_string(),
                        "peer": peer,
                        "registered_count": project.peers.len(),
                    }))
                    .expect("serialize federation.peer-add")
                );
            } else {
                println!(
                    "{} peer {} (pubkey {}…) at {}",
                    style::ok("registered"),
                    id,
                    &peer.public_key[..16],
                    peer.url
                );
            }
        }
        FederationAction::PeerList { frontier, json } => {
            let project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "command": "federation.peer-list",
                        "frontier": frontier.display().to_string(),
                        "peers": project.peers,
                    }))
                    .expect("serialize federation.peer-list")
                );
            } else {
                println!();
                println!(
                    "  {}",
                    format!("VELA · FEDERATION · PEERS · {}", frontier.display())
                        .to_uppercase()
                        .dimmed()
                );
                println!("  {}", style::tick_row(60));
                if project.peers.is_empty() {
                    println!("  (no peers registered)");
                } else {
                    for p in &project.peers {
                        let note_suffix = if p.note.is_empty() {
                            String::new()
                        } else {
                            format!("  · {}", p.note)
                        };
                        println!(
                            "  {:<24}  {}  {}…{note_suffix}",
                            p.id,
                            p.url,
                            &p.public_key[..16]
                        );
                    }
                }
            }
        }
        FederationAction::Sync {
            frontier,
            peer_id,
            url,
            dry_run,
            json,
        } => {
            use crate::federation;

            let mut project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let Some(peer) = project.peers.iter().find(|p| p.id == peer_id).cloned() else {
                fail(&format!(
                    "peer '{peer_id}' not in registry; run `vela federation peer add` first"
                ));
            };
            let frontier_id = project.frontier_id();
            let resolved_url = url.unwrap_or_else(|| {
                let base = peer.url.trim_end_matches('/');
                format!("{base}/manifest/{frontier_id}.json")
            });

            let peer_state = federation::fetch_peer_frontier(&resolved_url)
                .unwrap_or_else(|e| fail_return(&e));

            if dry_run {
                let conflicts = federation::diff_frontiers(&project, &peer_state);
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({
                            "ok": true,
                            "command": "federation.sync",
                            "dry_run": true,
                            "peer_id": peer_id,
                            "peer_url": resolved_url,
                            "conflicts": conflicts,
                        }))
                        .expect("serialize federation.sync (dry-run)")
                    );
                } else {
                    println!(
                        "{} dry-run vs {peer_id} ({}): {} conflict(s)",
                        style::ok("ok"),
                        resolved_url,
                        conflicts.len()
                    );
                    for c in &conflicts {
                        println!("  · {} {} {}", c.kind.as_str(), c.finding_id, c.detail);
                    }
                }
                return;
            }

            let report = federation::sync_with_peer(&mut project, &peer_id, &peer_state);
            repo::save_to_path(&frontier, &project).unwrap_or_else(|e| fail_return(&e));

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "command": "federation.sync",
                        "peer_id": peer_id,
                        "peer_url": resolved_url,
                        "report": report,
                    }))
                    .expect("serialize federation.sync")
                );
            } else {
                println!(
                    "{} synced with {} ({})",
                    style::ok("ok"),
                    peer_id,
                    resolved_url
                );
                println!(
                    "  our:    {}",
                    &report.our_snapshot_hash[..16.min(report.our_snapshot_hash.len())]
                );
                println!(
                    "  peer:   {}",
                    &report.peer_snapshot_hash[..16.min(report.peer_snapshot_hash.len())]
                );
                println!(
                    "  conflicts: {}  events appended: {}",
                    report.conflicts.len(),
                    report.events_appended
                );
                for c in &report.conflicts {
                    println!("  · {} {} {}", c.kind.as_str(), c.finding_id, c.detail);
                }
            }
        }
        FederationAction::PeerRemove { frontier, id, json } => {
            let mut project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let before = project.peers.len();
            project.peers.retain(|p| p.id != id);
            if project.peers.len() == before {
                fail(&format!("peer '{id}' not found in registry"));
            }
            repo::save_to_path(&frontier, &project).unwrap_or_else(|e| fail_return(&e));

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": true,
                        "command": "federation.peer-remove",
                        "frontier": frontier.display().to_string(),
                        "removed": id,
                        "remaining": project.peers.len(),
                    }))
                    .expect("serialize federation.peer-remove")
                );
            } else {
                println!(
                    "{} peer {} ({} remaining)",
                    style::ok("removed"),
                    id,
                    project.peers.len()
                );
            }
        }
    }
}

/// Phase R (v0.5): walk the local Workbench draft queue. The Workbench
/// browser writes unsigned drafts to a queue file; this CLI is the only
/// place where the actor's private key reads its drafts and signs them.
/// The browser never sees the key.
fn cmd_queue(action: QueueAction) {
    use crate::queue;
    match action {
        QueueAction::List { queue_file, json } => {
            let path = queue_file.unwrap_or_else(queue::default_queue_path);
            let q = queue::load(&path).unwrap_or_else(|e| fail_return(&e));
            if json {
                let payload = json!({
                    "ok": true,
                    "command": "queue.list",
                    "queue_file": path.display().to_string(),
                    "schema": q.schema,
                    "actions": q.actions,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).expect("failed to serialize queue.list")
                );
            } else {
                println!();
                println!(
                    "  {}",
                    format!("VELA · QUEUE · LIST · {}", path.display())
                        .to_uppercase()
                        .dimmed()
                );
                println!("  {}", style::tick_row(60));
                if q.actions.is_empty() {
                    println!("  (queue is empty)");
                } else {
                    for (idx, action) in q.actions.iter().enumerate() {
                        println!(
                            "  [{idx}] {} → {}  queued {}",
                            action.kind,
                            action.frontier.display(),
                            action.queued_at
                        );
                    }
                }
            }
        }
        QueueAction::Clear { queue_file, json } => {
            let path = queue_file.unwrap_or_else(queue::default_queue_path);
            let dropped = queue::clear(&path).unwrap_or_else(|e| fail_return(&e));
            if json {
                let payload = json!({
                    "ok": true,
                    "command": "queue.clear",
                    "queue_file": path.display().to_string(),
                    "dropped": dropped,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize queue.clear")
                );
            } else {
                println!("{} dropped {dropped} queued action(s)", style::ok("ok"));
            }
        }
        QueueAction::Sign {
            actor,
            key,
            queue_file,
            yes_to_all,
            json,
        } => {
            let path = queue_file.unwrap_or_else(queue::default_queue_path);
            let q = queue::load(&path).unwrap_or_else(|e| fail_return(&e));
            if q.actions.is_empty() {
                if json {
                    println!("{}", json!({"ok": true, "signed": 0, "remaining": 0}));
                } else {
                    println!("{} queue is empty", style::ok("ok"));
                }
                return;
            }
            let key_hex = std::fs::read_to_string(&key)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|e| fail_return(&format!("read key {}: {e}", key.display())));
            let signing_key = parse_signing_key(&key_hex);
            let mut signed_count = 0usize;
            let mut remaining = Vec::new();
            for action in q.actions.iter() {
                if !yes_to_all && !confirm_action(action) {
                    remaining.push(action.clone());
                    continue;
                }
                match sign_and_apply(&signing_key, &actor, action) {
                    Ok(report) => {
                        signed_count += 1;
                        if !json {
                            println!(
                                "{} {} on {}  →  {}",
                                style::ok("signed"),
                                action.kind,
                                action.frontier.display(),
                                report
                            );
                        }
                    }
                    Err(error) => {
                        // Keep failed actions in the queue so the user can retry.
                        remaining.push(action.clone());
                        if !json {
                            eprintln!(
                                "{} {} on {}: {error}",
                                style::warn("failed"),
                                action.kind,
                                action.frontier.display()
                            );
                        }
                    }
                }
            }
            queue::replace_actions(&path, remaining.clone()).unwrap_or_else(|e| fail_return(&e));
            if json {
                let payload = json!({
                    "ok": true,
                    "command": "queue.sign",
                    "signed": signed_count,
                    "remaining": remaining.len(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).expect("failed to serialize queue.sign")
                );
            } else {
                println!(
                    "{} signed {signed_count} action(s); {} remaining in queue",
                    style::ok("ok"),
                    remaining.len()
                );
            }
        }
    }
}

fn parse_signing_key(hex_str: &str) -> ed25519_dalek::SigningKey {
    let bytes = hex::decode(hex_str)
        .unwrap_or_else(|e| fail_return(&format!("invalid private-key hex: {e}")));
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .unwrap_or_else(|_| fail_return("private key must be 32 bytes"));
    ed25519_dalek::SigningKey::from_bytes(&key_bytes)
}

fn confirm_action(action: &crate::queue::QueuedAction) -> bool {
    use std::io::{self, BufRead, Write};
    let mut stdout = io::stdout().lock();
    let _ = writeln!(
        stdout,
        "  sign {} on {}? [y/N] ",
        action.kind,
        action.frontier.display()
    );
    let _ = stdout.flush();
    drop(stdout);
    let stdin = io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Sign and apply a queued action. Returns a short summary string on
/// success (the resulting `vpr_…` or `vev_…`). The action is signed
/// locally and applied via the same `proposals::*_at_path` functions the
/// CLI uses — no HTTP roundtrip required.
fn sign_and_apply(
    signing_key: &ed25519_dalek::SigningKey,
    actor: &str,
    action: &crate::queue::QueuedAction,
) -> Result<String, String> {
    use crate::events::StateTarget;
    use crate::proposals;
    let args = &action.args;
    match action.kind.as_str() {
        "propose_review" | "propose_note" | "propose_revise_confidence" | "propose_retract" => {
            let kind = match action.kind.as_str() {
                "propose_review" => "finding.review",
                "propose_note" => "finding.note",
                "propose_revise_confidence" => "finding.confidence_revise",
                "propose_retract" => "finding.retract",
                _ => unreachable!(),
            };
            let target_id = args
                .get("target_finding_id")
                .and_then(Value::as_str)
                .ok_or("target_finding_id missing")?;
            let reason = args
                .get("reason")
                .and_then(Value::as_str)
                .ok_or("reason missing")?;
            let payload = match action.kind.as_str() {
                "propose_review" => {
                    let status = args
                        .get("status")
                        .and_then(Value::as_str)
                        .ok_or("status missing")?;
                    json!({"status": status})
                }
                "propose_note" => {
                    let text = args
                        .get("text")
                        .and_then(Value::as_str)
                        .ok_or("text missing")?;
                    json!({"text": text})
                }
                "propose_revise_confidence" => {
                    let new_score = args
                        .get("new_score")
                        .and_then(Value::as_f64)
                        .ok_or("new_score missing")?;
                    json!({"new_score": new_score})
                }
                "propose_retract" => json!({}),
                _ => unreachable!(),
            };
            let created_at = args
                .get("created_at")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
            let mut proposal = proposals::new_proposal(
                kind,
                StateTarget {
                    r#type: "finding".to_string(),
                    id: target_id.to_string(),
                },
                actor,
                "human",
                reason,
                payload,
                Vec::new(),
                Vec::new(),
            );
            proposal.created_at = created_at;
            proposal.id = proposals::proposal_id(&proposal);
            // Sign the proposal locally to validate parity with what the
            // server-side write tool would have signed; the queue-sign
            // path applies via the local file, not via HTTP.
            let _signature = crate::sign::sign_proposal(&proposal, signing_key)?;
            let result = proposals::create_or_apply(&action.frontier, proposal, false)
                .map_err(|e| format!("create_or_apply: {e}"))?;
            Ok(format!("proposal {}", result.proposal_id))
        }
        "accept_proposal" | "reject_proposal" => {
            let proposal_id = args
                .get("proposal_id")
                .and_then(Value::as_str)
                .ok_or("proposal_id missing")?;
            let reason = args
                .get("reason")
                .and_then(Value::as_str)
                .ok_or("reason missing")?;
            let timestamp = args
                .get("timestamp")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
            // Sign for parity; `accept_at_path`/`reject_at_path` apply locally.
            let preimage = json!({
                "action": if action.kind == "accept_proposal" { "accept" } else { "reject" },
                "proposal_id": proposal_id,
                "reviewer_id": actor,
                "reason": reason,
                "timestamp": timestamp,
            });
            let bytes = crate::canonical::to_canonical_bytes(&preimage)?;
            use ed25519_dalek::Signer;
            let _signature = hex::encode(signing_key.sign(&bytes).to_bytes());
            if action.kind == "accept_proposal" {
                let event_id =
                    crate::proposals::accept_at_path(&action.frontier, proposal_id, actor, reason)
                        .map_err(|e| format!("accept_at_path: {e}"))?;
                Ok(format!("event {event_id}"))
            } else {
                crate::proposals::reject_at_path(&action.frontier, proposal_id, actor, reason)
                    .map_err(|e| format!("reject_at_path: {e}"))?;
                Ok(format!("rejected {proposal_id}"))
            }
        }
        other => Err(format!("unsupported queued action kind '{other}'")),
    }
}

/// v0.8: frontier-level metadata commands. Manages cross-frontier
/// dependency declarations on a frontier file. The substrate enforces
/// that any link target of the form `vf_…@vfr_…` references a declared
/// dependency; these commands edit the declaration list.
/// v0.9: typed link addition. Until v0.9 the only way to add a link
/// was to hand-edit JSON; this command is the CLI on-ramp. Links go
/// directly onto `findings[i].links` (links are not a state-changing
/// proposal kind in v0).
/// v0.19: bundled entity resolution. See `crate::entity_resolve` for the
/// table + algorithm. CLI surface is two subcommands: `resolve` (mutates
/// the frontier file) and `list` (read-only inspection of the table).
fn cmd_entity(action: EntityAction) {
    use crate::entity_resolve;
    match action {
        EntityAction::Resolve {
            frontier,
            force,
            json,
        } => {
            let mut p = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let report = entity_resolve::resolve_frontier(&mut p, force);
            repo::save_to_path(&frontier, &p).unwrap_or_else(|e| fail_return(&e));
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": true,
                        "command": "entity.resolve",
                        "frontier_path": frontier.display().to_string(),
                        "report": report,
                    }))
                    .expect("serialize")
                );
            } else {
                println!(
                    "{} resolved {} of {} entities ({} already, {} unresolved) across {} findings",
                    style::ok("entity"),
                    report.resolved,
                    report.total_entities,
                    report.already_resolved,
                    report.unresolved_count,
                    report.findings_touched,
                );
                let unresolved_summary: std::collections::BTreeSet<&str> = report
                    .per_finding
                    .iter()
                    .flat_map(|f| f.unresolved.iter().map(String::as_str))
                    .collect();
                if !unresolved_summary.is_empty() {
                    let take = unresolved_summary.iter().take(8).collect::<Vec<_>>();
                    println!(
                        "  unresolved (first {}): {}",
                        take.len(),
                        take.iter().copied().cloned().collect::<Vec<_>>().join(", ")
                    );
                }
            }
        }
        EntityAction::List { json } => {
            let entries: Vec<serde_json::Value> = entity_resolve::iter_bundled()
                .map(|(name, etype, source, id)| {
                    serde_json::json!({
                        "canonical_name": name,
                        "entity_type": etype,
                        "source": source,
                        "id": id,
                    })
                })
                .collect();
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": true,
                        "command": "entity.list",
                        "count": entries.len(),
                        "entries": entries,
                    }))
                    .expect("serialize")
                );
            } else {
                println!("{} {} bundled entries", style::ok("entity"), entries.len());
                for e in &entries {
                    println!(
                        "  {:32}  {:18}  {} {}",
                        e["canonical_name"].as_str().unwrap_or("?"),
                        e["entity_type"].as_str().unwrap_or("?"),
                        e["source"].as_str().unwrap_or("?"),
                        e["id"].as_str().unwrap_or("?"),
                    );
                }
            }
        }
    }
}

fn cmd_link(action: LinkAction) {
    use crate::bundle::{Link, LinkRef};
    match action {
        LinkAction::Add {
            frontier,
            from,
            to,
            r#type,
            note,
            inferred_by,
            no_check_target,
            json,
        } => {
            validate_enum_arg("--type", &r#type, bundle::VALID_LINK_TYPES);
            if !["compiler", "reviewer", "author"].contains(&inferred_by.as_str()) {
                fail(&format!(
                    "invalid --inferred-by '{inferred_by}'. Valid: compiler, reviewer, author"
                ));
            }
            let parsed = LinkRef::parse(&to).unwrap_or_else(|e| {
                fail(&format!(
                    "invalid --to '{to}': {e}. Expected vf_<hex> or vf_<hex>@vfr_<hex>"
                ))
            });
            let mut p = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let source_idx = p
                .findings
                .iter()
                .position(|f| f.id == from)
                .unwrap_or_else(|| {
                    fail_return(&format!("--from finding '{from}' not in frontier"))
                });
            if let LinkRef::Local { vf_id } = &parsed
                && !p.findings.iter().any(|f| &f.id == vf_id)
            {
                fail(&format!(
                    "local --to target '{vf_id}' not in frontier; add the target finding first"
                ));
            }
            if let LinkRef::Cross { vfr_id, .. } = &parsed
                && p.dep_for_vfr(vfr_id).is_none()
            {
                fail(&format!(
                    "cross-frontier --to references vfr_id '{vfr_id}' but no matching dep is declared. Run `vela frontier add-dep {vfr_id} --locator <url> --snapshot <hash>` first."
                ));
            }

            // v0.16: best-effort cross-frontier target-status check. The
            // substrate doctrine is "client verifies on read", but at
            // link-add time it's worth a one-shot fetch to warn the user
            // if their target has been superseded. Failure to fetch is
            // a hint, not a hard error — the link still records.
            let mut target_warning: Option<String> = None;
            if let LinkRef::Cross {
                vfr_id: target_vfr,
                vf_id: target_vf,
            } = &parsed
                && !no_check_target
                && let Some(dep) = p.dep_for_vfr(target_vfr)
                && let Some(locator) = dep.locator.as_deref()
                && (locator.starts_with("http://") || locator.starts_with("https://"))
            {
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .build()
                    .ok();
                if let Some(client) = client
                    && let Ok(resp) = client.get(locator).send()
                    && resp.status().is_success()
                    && let Ok(dep_project) = resp.json::<crate::project::Project>()
                {
                    if let Some(target_finding) =
                        dep_project.findings.iter().find(|f| &f.id == target_vf)
                    {
                        if target_finding.flags.superseded {
                            target_warning = Some(format!(
                                "warn · cross-frontier target '{target_vf}' in '{target_vfr}' has flags.superseded = true. \
You may be linking to outdated wording. Pull --transitive and inspect the supersedes chain to find the current finding. \
Use --no-check-target to skip this check."
                            ));
                        }
                    } else {
                        target_warning = Some(format!(
                            "warn · cross-frontier target '{target_vf}' not found in dep '{target_vfr}' (fetched from {locator}). \
The target may have been removed or never existed in the pinned snapshot."
                        ));
                    }
                }
            }

            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let link = Link {
                target: to.clone(),
                link_type: r#type.clone(),
                note: note.clone(),
                inferred_by: inferred_by.clone(),
                created_at: now,
            };
            p.findings[source_idx].links.push(link);
            project::recompute_stats(&mut p);
            repo::save_to_path(&frontier, &p).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "link.add",
                "frontier": frontier.display().to_string(),
                "from": from,
                "to": to,
                "type": r#type,
                "cross_frontier": parsed.is_cross_frontier(),
            });
            if json {
                let mut p2 = payload.clone();
                if let Some(w) = &target_warning
                    && let serde_json::Value::Object(m) = &mut p2
                {
                    m.insert(
                        "target_warning".to_string(),
                        serde_json::Value::String(w.clone()),
                    );
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&p2).expect("failed to serialize link.add")
                );
            } else {
                println!(
                    "{} {} --[{}]--> {}{}",
                    style::ok("link"),
                    from,
                    r#type,
                    to,
                    if parsed.is_cross_frontier() {
                        " (cross-frontier)"
                    } else {
                        ""
                    }
                );
                if let Some(w) = target_warning {
                    println!("  {w}");
                }
            }
        }
    }
}

fn cmd_frontier(action: FrontierAction) {
    use crate::project::ProjectDependency;
    use crate::repo;
    match action {
        FrontierAction::New {
            path,
            name,
            description,
            force,
            json,
        } => {
            if path.exists() && !force {
                fail(&format!(
                    "{} already exists; pass --force to overwrite",
                    path.display()
                ));
            }
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let project = project::Project {
                vela_version: project::VELA_SCHEMA_VERSION.to_string(),
                schema: project::VELA_SCHEMA_URL.to_string(),
                frontier_id: None,
                project: project::ProjectMeta {
                    name: name.clone(),
                    description: description.clone(),
                    compiled_at: now,
                    compiler: project::VELA_COMPILER_VERSION.to_string(),
                    papers_processed: 0,
                    errors: 0,
                    dependencies: Vec::new(),
                },
                stats: project::ProjectStats::default(),
                findings: Vec::new(),
                sources: Vec::new(),
                evidence_atoms: Vec::new(),
                condition_records: Vec::new(),
                review_events: Vec::new(),
                confidence_updates: Vec::new(),
                events: Vec::new(),
                proposals: Vec::new(),
                proof_state: proposals::ProofState::default(),
                signatures: Vec::new(),
                actors: Vec::new(),
                replications: Vec::new(),
                datasets: Vec::new(),
                code_artifacts: Vec::new(),
                predictions: Vec::new(),
                resolutions: Vec::new(),
            peers: Vec::new(),
            };
            repo::save_to_path(&path, &project).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "frontier.new",
                "path": path.display().to_string(),
                "name": name,
                "schema": project::VELA_SCHEMA_URL,
                "vela_version": env!("CARGO_PKG_VERSION"),
                "next_steps": [
                    "vela finding add <path> --assertion '...' --author 'reviewer:you' --apply",
                    "vela sign generate-keypair --out keys",
                    "vela actor add <path> reviewer:you --pubkey \"$(cat keys/public.key)\"",
                    "vela registry publish <path> --owner reviewer:you --key keys/private.key --locator <url> --to https://vela-hub.fly.dev",
                ],
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize frontier.new")
                );
            } else {
                println!(
                    "{} scaffolded frontier '{name}' at {}",
                    style::ok("frontier"),
                    path.display()
                );
                println!("  next steps:");
                println!(
                    "    1. vela finding add {} --assertion '...' --author 'reviewer:you' --apply",
                    path.display()
                );
                println!("    2. vela sign generate-keypair --out keys");
                println!(
                    "    3. vela actor add {} reviewer:you --pubkey \"$(cat keys/public.key)\"",
                    path.display()
                );
                println!(
                    "    4. vela registry publish {} --owner reviewer:you --key keys/private.key --locator <url> --to https://vela-hub.fly.dev",
                    path.display()
                );
            }
        }
        FrontierAction::AddDep {
            frontier,
            vfr_id,
            locator,
            snapshot,
            name,
            json,
        } => {
            let mut p = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            if p.project
                .dependencies
                .iter()
                .any(|d| d.vfr_id.as_deref() == Some(&vfr_id))
            {
                fail(&format!(
                    "cross-frontier dependency '{vfr_id}' already declared; remove it first via `vela frontier remove-dep`"
                ));
            }
            let dep = ProjectDependency {
                name: name.unwrap_or_else(|| vfr_id.clone()),
                source: "vela.hub".into(),
                version: None,
                pinned_hash: None,
                vfr_id: Some(vfr_id.clone()),
                locator: Some(locator.clone()),
                pinned_snapshot_hash: Some(snapshot.clone()),
            };
            p.project.dependencies.push(dep);
            repo::save_to_path(&frontier, &p).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "frontier.add-dep",
                "frontier": frontier.display().to_string(),
                "vfr_id": vfr_id,
                "locator": locator,
                "pinned_snapshot_hash": snapshot,
                "declared_count": p.project.dependencies.len(),
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize frontier.add-dep")
                );
            } else {
                println!(
                    "{} declared cross-frontier dep {vfr_id}",
                    style::ok("frontier")
                );
                println!("  locator:  {locator}");
                println!("  snapshot: {snapshot}");
            }
        }
        FrontierAction::ListDeps { frontier, json } => {
            let p = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let deps: Vec<&ProjectDependency> = p.project.dependencies.iter().collect();
            if json {
                let payload = json!({
                    "ok": true,
                    "command": "frontier.list-deps",
                    "frontier": frontier.display().to_string(),
                    "count": deps.len(),
                    "dependencies": deps,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize frontier.list-deps")
                );
            } else {
                println!();
                println!(
                    "  {}",
                    format!("VELA · FRONTIER · LIST-DEPS · {}", frontier.display())
                        .to_uppercase()
                        .dimmed()
                );
                println!("  {}", style::tick_row(60));
                if deps.is_empty() {
                    println!("  (no dependencies declared)");
                } else {
                    for d in &deps {
                        let kind = if d.is_cross_frontier() {
                            "cross-frontier"
                        } else {
                            "compile-time"
                        };
                        println!("  · {} [{kind}]", d.name);
                        if let Some(v) = &d.vfr_id {
                            println!("    vfr_id:   {v}");
                        }
                        if let Some(l) = &d.locator {
                            println!("    locator:  {l}");
                        }
                        if let Some(s) = &d.pinned_snapshot_hash {
                            println!("    snapshot: {s}");
                        }
                    }
                }
            }
        }
        FrontierAction::RemoveDep {
            frontier,
            vfr_id,
            json,
        } => {
            let mut p = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            // Refuse if any link still references this vfr_id.
            for f in &p.findings {
                for l in &f.links {
                    if let Ok(crate::bundle::LinkRef::Cross { vfr_id: ref v, .. }) =
                        crate::bundle::LinkRef::parse(&l.target)
                        && v == &vfr_id
                    {
                        fail(&format!(
                            "cannot remove dep '{vfr_id}': finding {} still links to it via {}",
                            f.id, l.target
                        ));
                    }
                }
            }
            let before = p.project.dependencies.len();
            p.project
                .dependencies
                .retain(|d| d.vfr_id.as_deref() != Some(&vfr_id));
            let removed = before - p.project.dependencies.len();
            if removed == 0 {
                fail(&format!("no cross-frontier dependency '{vfr_id}' found"));
            }
            repo::save_to_path(&frontier, &p).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "frontier.remove-dep",
                "frontier": frontier.display().to_string(),
                "vfr_id": vfr_id,
                "removed": removed,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize frontier.remove-dep")
                );
            } else {
                println!(
                    "{} removed cross-frontier dep {vfr_id}",
                    style::ok("frontier")
                );
            }
        }
        FrontierAction::RefreshDeps {
            frontier,
            from,
            dry_run,
            json,
        } => {
            let mut p = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let cross_deps: Vec<String> = p
                .project
                .dependencies
                .iter()
                .filter_map(|d| d.vfr_id.clone())
                .collect();
            if cross_deps.is_empty() {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({
                            "ok": true,
                            "command": "frontier.refresh-deps",
                            "frontier": frontier.display().to_string(),
                            "from": from,
                            "dry_run": dry_run,
                            "deps": [],
                            "summary": { "total": 0, "refreshed": 0, "unchanged": 0, "missing": 0, "unreachable": 0 },
                        })).expect("serialize")
                    );
                } else {
                    println!(
                        "{} no cross-frontier deps declared in {}",
                        style::ok("frontier"),
                        frontier.display()
                    );
                }
                return;
            }
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .unwrap_or_else(|e| fail_return(&format!("http client init failed: {e}")));
            let base = from.trim_end_matches('/');
            #[derive(serde::Deserialize)]
            struct HubEntry {
                latest_snapshot_hash: String,
            }
            let mut per_dep: Vec<serde_json::Value> = Vec::new();
            let (mut refreshed, mut unchanged, mut missing, mut unreachable) =
                (0u32, 0u32, 0u32, 0u32);
            for vfr in &cross_deps {
                let url = format!("{base}/entries/{vfr}");
                let resp = client.get(&url).send();
                let outcome = match resp {
                    Ok(r) if r.status().as_u16() == 404 => {
                        missing += 1;
                        json!({ "vfr_id": vfr, "status": "missing", "url": url })
                    }
                    Ok(r) if !r.status().is_success() => {
                        unreachable += 1;
                        json!({ "vfr_id": vfr, "status": "unreachable", "http_status": r.status().as_u16() })
                    }
                    Err(e) => {
                        unreachable += 1;
                        json!({ "vfr_id": vfr, "status": "unreachable", "error": e.to_string() })
                    }
                    Ok(r) => match r.json::<HubEntry>() {
                        Err(e) => {
                            unreachable += 1;
                            json!({ "vfr_id": vfr, "status": "unreachable", "error": format!("invalid hub response: {e}") })
                        }
                        Ok(entry) => {
                            // Locate the dep in our project to compare + (maybe) mutate.
                            match p
                                .project
                                .dependencies
                                .iter()
                                .position(|d| d.vfr_id.as_deref() == Some(vfr.as_str()))
                            {
                                None => {
                                    unreachable += 1;
                                    json!({ "vfr_id": vfr, "status": "unreachable", "error": "dep disappeared mid-scan" })
                                }
                                Some(idx) => {
                                    let local_pin =
                                        p.project.dependencies[idx].pinned_snapshot_hash.clone();
                                    let new_pin = entry.latest_snapshot_hash;
                                    if local_pin.as_deref() == Some(new_pin.as_str()) {
                                        unchanged += 1;
                                        json!({ "vfr_id": vfr, "status": "unchanged", "snapshot": new_pin })
                                    } else {
                                        if !dry_run {
                                            p.project.dependencies[idx].pinned_snapshot_hash =
                                                Some(new_pin.clone());
                                        }
                                        refreshed += 1;
                                        json!({
                                            "vfr_id": vfr,
                                            "status": "refreshed",
                                            "old_snapshot": local_pin,
                                            "new_snapshot": new_pin,
                                        })
                                    }
                                }
                            }
                        }
                    },
                };
                per_dep.push(outcome);
            }
            if !dry_run && refreshed > 0 {
                repo::save_to_path(&frontier, &p).unwrap_or_else(|e| fail_return(&e));
            }
            let payload = json!({
                "ok": true,
                "command": "frontier.refresh-deps",
                "frontier": frontier.display().to_string(),
                "from": from,
                "dry_run": dry_run,
                "deps": per_dep,
                "summary": {
                    "total": cross_deps.len(),
                    "refreshed": refreshed,
                    "unchanged": unchanged,
                    "missing": missing,
                    "unreachable": unreachable,
                },
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize frontier.refresh-deps")
                );
            } else {
                let mode = if dry_run { " (dry-run)" } else { "" };
                println!(
                    "{} refresh-deps{mode} · {} total · {refreshed} refreshed · {unchanged} unchanged · {missing} missing · {unreachable} unreachable",
                    style::ok("frontier"),
                    cross_deps.len()
                );
                for d in &per_dep {
                    let vfr = d["vfr_id"].as_str().unwrap_or("?");
                    let status = d["status"].as_str().unwrap_or("?");
                    match status {
                        "refreshed" => println!(
                            "  {vfr}  refreshed  {} → {}",
                            d["old_snapshot"]
                                .as_str()
                                .unwrap_or("(none)")
                                .chars()
                                .take(16)
                                .collect::<String>(),
                            d["new_snapshot"]
                                .as_str()
                                .unwrap_or("?")
                                .chars()
                                .take(16)
                                .collect::<String>(),
                        ),
                        "unchanged" => println!("  {vfr}  unchanged"),
                        "missing" => println!("  {vfr}  missing on hub"),
                        _ => println!("  {vfr}  unreachable"),
                    }
                }
            }
        }
        FrontierAction::Diff {
            frontier,
            since,
            week,
            json,
        } => cmd_frontier_diff(&frontier, since.as_deref(), week.as_deref(), json),
    }
}

/// v0.32: structured diff of findings added/updated/contradicted in a
/// time window. Read-only over canonical state; does not modify the
/// frontier and does not need a signing key.
///
/// Window resolution priority: `--since` > `--week` > current ISO week.
/// If `--since` is given, the upper bound is "now" (UTC); the diff
/// covers `[since, now)`. If `--week` is given (or defaulted), the
/// window is `[Mon 00:00 UTC, next Mon 00:00 UTC)`.
fn cmd_frontier_diff(
    frontier: &Path,
    since: Option<&str>,
    week: Option<&str>,
    json: bool,
) {
    let project = repo::load_from_path(frontier).unwrap_or_else(|e| fail_return(&e));

    // ── Resolve the window ──
    let now = chrono::Utc::now();
    let (window_start, window_end, week_label): (
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
        Option<String>,
    ) = if let Some(s) = since {
        let parsed = chrono::DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(|e| fail_return(&format!("invalid --since timestamp '{s}': {e}")));
        (parsed, now, None)
    } else {
        let key = week
            .map(str::to_owned)
            .unwrap_or_else(|| iso_week_key_for(now.date_naive()));
        let (start, end) = iso_week_bounds(&key)
            .unwrap_or_else(|e| fail_return(&format!("invalid --week '{key}': {e}")));
        (start, end, Some(key))
    };

    // ── Bucket findings ──
    let mut added: Vec<&crate::bundle::FindingBundle> = Vec::new();
    let mut updated: Vec<&crate::bundle::FindingBundle> = Vec::new();
    let mut new_contradictions: Vec<&crate::bundle::FindingBundle> = Vec::new();
    let mut cumulative: usize = 0;

    for f in &project.findings {
        let created = chrono::DateTime::parse_from_rfc3339(&f.created)
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok();
        let updated_ts = f
            .updated
            .as_deref()
            .and_then(|u| chrono::DateTime::parse_from_rfc3339(u).ok())
            .map(|d| d.with_timezone(&chrono::Utc));

        if let Some(c) = created
            && c < window_end
        {
            cumulative += 1;
        }

        if let Some(c) = created
            && c >= window_start
            && c < window_end
        {
            added.push(f);
            let is_tension = f.flags.contested || f.assertion.assertion_type == "tension";
            if is_tension {
                new_contradictions.push(f);
            }
            continue;
        }
        if let Some(u) = updated_ts
            && u >= window_start
            && u < window_end
        {
            updated.push(f);
        }
    }

    // ── Render ──
    let summary_for = |list: &[&crate::bundle::FindingBundle]| -> Vec<serde_json::Value> {
        list.iter()
            .map(|f| {
                json!({
                    "id": f.id,
                    "assertion": f.assertion.text,
                    "evidence_type": f.evidence.evidence_type,
                    "confidence": f.confidence.score,
                    "doi": f.provenance.doi,
                    "pmid": f.provenance.pmid,
                })
            })
            .collect()
    };

    let payload = json!({
        "ok": true,
        "command": "frontier.diff",
        "frontier": frontier.display().to_string(),
        "frontier_id": project.frontier_id,
        "window": {
            "start": window_start.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "end": window_end.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "iso_week": week_label,
        },
        "totals": {
            "added": added.len(),
            "updated": updated.len(),
            "new_contradictions": new_contradictions.len(),
            "cumulative_claims": cumulative,
        },
        "added": summary_for(&added),
        "updated": summary_for(&updated),
        "new_contradictions": summary_for(&new_contradictions),
    });

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .expect("failed to serialize frontier.diff")
        );
        return;
    }

    let label = week_label
        .clone()
        .unwrap_or_else(|| format!("since {}", window_start.format("%Y-%m-%d %H:%M UTC")));
    println!();
    println!(
        "  {}",
        format!("VELA · FRONTIER · DIFF · {label}").to_uppercase().dimmed()
    );
    println!("  {}", style::tick_row(60));
    println!(
        "  range:           {} → {}",
        window_start.format("%Y-%m-%d %H:%M"),
        window_end.format("%Y-%m-%d %H:%M")
    );
    println!("  added:           {}", added.len());
    println!("  updated:         {}", updated.len());
    println!("  contradictions:  {}", new_contradictions.len());
    println!("  cumulative:      {cumulative}");
    if added.is_empty() && updated.is_empty() {
        println!();
        println!("  (quiet window — no findings added or updated)");
    } else {
        println!();
        println!("  added:");
        for f in &added {
            println!(
                "    · {}  {}",
                f.id.dimmed(),
                truncate(&f.assertion.text, 88)
            );
        }
        if !updated.is_empty() {
            println!();
            println!("  updated:");
            for f in &updated {
                println!(
                    "    · {}  {}",
                    f.id.dimmed(),
                    truncate(&f.assertion.text, 88)
                );
            }
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// ISO 8601 week key in `YYYY-Www` form for a given calendar date.
fn iso_week_key_for(d: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    let iso = d.iso_week();
    format!("{:04}-W{:02}", iso.year(), iso.week())
}

/// Resolve `YYYY-Www` to its UTC bounds:
/// `[Monday 00:00 UTC, next Monday 00:00 UTC)`.
fn iso_week_bounds(
    key: &str,
) -> Result<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>), String> {
    let (year_str, week_str) = key
        .split_once("-W")
        .ok_or_else(|| format!("expected YYYY-Www, got '{key}'"))?;
    let year: i32 = year_str
        .parse()
        .map_err(|e| format!("bad year in '{key}': {e}"))?;
    let week: u32 = week_str
        .parse()
        .map_err(|e| format!("bad week in '{key}': {e}"))?;
    let monday = chrono::NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Mon)
        .ok_or_else(|| format!("invalid ISO week: {key}"))?;
    let next_monday = monday + chrono::Duration::days(7);
    let start = monday
        .and_hms_opt(0, 0, 0)
        .expect("00:00 valid")
        .and_utc();
    let end = next_monday
        .and_hms_opt(0, 0, 0)
        .expect("00:00 valid")
        .and_utc();
    Ok((start, end))
}

/// Phase S (v0.5): registry CLI — publish/pull a frontier through a
/// signed manifest. Verifiable distribution: any third party can pull
/// and confirm the snapshot and event-log hashes match what the owner
/// signed.
fn cmd_registry(action: RegistryAction) {
    use crate::registry;
    let default_registry = || -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".vela")
            .join("registry")
            .join("entries.json")
    };
    match action {
        RegistryAction::DependsOn { vfr_id, from, json } => {
            let base = from.trim_end_matches('/');
            let url = format!("{base}/entries/{vfr_id}/depends-on");
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|e| fail_return(&format!("http client init: {e}")));
            let resp = client
                .get(&url)
                .send()
                .unwrap_or_else(|e| fail_return(&format!("GET {url}: {e}")));
            if !resp.status().is_success() {
                fail(&format!("GET {url}: HTTP {}", resp.status()));
            }
            let body: serde_json::Value = resp
                .json()
                .unwrap_or_else(|e| fail_return(&format!("parse response: {e}")));
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&body).expect("serialize")
                );
            } else {
                let dependents = body
                    .get("dependents")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let count = dependents.len();
                println!(
                    "{} {count} {} on {vfr_id}",
                    style::ok("registry"),
                    if count == 1 {
                        "frontier depends"
                    } else {
                        "frontiers depend"
                    },
                );
                for e in &dependents {
                    let v = e.get("vfr_id").and_then(|v| v.as_str()).unwrap_or("?");
                    let n = e.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                    let o = e
                        .get("owner_actor_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    println!("  {v}  {n}  ({o})");
                }
            }
        }
        RegistryAction::Mirror {
            vfr_id,
            from,
            to,
            json,
        } => {
            let src_base = from.trim_end_matches('/');
            let dst_base = to.trim_end_matches('/');
            let src_url = format!("{src_base}/entries/{vfr_id}");
            let dst_url = format!("{dst_base}/entries");
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|e| fail_return(&format!("http client init: {e}")));

            let entry: serde_json::Value = client
                .get(&src_url)
                .send()
                .unwrap_or_else(|e| fail_return(&format!("GET {src_url}: {e}")))
                .error_for_status()
                .unwrap_or_else(|e| fail_return(&format!("GET {src_url}: {e}")))
                .json()
                .unwrap_or_else(|e| fail_return(&format!("parse {src_url}: {e}")));

            let resp = client
                .post(&dst_url)
                .header("content-type", "application/json")
                .body(
                    serde_json::to_vec(&entry)
                        .unwrap_or_else(|e| fail_return(&format!("serialize: {e}"))),
                )
                .send()
                .unwrap_or_else(|e| fail_return(&format!("POST {dst_url}: {e}")));
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().unwrap_or_default();
                fail(&format!(
                    "POST {dst_url}: HTTP {status}: {}",
                    body.chars().take(300).collect::<String>()
                ));
            }
            let body: serde_json::Value = resp
                .json()
                .unwrap_or_else(|e| fail_return(&format!("parse POST response: {e}")));
            let duplicate = body
                .get("duplicate")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let payload = json!({
                "ok": true,
                "command": "registry.mirror",
                "vfr_id": vfr_id,
                "from": src_base,
                "to": dst_base,
                "duplicate_on_destination": duplicate,
                "destination_response": body,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).expect("serialize")
                );
            } else {
                println!(
                    "{} mirrored {vfr_id} from {src_base} → {dst_base}{}",
                    style::ok("registry"),
                    if duplicate {
                        " (duplicate; signature already known)"
                    } else {
                        " (fresh insert)"
                    }
                );
            }
        }
        RegistryAction::List { from, json } => {
            // Phase γ-hub (v0.7): `--from <https://...>` fetches the
            // registry over HTTPS; bare paths and file:// resolve locally.
            let (label, registry_data) = match &from {
                Some(loc) if loc.starts_with("http") => (
                    loc.clone(),
                    registry::load_any(loc).unwrap_or_else(|e| fail_return(&e)),
                ),
                Some(loc) => {
                    let p = registry::resolve_local(loc).unwrap_or_else(|e| fail_return(&e));
                    (
                        p.display().to_string(),
                        registry::load_local(&p).unwrap_or_else(|e| fail_return(&e)),
                    )
                }
                None => {
                    let p = default_registry();
                    (
                        p.display().to_string(),
                        registry::load_local(&p).unwrap_or_else(|e| fail_return(&e)),
                    )
                }
            };
            let r = registry_data;
            let path_label = label;
            if json {
                let payload = json!({
                    "ok": true,
                    "command": "registry.list",
                    "registry": path_label,
                    "entry_count": r.entries.len(),
                    "entries": r.entries,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize registry.list")
                );
            } else {
                println!();
                println!(
                    "  {}",
                    format!("VELA · REGISTRY · LIST · {}", path_label)
                        .to_uppercase()
                        .dimmed()
                );
                println!("  {}", style::tick_row(60));
                if r.entries.is_empty() {
                    println!("  (registry is empty)");
                } else {
                    for entry in &r.entries {
                        println!(
                            "  {} {} ({})  by {}  published {}",
                            entry.vfr_id,
                            entry.name,
                            entry.network_locator,
                            entry.owner_actor_id,
                            entry.signed_publish_at
                        );
                    }
                }
            }
        }
        RegistryAction::Publish {
            frontier,
            owner,
            key,
            locator,
            to,
            json,
        } => {
            // Load frontier and compute its current snapshot+event_log hashes.
            let frontier_data = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let snapshot_hash = events::snapshot_hash(&frontier_data);
            let event_log_hash = events::event_log_hash(&frontier_data.events);
            let vfr_id = frontier_data.frontier_id();
            let name = frontier_data.project.name.clone();

            // Look up the owner's pubkey from the frontier's actor registry.
            let pubkey = frontier_data
                .actors
                .iter()
                .find(|actor| actor.id == owner)
                .map(|actor| actor.public_key.clone())
                .unwrap_or_else(|| {
                    fail_return(&format!(
                        "owner '{owner}' is not registered in the frontier; run `vela actor add` first"
                    ))
                });

            // Read and parse the private key.
            let key_hex = std::fs::read_to_string(&key)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|e| fail_return(&format!("read key {}: {e}", key.display())));
            let signing_key = parse_signing_key(&key_hex);

            // Sanity check: pubkey on disk matches pubkey in the registry.
            let derived = hex::encode(signing_key.verifying_key().to_bytes());
            if derived != pubkey {
                fail(&format!(
                    "private key does not match registered pubkey for owner '{owner}'"
                ));
            }

            let mut entry = registry::RegistryEntry {
                schema: registry::ENTRY_SCHEMA.to_string(),
                vfr_id: vfr_id.clone(),
                name: name.clone(),
                owner_actor_id: owner.clone(),
                owner_pubkey: pubkey,
                latest_snapshot_hash: snapshot_hash,
                latest_event_log_hash: event_log_hash,
                network_locator: locator,
                signed_publish_at: chrono::Utc::now().to_rfc3339(),
                signature: String::new(),
            };
            entry.signature =
                registry::sign_entry(&entry, &signing_key).unwrap_or_else(|e| fail_return(&e));

            // Phase A2 (v0.7): when `--to` is an HTTPS URL we POST the
            // signed entry to a hub; otherwise we resolve a local file
            // and append. The signing path above is shared.
            let to_is_remote = matches!(
                to.as_deref(),
                Some(loc) if loc.starts_with("http://") || loc.starts_with("https://")
            );
            let (registry_label, duplicate) = if to_is_remote {
                let hub_url = to.clone().unwrap();
                let resp =
                    registry::publish_remote(&entry, &hub_url).unwrap_or_else(|e| fail_return(&e));
                (hub_url, resp.duplicate)
            } else {
                let registry_path = match &to {
                    Some(loc) => registry::resolve_local(loc).unwrap_or_else(|e| fail_return(&e)),
                    None => default_registry(),
                };
                registry::publish_entry(&registry_path, entry.clone())
                    .unwrap_or_else(|e| fail_return(&e));
                (registry_path.display().to_string(), false)
            };

            let payload = json!({
                "ok": true,
                "command": "registry.publish",
                "registry": registry_label,
                "vfr_id": vfr_id,
                "name": name,
                "owner": owner,
                "snapshot_hash": entry.latest_snapshot_hash,
                "event_log_hash": entry.latest_event_log_hash,
                "signed_publish_at": entry.signed_publish_at,
                "signature": entry.signature,
                "duplicate": duplicate,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize registry.publish")
                );
            } else {
                let dup_suffix = if duplicate { " (duplicate, no-op)" } else { "" };
                println!(
                    "{} published {vfr_id} → {}{}",
                    style::ok("registry"),
                    registry_label,
                    dup_suffix
                );
                println!("  snapshot:  {}", entry.latest_snapshot_hash);
                println!("  event_log: {}", entry.latest_event_log_hash);
                println!("  signature: {}…", &entry.signature[..16]);
            }
        }
        RegistryAction::Pull {
            vfr_id,
            from,
            out,
            transitive,
            depth,
            json,
        } => {
            // Phase γ-hub (v0.7): both the registry and the frontier
            // can live behind https:// now. Local file:// and bare-path
            // remain supported.
            let (registry_label, registry_data) = match &from {
                Some(loc) if loc.starts_with("http") => (
                    loc.clone(),
                    registry::load_any(loc).unwrap_or_else(|e| fail_return(&e)),
                ),
                Some(loc) => {
                    let p = registry::resolve_local(loc).unwrap_or_else(|e| fail_return(&e));
                    (
                        p.display().to_string(),
                        registry::load_local(&p).unwrap_or_else(|e| fail_return(&e)),
                    )
                }
                None => {
                    let p = default_registry();
                    (
                        p.display().to_string(),
                        registry::load_local(&p).unwrap_or_else(|e| fail_return(&e)),
                    )
                }
            };
            let entry = registry::find_latest(&registry_data, &vfr_id)
                .unwrap_or_else(|| fail_return(&format!("{vfr_id} not found in registry")));

            if transitive {
                // v0.8: --transitive walks the dep graph. `out` is
                // interpreted as a directory; the primary lands at
                // out/<vfr>.json, deps at out/<dep_vfr>.json.
                let result = registry::pull_transitive(&registry_data, &vfr_id, &out, depth)
                    .unwrap_or_else(|e| fail_return(&format!("transitive pull failed: {e}")));

                let dep_paths_json: serde_json::Value = serde_json::Value::Object(
                    result
                        .deps
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::json!(v.display().to_string())))
                        .collect(),
                );
                let payload = json!({
                    "ok": true,
                    "command": "registry.pull",
                    "registry": registry_label,
                    "vfr_id": vfr_id,
                    "transitive": true,
                    "depth": depth,
                    "out_dir": out.display().to_string(),
                    "primary": result.primary_path.display().to_string(),
                    "verified": result.verified,
                    "deps": dep_paths_json,
                });
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&payload)
                            .expect("failed to serialize registry.pull")
                    );
                } else {
                    println!(
                        "{} pulled {vfr_id} (transitive) → {}",
                        style::ok("registry"),
                        out.display()
                    );
                    println!("  verified {} frontier(s):", result.verified.len());
                    for v in &result.verified {
                        println!("    · {v}");
                    }
                    println!("  every cross-frontier dependency's pinned snapshot hash matched");
                }
                return;
            }

            // Fetch the frontier from its locator (file:// or https://)
            // and verify hashes + signature.
            registry::fetch_frontier_to(&entry.network_locator, &out)
                .unwrap_or_else(|e| fail_return(&format!("fetch frontier: {e}")));
            registry::verify_pull(&entry, &out).unwrap_or_else(|e| {
                let _ = std::fs::remove_file(&out);
                fail_return(&format!("pull verification failed: {e}"))
            });

            let payload = json!({
                "ok": true,
                "command": "registry.pull",
                "registry": registry_label,
                "vfr_id": vfr_id,
                "out": out.display().to_string(),
                "snapshot_hash": entry.latest_snapshot_hash,
                "event_log_hash": entry.latest_event_log_hash,
                "verified": true,
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload)
                        .expect("failed to serialize registry.pull")
                );
            } else {
                println!(
                    "{} pulled {vfr_id} → {}",
                    style::ok("registry"),
                    out.display()
                );
                println!("  verified snapshot+event_log hashes match registry; signature ok");
            }
        }
    }
}

fn print_stats_json(path: &Path) {
    let frontier = repo::load_from_path(path).expect("Failed to load frontier");
    let source_hash = hash_path(path).expect("failed to hash frontier");
    let payload = json!({
        "ok": true,
        "command": "stats",
        "schema_version": project::VELA_SCHEMA_VERSION,
        "frontier": {
            "name": &frontier.project.name,
            "description": &frontier.project.description,
            "source": path.display().to_string(),
            "hash": format!("sha256:{source_hash}"),
            "compiled_at": &frontier.project.compiled_at,
            "compiler": &frontier.project.compiler,
            "papers_processed": frontier.project.papers_processed,
            "errors": frontier.project.errors,
        },
        "stats": frontier.stats,
        "proposals": proposals::summary(&frontier),
        "proof_state": frontier.proof_state,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("failed to serialize stats")
    );
}

fn cmd_search(
    source: Option<&Path>,
    query: &str,
    entity: Option<&str>,
    assertion_type: Option<&str>,
    all: Option<&Path>,
    limit: usize,
    json_output: bool,
) {
    if let Some(dir) = all {
        search::run_all(dir, query, entity, assertion_type, limit);
        return;
    }
    let Some(src) = source else {
        fail("Provide --source <frontier> or --all <directory>.");
    };
    if json_output {
        let results = search::search(src, query, entity, assertion_type, limit);
        let loaded = repo::load_from_path(src).expect("Failed to load frontier");
        let source_hash = hash_path(src).expect("failed to hash frontier");
        let payload = json!({
            "ok": true,
            "command": "search",
            "schema_version": project::VELA_SCHEMA_VERSION,
            "query": query,
            "frontier": {
                "name": &loaded.project.name,
                "source": src.display().to_string(),
                "hash": format!("sha256:{source_hash}"),
            },
            "filters": {
                "entity": entity,
                "assertion_type": assertion_type,
                "limit": limit,
            },
            "count": results.len(),
            "results": results.iter().map(|result| json!({
                "id": &result.id,
                "score": result.score,
                "assertion": &result.assertion,
                "assertion_type": &result.assertion_type,
                "confidence": result.confidence,
                "entities": &result.entities,
                "doi": &result.doi,
            })).collect::<Vec<_>>()
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("failed to serialize search results")
        );
    } else {
        search::run(src, query, entity, assertion_type, limit);
    }
}

fn cmd_tensions(source: &Path, both_high: bool, cross_domain: bool, top: usize, json_output: bool) {
    let frontier = repo::load_from_path(source).expect("Failed to load frontier");
    let result = tensions::analyze(&frontier, both_high, cross_domain, top);
    if json_output {
        let source_hash = hash_path(source).expect("failed to hash frontier");
        let payload = json!({
            "ok": true,
            "command": "tensions",
            "schema_version": project::VELA_SCHEMA_VERSION,
            "frontier": {
                "name": &frontier.project.name,
                "source": source.display().to_string(),
                "hash": format!("sha256:{source_hash}"),
            },
            "filters": {
                "both_high": both_high,
                "cross_domain": cross_domain,
                "top": top,
            },
            "count": result.len(),
            "tensions": result.iter().map(|t| json!({
                "score": t.score,
                "resolved": t.resolved,
                "superseding_id": &t.superseding_id,
                "finding_a": {
                    "id": &t.finding_a.id,
                    "assertion": &t.finding_a.assertion,
                    "confidence": t.finding_a.confidence,
                    "assertion_type": &t.finding_a.assertion_type,
                    "citation_count": t.finding_a.citation_count,
                    "contradicts_count": t.finding_a.contradicts_count,
                },
                "finding_b": {
                    "id": &t.finding_b.id,
                    "assertion": &t.finding_b.assertion,
                    "confidence": t.finding_b.confidence,
                    "assertion_type": &t.finding_b.assertion_type,
                    "citation_count": t.finding_b.citation_count,
                    "contradicts_count": t.finding_b.contradicts_count,
                }
            })).collect::<Vec<_>>()
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("failed to serialize tensions")
        );
    } else {
        tensions::print_tensions(&result);
    }
}

fn cmd_gaps(action: GapsAction) {
    match action {
        GapsAction::Rank {
            frontier,
            top,
            domain,
            json,
        } => cmd_gap_rank(&frontier, top, domain.as_deref(), json),
    }
}

fn cmd_gap_rank(frontier_path: &Path, top: usize, domain: Option<&str>, json_output: bool) {
    let frontier = repo::load_from_path(frontier_path).expect("Failed to load frontier");
    let mut ranked = frontier
        .findings
        .iter()
        .filter(|finding| finding.flags.gap || finding.flags.negative_space)
        .filter(|finding| {
            domain.is_none_or(|domain| {
                finding
                    .assertion
                    .text
                    .to_lowercase()
                    .contains(&domain.to_lowercase())
                    || finding
                        .assertion
                        .entities
                        .iter()
                        .any(|entity| entity.name.to_lowercase().contains(&domain.to_lowercase()))
            })
        })
        .map(|finding| {
            let dependency_count = frontier
                .findings
                .iter()
                .flat_map(|candidate| candidate.links.iter())
                .filter(|link| link.target == finding.id)
                .count();
            let score = dependency_count as f64 + finding.confidence.score;
            json!({
                "id": &finding.id,
                "kind": "candidate_gap_review_lead",
                "assertion": &finding.assertion.text,
                "score": score,
                "dependency_count": dependency_count,
                "confidence": finding.confidence.score,
                "evidence_type": &finding.evidence.evidence_type,
                "entities": finding.assertion.entities.iter().map(|e| &e.name).collect::<Vec<_>>(),
                "recommended_action": "Review source scope and missing evidence before treating this as an experiment target.",
                "caveats": ["Candidate gap rankings are review leads, not guaranteed underexplored areas or experiment targets."],
            })
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        b.get("score")
            .and_then(Value::as_f64)
            .partial_cmp(&a.get("score").and_then(Value::as_f64))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.truncate(top);
    if json_output {
        let source_hash = hash_path(frontier_path).expect("failed to hash frontier");
        let payload = json!({
            "ok": true,
            "command": "gaps rank",
            "schema_version": project::VELA_SCHEMA_VERSION,
            "frontier": {
                "name": &frontier.project.name,
                "source": frontier_path.display().to_string(),
                "hash": format!("sha256:{source_hash}"),
            },
            "filters": {
                "top": top,
                "domain": domain,
            },
            "count": ranked.len(),
            "ranking_label": "candidate gap review leads",
            "caveats": ["These rankings are navigation signals over flagged findings, not scientific conclusions."],
            "review_leads": ranked.clone(),
            "gaps": ranked,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("failed to serialize gap ranking")
        );
    } else {
        println!();
        println!("  {}", "CANDIDATE GAP REVIEW LEADS".dimmed());
        println!("  {}", style::tick_row(60));
        println!("  review source scope; these are not guaranteed experiment targets.");
        println!();
        for (idx, gap) in ranked.iter().enumerate() {
            println!(
                "  {}. [{}] score={} {}",
                idx + 1,
                gap["id"].as_str().unwrap_or("?"),
                gap["score"].as_f64().unwrap_or(0.0),
                gap["assertion"].as_str().unwrap_or("")
            );
        }
    }
}

async fn cmd_bridge(inputs: &[PathBuf], check_novelty: bool, top_n: usize) {
    if inputs.len() < 2 {
        fail("need at least 2 frontier files for bridge detection.");
    }
    println!();
    println!("  {}", "VELA · BRIDGE · V0.36.0".dimmed());
    println!("  {}", style::tick_row(60));
    println!("  loading {} frontiers...", inputs.len());
    let mut named_projects = Vec::<(String, project::Project)>::new();
    let mut total_findings = 0;
    for path in inputs {
        let frontier = repo::load_from_path(path).expect("Failed to load frontier");
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        println!("  {} · {} findings", name, frontier.stats.findings);
        total_findings += frontier.stats.findings;
        named_projects.push((name, frontier));
    }
    let refs = named_projects
        .iter()
        .map(|(name, frontier)| (name.as_str(), frontier))
        .collect::<Vec<_>>();
    let mut bridges = bridge::detect_bridges(&refs);
    if check_novelty && !bridges.is_empty() {
        let client = Client::new();
        let check_count = bridges.len().min(top_n);
        println!("  running rough PubMed prior-art checks for top {check_count} bridges...");
        for bridge_item in bridges.iter_mut().take(check_count) {
            let query = bridge::novelty_query(&bridge_item.entity_name, bridge_item);
            match bridge::check_novelty(&client, &query).await {
                Ok(count) => bridge_item.pubmed_count = Some(count),
                Err(e) => eprintln!(
                    "  {} prior-art check failed for {}: {e}",
                    style::err_prefix(),
                    bridge_item.entity_name
                ),
            }
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        }
    }
    print!("{}", bridge::format_report(&bridges, total_findings));
}

struct BenchArgs {
    frontier: Option<PathBuf>,
    gold: Option<PathBuf>,
    entity_gold: Option<PathBuf>,
    link_gold: Option<PathBuf>,
    suite: Option<PathBuf>,
    suite_ready: bool,
    min_f1: Option<f64>,
    min_precision: Option<f64>,
    min_recall: Option<f64>,
    no_thresholds: bool,
    json: bool,
}

/// v0.26 VelaBench: compare a candidate frontier (typically agent-
/// generated) against a gold frontier. Pure data comparison —
/// no LLM call, no network, deterministic. Exits non-zero when
/// the composite falls below `threshold` (default 0.0 = report only).
fn cmd_agent_bench(
    gold: &Path,
    candidate: &Path,
    sources: Option<&Path>,
    threshold: Option<f64>,
    report_path: Option<&Path>,
    json_out: bool,
) {
    let input = crate::agent_bench::BenchInput {
        gold_path: gold.to_path_buf(),
        candidate_path: candidate.to_path_buf(),
        sources: sources.map(Path::to_path_buf),
        threshold: threshold.unwrap_or(0.0),
    };
    let report = match crate::agent_bench::run(input) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} bench failed: {e}", style::err_prefix());
            std::process::exit(1);
        }
    };

    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
    if let Some(path) = report_path
        && let Err(e) = std::fs::write(path, &json)
    {
        eprintln!(
            "{} failed to write report to {}: {e}",
            style::err_prefix(),
            path.display()
        );
    }

    if json_out {
        println!("{json}");
    } else {
        println!();
        println!("  {}", "VELA · BENCH · AGENT STATE-UPDATE".dimmed());
        println!("  {}", style::tick_row(60));
        print!("{}", crate::agent_bench::render_pretty(&report));
        println!();
    }

    if !report.pass {
        std::process::exit(1);
    }
}

fn cmd_bench(args: BenchArgs) {
    if args.suite_ready {
        let suite_path = args
            .suite
            .unwrap_or_else(|| PathBuf::from("benchmarks/suites/bbb-core.json"));
        let payload =
            benchmark::suite_ready_report(&suite_path).unwrap_or_else(|e| fail_return(&e));
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("failed to serialize suite-ready report")
        );
        if payload.get("ok").and_then(Value::as_bool) != Some(true) {
            std::process::exit(1);
        }
        return;
    }
    if let Some(suite_path) = args.suite {
        let payload = benchmark::run_suite(&suite_path).unwrap_or_else(|e| fail_return(&e));
        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&payload)
                    .expect("failed to serialize benchmark suite")
            );
        } else {
            let ok = payload.get("ok").and_then(Value::as_bool) == Some(true);
            let metrics = payload.get("metrics").unwrap_or(&Value::Null);
            println!();
            println!("  {}", "VELA · BENCH · SUITE".dimmed());
            println!("  {}", style::tick_row(60));
            println!("  suite: {}", suite_path.display());
            println!(
                "  status: {}",
                if ok {
                    style::ok("pass")
                } else {
                    style::lost("fail")
                }
            );
            println!(
                "  tasks: {}/{} passed",
                metrics
                    .get("tasks_passed")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                metrics
                    .get("tasks_total")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
            );
        }
        if payload.get("ok").and_then(Value::as_bool) != Some(true) {
            std::process::exit(1);
        }
        return;
    }

    let frontier = args
        .frontier
        .unwrap_or_else(|| PathBuf::from("frontiers/bbb-alzheimer.json"));
    let thresholds = benchmark::BenchmarkThresholds {
        min_f1: if args.no_thresholds {
            None
        } else {
            args.min_f1.or(Some(0.05))
        },
        min_precision: if args.no_thresholds {
            None
        } else {
            args.min_precision
        },
        min_recall: if args.no_thresholds {
            None
        } else {
            args.min_recall
        },
        ..Default::default()
    };
    if let Some(path) = args.link_gold {
        print_benchmark_or_exit(benchmark::task_envelope(
            &frontier,
            None,
            benchmark::BenchmarkMode::Link,
            Some(&path),
            &thresholds,
            None,
        ));
    } else if let Some(path) = args.entity_gold {
        print_benchmark_or_exit(benchmark::task_envelope(
            &frontier,
            None,
            benchmark::BenchmarkMode::Entity,
            Some(&path),
            &thresholds,
            None,
        ));
    } else if let Some(path) = args.gold {
        if args.json {
            print_benchmark_or_exit(benchmark::task_envelope(
                &frontier,
                None,
                benchmark::BenchmarkMode::Finding,
                Some(&path),
                &thresholds,
                None,
            ));
        } else {
            benchmark::run(&frontier, &path, false);
        }
    } else {
        fail("Provide --suite, --gold, --entity-gold, or --link-gold.");
    }
}

fn print_benchmark_or_exit(result: Result<Value, String>) {
    let payload = result.unwrap_or_else(|e| fail_return(&e));
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("failed to serialize benchmark report")
    );
    if payload.get("ok").and_then(Value::as_bool) != Some(true) {
        std::process::exit(1);
    }
}

fn cmd_packet(action: PacketAction) {
    let (result, json_output) = match action {
        PacketAction::Inspect { path, json } => (packet::inspect(&path), json),
        PacketAction::Validate { path, json } => (packet::validate(&path), json),
    };
    match result {
        Ok(output) if json_output => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "command": "packet",
                    "result": output,
                }))
                .expect("failed to serialize packet response")
            );
        }
        Ok(output) => println!("{output}"),
        Err(e) => fail(&e),
    }
}

fn cmd_init(path: &Path, name: &str) {
    let vela_dir = path.join(".vela");
    if vela_dir.exists() {
        fail(&format!(
            "already initialized: {} exists",
            vela_dir.display()
        ));
    }
    std::fs::create_dir_all(&vela_dir).expect("Failed to create .vela/");
    let config = format!(
        r#"# Vela Project Configuration
[project]
name = "{name}"
description = ""
compiler = "vela/0.2.0"
papers_processed = 0
"#
    );
    std::fs::write(vela_dir.join("config.toml"), config).expect("Failed to write config.toml");
    for dir in ["findings", "events", "proposals"] {
        std::fs::create_dir_all(vela_dir.join(dir))
            .unwrap_or_else(|e| panic!("Failed to create .vela/{dir}/: {e}"));
    }
    let proof_state = serde_json::to_string_pretty(&proposals::ProofState::default())
        .expect("Failed to serialize proof state");
    std::fs::write(vela_dir.join("proof-state.json"), proof_state)
        .expect("Failed to write .vela/proof-state.json");
    if !path.join(".git").exists() {
        println!(
            "  {} run `git init` to enable version control",
            "hint ·".dimmed()
        );
    }
    println!(
        "{} initialized Vela repository in {}",
        style::ok("ok"),
        path.display()
    );
}

fn cmd_import(frontier_path: &Path, into: Option<&Path>) {
    let frontier = repo::load_from_path(frontier_path).unwrap_or_else(|e| fail_return(&e));
    let target = into
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(frontier.project.name.replace(' ', "-").to_lowercase()));
    repo::init_repo(&target, &frontier).unwrap_or_else(|e| fail(&e));
    println!(
        "{} {} findings · {}",
        style::ok("imported"),
        frontier.findings.len(),
        target.display()
    );
}

fn cmd_propagate(
    path: &Path,
    retract: Option<String>,
    reduce_confidence: Option<String>,
    to: Option<f64>,
    output: Option<&Path>,
) {
    let mut frontier = repo::load_from_path(path).expect("Failed to load frontier");
    let (finding_id, action, label) = if let Some(id) = retract {
        (id, propagate::PropagationAction::Retracted, "retraction")
    } else if let Some(id) = reduce_confidence {
        let score = to.unwrap_or_else(|| fail_return("--reduce-confidence requires --to <score>"));
        if !(0.0..=1.0).contains(&score) {
            fail("--to must be between 0.0 and 1.0");
        }
        (
            id,
            propagate::PropagationAction::ConfidenceReduced { new_score: score },
            "confidence reduction",
        )
    } else {
        fail("specify --retract <id> or --reduce-confidence <id> --to <score>");
    };
    if !frontier.findings.iter().any(|f| f.id == finding_id) {
        fail(&format!("finding not found: {finding_id}"));
    }
    let result = propagate::propagate_correction(&mut frontier, &finding_id, action);
    // v0.36.2: persist propagation events into the canonical review
    // log. Pre-v0.36.2 these were emitted to stdout and lost — the
    // kernel forgot why a finding was flagged the moment the command
    // returned.
    frontier.review_events.extend(result.events.clone());
    project::recompute_stats(&mut frontier);
    propagate::print_result(&result, label, &finding_id);
    let out = output.unwrap_or(path);
    repo::save_to_path(out, &frontier).expect("Failed to save frontier");
    println!("  output: {}", out.display());
}


fn cmd_mcp_setup(source: Option<&Path>, frontiers: Option<&Path>) {
    let source_desc = source
        .map(|p| p.display().to_string())
        .or_else(|| frontiers.map(|p| p.display().to_string()))
        .unwrap_or_else(|| "frontier.json".to_string());
    let args = if let Some(path) = source {
        format!(r#""serve", "{}""#, path.display())
    } else if let Some(path) = frontiers {
        format!(r#""serve", "--frontiers", "{}""#, path.display())
    } else {
        r#""serve", "frontier.json""#.to_string()
    };
    println!(
        r#"Add this MCP server configuration to your client:

{{
  "mcpServers": {{
    "vela": {{
      "command": "vela",
      "args": [{args}]
    }}
  }}
}}

Source: {source_desc}"#
    );
}

fn parse_entities(input: &str) -> Vec<(String, String)> {
    if input.trim().is_empty() {
        return Vec::new();
    }
    input
        .split(',')
        .filter_map(|pair| {
            let parts = pair.trim().splitn(2, ':').collect::<Vec<_>>();
            if parts.len() == 2 {
                Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
            } else {
                eprintln!(
                    "{} skipping malformed entity '{}'",
                    style::warn("warn"),
                    pair.trim()
                );
                None
            }
        })
        .collect()
}

fn hash_path(path: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    if path.is_file() {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to read {} for hashing: {e}", path.display()))?;
        hasher.update(&bytes);
    } else if path.is_dir() {
        let mut files = Vec::new();
        collect_hash_files(path, path, &mut files)?;
        files.sort();
        for rel in files {
            hasher.update(rel.to_string_lossy().as_bytes());
            let bytes = std::fs::read(path.join(&rel))
                .map_err(|e| format!("Failed to read {} for hashing: {e}", rel.display()))?;
            hasher.update(bytes);
        }
    } else {
        return Err(format!("Cannot hash missing path {}", path.display()));
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_hash_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {e}", dir.display()))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_hash_files(root, &path, files)?;
        } else if path.is_file() {
            files.push(
                path.strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn schema_error_suggestion(error: &str) -> &'static str {
    if schema_error_action(error).is_some() {
        "Run `vela normalize` to repair deterministic frontier state."
    } else {
        "Inspect and correct the referenced frontier field."
    }
}

fn schema_error_fix(error: &str) -> bool {
    schema_error_action(error).is_some()
}

fn schema_error_action(error: &str) -> Option<&'static str> {
    if error.contains("stats.findings")
        || error.contains("stats.links")
        || error.contains("Invalid compiler")
        || error.contains("Invalid vela_version")
        || error.contains("Invalid schema")
    {
        Some("normalize_metadata_and_stats")
    } else if error.contains("does not match content-address") {
        Some("rewrite_ids")
    } else {
        None
    }
}

fn build_repair_plan(diagnostics: &[Value]) -> Vec<Value> {
    let mut actions = std::collections::BTreeMap::<String, usize>::new();
    for diagnostic in diagnostics {
        if let Some(action) = diagnostic.get("normalize_action").and_then(Value::as_str) {
            *actions.entry(action.to_string()).or_default() += 1;
        }
    }
    actions
        .into_iter()
        .map(|(action, count)| {
            let command = if action == "rewrite_ids" {
                "vela normalize <frontier> --write --rewrite-ids --id-map id-map.json"
            } else {
                "vela normalize <frontier> --write"
            };
            json!({
                "action": action,
                "count": count,
                "command": command,
            })
        })
        .collect()
}

fn empty_signal_report() -> signals::SignalReport {
    signals::SignalReport {
        schema: "vela.signals.v0".to_string(),
        frontier: "unavailable".to_string(),
        signals: Vec::new(),
        review_queue: Vec::new(),
        proof_readiness: signals::ProofReadiness {
            status: "unavailable".to_string(),
            blockers: 0,
            warnings: 0,
            caveats: vec!["Frontier could not be loaded for signal analysis.".to_string()],
        },
    }
}

fn print_signal_summary(report: &signals::SignalReport, strict: bool) {
    println!();
    println!("  {}", "SIGNALS".dimmed());
    println!("  {}", style::tick_row(60));
    println!("  total signals:   {}", report.signals.len());
    println!("  proof readiness: {}", report.proof_readiness.status);
    if !report.review_queue.is_empty() {
        println!("  review queue:    {} items", report.review_queue.len());
    }
    if strict && report.proof_readiness.status != "ready" {
        println!(
            "  {} proof readiness has blocking signals.",
            style::lost("strict check failed")
        );
    }
}

fn append_packet_json_file(
    packet_dir: &Path,
    relative_path: &str,
    value: &Value,
) -> Result<(), String> {
    let content = serde_json::to_vec_pretty(value)
        .map_err(|e| format!("Failed to serialize packet JSON file: {e}"))?;
    let path = packet_dir.join(relative_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, &content)
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
    let entry = json!({
        "path": relative_path,
        "sha256": hex::encode(Sha256::digest(&content)),
        "bytes": content.len(),
    });

    for manifest_name in ["manifest.json", "packet.lock.json"] {
        let manifest_path = packet_dir.join(manifest_name);
        let data = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read {}: {e}", manifest_path.display()))?;
        let mut manifest: Value = serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse {}: {e}", manifest_path.display()))?;
        let array_key = if manifest_name == "manifest.json" {
            "included_files"
        } else {
            "files"
        };
        let files = manifest
            .get_mut(array_key)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| format!("{} missing {array_key} array", manifest_path.display()))?;
        files.retain(|file| {
            file.get("path")
                .and_then(Value::as_str)
                .is_none_or(|path| path != relative_path)
        });
        files.push(entry.clone());
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest)
                .map_err(|e| format!("Failed to serialize {}: {e}", manifest_path.display()))?,
        )
        .map_err(|e| format!("Failed to write {}: {e}", manifest_path.display()))?;
    }

    let lock_path = packet_dir.join("packet.lock.json");
    let lock_content = std::fs::read(&lock_path)
        .map_err(|e| format!("Failed to read {}: {e}", lock_path.display()))?;
    let lock_entry = json!({
        "path": "packet.lock.json",
        "sha256": hex::encode(Sha256::digest(&lock_content)),
        "bytes": lock_content.len(),
    });
    let manifest_path = packet_dir.join("manifest.json");
    let data = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read {}: {e}", manifest_path.display()))?;
    let mut manifest: Value = serde_json::from_str(&data)
        .map_err(|e| format!("Failed to parse {}: {e}", manifest_path.display()))?;
    let files = manifest
        .get_mut("included_files")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("{} missing included_files array", manifest_path.display()))?;
    files.retain(|file| {
        file.get("path")
            .and_then(Value::as_str)
            .is_none_or(|path| path != "packet.lock.json")
    });
    files.push(lock_entry);
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest)
            .map_err(|e| format!("Failed to serialize {}: {e}", manifest_path.display()))?,
    )
    .map_err(|e| format!("Failed to write {}: {e}", manifest_path.display()))?;
    Ok(())
}

fn print_tool_check_report(report: &Value) {
    let summary = report.get("summary").unwrap_or(&Value::Null);
    let frontier = report.get("frontier").unwrap_or(&Value::Null);
    println!();
    println!("  {}", "VELA · SERVE · CHECK-TOOLS".dimmed());
    println!("  {}", style::tick_row(60));
    println!(
        "frontier: {}",
        frontier
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    );
    println!(
        "findings: {}",
        frontier
            .get("findings")
            .and_then(Value::as_u64)
            .unwrap_or_default()
    );
    println!(
        "checks: {} passed, {} failed",
        summary
            .get("passed")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        summary
            .get("failed")
            .and_then(Value::as_u64)
            .unwrap_or_default()
    );
    if let Some(tools) = report.get("tools").and_then(Value::as_array) {
        let names = tools
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        println!("tools: {names}");
    }
    if let Some(checks) = report.get("checks").and_then(Value::as_array) {
        for check in checks {
            let status = if check.get("ok").and_then(Value::as_bool) == Some(true) {
                style::ok("ok")
            } else {
                style::lost("lost")
            };
            println!(
                "  {} {}",
                status,
                check
                    .get("tool")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            );
        }
    }
}

fn print_state_report(report: &state::StateCommandReport, json_output: bool) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(report).expect("failed to serialize state command report")
        );
    } else {
        println!("{}", report.message);
        println!("  frontier: {}", report.frontier);
        println!("  finding:  {}", report.finding_id);
        println!("  proposal: {}", report.proposal_id);
        println!("  status:   {}", report.proposal_status);
        if let Some(event_id) = &report.applied_event_id {
            println!("  event:    {}", event_id);
        }
        println!("  wrote:    {}", report.wrote_to);
    }
}

fn print_history(payload: &Value) {
    let finding = payload.get("finding").unwrap_or(&Value::Null);
    println!("vela history");
    println!(
        "  finding: {}",
        finding
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    );
    println!(
        "  assertion: {}",
        finding
            .get("assertion")
            .and_then(Value::as_str)
            .unwrap_or("")
    );
    println!(
        "  confidence: {:.3}",
        finding
            .get("confidence")
            .and_then(Value::as_f64)
            .unwrap_or_default()
    );
    let reviews = payload
        .get("review_events")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let updates = payload
        .get("confidence_updates")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let annotations = finding
        .get("annotations")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let sources = payload
        .get("sources")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let atoms = payload
        .get("evidence_atoms")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let conditions = payload
        .get("condition_records")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let proposals = payload
        .get("proposals")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let events = payload
        .get("events")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    println!("  review events:      {reviews}");
    println!("  confidence updates: {updates}");
    println!("  annotations:        {annotations}");
    println!("  sources:            {sources}");
    println!("  evidence atoms:     {atoms}");
    println!("  condition records:  {conditions}");
    println!("  proposals:          {proposals}");
    println!("  canonical events:   {events}");
    if let Some(status) = payload
        .get("proof_state")
        .and_then(|value| value.get("latest_packet"))
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
    {
        println!("  proof state:        {status}");
    }
    if let Some(events) = payload.get("review_events").and_then(Value::as_array) {
        for event in events.iter().take(8) {
            println!(
                "  - {} {} {}",
                event
                    .get("reviewed_at")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                event.get("id").and_then(Value::as_str).unwrap_or(""),
                event.get("reason").and_then(Value::as_str).unwrap_or("")
            );
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ProofTrace {
    pub trace_version: String,
    pub command: Vec<String>,
    pub source: String,
    pub source_hash: String,
    pub schema_version: String,
    pub checked_artifacts: Vec<String>,
    pub benchmark: Option<Value>,
    pub packet_manifest: String,
    pub packet_validation: String,
    pub caveats: Vec<String>,
    pub status: String,
    pub trace_path: String,
}

const SCIENCE_SUBCOMMANDS: &[&str] = &[
    "compile-notes",
    "compile-code",
    "compile-data",
    "review-pending",
    "find-tensions",
    "plan-experiments",
    "scout",
    "check",
    "normalize",
    "proof",
    "serve",
    "stats",
    "search",
    "tensions",
    "gaps",
    "bridge",
    "export",
    "packet",
    "bench",
    "conformance",
    "version",
    "sign",
    "actor",
    "frontier",
    "queue",
    "registry",
    "init",
    "import",
    "diff",
    "proposals",
    "finding",
    "link",
    "entity",
    "review",
    "note",
    "caveat",
    "revise",
    "reject",
    "history",
    "import-events",
    "retract",
    "propagate",
    // v0.32: replication as a first-class kernel object.
    "replicate",
    "replications",
    // v0.33: computational provenance — datasets and code as
    // first-class kernel objects.
    "dataset-add",
    "datasets",
    "code-add",
    "code-artifacts",
    // v0.34: predictions and resolutions — the epistemic accountability
    // ledger.
    "predict",
    "resolve",
    "predictions",
    "calibration",
    // v0.35: inference layer — consensus aggregation over claim-similar
    // findings.
    "consensus",
    // v0.39: federation — peer registry + sync runtime.
    "federation",
    // v0.40: causal reasoning — identifiability audit.
    "causal",
];

pub fn is_science_subcommand(name: &str) -> bool {
    SCIENCE_SUBCOMMANDS.contains(&name)
}

fn print_strict_help() {
    println!(
        r#"Vela 0.36.0
Portable frontier state for science.

Usage:
  vela <COMMAND>

Core commands:
  compile       Compile a topic or local paper folder into frontier.json
  check         Validate a frontier, repo, or proof packet
  normalize     Apply deterministic frontier-state repairs
  proof         Export and validate a proof packet
  serve         Serve a frontier over MCP or HTTP
  stats         Show frontier statistics
  search        Search findings
  tensions      List candidate contradictions and tensions
  gaps          Inspect and rank candidate gap review leads
  bridge        Find candidate cross-domain connections
  scout              Run Literature Scout against a folder of PDFs (writes proposals)
  compile-notes      Run Notes Compiler against a Markdown vault (writes proposals)
  compile-code       Run Code & Notebook Analyst against a research repo (writes proposals)
  compile-data       Run Datasets agent against a folder of CSV/TSV/Parquet (writes proposals)
  review-pending     Run Reviewer Agent: score every pending proposal (writes notes)
  find-tensions      Run Contradiction Finder: surface real contradictions among findings
  plan-experiments   Run Experiment Planner: propose experiments for open questions / hypotheses
  ingest             Add manual or file-derived findings
  jats          Compile findings from JATS XML or PMC input
  export        Export frontier artifacts
  packet        Inspect or validate proof packets
  bench         Run deterministic benchmark gates
  conformance   Run protocol conformance vectors
  sign          Optional signing and signature verification
  version       Show version information

State commands:
  init          Initialize a .vela frontier repo
  import        Import frontier.json into a .vela repo
  diff          Compare two frontiers
  proposals     Inspect, validate, export, import, accept, or reject write proposals
  finding       Add or manage finding bundles as frontier state
  link          Add typed links between findings (incl. cross-frontier vf_at-vfr targets)
  entity        Resolve unresolved entities against a bundled common-entity table (v0.19)
  frontier      Scaffold (`new`) and manage frontier metadata + cross-frontier deps
  actor         Register Ed25519 publisher identities in a frontier
  registry      Publish, list, or pull frontiers (open hub at https://vela-hub.fly.dev)
  review        Create a review proposal or review interactively
  note          Add a lightweight note to a finding
  caveat        Create an explicit caveat proposal
  revise        Create a confidence revision proposal
  reject        Create a rejection proposal
  history       Show state-transition history for one finding
  import-events  Import review/state events from a packet or JSON file
  retract       Create a retraction proposal
  propagate     Simulate impact over declared dependency links

Quick start:
  vela compile ./papers --output frontier.json
  vela check frontier.json --strict --json
  FINDING_ID=$(jq -r '.findings[0].id' frontier.json)
  vela review frontier.json "$FINDING_ID" --status contested --reason "Mouse-only evidence" --reviewer reviewer:demo --apply

Publish your own frontier (see docs/PUBLISHING.md):
  vela frontier new ./frontier.json --name "Your bounded question"
  vela finding add ./frontier.json --assertion "..." --author "reviewer:you" --apply
  vela sign generate-keypair --out keys
  vela actor add ./frontier.json reviewer:you --pubkey "$(cat keys/public.key)"
  vela registry publish ./frontier.json --owner reviewer:you --key keys/private.key \
      --locator <https-url> --to https://vela-hub.fly.dev
"#
    );
}

/// v0.22 Agent Inbox: pluggable handler for `vela scout`.
///
/// The substrate library can't import `vela-scientist` (cyclic
/// dependency), so the scout dispatch in this module looks up a
/// handler installed by the binary at startup. The `vela` CLI in
/// `crates/vela-cli` registers a real handler via
/// `register_scout_handler`. Library callers that want scout
/// behaviour install their own.
pub type ScoutHandler = fn(
    folder: PathBuf,
    frontier: PathBuf,
    backend: Option<String>,
    dry_run: bool,
    json: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>>;

static SCOUT_HANDLER: OnceLock<ScoutHandler> = OnceLock::new();

/// Install the scout handler. Idempotent — second registrations are
/// silently ignored so a misbehaving consumer can't unseat the
/// binary's wiring mid-run.
pub fn register_scout_handler(handler: ScoutHandler) {
    let _ = SCOUT_HANDLER.set(handler);
}

/// v0.23 Agent Inbox: pluggable handler for `vela compile-notes`.
/// Same OnceLock pattern as the scout handler; the binary
/// registers it at startup.
pub type NotesHandler = fn(
    vault: PathBuf,
    frontier: PathBuf,
    backend: Option<String>,
    max_files: Option<usize>,
    max_items_per_category: Option<usize>,
    dry_run: bool,
    json: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>>;

static NOTES_HANDLER: OnceLock<NotesHandler> = OnceLock::new();

/// Install the notes-compiler handler. Idempotent.
pub fn register_notes_handler(handler: NotesHandler) {
    let _ = NOTES_HANDLER.set(handler);
}

/// v0.24 Agent Inbox: pluggable handler for `vela compile-code`.
pub type CodeHandler = fn(
    root: PathBuf,
    frontier: PathBuf,
    backend: Option<String>,
    max_files: Option<usize>,
    dry_run: bool,
    json: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>>;

static CODE_HANDLER: OnceLock<CodeHandler> = OnceLock::new();

/// Install the code-analyst handler. Idempotent.
pub fn register_code_handler(handler: CodeHandler) {
    let _ = CODE_HANDLER.set(handler);
}

/// v0.25 Agent Inbox: pluggable handler for `vela compile-data`.
pub type DatasetsHandler = fn(
    root: PathBuf,
    frontier: PathBuf,
    backend: Option<String>,
    sample_rows: Option<usize>,
    dry_run: bool,
    json: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>>;

static DATASETS_HANDLER: OnceLock<DatasetsHandler> = OnceLock::new();

/// Install the datasets handler. Idempotent.
pub fn register_datasets_handler(handler: DatasetsHandler) {
    let _ = DATASETS_HANDLER.set(handler);
}


/// v0.28 Agent Inbox: handler for `vela review-pending`.
pub type ReviewerHandler = fn(
    frontier: PathBuf,
    backend: Option<String>,
    max_proposals: Option<usize>,
    batch_size: usize,
    dry_run: bool,
    json: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>>;

static REVIEWER_HANDLER: OnceLock<ReviewerHandler> = OnceLock::new();

/// Install the reviewer-agent handler. Idempotent.
pub fn register_reviewer_handler(handler: ReviewerHandler) {
    let _ = REVIEWER_HANDLER.set(handler);
}

/// v0.28 Agent Inbox: handler for `vela find-tensions`.
pub type TensionsHandler = fn(
    frontier: PathBuf,
    backend: Option<String>,
    max_findings: Option<usize>,
    dry_run: bool,
    json: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>>;

static TENSIONS_HANDLER: OnceLock<TensionsHandler> = OnceLock::new();

/// Install the contradiction-finder handler. Idempotent.
pub fn register_tensions_handler(handler: TensionsHandler) {
    let _ = TENSIONS_HANDLER.set(handler);
}

/// v0.28 Agent Inbox: handler for `vela plan-experiments`.
pub type ExperimentsHandler = fn(
    frontier: PathBuf,
    backend: Option<String>,
    max_findings: Option<usize>,
    dry_run: bool,
    json: bool,
) -> Pin<Box<dyn Future<Output = ()> + Send>>;

static EXPERIMENTS_HANDLER: OnceLock<ExperimentsHandler> = OnceLock::new();

/// Install the experiment-planner handler. Idempotent.
pub fn register_experiments_handler(handler: ExperimentsHandler) {
    let _ = EXPERIMENTS_HANDLER.set(handler);
}

pub fn run_from_args() {
    style::init();
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(String::as_str) {
        None | Some("-h" | "--help") => {
            print_strict_help();
            return;
        }
        Some("-V" | "--version" | "version") => {
            println!("vela 0.36.0");
            return;
        }
        Some(cmd) if !is_science_subcommand(cmd) => {
            eprintln!(
                "{} unknown or non-release command: {cmd}",
                style::err_prefix()
            );
            eprintln!("run `vela --help` for the strict v0 command surface.");
            std::process::exit(2);
        }
        Some(_) => {}
    }
    let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    runtime.block_on(run_command());
}

fn fail(message: &str) -> ! {
    eprintln!("{} {message}", style::err_prefix());
    std::process::exit(1);
}

/// Validate that a CLI string argument is one of the allowed enum values.
/// On mismatch, prints a friendly error naming the flag and the valid set
/// and exits with code 1. Used at finding-add time so users learn before
/// strict validation rejects the resulting frontier.
fn validate_enum_arg(flag: &str, value: &str, valid: &[&str]) {
    if !valid.contains(&value) {
        fail(&format!(
            "invalid {flag} '{value}'. Valid: {}",
            valid.join(", ")
        ));
    }
}

fn fail_return<T>(message: &str) -> T {
    fail(message)
}
