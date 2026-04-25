use crate::{
    benchmark, bridge, bundle, confidence, conformance, corpus, diff, events, export, extract,
    fetch, ingest, jats, link, lint, llm, normalize, packet, project, propagate, proposals, repo,
    review, search, serve, sign, signals, sources, state, tensions, validate,
};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use clap::{Parser, Subcommand};
use colored::Colorize;
use indicatif::ProgressBar;

use crate::cli_style as style;
use reqwest::Client;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

#[derive(Parser)]
#[command(name = "vela", version = "0.9.0")]
#[command(about = "Portable frontier state for science")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a frontier from a topic or local paper folder
    Compile {
        /// Research topic, file, or local paper folder to compile
        topic: String,
        /// Max papers to process for topic compile
        #[arg(short = 'n', long, default_value = "50")]
        papers: usize,
        /// Output frontier file
        #[arg(short, long, default_value = "frontier.json")]
        output: PathBuf,
        /// LLM backend: gemini, openrouter, groq, anthropic
        #[arg(short, long)]
        backend: Option<String>,
        /// Fetch PMC full text for topic compile
        #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
        fulltext: bool,
        /// Use OpenRouter free-tier backend
        #[arg(long)]
        free: bool,
    },
    /// Ingest manual or file-derived findings into a frontier
    Ingest {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Assertion text for manual ingest
        #[arg(long)]
        assertion: Option<String>,
        /// Ingest PDF file
        #[arg(long)]
        pdf: Option<PathBuf>,
        /// Ingest CSV file
        #[arg(long)]
        csv: Option<PathBuf>,
        /// Ingest text or Markdown file
        #[arg(long)]
        text: Option<PathBuf>,
        /// Ingest by DOI
        #[arg(long)]
        doi: Option<String>,
        /// Ingest every supported file from a directory
        #[arg(long)]
        dir: Option<PathBuf>,
        /// LLM backend for file ingestion
        #[arg(short, long)]
        backend: Option<String>,
        /// Assertion type
        #[arg(long, default_value = "mechanism")]
        r#type: String,
        /// Evidence type
        #[arg(long, default_value = "experimental")]
        evidence: String,
        /// Species
        #[arg(long)]
        species: Option<String>,
        /// Method used
        #[arg(long, default_value = "")]
        method: String,
        /// Confidence score from 0.0 to 1.0
        #[arg(long, default_value = "0.7")]
        confidence: f64,
        /// Entities as comma-separated name:type pairs
        #[arg(long, default_value = "")]
        entities: String,
        /// Direction: positive, negative, bidirectional, or null
        #[arg(long)]
        direction: Option<String>,
        /// Source description
        #[arg(long, default_value = "manual ingest")]
        source: String,
    },
    /// Compile findings from JATS XML or a PMC ID
    Jats {
        /// JATS XML file path or PMC ID
        source: String,
        /// Output frontier file or Vela repo
        #[arg(short, long, default_value = "frontier.json")]
        output: PathBuf,
        /// LLM backend for extraction
        #[arg(short, long)]
        backend: Option<String>,
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
    /// Run deterministic benchmark gates
    Bench {
        /// Frontier file for single-task benchmark
        frontier: Option<PathBuf>,
        #[arg(long)]
        gold: Option<PathBuf>,
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
        /// draft. Required in non-interactive contexts.
        #[arg(long)]
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
        /// Entities as comma-separated name:type pairs. Entity types: gene, protein, compound, disease, cell_type, organism, pathway, assay, anatomical_structure, other
        #[arg(long, default_value = "")]
        entities: String,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
        /// Immediately accept and apply the proposal locally
        #[arg(long)]
        apply: bool,
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
        Commands::Compile {
            topic,
            papers,
            output,
            backend,
            fulltext,
            free,
        } => {
            let backend = if free {
                Some("openrouter".to_string())
            } else {
                backend
            };
            cmd_compile(&topic, papers, &output, backend.as_deref(), fulltext).await;
        }
        Commands::Ingest {
            frontier,
            assertion,
            pdf,
            csv,
            text,
            doi,
            dir,
            backend,
            r#type,
            evidence,
            species,
            method,
            confidence,
            entities,
            direction,
            source,
        } => {
            cmd_ingest(
                &frontier,
                assertion,
                pdf,
                csv,
                text,
                doi,
                dir,
                backend.as_deref(),
                r#type,
                evidence,
                species,
                method,
                confidence,
                entities,
                direction,
                source,
            )
            .await;
        }
        Commands::Jats {
            source,
            output,
            backend,
        } => cmd_jats(&source, &output, backend.as_deref()).await,
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
            entity_gold,
            link_gold,
            suite,
            suite_ready,
            min_f1,
            min_precision,
            min_recall,
            no_thresholds,
            json,
        } => cmd_bench(BenchArgs {
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
        }),
        Commands::Conformance { dir } => {
            let _ = conformance::run(&dir);
        }
        Commands::Version => println!("vela 0.9.0"),
        Commands::Sign { action } => cmd_sign(action),
        Commands::Actor { action } => cmd_actor(action),
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
                    },
                    apply,
                )
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
    }
}

