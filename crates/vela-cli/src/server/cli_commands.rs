//! CLI command surface — the clap `Commands` enum and its `*Action`
//! subcommand enums, split out of `cli.rs` so the ~5k lines of command
//! definitions live apart from the handler functions and dispatch. Pure
//! data: the handlers and `run_command` dispatch stay in `cli.rs`.
//!
//! ## Flag-naming conventions (one name per concept, no aliases)
//! - **Acting identity** → `--as`, everywhere a command acts under an
//!   identity (land, attach, artifact retirement…).
//!   The value defaults from the configured identity (`vela id`) or
//!   `$VELA_ACTOR_ID`, so the flag is usually omitted entirely.
//!   `--verifier-actor` names a mechanical verifier identity (CI, lean
//!   keypairs) that is never a decision-maker.
//! - **Signing key** → `--key`. Defaults from `vela id`.
//! - **Targets** → `--hub` (a registry/peer base URL the client talks to),
//!   `--to` (a publish/append destination), `--from` (a read source). One
//!   meaning each; do not overload.

use clap::{ArgGroup, Subcommand};
use std::path::PathBuf;

/// One meaning per flag, everywhere (the audit's top finding was
/// semantic drift). These are the canonical help strings, referenced by
/// every variant that carries the flag.
pub(crate) const HELP_AS: &str = "Acting identity for this write (reviewer:<you> or agent:<name>). Optional: defaults to your `vela id`";
pub(crate) const HELP_REQUIRED_AS: &str =
    "Exact acting identity for this write (reviewer:<you> or agent:<name>)";
