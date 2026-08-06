//! CLI command surface. This module contains Clap data only; handlers and
//! dispatch stay in `cli.rs` and `command_handlers.rs`.
//!
//! ## Flag-naming conventions (one name per concept, no aliases)
//! - **Acting identity** → `--as` for producer or verifier evidence.
//!   It may default from `$VELA_ACTOR_ID`; a human Decision never does.
//! - **Frontier** → `--frontier <path>`, accepted by every verb that acts on an
//!   existing Frontier. All of those verbs but two also take it as the leading
//!   positional; `start` and `submit` do not, because their leading positional
//!   is already the Target and the Submission file. Omitted entirely, the
//!   Frontier is discovered upward from the current directory, exactly as
//!   `vela status` does — one resolution behaviour, everywhere. Where a verb
//!   takes both a Frontier and an object the object binds last, so
//!   `vela why <claim>` and `vela why <frontier> <claim>` are the same request.
//!   `crate::cli::frontier_arg` is the single implementation, and it documents
//!   the one tie-break (`log`, whose object is optional too). The test
//!   `every_frontier_verb_accepts_both_spellings` holds this paragraph to the
//!   parsed surface.
//!
//!   Two positionals are deliberately NOT Frontier arguments, take neither
//!   `--frontier` nor discovery, and keep `default_value = "."`:
//!   `init <path>` names a directory to create, which discovery must not
//!   redirect to an enclosing Frontier; and `reproduce <path>` names a
//!   reproduction scope — a witness file, a directory of witnesses, or a
//!   Frontier — so a bare `vela reproduce` means "reproduce what is here",
//!   not "walk up until something replays".

use clap::{ArgGroup, Subcommand};
use std::path::PathBuf;

/// One meaning per flag, everywhere (the audit's top finding was
/// semantic drift). These are the canonical help strings, referenced by
/// every variant that carries the flag.
pub(crate) const HELP_AS: &str =
    "Acting identity for this write (agent:<name>). Optional: defaults to $VELA_ACTOR_ID";
pub(crate) const HELP_REQUIRED_AS: &str =
    "Exact acting identity for this write (agent:<name>, ci:<name>, or verifier:<name>)";
pub(crate) const HELP_AS_OF: &str = "Answer as of this RFC3339 instant, e.g. 2026-07-02T16:00:00Z";
pub(crate) const HELP_JSON: &str = "Output stable JSON for programmatic callers";
/// The one Frontier help string. Every verb that acts on an existing Frontier
/// carries it on both spellings, so `--help` states the same contract wherever
/// the reader lands.
pub(crate) const HELP_FRONTIER: &str =
    "Frontier repository. Optional: discovered upward from the current directory";