pub async fn cmd_compile(
    topic: &str,
    max_papers: usize,
    output: &PathBuf,
    backend: Option<&str>,
    fulltext: bool,
) {
    let compile_start = Instant::now();

    let local_source = Path::new(topic);
    if local_source.exists() {
        match corpus::compile_local_corpus(local_source, output, backend).await {
            Ok(report) => {
                println!();
                println!("  {}", "VELA · COMPILE · V0.9.0".dimmed());
                println!("  {}", style::tick_row(60));
                println!("source: {}", local_source.display());
                println!("mode: local corpus");
                println!("findings: {}", report.summary.findings);
                println!("accepted sources: {}", report.summary.accepted);
                println!("skipped sources: {}", report.summary.skipped);
                println!("errors: {}", report.summary.errors);
                println!("output: {}", output.display());
                println!("report: {}", report.artifacts.compile_report);
                println!("quality table: {}", report.artifacts.quality_table);
                println!("frontier quality: {}", report.artifacts.frontier_quality);
                if !report.warnings.is_empty() {
                    println!();
                    println!("warnings:");
                    for warning in &report.warnings {
                        println!("  - {warning}");
                    }
                }
                println!();
                println!("next: vela check {} --strict --json", output.display());
            }
            Err(e) => fail(&e),
        }
        return;
    }

    let config = match llm::LlmConfig::from_env(backend) {
        Ok(c) => Some(c),
        Err(e) if backend.is_some() => fail(&e),
        Err(_) => None,
    };
    let client = Client::new();

    println!();
    println!("  {}", "VELA · COMPILE · V0.9.0".dimmed());
    println!("  {}", style::tick_row(60));
    println!("topic: {topic}");
    println!("papers: {max_papers}");
    println!(
        "backend: {}",
        config
            .as_ref()
            .map_or("deterministic abstract fallback", |c| c.backend.label())
    );
    if config.is_none() {
        println!(
            "{}",
            "note: set GOOGLE_API_KEY, OPENROUTER_API_KEY, GROQ_API_KEY, or ANTHROPIC_API_KEY for richer extraction"
                .dimmed()
        );
    }
    println!();

    let t = Instant::now();
    println!("{}", stage_header(1, 9, "fetching papers..."));
    let mut papers = fetch::fetch_papers(&client, topic, max_papers)
        .await
        .unwrap_or_else(|e| {
            eprintln!("  {} {e}", style::err_prefix());
            std::process::exit(1);
        });
    println!(
        "  {} {} papers with abstracts {}",
        "·".dimmed(),
        papers.len(),
        stage_elapsed(t)
    );
    println!();

    if papers.is_empty() {
        println!("no papers found.");
        return;
    }

    let t = Instant::now();
    if fulltext {
        println!("{}", stage_header(2, 9, "fetching PMC full text..."));
        let enriched = fetch::fetch_fulltext(&client, &mut papers).await;
        println!(
            "  {} {} papers ({} full text, {} abstract-only) {}",
            "·".dimmed(),
            papers.len(),
            enriched,
            papers.len().saturating_sub(enriched),
            stage_elapsed(t)
        );
    } else {
        println!(
            "{}",
            stage_header(2, 9, "skipping PMC full text (--fulltext=false)")
        );
    }
    println!();

    let t = Instant::now();
    println!("{}", stage_header(3, 9, "extracting findings..."));
    let pb = ProgressBar::new(papers.len() as u64);
    pb.set_style(style::progress_style("papers"));
    pb.set_message("0 findings, 0 errors");

    let semaphore = Arc::new(Semaphore::new(8));
    let findings_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let errors_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut handles = Vec::new();

    for paper in papers.iter().cloned() {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed unexpectedly");
        let client = client.clone();
        let config = config.clone();
        let fc = findings_count.clone();
        let ec = errors_count.clone();
        let pb = pb.clone();
        handles.push(tokio::spawn(async move {
            let result = if let Some(config) = &config {
                extract::extract_paper(&client, config, &paper).await
            } else {
                Ok(extract::extract_paper_offline(&paper))
            };
            drop(permit);
            match &result {
                Ok(bundles) => {
                    fc.fetch_add(bundles.len(), std::sync::atomic::Ordering::Relaxed);
                }
                Err(_) => {
                    ec.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
            let f = fc.load(std::sync::atomic::Ordering::Relaxed);
            let e = ec.load(std::sync::atomic::Ordering::Relaxed);
            pb.set_message(format!("{f} findings, {e} errors"));
            pb.inc(1);
            (paper, result)
        }));
    }

    let total_papers = papers.len();
    let mut all_bundles = Vec::new();
    let mut errors = 0usize;
    for (idx, handle) in handles.into_iter().enumerate() {
        let (paper, result) = handle.await.expect("extraction task panicked");
        match result {
            Ok(bundles) => {
                let count = bundles.len();
                all_bundles.extend(bundles);
                let authors = paper
                    .authors
                    .iter()
                    .take(2)
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>();
                pb.println(format!(
                    "  {} {} findings · {} ({})",
                    format!("[{}/{}]", idx + 1, total_papers).dimmed(),
                    count,
                    authors.join(", "),
                    paper.year.map(|y| y.to_string()).unwrap_or_default()
                ));
            }
            Err(e) => {
                errors += 1;
                pb.println(format!(
                    "  {} {} {}",
                    format!("[{}/{}]", idx + 1, total_papers).dimmed(),
                    style::err_prefix(),
                    safe_trunc(&e, 60)
                ));
            }
        }
    }
    pb.finish_and_clear();
    println!(
        "  {} {} findings from {} papers ({} errors) {}",
        "·".dimmed(),
        all_bundles.len(),
        papers.len(),
        errors,
        stage_elapsed(t)
    );
    println!();

    if all_bundles.is_empty() {
        println!("no findings extracted.");
        return;
    }

    dedupe_findings(&mut all_bundles);

    let t = Instant::now();
    println!("{}", stage_header(4, 9, "normalizing entities..."));
    let (type_fixes, name_fixes) = normalize::normalize_findings(&mut all_bundles);
    if type_fixes > 0 || name_fixes > 0 {
        println!("  normalized {type_fixes} entity types, {name_fixes} entity names");
    }
    println!("  {}", stage_elapsed(t));
    println!();

    let t = Instant::now();
    println!("{}", stage_header(5, 9, "grounding confidence scores..."));
    let confidence_updates = confidence::ground_confidence(&mut all_bundles);
    println!(
        "  {} findings adjusted {}",
        confidence_updates.len(),
        stage_elapsed(t)
    );
    println!();

    let t = Instant::now();
    println!("{}", stage_header(6, 9, "resolving entity identifiers..."));
    let (resolved, skipped) = crate::resolve::resolve_entities(&client, &mut all_bundles).await;
    println!(
        "  {} {} resolved, {} unresolved {}",
        "·".dimmed(),
        resolved,
        skipped,
        stage_elapsed(t)
    );
    println!();

    let t = Instant::now();
    println!("{}", stage_header(7, 9, "entity-overlap linking..."));
    let det_links = link::deterministic_links(&mut all_bundles);
    println!(
        "  {} {} deterministic links {}",
        "·".dimmed(),
        det_links,
        stage_elapsed(t)
    );
    println!();

    let t = Instant::now();
    println!("{}", stage_header(8, 9, "LLM link inference..."));
    let llm_links = if let Some(config) = &config {
        link::infer_links(&client, config, &mut all_bundles)
            .await
            .unwrap_or(0)
    } else {
        println!(
            "  {}",
            "skipping LLM link inference; no LLM API key configured.".dimmed()
        );
        0
    };
    println!(
        "  {} {} LLM links inferred {}",
        "·".dimmed(),
        llm_links,
        stage_elapsed(t)
    );
    println!();

    let t = Instant::now();
    println!("{}", stage_header(9, 9, "assembling frontier..."));
    let frontier = project::assemble(
        topic,
        all_bundles,
        papers.len(),
        errors,
        &format!(
            "Compiled from {} papers on '{}'. Source: OpenAlex.",
            papers.len(),
            topic
        ),
    );
    repo::save_to_path(output, &frontier).unwrap_or_else(|e| {
        let json =
            serde_json::to_string_pretty(&frontier).expect("failed to serialize frontier to JSON");
        std::fs::write(output, json).unwrap_or_else(|_| panic!("Failed to write output: {e}"));
    });
    println!("  {} ", stage_elapsed(t));
    println!();
    println!("  {}", "SUMMARY".dimmed());
    println!("  {}", style::tick_row(60));
    println!("  findings:       {}", frontier.stats.findings);
    println!("  links:          {}", frontier.stats.links);
    println!("  replicated:     {}", frontier.stats.replicated);
    println!("  avg confidence: {}", frontier.stats.avg_confidence);
    println!("  gaps:           {}", frontier.stats.gaps);
    println!("  contested:      {}", frontier.stats.contested);
    println!("  output:         {}", output.display());
    println!(
        "  total time:     {:.1}s",
        compile_start.elapsed().as_secs_f64()
    );
    println!();
}

#[allow(clippy::too_many_arguments)]
async fn cmd_ingest(
    frontier: &Path,
    assertion: Option<String>,
    pdf: Option<PathBuf>,
    csv: Option<PathBuf>,
    text: Option<PathBuf>,
    doi: Option<String>,
    dir: Option<PathBuf>,
    backend: Option<&str>,
    assertion_type: String,
    evidence_type: String,
    species: Option<String>,
    method: String,
    confidence_score: f64,
    entities: String,
    direction: Option<String>,
    source: String,
) {
    if let Some(dir_path) = dir {
        let entries = std::fs::read_dir(&dir_path)
            .unwrap_or_else(|e| {
                eprintln!(
                    "{} error reading {}: {e}",
                    style::err_prefix(),
                    dir_path.display()
                );
                std::process::exit(1);
            })
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        for entry in &entries {
            let path = entry.path();
            match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
                "pdf" => {
                    ingest::run_file_ingest(
                        frontier,
                        Some(&path),
                        None,
                        None,
                        None,
                        backend,
                        None,
                        None,
                        None,
                    )
                    .await;
                }
                "csv" | "tsv" => {
                    ingest::run_file_ingest(
                        frontier,
                        None,
                        Some(&path),
                        None,
                        None,
                        backend,
                        None,
                        None,
                        None,
                    )
                    .await;
                }
                "txt" | "md" => {
                    ingest::run_file_ingest(
                        frontier,
                        None,
                        None,
                        Some(&path),
                        None,
                        backend,
                        None,
                        None,
                        None,
                    )
                    .await;
                }
                _ => {}
            }
        }
        println!("{} checked {} files.", style::ok("ok"), entries.len());
        return;
    }

    if pdf.is_some() || csv.is_some() || text.is_some() || doi.is_some() {
        ingest::run_file_ingest(
            frontier,
            pdf.as_deref(),
            csv.as_deref(),
            text.as_deref(),
            doi.as_deref(),
            backend,
            None,
            None,
            None,
        )
        .await;
        return;
    }

    let Some(assertion_text) = assertion else {
        fail("Provide --assertion, --pdf, --csv, --text, --doi, or --dir.");
    };
    if !(0.0..=1.0).contains(&confidence_score) {
        fail("--confidence must be between 0.0 and 1.0");
    }
    let parsed_entities = parse_entities(&entities);
    ingest::run(
        frontier,
        ingest::IngestArgs {
            assertion_text,
            assertion_type,
            evidence_type,
            species,
            method,
            confidence_score,
            entities: parsed_entities,
            direction,
            source,
        },
    );
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
    let confidence_updates = bundle::recompute_all_confidence(&mut frontier.findings);
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
    println!("  {}", "FRONTIER · V0.9.0".dimmed());
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
                println!("  total findings: {}", report.total_findings);
                println!("  signed:         {}", report.signed);
                println!("  unsigned:       {}", report.unsigned);
                println!("  valid:          {}", report.valid);
                println!("  invalid:        {}", report.invalid);
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
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).expect("failed to serialize link.add")
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
    }
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
    println!("  {}", "VELA · BRIDGE · V0.9.0".dimmed());
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
    project::recompute_stats(&mut frontier);
    propagate::print_result(&result, label, &finding_id);
    let out = output.unwrap_or(path);
    repo::save_to_path(out, &frontier).expect("Failed to save frontier");
    println!("  output: {}", out.display());
}

