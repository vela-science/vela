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

use clap::Subcommand;
use std::path::PathBuf;

/// One meaning per flag, everywhere (the audit's top finding was
/// semantic drift). These are the canonical help strings, referenced by
/// every variant that carries the flag.
pub(crate) const HELP_KEY: &str = "Path to an Ed25519 private key (hex seed file). Optional: defaults to your `vela id` identity key";
pub(crate) const HELP_AS: &str = "Acting identity for this write (reviewer:<you> or agent:<name>). Optional: defaults to your `vela id`";
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
    /// The active policy: rules, signature state, what it admitted lately.
    Show {
        frontier: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Summarize recent deferred asks and the smallest policy template that
    /// would cover them. This command is read-only and grants no authority.
    Suggest {
        frontier: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Seal a policy from a template (witness-rederivation,
    /// statement-drafts, notes-threshold). Sealed carries NO authority
    /// until `vela policy sign`.
    Draft {
        /// Template followed by an optional frontier, or only the frontier
        /// with --from-suggest.
        #[arg(num_args = 0..=2)]
        operands: Vec<String>,
        /// Seal the exact rules returned by `vela policy suggest`.
        #[arg(long)]
        from_suggest: bool,
        /// Replace an existing SIGNED active policy (deliberate act).
        #[arg(long)]
        replace: bool,
        #[arg(long)]
        json: bool,
    },
    /// Dry-run the active/sealed policy over every pending proposal.
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
    /// Prepare or execute one protected, exact policy-head decision. Without
    /// --confirm-root/--confirm-at this command is key-free and read-only.
    Decide {
        frontier: Option<PathBuf>,
        /// Activate the first signed policy head for this exact policy id.
        #[arg(long, conflicts_with_all = ["rotate", "revoke"])]
        activate: Option<String>,
        /// Rotate the signed policy head to this exact policy id.
        #[arg(long, conflicts_with_all = ["activate", "revoke"])]
        rotate: Option<String>,
        /// Revoke the current signed policy head.
        #[arg(long, conflicts_with_all = ["activate", "rotate"])]
        revoke: bool,
        /// Human-readable rationale bound into the Decision Plan and event.
        #[arg(long)]
        reason: String,
        /// Exact root returned by the key-free preview.
        #[arg(long, requires = "confirm_at")]
        confirm_root: Option<String>,
        /// Exact observation time returned by the key-free preview.
        #[arg(long, requires = "confirm_root")]
        confirm_at: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// THE ceremony: review the sealed policy, one confirm, one key
    /// read — standing policy authority is reassessed. Humans only.
    Sign {
        frontier: Option<PathBuf>,
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        /// Skip the confirm prompt (the policy is still shown).
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Close the lane with one signed causal review; the active signature
    /// loses authority while snapshots retain past admissions.
    Revoke {
        /// Why (recorded next to the revocation).
        #[arg(long)]
        reason: String,
        frontier: Option<PathBuf>,
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
    /// Prepare a pending human-governance proposal that retires an unused
    /// prelaunch policy byte pair which current policy parsing rejects. This
    /// command is keyless; only the existing `vela sign` ceremony can accept.
    RetireLegacy {
        frontier: Option<PathBuf>,
        /// Why these unsupported prelaunch bytes should be retired.
        #[arg(long)]
        reason: String,
        #[arg(long = "as", help = HELP_AS)]
        actor: String,
        #[arg(long)]
        json: bool,
    },
    /// Every policy-lane admission, grouped by policy.
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
    /// for nonfinalizing writes. The public hub serves the same
    /// read surface at hub.constellate.science/mcp with no clone at all.
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
        /// the task-first `work` tool). Human finalization is terminal-only.
        #[arg(long, default_value = "read-only")]
        profile: String,
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
    /// The verification gate: deliverable-grade and verifier-attachment
    /// checks. `vela verify` proves the *log* is what was signed; `vela
    /// gate` proves a *claim* earned its status — ≥2 independent matched
    /// verifier attachments and a surviving adversarial probe, never a
    /// self-reported "verified" string. See `vela_protocol::verifier_attachment`
    /// and `vela_edge::deliverable_grade`.
    #[command(after_long_help = crate::cli::help_text::GATE)]
    Gate {
        #[command(subcommand)]
        action: GateAction,
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
        #[arg(long)]
        json: bool,
    },
    /// Your Vela identity: set up a key once, then land and sign
    /// with no `--key`/`--actor` flags. `vela id create` is the
    /// one-time onboarding step.
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
    /// Inspect, decide, or producer-withdraw one exact proposal.
    #[command(after_long_help = crate::cli::help_text::REVIEW)]
    Review {
        #[command(subcommand)]
        action: ReviewAction,
    },
    /// Migrate a frontier repository format without rewriting canonical state.
    #[command(after_long_help = crate::cli::help_text::MIGRATE)]
    Migrate {
        /// Frontier repository directory.
        frontier: PathBuf,
        /// Target repository format.
        #[arg(long = "to", default_value = "0.900")]
        target_version: String,
        /// Preview the exact migration without writing.
        #[arg(long, conflicts_with = "apply", required_unless_present = "apply")]
        check: bool,
        /// Apply the previewed repository-format migration.
        #[arg(long, conflicts_with = "check", required_unless_present = "check")]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Inspect finding bundles as the core frontier primitive.
    #[command(after_long_help = crate::cli::help_text::FINDING)]
    Finding {
        #[command(subcommand)]
        command: FindingCommands,
    },
    /// Manage content-addressed artifacts without changing linked claims
    #[command(after_long_help = crate::cli::help_text::ARTIFACT)]
    Artifact {
        #[command(subcommand)]
        command: ArtifactCommands,
    },
    /// THE human proposal-decision ceremony: one frontier, one exact
    /// semantic set, one confirm, one key read, then an exact Git
    /// publication attempt. Scripted forms first render a root, then mutate
    /// only with `sign <vpr_id> --yes --confirm-root <sha256:...>
    /// --confirm-at <RFC3339>`. `sign <file>` remains the detached-byte
    /// ceremony. Agents are refused (exit 4).
    #[command(after_long_help = crate::cli::help_text::SIGN)]
    Sign {
        /// A proposal id to preview/decide, or a file path to
        /// sign detached. Omit for the interactive session.
        target: Option<String>,
        /// Frontier path. Optional when exactly one frontier is discoverable.
        #[arg(long)]
        frontier: Option<PathBuf>,
        /// Confirm a scripted single-item decision. Mutation also requires
        /// --confirm-root matching the root rendered by a prior invocation.
        #[arg(long)]
        yes: bool,
        /// Exact Decision Plan root rendered by the prior scripted preview.
        #[arg(long, requires = "target", conflicts_with_all = ["preview", "reset"])]
        confirm_root: Option<String>,
        /// Exact RFC3339 observation instant echoed by the prior scripted
        /// preview; required with --confirm-root to reproduce timestamped bytes.
        /// The pair expires after the documented 15-minute review window.
        #[arg(long, requires = "target", conflicts_with_all = ["preview", "reset"])]
        confirm_at: Option<String>,
        /// Decision reason for scripted accepts.
        #[arg(long)]
        reason: Option<String>,
        /// Discard the saved interactive session (your in-progress
        /// verdicts) and start clean. Use this if a resumed session shows
        /// choices you want to redo.
        #[arg(long)]
        reset: bool,
        /// Read-only Decision Brief page. Never resolves or reads a key.
        #[arg(
            long,
            conflicts_with_all = ["target", "yes", "confirm_root", "confirm_at", "reason", "reset", "sk", "key"]
        )]
        preview: bool,
        /// Opaque continuation returned by a prior --preview --json page.
        #[arg(long, requires = "preview")]
        cursor: Option<String>,
        /// Decision Brief page size (default 25, maximum 100).
        #[arg(long, requires = "preview")]
        limit: Option<usize>,
        /// Hardware touch-to-sign. Accepted but intentionally NOT wired yet —
        /// the recommended path is an OpenPGP/PKCS#11 Ed25519 token (raw
        /// Ed25519, zero verifier change), not FIDO2; see
        /// docs/HARDWARE_SIGNING_PROPOSAL.md. Using it errors with that guidance.
        #[arg(long)]
        sk: bool,
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },

    /// THE offer: ranked open targets with the compounding payload
    /// pre-loaded (premises, banked routes, attempts, dead channels).
    /// `--json` is the agent contract. Take one with `vela work`.
    #[command(after_long_help = crate::cli::help_text::NEXT)]
    Next {
        /// Frontier path. Optional: discovered upward.
        frontier: Option<PathBuf>,
        #[arg(long, default_value_t = 5)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },

    /// Open a work session on a target: claim the lease, print the
    /// briefing, materialize the context bundle. Close with `vela land`.
    #[command(after_long_help = crate::cli::help_text::WORK)]
    Work {
        /// The target (obligation id, e.g. erdos:617). Omit to list
        /// open sessions.
        target: Option<String>,
        #[arg(long)]
        frontier: Option<PathBuf>,
        /// Lease seconds (default from work.lease_ttl_seconds config).
        #[arg(long)]
        ttl: Option<u64>,
        /// Release the lease/session instead of opening one.
        #[arg(long)]
        drop: bool,
        /// Why this lease is being released without landing. With --drop a
        /// truthful default is used when omitted.
        #[arg(long)]
        reason: Option<String>,
        #[arg(long, help = HELP_AS)]
        r#as: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Land a result: record -> propose -> route by the signed policy.
    /// Permit admits canonically (the autonomy lane); Defer parks it in
    /// the human's sign queue; Deny refuses canonical admission. The positional is a
    /// vela.receipt.v1 JSON — the portable contract ANY tool exports.
    #[command(after_long_help = crate::cli::help_text::LAND)]
    Land {
        /// Path to a receipt JSON. Or use --claim/--artifact/--caveat.
        receipt: Option<PathBuf>,
        #[arg(long)]
        frontier: Option<PathBuf>,
        #[arg(long)]
        claim: Option<String>,
        /// Scientific claim type. Required for flag authoring unless an active
        /// work session supplies it unambiguously.
        #[arg(long = "type")]
        claim_type: Option<String>,
        /// Re-execution class: exact, bounded, approximate, unavailable, or unknown.
        #[arg(long)]
        replayability: Option<String>,
        #[arg(long)]
        artifact: Vec<String>,
        #[arg(long)]
        caveat: Vec<String>,
        /// Observable expected before the test was performed. Exactly one of
        /// this flag and --not-applicable is required when authoring a
        /// scientific chain.
        #[arg(long)]
        predicted_observable: Option<String>,
        /// Declare explicitly that this theoretical/source-only result has no
        /// applicable predicted observable.
        #[arg(long)]
        not_applicable: bool,
        /// Test or check that was actually performed.
        #[arg(long)]
        performed_test: Option<String>,
        /// Result observed from the performed test.
        #[arg(long)]
        result: Option<String>,
        /// Ordered artifact/root references supporting the result.
        #[arg(long)]
        evidence: Vec<String>,
        /// Ordered artifact/root references that count against or bound the
        /// result.
        #[arg(long)]
        counterevidence: Vec<String>,
        /// Select the active work target explicitly. Required when this actor
        /// owns more than one active session.
        #[arg(long)]
        work: Option<String>,
        #[arg(long, help = HELP_AS)]
        r#as: Option<String>,
        /// Publish now: commit locally AND push. Without it, land commits
        /// locally and you publish deliberately with `git push`.
        #[arg(long)]
        push: bool,
        #[arg(long)]
        json: bool,
    },

    /// Continuous-integration verbs for a frontier's GitHub Action.
    #[command(after_long_help = crate::cli::help_text::CI)]
    Ci {
        #[command(subcommand)]
        action: CiAction,
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

    /// Standing rules: the ceremony that pays compound interest. A
    /// policy you sign ONCE lets agents land whole classes of gated
    /// work with no per-item key ceremony; everything outside policy
    /// waits in `vela sign`. show / draft / test / sign / revoke / log.
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
pub(crate) enum IdAction {
    /// Move an existing human seed into the local OS-protected signer helper,
    /// or authorize the exact helper/mode after a package update. Enrollment
    /// authenticates once before reading the plaintext source; successful
    /// migration removes that source on macOS, Windows, and Linux.
    Protect {
        #[arg(long)]
        user_presence: bool,
        #[arg(long)]
        remove_source_key: bool,
        /// session: authenticate a bounded signer session; always: reauthenticate every use.
        #[arg(long, default_value = "session", value_parser = ["session", "always"])]
        mode: String,
        #[arg(long)]
        json: bool,
    },
    /// Pin the current `vela` binary's hash (a human, confirm-gated act) so
    /// every ceremony verifies the binary first — the clear-signing invariant.
    /// You rarely need this by hand: the interactive `vela sign` offers to pin
    /// on first run and to re-pin in place when it sees the binary changed.
    /// Use `--status` to inspect the pin.
    PinBinary {
        /// Show the pin state without recording.
        #[arg(long)]
        status: bool,
        /// Skip the confirm prompt.
        #[arg(long)]
        yes: bool,
    },
    /// One-time setup: generate a key, store it, and remember your actor id
    /// After this, `vela land` / `vela sign` need no `--key`/`--actor` flags.
    Create {
        /// Your handle, e.g. `alice`. Becomes `reviewer:alice` (or
        /// `agent:alice` with --agent). Defaults to `$USER`.
        #[arg(long)]
        handle: Option<String>,
        /// Register as an agent identity (`agent:<handle>`) instead of a
        /// human reviewer.
        #[arg(long)]
        agent: bool,
        /// Overwrite an existing identity.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show the current identity (actor id, public key, key path).
    Show {
        #[arg(long)]
        json: bool,
    },
    /// Adopt an existing private key as your identity (e.g. one a
    /// teammate generated, or a key you already use elsewhere).
    Import {
        /// Path to the existing Ed25519 private key (hex seed).
        #[arg(long)]
        key: PathBuf,
        /// Your handle, e.g. `alice`. Defaults to `$USER`.
        #[arg(long)]
        handle: Option<String>,
        #[arg(long)]
        agent: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Generate a fresh Ed25519 keypair (files only; registers nothing).
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

#[derive(Subcommand)]
pub(crate) enum CiAction {
    /// The whole auto-merge decision in one call: which proposals a PR adds,
    /// whether each is `machine_verified` and a genuine beat, and whether the PR
    /// only touched the append-only store. Exit 0 iff the PR may auto-merge, so
    /// an Action is `vela ci verdict … && gh pr merge`.
    Verdict {
        #[arg(long, default_value = ".")]
        frontier: PathBuf,
        /// The base ref the PR merges into (e.g. `origin/main`). CI must fetch
        /// it (actions/checkout with fetch-depth: 0).
        #[arg(long)]
        base: String,
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

#[derive(Subcommand)]
pub(crate) enum ActorAction {
    /// Bootstrap an empty actor registry from the configured local identity.
    /// This is a one-time initialization step, not a general registry writer.
    Add {
        frontier: PathBuf,
        /// v0.43: Optional ORCID identifier for cross-system identity.
        /// Format `0000-0000-0000-000X`. Accepts bare form, URL form
        /// (`https://orcid.org/0000-...`), or `orcid:` prefix.
        #[arg(long)]
        orcid: Option<String>,
        /// v0.51: Optional read-side access clearance.
        /// `public` (default), `restricted`, or `classified`. Higher
        /// clearance permits reading lower-tier objects through
        /// `vela serve`'s actor-aware MCP/HTTP read paths.
        #[arg(long)]
        clearance: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// List registered actors in a frontier
    List {
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Preview or perform the human-only temporal activation ceremony.
    Activate {
        frontier: PathBuf,
        /// Exact pre-registration Git commit.
        #[arg(long)]
        anchor: String,
        /// Actor id to activate. Defaults to the configured identity.
        #[arg(long)]
        actor: Option<String>,
        /// Render the exact key-free preview without reading a private key.
        #[arg(long)]
        preview: bool,
        /// Skip the interactive confirmation after rendering the preview.
        #[arg(long)]
        yes: bool,
        /// Exact root returned by a prior preview. Required with --yes.
        #[arg(long, requires = "yes")]
        confirm_root: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum FrontierAction {
    /// Scaffold a fresh `frontier.json` stub. The result passes
    /// `vela check --strict` immediately and is ready to accept
    /// Receipt v1 work via `vela land`. Prefer `vela init` for new
    /// work: it creates the event-logged `.vela/` repo, and `git push`
    /// publishes to any Hub-configured source repository.
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
    /// List the frontier's declared dependencies.
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

// Finding and artifact nouns stay on the compact 0.9 surface.
#[derive(Subcommand)]
pub(crate) enum FindingCommands {
    /// v0.327: Read-only projection of one finding: assertion,
    /// evidence atoms, conditions, confidence with basis and
    /// actor-classified reviewed-state, typed links, and provenance.
    /// Deep inspection without raw-JSON spelunking.
    Show {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Finding id (`vf_<hex>`)
        finding_id: String,
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
    /// a human decides it through `vela sign`.
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
    /// Show one exact Decision Brief.
    Show {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Preview one exact Decision Brief without entering the signing path.
    Preview {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Prepare or approve exactly one protected human decision.
    Decide {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long, conflicts_with = "reject", required_unless_present = "reject")]
        accept: bool,
        #[arg(long, conflicts_with = "accept", required_unless_present = "accept")]
        reject: bool,
        #[arg(long)]
        reason: String,
        #[arg(long, requires = "confirm_at")]
        confirm_root: Option<String>,
        #[arg(long, requires = "confirm_root")]
        confirm_at: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Withdraw one pending Receipt-bound proposal as its producer.
    Withdraw {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long = "as", help = HELP_AS)]
        actor: String,
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
