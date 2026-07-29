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

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Verify the current repository origin and authority boundary.
    #[command(hide = true)]
    Repository {
        #[command(subcommand)]
        action: RepositoryAction,
    },
    /// Is the LOG intact: replay, signatures, hash parity (--strict is
    /// the bar CI and the hub hold a repo to). Checks the record, not
    /// the science — `vela reproduce` re-runs the verifiers themselves.
    #[command(after_long_help = crate::cli::help_text::CHECK)]
    Check {
        /// Current Frontier repository. Defaults to the current directory.
        source: Option<PathBuf>,
        /// Require the complete current repository verification gate.
        ///
        /// Current repositories always fail closed; this flag remains the
        /// explicit publication spelling used by CI and documentation.
        #[arg(long)]
        strict: bool,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
    },
    /// Diagnose the current Frontier and report one recovery action.
    #[command(after_long_help = crate::cli::help_text::DOCTOR)]
    Doctor {
        /// Frontier JSON file or Vela repo. Defaults to the release frontier
        /// when run from the repository root.
        frontier: Option<PathBuf>,
        /// Include tool inventory, setup diagnostics, and every suggested command.
        #[arg(long)]
        all: bool,
        /// Output stable JSON.
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
    /// Recent covered repository-authority events, newest first.
    #[command(after_long_help = crate::cli::help_text::LOG)]
    Log {
        frontier: Option<PathBuf>,
        /// A full current object id: restrict the log to its covered history.
        object_id: Option<String>,
        /// How many recent events to show.
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Filter to events matching this kind (substring match).
        #[arg(long)]
        kind: Option<String>,
        /// Include events no later than this RFC3339 instant.
        #[arg(long = "as-of", help = HELP_AS_OF)]
        as_of: Option<String>,
        /// Output stable JSON.
        #[arg(long)]
        json: bool,
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
    /// `.github/copilot-instructions.md` regenerate from
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
    /// Initialize the standard repository-authority writer for a fresh Frontier.
    #[command(hide = true)]
    Authority {
        #[command(subcommand)]
        action: AuthorityAction,
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
    /// Seal, diagnose, or inspect the optional derived producer target index.
    /// These commands never grant scientific authority.
    #[command(hide = true)]
    TargetIndex {
        #[command(subcommand)]
        action: TargetIndexAction,
    },
    /// Inspect one current Claim.
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
        /// A Claim, Submission, Registration Record, Verification Record,
        /// Proposal, Artifact, or covered authority Event id.
        object_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Explain why one current Claim has its derived standing.
    ///
    /// This is a root-bound read projection. It never changes authority.
    #[command(after_long_help = crate::cli::help_text::WHY)]
    Why {
        /// Frontier repository directory.
        frontier: PathBuf,
        /// Current Claim id (`vcl_...`).
        claim_id: String,
        #[arg(long)]
        json: bool,
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
    #[command(group(
        ArgGroup::new("submission_change")
            .multiple(false)
            .args(["corrects", "supersedes"])
    ))]
    #[command(after_long_help = crate::cli::help_text::SUBMIT)]
    Submit {
        /// Path to a signed Submission v1. Or author a new Claim from an active
        /// Attempt; exact corrections and supersessions may be authored without
        /// inventing a work target.
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
        /// Full accepted Claim ID that this Submission corrects.
        #[arg(long, conflicts_with = "submission", requires = "target_root")]
        corrects: Option<String>,
        /// Full accepted Claim ID that this Submission supersedes.
        #[arg(long, conflicts_with = "submission", requires = "target_root")]
        supersedes: Option<String>,
        /// Full root of the exact accepted Claim named by --corrects or
        /// --supersedes.
        #[arg(long, conflicts_with = "submission", requires = "submission_change")]
        target_root: Option<String>,
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
        /// Select the active Attempt explicitly. Required for add_claim
        /// authoring; optional for an exact correction or supersession.
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
        /// Exact currently installed root when advancing a verified origin pin.
        #[arg(long)]
        previous_record_root: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum RepositoryAction {
    /// Verify one current repository and its authority history.
    Verify {
        #[arg(default_value = ".")]
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

// Claim and artifact nouns stay on the compact current surface.
#[derive(Subcommand)]
pub(crate) enum ClaimCommands {
    /// Read-only projection of one current Claim.
    Show {
        /// Current Frontier repository
        frontier: PathBuf,
        /// Current Claim (`vcl_<hex>`) id
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
}