/// The positional Frontier on a verb that also takes an object. Stated
/// explicitly because the slot is only the Frontier when both are supplied.
pub(crate) const HELP_FRONTIER_BEFORE_OBJECT: &str = "Frontier repository, when both arguments are given. With one argument that argument is the object and the Frontier is discovered upward";

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Is the LOG intact: replay, signatures, and hash parity. Checks the record, not
    /// the science — `vela reproduce` re-runs the verifiers themselves.
    #[command(after_long_help = crate::cli::help_text::REPLAY)]
    Replay {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER)]
        frontier: Option<PathBuf>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
        /// Output stable JSON
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Show the Frontier's current Standing, review queue, and integrity state.
    #[command(after_long_help = crate::cli::help_text::STATUS)]
    Status {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER)]
        frontier: Option<PathBuf>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
        /// Output stable JSON for programmatic callers.
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// List the Claims this Frontier holds: id, one-line assertion, Standing,
    /// and origin era. The one verb that produces the full `vcl_` ids `show`
    /// and `why` require.
    #[command(after_long_help = crate::cli::help_text::CLAIMS)]
    Claims {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER)]
        frontier: Option<PathBuf>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
        /// Standing filter: accepted, pending_review, or all.
        #[arg(long)]
        status: Option<String>,
        /// Maximum number of Claims to return.
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Opaque continuation cursor from the previous page.
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Recent covered repository-authority events, newest first.
    #[command(after_long_help = crate::cli::help_text::LOG)]
    #[command(override_usage = "vela log [OPTIONS] [FRONTIER] [OBJECT_ID]")]
    Log {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER)]
        frontier: Option<String>,
        /// A full current object id: restrict the log to its covered history.
        /// Given alone, the Frontier is discovered.
        object_id: Option<String>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
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
    #[command(override_usage = "vela show [OPTIONS] [FRONTIER] <OBJECT_ID>")]
    Show {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER_BEFORE_OBJECT)]
        first: Option<String>,
        /// A Claim, Submission, Verification Record, Proposal, Artifact, or
        /// covered authority Event id.
        #[arg(value_name = "OBJECT_ID")]
        second: Option<String>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Explain why one current Claim has its derived standing.
    ///
    /// This is a root-bound read projection. It never changes authority.
    #[command(after_long_help = crate::cli::help_text::WHY)]
    #[command(override_usage = "vela why [OPTIONS] [FRONTIER] <CLAIM_ID>")]
    Why {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER_BEFORE_OBJECT)]
        first: Option<String>,
        /// Current Claim id (`vcl_...`).
        #[arg(value_name = "CLAIM_ID")]
        second: Option<String>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// THE offer: ranked open targets with the compounding payload
    /// pre-loaded (premises, banked routes, attempts, dead channels).
    /// `--json` is the agent contract. Take one with `vela start`.
    #[command(after_long_help = crate::cli::help_text::NEXT)]
    Next {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER)]
        frontier: Option<PathBuf>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
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
        #[arg(long, value_name = "PATH", help = HELP_FRONTIER)]
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
        /// This slot is the Submission, never the Frontier: use --frontier.
        submission: Option<PathBuf>,
        #[arg(long, value_name = "PATH", help = HELP_FRONTIER)]
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
    #[command(override_usage = "vela verification record [OPTIONS] [FRONTIER] <PROPOSAL>")]
    Record {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER_BEFORE_OBJECT)]
        first: Option<String>,
        /// Exact current pending Proposal (`vpr_...`).
        #[arg(value_name = "PROPOSAL")]
        second: Option<String>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
        /// Named verifier profile used for this observation.
        #[arg(long)]
        profile: String,
        /// Frontier-relative method manifest whose exact bytes bind the
        /// Verification environment. It must be tracked, clean, and retained
        /// in the current Git commit.
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
    #[command(override_usage = "vela verification import [OPTIONS] [FRONTIER] <RECORD>")]
    Import {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER_BEFORE_OBJECT)]
        first: Option<String>,
        /// Signed Verification Record JSON to validate and retain.
        #[arg(value_name = "RECORD")]
        second: Option<String>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
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
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER)]
        frontier: Option<PathBuf>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
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
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER)]
        frontier: Option<PathBuf>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// List compact proposal summaries. Defaults to pending review.
    List {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER)]
        frontier: Option<PathBuf>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
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
    #[command(override_usage = "vela review show [OPTIONS] [FRONTIER] <PROPOSAL_ID>")]
    Show {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER_BEFORE_OBJECT)]
        first: Option<String>,
        /// Exact Proposal ID (`vpr_...`).
        #[arg(value_name = "PROPOSAL_ID")]
        second: Option<String>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
        #[arg(long, help = HELP_JSON)]
        json: bool,
    },
    /// Accept exactly one Proposal through repository authority.
    #[command(
        override_usage = "vela review accept [OPTIONS] [FRONTIER] <PROPOSAL_ID> --reason <REASON>"
    )]
    Accept {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER_BEFORE_OBJECT)]
        first: Option<String>,
        /// Exact pending Proposal ID (`vpr_...`).
        #[arg(value_name = "PROPOSAL_ID")]
        second: Option<String>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
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
    #[command(
        override_usage = "vela review reject [OPTIONS] [FRONTIER] <PROPOSAL_ID> --reason <REASON>"
    )]
    Reject {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER_BEFORE_OBJECT)]
        first: Option<String>,
        /// Exact pending Proposal ID (`vpr_...`).
        #[arg(value_name = "PROPOSAL_ID")]
        second: Option<String>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
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
    #[command(
        override_usage = "vela review withdraw [OPTIONS] [FRONTIER] <PROPOSAL_ID> --as <ACTOR> --reason <REASON>"
    )]
    Withdraw {
        #[arg(value_name = "FRONTIER", help = HELP_FRONTIER_BEFORE_OBJECT)]
        first: Option<String>,
        /// Exact still-pending Proposal ID (`vpr_...`).
        #[arg(value_name = "PROPOSAL_ID")]
        second: Option<String>,
        #[arg(long = "frontier", value_name = "PATH", help = HELP_FRONTIER)]
        frontier_flag: Option<PathBuf>,
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