async fn cmd_jats(source: &str, output: &Path, backend: Option<&str>) {
    let start = Instant::now();
    let config = llm::LlmConfig::from_env(backend).unwrap_or_else(|e| fail_return(&e));
    let client = Client::new();
    println!();
    println!("  {}", "VELA · JATS · V0.9.0".dimmed());
    println!("  {}", style::tick_row(60));
    println!("source: {source}");
    println!("backend: {}", config.backend.label());
    println!();

    println!("{}", stage_header(1, 6, "loading JATS XML..."));
    let xml = if source.to_lowercase().starts_with("pmc") {
        jats::fetch_pmc_jats(&client, source)
            .await
            .unwrap_or_else(|e| fail_return(&e))
    } else {
        std::fs::read_to_string(source)
            .unwrap_or_else(|e| fail_return(&format!("Failed to read {source}: {e}")))
    };
    let parsed = jats::parse_jats(&xml).unwrap_or_else(|e| fail_return(&e));
    let paper = jats::jats_to_paper(&parsed);
    let mut bundles = extract::extract_paper(&client, &config, &paper)
        .await
        .unwrap_or_else(|e| fail_return(&e));
    if bundles.is_empty() {
        println!("no findings extracted.");
        return;
    }
    let base_prov = jats::jats_to_provenance(&parsed);
    for bundle in &mut bundles {
        bundle.provenance.doi = base_prov.doi.clone();
        bundle.provenance.pmid = base_prov.pmid.clone();
        bundle.provenance.pmc = base_prov.pmc.clone();
        bundle.provenance.title = base_prov.title.clone();
        bundle.provenance.journal = base_prov.journal.clone();
        bundle.provenance.year = base_prov.year;
        if !base_prov.authors.is_empty() {
            bundle.provenance.authors = base_prov.authors.clone();
        }
    }
    normalize::normalize_findings(&mut bundles);
    link::deterministic_links(&mut bundles);
    let description = format!("Compiled from JATS: {}", parsed.title);
    let frontier = project::assemble(&parsed.title, bundles, 1, 0, &description);
    repo::save_to_path(output, &frontier).unwrap_or_else(|e| fail(&e));
    println!("  findings: {}", frontier.stats.findings);
    println!("  links:    {}", frontier.stats.links);
    println!("  output:   {}", output.display());
    println!("  time:     {:.1}s", start.elapsed().as_secs_f64());
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

fn stage_header(stage: u32, total: u32, label: &str) -> String {
    format!("{} {}", format!("[{stage}/{total}]").dimmed(), label)
}

fn stage_elapsed(start: Instant) -> String {
    format!("({:.1}s)", start.elapsed().as_secs_f64())
        .dimmed()
        .to_string()
}

fn safe_trunc(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn dedupe_findings(all_bundles: &mut Vec<bundle::FindingBundle>) {
    let pre_dedup = all_bundles.len();
    let mut best_by_id = std::collections::HashMap::<String, usize>::new();
    for (idx, bundle) in all_bundles.iter().enumerate() {
        best_by_id
            .entry(bundle.id.clone())
            .and_modify(|existing_idx| {
                if all_bundles[idx].confidence.score > all_bundles[*existing_idx].confidence.score {
                    *existing_idx = idx;
                }
            })
            .or_insert(idx);
    }
    if best_by_id.len() < pre_dedup {
        let mut keep_indices = best_by_id.into_values().collect::<Vec<_>>();
        keep_indices.sort_unstable();
        let deduped = keep_indices
            .into_iter()
            .map(|i| all_bundles[i].clone())
            .collect::<Vec<_>>();
        let removed = pre_dedup - deduped.len();
        *all_bundles = deduped;
        println!(
            "  {} {} duplicate findings removed (kept higher confidence)",
            "->".dimmed(),
            removed
        );
        println!();
    }
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
    "compile",
    "ingest",
    "jats",
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
    "review",
    "note",
    "caveat",
    "revise",
    "reject",
    "history",
    "import-events",
    "retract",
    "propagate",
];

pub fn is_science_subcommand(name: &str) -> bool {
    SCIENCE_SUBCOMMANDS.contains(&name)
}

fn print_strict_help() {
    println!(
        r#"Vela 0.9.0
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
  ingest        Add manual or file-derived findings
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

pub fn run_from_args() {
    style::init();
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(String::as_str) {
        None | Some("-h" | "--help") => {
            print_strict_help();
            return;
        }
        Some("-V" | "--version" | "version") => {
            println!("vela 0.9.0");
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