pub(crate) const HELP_AS_OF: &str = "Answer as of this RFC3339 instant, e.g. 2026-07-02T16:00:00Z";

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Print one key's effective value.
    Get {
        key: String,
        #[arg(long)]
        frontier: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Set a key (user scope by default; --frontier writes the shared,
    /// committed frontier file — allowlisted keys only).
    Set {
        key: String,
        value: String,
        #[arg(long)]
        frontier: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Remove a key from a scope.
    Unset {
        key: String,
        #[arg(long)]
        frontier: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Every key, its effective value, and where it came from.
    List {
        #[arg(long)]
        frontier: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum PolicyAction {
    /// Inspect a frozen Era-0 policy and its historical admissions.
    Show {
        frontier: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Re-evaluate frozen Era-0 policy outcomes without granting authority.
    Test {
        frontier: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Evaluate one pending proposal against the current policy without
    /// mutating the frontier. Intended for CI and diagnostic tooling.
    EvaluateProposal {
        /// Proposal id and optional frontier, in either order.
        #[arg(num_args = 1..=2)]
        operands: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Inspect historical policy-lane admissions, grouped by frozen policy.
    Log {
        frontier: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Is the LOG intact: replay, signatures, hash parity (--strict is
    /// the bar CI and the hub hold a repo to). Checks the record, not
    /// the science — `vela reproduce` re-runs the verifiers themselves.
    #[command(after_long_help = crate::cli::help_text::CHECK)]
    Check {
        /// Frontier JSON file, Vela repo, or proof packet
        source: Option<PathBuf>,
        /// Run schema validation
        #[arg(long)]
        schema: bool,
        /// Run frontier lint checks
        #[arg(long)]
        stats: bool,
        /// Run the Evidence-CI readiness check (source, evidence, condition,
        /// confidence, policy). Folds in the standalone `evidence-ci` verb.
        #[arg(long)]
        evidence: bool,
        /// Run conformance vectors
        #[arg(long)]
        conformance: bool,
        /// Conformance test directory
        #[arg(long, default_value = "conformance")]
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
    /// Diagnose first-user checkout, frontier, proof, and serve readiness.
    #[command(after_long_help = crate::cli::help_text::DOCTOR)]
    Doctor {
        /// Frontier JSON file or Vela repo. Defaults to the release frontier
        /// when run from the repository root.
        frontier: Option<PathBuf>,
        /// Local serve port to check.
        #[arg(long, default_value_t = 3741)]
        port: u16,
        /// Include tool inventory, setup diagnostics, and every suggested command.
        #[arg(long)]
        all: bool,
        /// Output stable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Export and validate a proof packet
    #[command(after_long_help = crate::cli::help_text::PROOF)]
    Proof {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Output proof packet directory
        #[arg(long, short = 'o', default_value = "proof-packet")]
        out: PathBuf,
        /// Proof packet template
        #[arg(long, default_value = "generic")]
        template: String,
        /// Record latest proof packet state back into the input frontier
        #[arg(long)]
        record_proof_state: bool,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
    },
    /// Make this frontier queryable by AI agents: an MCP server any
    /// client (Claude Code, Cursor, …) can attach to, over stdio or
    /// HTTP. Profiles gate what tools exist: read-only (default), or draft
    /// for nonfinalizing writes.
    #[command(after_long_help = crate::cli::help_text::SERVE)]
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
        /// Include first external frontier adoption guidance in --check-tools output
        #[arg(long)]
        adoption: bool,
        /// MCP exposure profile: `read-only` (default) or `draft` (adds only
        /// the non-finalizing `attempt` tool). Human finalization is
        /// unavailable through MCP.
        #[arg(long)]
        profile: Option<String>,
        /// Output stable JSON for --check-tools
        #[arg(long)]
        json: bool,
    },
    /// v0.42: Show what's pending right now — the daily-driver
    /// equivalent of `git status`. One screen: counts, the inbox,
    /// the audit. Read in two seconds.
    #[command(after_long_help = crate::cli::help_text::STATUS)]
    Status {
        frontier: Option<PathBuf>,
        /// Output stable JSON for programmatic callers.
        #[arg(long)]
        json: bool,
    },
    /// v0.42: Recent canonical events in human-readable form. The
    /// `git log` analogue. Default newest-first; cap on count.
    #[command(after_long_help = crate::cli::help_text::LOG)]
    Log {
        frontier: Option<PathBuf>,
        /// A finding id (`vf_…`): show that finding's state-transition
        /// history instead of the frontier-wide event log.
        finding_id: Option<String>,
        /// How many recent events to show (frontier-wide mode).
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Filter to events matching this kind (substring match).
        #[arg(long)]
        kind: Option<String>,
        /// Finding mode: state as of this RFC3339 instant.
        #[arg(long = "as-of", help = HELP_AS_OF)]
        as_of: Option<String>,
        /// Output stable JSON.
        #[arg(long)]
        json: bool,
    },
    /// The advanced historical verification gate: deliverable-grade and
    /// verifier-attachment checks. `vela check` proves the log is internally
    /// consistent; `vela gate` evaluates whether a historical claim earned
    /// its status — ≥2 independent matched
    /// verifier attachments and a surviving adversarial probe, never a
    /// self-reported "verified" string. See `vela_protocol::verifier_attachment`
    /// and `vela_edge::deliverable_grade`.
    #[command(after_long_help = crate::cli::help_text::GATE)]
    Gate {
        #[command(subcommand)]
        action: GateAction,
    },
    /// Retain and inspect non-authorizing Verification Records.
    #[command(after_long_help = crate::cli::help_text::VERIFY)]
    Verification {
        #[command(subcommand)]
        action: VerifyAction,
    },
    /// Generate vendor agent-config adapters from the canonical `VELA.md`
    /// (one source of truth; the adapter files are disposable, regenerable
    /// leaves). `AGENTS.md`, `CLAUDE.md`, `.cursor/rules/vela.mdc`,
    /// `.github/copilot-instructions.md`, and `.mcp.json` regenerate from
    /// VELA.md; the deletion test holds (delete them, sync, they return).
    #[command(after_long_help = crate::cli::help_text::AGENTS)]
    Agents {
        #[command(subcommand)]
        action: AgentsAction,
    },
    /// Is the SCIENCE intact: re-run every stored witness through the
    /// frozen exact verifiers, from scratch — same input, same answer,
    /// any machine. Complements `vela check`, which verifies the log
    /// rather than the results.
    #[command(after_long_help = crate::cli::help_text::REPRODUCE)]
    Reproduce {
        /// A witness JSON file, or a directory (reproduces every
        /// `*.witness.json` under it, or a `witnesses/` subdir).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Reproduce only the immutable artifacts bound to this proposal.
        #[arg(long)]
        proposal: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Inspect or create an agent identity used for bounded producer work.
    #[command(after_long_help = crate::cli::help_text::ID)]
    Id {
        #[command(subcommand)]
        action: IdAction,
    },
    /// Manage the frontier's registered actor identities (Phase M, v0.4)
    #[command(after_long_help = crate::cli::help_text::ACTOR)]
    Actor {
        #[command(subcommand)]
        action: ActorAction,
    },
    /// Initialize the standard repository-authority writer for a fresh Frontier.
    #[command(hide = true)]
    Authority {
        #[command(subcommand)]
        action: AuthorityAction,
    },
    /// Inspect and materialize frontier-level state, including read-only
    /// cross-frontier dependency projections.
    #[command(after_long_help = crate::cli::help_text::FRONTIER)]
    Frontier {
        #[command(subcommand)]
        action: FrontierAction,
    },
    /// Initialize a minimal .vela frontier repository.
    #[command(after_long_help = crate::cli::help_text::INIT)]
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Human-readable frontier name. Required in --json mode.
        #[arg(long)]
        name: Option<String>,
        /// The bounded research question. Required in --json mode.
        #[arg(long)]
        scope: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Inspect or perform one exact Proposal action.
    #[command(after_long_help = crate::cli::help_text::REVIEW)]
    Review {
        #[command(subcommand)]
        action: ReviewAction,
    },
    /// Manage the lifecycle of an exact Proposal without conflating producer
    /// withdrawal with review authority.
    #[command(after_long_help = crate::cli::help_text::PROPOSAL)]
    Proposal {
        #[command(subcommand)]
        action: ProposalAction,
    },
    /// Seal, diagnose, or inspect the optional derived producer target index.
    /// These commands never grant scientific authority.
    #[command(hide = true)]
    TargetIndex {
        #[command(subcommand)]
        action: TargetIndexAction,
    },
    /// Inspect one current Claim or historical Finding-era record.
    #[command(after_long_help = crate::cli::help_text::CLAIM)]
    Claim {
        #[command(subcommand)]
        command: ClaimCommands,
    },
    /// Show one exact Vela object by its stable id.
    #[command(after_long_help = crate::cli::help_text::SHOW)]
    Show {
        /// Frontier repository directory.
        frontier: PathBuf,
        /// A Claim/Finding, Submission, Registration Record, Verification
        /// Record, Proposal, Event, Artifact, or historical record id.
        object_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Explain why one Claim/Finding currently has its derived standing.
    ///
    /// This is a root-bound read projection. It never changes authority.
    #[command(after_long_help = crate::cli::help_text::WHY)]
    Why {
        /// Frontier repository directory.
        frontier: PathBuf,
        /// Current Claim id (`vcl_...`) or historical Finding id (`vf_...`).
        claim_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Manage content-addressed artifacts without changing linked claims
    #[command(after_long_help = crate::cli::help_text::ARTIFACT)]
    Artifact {
        #[command(subcommand)]
        command: ArtifactCommands,
    },
    /// THE offer: ranked open targets with the compounding payload
    /// pre-loaded (premises, banked routes, attempts, dead channels).
    /// `--json` is the agent contract. Take one with `vela start`.
    #[command(after_long_help = crate::cli::help_text::NEXT)]
    Next {
        /// Frontier path. Optional: discovered upward.
        frontier: Option<PathBuf>,
        #[arg(long, default_value_t = 5)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },

    /// Start one bounded Attempt on a target: claim the lease, print the
    /// briefing, and bind the exact starting state.
    #[command(after_long_help = crate::cli::help_text::START)]
    Start {
        /// The target (obligation id, e.g. erdos:617). Omit to list
        /// open Attempts.
        target: Option<String>,
        #[arg(long)]
        frontier: Option<PathBuf>,
        /// Lease seconds (default from work.lease_ttl_seconds config).
        #[arg(long)]
        ttl: Option<u64>,
        /// Release the lease/session instead of opening one.
        #[arg(long)]
        drop: bool,
        /// Why this Attempt is being abandoned. With --drop a
        /// truthful default is used when omitted.
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, help = HELP_AS)]
        r#as: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Register one authenticated Submission and create a pending Proposal.
    /// This producer action cannot create Verification, a Decision, an Event,
    /// or accepted scientific state.
    #[command(after_long_help = crate::cli::help_text::SUBMIT)]
    Submit {
        /// Path to a signed Submission v1. Or author one from an active Attempt
        /// with --claim, --type, --artifact, and --caveat.
        submission: Option<PathBuf>,
        #[arg(long)]
        frontier: Option<PathBuf>,
        #[arg(long)]
        claim: Option<String>,
        /// Scientific claim type.
        #[arg(long = "type")]
        claim_type: Option<String>,
        /// Scope conditions under which the claim is asserted.
        #[arg(long)]
        condition: Vec<String>,
        /// Re-execution class: exact, bounded, approximate, unavailable, or unknown.
        #[arg(long)]
        replayability: Option<String>,
        #[arg(long)]
        artifact: Vec<String>,
        #[arg(long)]
        caveat: Vec<String>,
        /// Producer-reported check, formatted METHOD:OUTCOME. This is not a
        /// Verification Record.
        #[arg(long = "check")]
        producer_check: Vec<String>,
        /// Independent verification required before acceptance.
        #[arg(long = "requires-verification")]
        verification_requirement: Vec<String>,
        /// Full root of the exact target packet executed by this producer.
        #[arg(long, conflicts_with = "submission", requires_all = ["profile_root", "verifier_capsule_root", "result_contract_root"])]
        packet_root: Option<String>,
        /// Full root of the exact producer profile used for this result.
        #[arg(long, conflicts_with = "submission", requires_all = ["packet_root", "verifier_capsule_root", "result_contract_root"])]
        profile_root: Option<String>,
        /// Full root of the exact frozen verifier capsule.
        #[arg(long, conflicts_with = "submission", requires_all = ["packet_root", "profile_root", "result_contract_root"])]
        verifier_capsule_root: Option<String>,
        /// Full root of the exact positive result contract checked by the capsule.
        #[arg(long, conflicts_with = "submission", requires_all = ["packet_root", "profile_root", "verifier_capsule_root"])]
        result_contract_root: Option<String>,
        /// Select the active Attempt explicitly. Required when this actor owns
        /// more than one active Attempt.
        #[arg(long)]
        attempt: Option<String>,
        #[arg(long, help = HELP_AS)]
        r#as: Option<String>,
        /// Publish now: commit locally AND push. Without it, submit commits
        /// locally and you publish deliberately with `git push`.
        #[arg(long)]
        push: bool,
        #[arg(long)]
        json: bool,
    },

    /// Plain configuration: how YOUR tools behave — never what enters
    /// the record (that is `vela policy`) or who you are (`vela id`).
    /// A closed, validated key set with visible origins; frontier scope
    /// is allowlisted and can only narrow, never widen.
    #[command(after_long_help = crate::cli::help_text::CONFIG)]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Read-only inspection of frozen Era-0 policy inputs and admissions.
    /// Repository authority and restricted Cedar own all new writes.
    #[command(after_long_help = crate::cli::help_text::POLICY)]
    Policy {
        #[command(subcommand)]
        action: PolicyAction,
    },

    /// Emit shell completions for bash, zsh, or fish.
    #[command(hide = true)]
    Completions {
        /// bash | zsh | fish
        shell: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum TargetIndexAction {
    /// Derive all seal-owned roots and exact packet digests from a closed
    /// domain candidate. Check is write-free; apply writes only targets.json.
    #[command(group(ArgGroup::new("target_index_seal_mode").required(true).multiple(false).args(["check", "apply"])))]
    Seal {
        frontier: PathBuf,
        #[arg(long)]
        candidate: PathBuf,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Report stale codes and the exact candidate-seal check without changing
    /// roots, packets, or target semantics.
    Repair {
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Inspect the complete index or one exact full target ID. Inspection
    /// never creates an offer or lease.
    Inspect {
        frontier: PathBuf,
        target_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum IdAction {
    /// Create a file-backed agent identity for bounded producer work.
    Create {
        /// Agent handle, e.g. `worker-1`. Becomes `agent:worker-1`.
        #[arg(long)]
        handle: Option<String>,
        /// Required compatibility guard: Vela no longer creates human
        /// signing identities.
        #[arg(long)]
        agent: bool,
        /// Overwrite an existing identity.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show the current agent identity.
    Show {
        #[arg(long)]
        json: bool,
    },
    /// Adopt an existing agent private key.
    #[command(hide = true)]
    Import {
        /// Path to the existing Ed25519 private key (hex seed).
        #[arg(long)]
        key: PathBuf,
        /// Your handle, e.g. `alice`. Defaults to `$USER`.
        #[arg(long)]
        handle: Option<String>,
        #[arg(long, required = true)]
        agent: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Generate a fresh Ed25519 keypair (files only; registers nothing).
    #[command(hide = true)]
    Keygen {
        #[arg(long, default_value = ".vela/keys")]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

/// Experiment-plane receipts (Inevitability Program Phase 0); nested
/// under `vela foundry experiment`.
#[derive(Subcommand)]
pub(crate) enum AgentsAction {
    /// Regenerate the adapter files from VELA.md (idempotent; writes only
    /// what changed).
    Sync {
        /// Worktree root holding VELA.md (default: current directory).
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Check that the adapters are in sync with VELA.md. Exit 1 on drift or
    /// a missing adapter (use in CI).
    Doctor {
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Show which adapters would change on the next `sync`.
    Diff {
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

/// `vela gate` — the verification gate over a claim.
#[derive(Subcommand)]
pub(crate) enum GateAction {
    /// L5 anti-inflation: require a deliverable grade and block
    /// solve-language unless the grade is an actual solve. Exit 1 on a
    /// gate failure (e.g. an `improved_published_bound` whose claim text
    /// says "resolves #647").
    Grade {
        /// The claim text to lint.
        #[arg(long)]
        claim: String,
        /// The deliverable grade (e.g. `improved_published_bound`,
        /// `unconditional_solve`, `new_oeis_term`). Omit to see the
        /// "grade required" failure.
        #[arg(long)]
        grade: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Derive the verification gate status (G1 independence + G2
    /// claim-match + G3 surviving probe + G4 well-formed) for a claim
    /// from a JSON array of verifier attachments. There is no setter:
    /// the status is computed, never stored. Exit 1 unless the gate
    /// derives `verified`.
    Check {
        /// The exact claim text the attachments must be bound to.
        #[arg(long)]
        claim: String,
        /// Path to a JSON array of `VerifierAttachment` objects.
        #[arg(long)]
        attachments: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Print the deliverable-grade taxonomy and verifier-method /
    /// probe-kind vocabularies (the closed sets the gate accepts).
    Vocab {
        #[arg(long)]
        json: bool,
    },
}

/// `vela verification` — durable, non-authorizing verifier evidence.
#[derive(Subcommand)]
pub(crate) enum VerifyAction {
    /// Import one signed, content-addressed Verification Record.
    Import {
        frontier: PathBuf,
        record: PathBuf,
        #[arg(long = "as", help = HELP_REQUIRED_AS)]
        actor: String,
        /// Publish now: commit locally and push.
        #[arg(long)]
        push: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum ActorAction {
    /// List registered actors in a frontier
    List {
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum AuthorityAction {
    /// Bind a fresh Frontier to one loaded OpenSSH-agent Ed25519 identity.
    Init {
        #[arg(default_value = ".")]
        frontier: PathBuf,
        /// Full OpenSSH SHA256 fingerprint, key ID, or raw public-key hex.
        /// Omit only when the agent exposes exactly one Ed25519 identity.
        #[arg(long)]
        key: Option<String>,
        /// Why this repository authority is being established.
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    /// Manage independently distributed repository-authority trust roots.
    Trust {
        #[command(subcommand)]
        action: AuthorityTrustAction,
    },
}

#[derive(Subcommand)]
pub(crate) enum AuthorityTrustAction {
    /// Install one public local pin for the exact sequence-1 authority record.
    Pin {
        #[arg(default_value = ".")]
        frontier: PathBuf,
        /// Full sequence-1 authority-record root from an independent channel.
        #[arg(long)]
        record_root: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum FrontierTrustAction {
    /// Preview or install one out-of-band pin for the first administrator boundary.
    Pin {
        /// Frontier repository directory.
        frontier: PathBuf,
        /// Full content root obtained from a trusted out-of-band source.
        #[arg(long)]
        boundary_root: String,
        /// Exact plan root returned by a prior key-free preview.
        #[arg(long, requires = "confirm_at")]
        confirm_root: Option<String>,
        /// Exact observation time returned by the same preview.
        #[arg(long, requires = "confirm_root")]
        confirm_at: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum FrontierAction {
    /// Manage user-local trust pins for repository administrator boundaries.
    Trust {
        #[command(subcommand)]
        action: FrontierTrustAction,
    },
    /// Retired legacy v0.1 initializer. Use `vela init`.
    #[command(hide = true)]
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
    /// Replay a split frontier repository into frontier.json and vela.lock.
    Materialize {
        /// Frontier repository directory.
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// List exact dependencies. Vela 0.914 has no later dependency-update writer.
    ListDeps {
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Compare two frontier repositories or snapshots.
    Diff {
        left: PathBuf,
        right: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        quiet: bool,
    },
    /// Recover one interrupted path-exact Git publication.
    RecoverPublication {
        #[arg(long)]
        operation: String,
        #[arg(long)]
        frontier: Option<PathBuf>,
        #[arg(long)]
        push: bool,
        #[arg(long)]
        json: bool,
    },
    /// Compact verified completed private recovery journals.
    #[command(hide = true)]
    CompactRecovery {
        /// Frontier repository directory.
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// v0.158: tag the current frontier state as a versioned
    /// release. Writes a content-addressed `vfrr_*` record to
    /// `<frontier-dir>/.vela/releases/<vfrr_*>.json`. Releases
    /// are immutable; the substrate-side equivalent of a paper
    /// edition or software version tag.
    Release {
        /// Frontier path.
        frontier: PathBuf,
        /// Human-readable release name (e.g. `v1.0`, `2026-Q2`,
        /// `pre-print`). Required, non-empty.
        #[arg(long)]
        name: String,
        /// Optional release notes (changelog, scope, attribution).
        #[arg(long)]
        notes: Option<String>,
        /// Optional previous release id to chain. When omitted,
        /// the substrate looks up the latest release in
        /// `<frontier-dir>/.vela/releases/` and chains there.
        #[arg(long)]
        previous: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// v0.158: list every release recorded for a frontier.
    Releases {
        /// Frontier path.
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Audit readiness across strict check, proof, Evidence CI,
    /// health, stats, and review-work queues.
    Audit {
        /// Frontier repo directory or frontier JSON file.
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Rank the frontier's OPEN findings by accumulating structural support —
    /// which open finding is closest to a verifier-run from done. A projection
    /// (advice, never authority), with the popularity baseline and the
    /// inspectable evidence behind each score. Adapts Garg's Frontier Graph
    /// method to a verifier-gated substrate; validated forward by the loop.
    Rank {
        /// Frontier repo directory or frontier JSON file.
        frontier: PathBuf,
        /// How many top candidates to show.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
}

// Claim and artifact nouns stay on the compact current surface.
#[derive(Subcommand)]
pub(crate) enum ClaimCommands {
    /// Read-only projection of one Claim or historical Finding-era record.
    Show {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Current Claim (`vcl_<hex>`) or historical Finding (`vf_<hex>`) id
        claim_id: String,
        /// record | standing | evidence | attribution
        #[arg(long, default_value = "record")]
        view: String,
        /// Emit stable JSON instead of the human view
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum ArtifactCommands {
    /// Propose retiring an accepted artifact. The proposal stays pending until
    /// a human performs one direct action on that exact Proposal.
    Retract {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Content-addressed artifact id (`va_<hex>`)
        artifact_id: String,
        /// Why this artifact should stop carrying active proof-readiness weight
        #[arg(long)]
        reason: String,
        /// Acting identity. Agents may draft; only a human key may accept.
        #[arg(long = "as")]
        actor: String,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum ReviewAction {
    /// List compact proposal summaries. Defaults to pending review.
    List {
        frontier: PathBuf,
        #[arg(long)]
        status: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Show one pending Review Packet or one exact terminal decision record.
    Show {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Diff one exact proposed scientific-state change without entering the
    /// authority path.
    Diff {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Accept exactly one Proposal through repository authority.
    Accept {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    /// Reject exactly one Proposal through repository authority.
    Reject {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    /// Export proposal records from a frontier.
    Export {
        frontier: PathBuf,
        output: PathBuf,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum ProposalAction {
    /// Withdraw the producer's own pending, producer-bound Proposal.
    Withdraw {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long = "as", help = HELP_REQUIRED_AS)]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
}
