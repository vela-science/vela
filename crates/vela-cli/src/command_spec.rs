//! CLI command surface. This module contains Clap data only; handlers and
//! dispatch stay in `cli.rs` and `command_handlers.rs`.
//!
//! ## Flag-naming conventions (one name per concept, no aliases)
//! - **Acting identity** → `--as` for producer or verifier evidence.
//!   It may default from `$VELA_ACTOR_ID`; a human Decision never does.
//! - **Frontier** → `--frontier` or the positional Frontier path, according to
//!   the command's ordinary reading order.

use clap::{ArgGroup, Subcommand};
use std::path::PathBuf;

/// One meaning per flag, everywhere (the audit's top finding was
/// semantic drift). These are the canonical help strings, referenced by
/// every variant that carries the flag.
pub(crate) const HELP_AS: &str =
    "Acting identity for this write (agent:<name>). Optional: defaults to $VELA_ACTOR_ID";
pub(crate) const HELP_REQUIRED_AS: &str =
    "Exact acting identity for this write (reviewer:<you> or agent:<name>)";
pub(crate) const HELP_AS_OF: &str = "Answer as of this RFC3339 instant, e.g. 2026-07-02T16:00:00Z";
pub(crate) const HELP_JSON: &str = "Output stable JSON for programmatic callers";

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Is the LOG intact: replay, signatures, and hash parity. Checks the record, not
    /// the science — `vela reproduce` re-runs the verifiers themselves.
    #[command(after_long_help = crate::cli::help_text::REPLAY)]
    Replay {
        /// Current Frontier repository. Defaults to the current directory.
        source: Option<PathBuf>,
        /// Output stable JSON
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Show the Frontier's current Standing, review queue, and integrity state.
    #[command(after_long_help = crate::cli::help_text::STATUS)]
    Status {
        /// Current Frontier repository. Defaults to upward discovery from the current directory.
        frontier: Option<PathBuf>,
        /// Output stable JSON for programmatic callers.
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Recent covered repository-authority events, newest first.
    #[command(after_long_help = crate::cli::help_text::LOG)]
    Log {
        /// Current Frontier repository. Defaults to upward discovery from the current directory.
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
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Retain and inspect non-authorizing Verification Records.
    #[command(after_long_help = crate::cli::help_text::VERIFY)]
    Verification {
        #[command(subcommand)]
        action: VerifyAction,
    },
    /// Is the SCIENCE intact: re-run every stored witness through the
    /// frozen exact verifiers, from scratch — same input, same answer,
    /// any machine. Complements `vela replay`, which verifies the log
    /// rather than the results.
    #[command(after_long_help = crate::cli::help_text::REPRODUCE)]
    Reproduce {
        /// A witness JSON file, or a directory (reproduces every
        /// `*.witness.json` under it, or a `witnesses/` subdir).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Reproduce the proposal's native witness, or locate its rooted
        /// source-local replay without executing repository code.
        #[arg(long)]
        proposal: Option<String>,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Manage independently distributed repository-authority trust roots.
    #[command(hide = true)]
    Authority {
        #[command(subcommand)]
        action: AuthorityAction,
    },
    /// Create a signed, replayable Frontier ready for scientific work.
    #[command(after_long_help = crate::cli::help_text::INIT)]
    Init {
        /// Directory to create as a native Frontier.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Human-readable frontier name. Required in --json mode.
        #[arg(long)]
        name: Option<String>,
        /// The bounded research question. Required in --json mode.
        #[arg(long)]
        scope: Option<String>,
        /// Full OpenSSH SHA256 fingerprint, key ID, or raw public-key hex for
        /// repository authority. Omit when the agent exposes exactly one
        /// Ed25519 identity.
        #[arg(long)]
        key: Option<String>,
        /// Why repository authority is being established.
        #[arg(long, default_value = "Establish repository authority.")]
        reason: String,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Inspect or perform one exact Proposal lifecycle action.
    #[command(after_long_help = crate::cli::help_text::REVIEW)]
    Review {
        #[command(subcommand)]
        action: ReviewAction,
    },
    /// Show one exact Vela object by its stable id.
    #[command(after_long_help = crate::cli::help_text::SHOW)]
    Show {
        /// Frontier repository directory.
        frontier: PathBuf,
        /// A Claim, Submission, Verification Record, Proposal, Artifact, or
        /// covered authority Event id.
        object_id: String,
        #[arg(long, help = HELP_JSON)]
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
        #[arg(long, help = HELP_JSON)]
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
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },

    /// Print a write-free briefing for one exact current Target.
    #[command(after_long_help = crate::cli::help_text::START)]
    Start {
        /// The target (obligation id, e.g. erdos:617).
        target: String,
        #[arg(long)]
        frontier: Option<PathBuf>,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },

    /// Retain one authenticated Submission and create a pending Proposal.
    /// This producer action cannot create Verification, a Decision, an Event,
    /// or accepted scientific state.
    #[command(group(
        ArgGroup::new("submission_change")
            .multiple(false)
            .args(["corrects", "supersedes"])
    ))]
    #[command(after_long_help = crate::cli::help_text::SUBMIT)]
    Submit {
        /// Path to a signed Submission v1. Or author a new Claim directly.
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
        #[arg(long, help = HELP_AS)]
        r#as: Option<String>,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },

    /// Emit shell completions for bash, zsh, or fish.
    #[command(hide = true)]
    Completions {
        /// bash | zsh | fish
        shell: String,
    },
}

/// `vela verification` — durable, non-authorizing verifier evidence.
#[derive(Subcommand)]
pub(crate) enum VerifyAction {
    /// Author, sign, and retain one scoped Verification Record over a current
    /// pending Proposal.
    Record {
        /// Current Frontier repository.
        frontier: PathBuf,
        /// Exact current pending Proposal (`vpr_...`).
        proposal: String,
        /// Named verifier profile used for this observation.
        #[arg(long)]
        profile: String,
        /// Frontier-relative method manifest whose exact bytes bind the
        /// Verification environment.
        #[arg(long)]
        method: PathBuf,
        /// Exact property observed by the verifier. Omit to use the Proposal's
        /// sole registered verification requirement.
        #[arg(long)]
        property: Option<String>,
        /// Retain an observation that does not satisfy a registered
        /// verification requirement.
        #[arg(long, requires = "property")]
        complementary: bool,
        /// pass | fail | error | inconclusive
        #[arg(long)]
        outcome: String,
        /// One explicit limit on what this observation establishes. Repeat for
        /// additional limits.
        #[arg(long = "does-not-establish", required = true)]
        does_not_establish: Vec<String>,
        /// Actor whose work was independently checked. Repeat when applicable.
        #[arg(long = "independent-of")]
        independent_of: Vec<String>,
        /// Dependency shared with the producer. Repeat when applicable.
        #[arg(long = "shared-dependency")]
        shared_dependency: Vec<String>,
        #[arg(long = "as", help = HELP_REQUIRED_AS)]
        actor: String,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Import one signed, content-addressed Verification Record.
    Import {
        /// Current Frontier repository.
        frontier: PathBuf,
        /// Signed Verification Record JSON to validate and retain.
        record: PathBuf,
        #[arg(long = "as", help = HELP_REQUIRED_AS)]
        actor: String,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum AuthorityAction {
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
        /// Current Frontier repository. Defaults to the current directory.
        #[arg(default_value = ".")]
        frontier: PathBuf,
        /// Full sequence-1 authority-record root from an independent channel.
        #[arg(long)]
        record_root: String,
        /// Exact currently installed root when advancing a verified origin pin.
        #[arg(long)]
        previous_record_root: Option<String>,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum ReviewAction {
    /// Derive the current consequence-only Decision Inbox.
    Inbox {
        /// Current Frontier repository.
        frontier: PathBuf,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// List compact proposal summaries. Defaults to pending review.
    List {
        /// Current Frontier repository.
        frontier: PathBuf,
        /// Standing filter: pending_review, accepted, rejected, withdrawn, or all.
        #[arg(long)]
        status: Option<String>,
        /// Maximum number of proposals to return.
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Opaque continuation cursor from the previous page.
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Show one pending Review Packet, Decision, or producer Withdrawal.
    Show {
        /// Current Frontier repository.
        frontier: PathBuf,
        /// Exact Proposal ID (`vpr_...`).
        proposal_id: String,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Accept exactly one Proposal through repository authority.
    Accept {
        /// Current Frontier repository.
        frontier: PathBuf,
        /// Exact pending Proposal ID (`vpr_...`).
        proposal_id: String,
        /// Require the exact Decision Inbox entry that was reviewed.
        #[arg(long)]
        if_entry_root: Option<String>,
        #[arg(long)]
        /// Human reason covered by the Decision signature.
        reason: String,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Reject exactly one Proposal through repository authority.
    Reject {
        /// Current Frontier repository.
        frontier: PathBuf,
        /// Exact pending Proposal ID (`vpr_...`).
        proposal_id: String,
        /// Require the exact Decision Inbox entry that was reviewed.
        #[arg(long)]
        if_entry_root: Option<String>,
        #[arg(long)]
        /// Human reason covered by the Decision signature.
        reason: String,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Withdraw your own still-pending Proposal using its Submission identity.
    Withdraw {
        /// Current Frontier repository.
        frontier: PathBuf,
        /// Exact still-pending Proposal ID (`vpr_...`).
        proposal_id: String,
        /// Exact producer identity that signed the retained Submission.
        #[arg(long = "as")]
        actor: String,
        #[arg(long)]
        /// Producer reason retained with the Withdrawal.
        reason: String,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
}
