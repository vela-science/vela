//! CLI command surface — the clap `Commands` enum and its `*Action`
//! subcommand enums, split out of `cli.rs` so the ~5k lines of command
//! definitions live apart from the handler functions and dispatch. Pure
//! data: the handlers and `run_command` dispatch stay in `cli.rs`.
//!
//! ## Flag-naming conventions (one name per concept, no aliases)
//! - **Acting identity** → `--as`, everywhere a command acts under an
//!   identity (accept, review, propose, attach, record, finding verbs…).
//!   The value defaults from the configured identity (`vela id`) or
//!   `$VELA_ACTOR_ID`, so the flag is usually omitted entirely.
//!   `--author` survives ONLY on `finding add`/`supersede` as source
//!   attribution (who authored the claim, not who is acting);
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
    /// Seal a policy from a template (witness-rederivation,
    /// statement-drafts, notes-threshold). Sealed carries NO authority
    /// until `vela policy sign`.
    Draft {
        /// Template name.
        template: String,
        frontier: Option<PathBuf>,
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
    /// THE ceremony: review the sealed policy, one confirm, one key
    /// read — the lane opens. Humans only.
    Sign {
        frontier: Option<PathBuf>,
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        /// Skip the confirm prompt (the policy is still shown).
        #[arg(long)]
        yes: bool,
    },
    /// Close the lane: the active signature loses authority; snapshots
    /// stay for replay verification of past admissions.
    Revoke {
        /// Why (recorded next to the revocation).
        #[arg(long)]
        reason: String,
        frontier: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
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
    Doctor {
        /// Frontier JSON file or Vela repo. Defaults to the release frontier
        /// when run from the repository root.
        frontier: Option<PathBuf>,
        /// Local serve port to check.
        #[arg(long, default_value_t = 3741)]
        port: u16,
        /// Output stable JSON.
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
    /// HTTP. Profiles gate what tools exist: read-only (default) ⊂
    /// draft ⊂ maintainer. The public hub serves the same read surface
    /// at hub.constellate.science/mcp with no clone at all.
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
        /// MCP exposure profile (memo §9.1): `read-only` (default), `draft`
        /// (adds the propose/draft write tools), or `maintainer` (all tools).
        /// Scopes which tools are listed AND executable. Agents should get
        /// read-only unless a human starts a scoped session.
        #[arg(long, default_value = "read-only")]
        profile: String,
        /// Output stable JSON for --check-tools
        #[arg(long)]
        json: bool,
    },
    /// v0.42: Show what's pending right now — the daily-driver
    /// equivalent of `git status`. One screen: counts, the inbox,
    /// the audit. Read in two seconds.
    Status {
        frontier: Option<PathBuf>,
        /// Output stable JSON for programmatic callers.
        #[arg(long)]
        json: bool,
    },
    /// v0.42: Recent canonical events in human-readable form. The
    /// `git log` analogue. Default newest-first; cap on count.
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
    Gate {
        #[command(subcommand)]
        action: GateAction,
    },
    /// Generate vendor agent-config adapters from the canonical `VELA.md`
    /// (one source of truth; the adapter files are disposable, regenerable
    /// leaves). `AGENTS.md`, `CLAUDE.md`, `.cursor/rules/vela.mdc`,
    /// `.github/copilot-instructions.md`, and `.mcp.json` regenerate from
    /// VELA.md; the deletion test holds (delete them, sync, they return).
    Agents {
        #[command(subcommand)]
        action: AgentsAction,
    },
    /// Is the SCIENCE intact: re-run every stored witness through the
    /// frozen exact verifiers, from scratch — same input, same answer,
    /// any machine. Complements `vela check`, which verifies the log
    /// rather than the results.
    Reproduce {
        /// A witness JSON file, or a directory (reproduces every
        /// `*.witness.json` under it, or a `witnesses/` subdir).
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// The derived credit view for a finding: the accountable human author(s) of
    /// record (valid signers only), the disclosed contributors (machines
    /// included), and which agent originated which unit. A pure projection over
    /// signatures + provenance — never signed, never authoritative, and it never
    /// invents an author. A machine holds no key, so it appears only as a
    /// contributor / originator, never as an author.
    Credit {
        /// The finding id (`vf_…`).
        finding_id: String,
        /// Frontier directory.
        #[arg(default_value = ".")]
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// The foundry: one unattended compounding turn (Phase 2). Produce a
    /// candidate with the frozen-verifier campaign, register its witness, and
    /// run it through the exact-lane de-human-gate — produce -> frozen-verify
    /// -> auto-admit -> machine_verified, with no human and no key. Dry-run by
    /// default (previews the gate); `--apply` records the admission.
    Foundry {
        #[command(subcommand)]
        action: FoundryAction,
    },
    /// Your Vela identity: set up a key once, then land and sign
    /// with no `--key`/`--actor`/`--hub` flags. `vela id create` is the
    /// one-time onboarding step.
    Id {
        #[command(subcommand)]
        action: IdAction,
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
    /// The index: bind a frontier's git remote once (register-git),
    /// then `git push` is publication. Verification verbs
    /// (witness-check, verify-chain, verify-log) hold hubs honest.
    Hub {
        #[command(subcommand)]
        action: HubAction,
    },
    /// Initialize a .vela frontier repo
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "unnamed")]
        name: String,
        #[arg(long, default_value = "default")]
        template: String,
        #[arg(long)]
        no_git: bool,
        #[arg(long)]
        json: bool,
    },
    /// Compare two frontiers, or preview one pending proposal
    /// against the current frontier.
    ///
    /// v0.74: when the first positional arg starts with `vpr_`,
    /// route to the existing `proposals preview` path so a single
    /// `vela diff <proposal_id>` shows the proposal-vs-frontier
    /// delta the README quotes. The two-arg form
    /// (`vela diff <frontier_a> <frontier_b>`) keeps its existing
    /// behavior.
    Diff {
        /// Frontier path A, a `vpr_*` proposal id for preview
        /// mode, or a `vfr_*` registry id (v0.140) resolved via
        /// the registry into a pulled snapshot before diffing.
        target: String,
        /// Frontier path B for two-frontier compare. Accepts a
        /// filesystem path or a `vfr_*` registry id (v0.140). Omit
        /// when `target` is a proposal id.
        frontier_b: Option<String>,
        /// Frontier root for proposal-preview mode. Defaults to
        /// `.` if the first positional is a proposal id and no
        /// `--frontier` flag is provided.
        #[arg(long)]
        frontier: Option<PathBuf>,
        /// Reviewer attribution for the proposal-preview mode.
        #[arg(long, default_value = "reviewer:preview")]
        reviewer: String,
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
    /// THE human ceremony: one interactive session over everything
    /// that awaits your key — deferred decisions, re-sign hygiene,
    /// unsigned governance artifacts — one confirm, one key read,
    /// self-publishes. Scripted forms: `sign <vpr_id> --yes`,
    /// `sign --batch <verdicts.json>`, `sign <file>` (detached bytes).
    /// Inside a frontier: that frontier. Outside: every registered
    /// frontier, one session. Agents are refused (exit 4) — this verb
    /// IS the human boundary.
    Sign {
        /// A proposal id to decide (with --yes), or a file path to
        /// sign detached. Omit for the interactive session.
        target: Option<String>,
        /// Frontier path. Optional: discovered upward, or all
        /// registered frontiers when outside one.
        #[arg(long)]
        frontier: Option<PathBuf>,
        /// Scripted single-item accept (no session).
        #[arg(long)]
        yes: bool,
        /// Decision reason for scripted accepts.
        #[arg(long)]
        reason: Option<String>,
        /// A pre-filled verdicts JSON (the fidelity batch contract):
        /// signs every row under one key read.
        #[arg(long)]
        batch: Option<PathBuf>,
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
        #[arg(long, help = HELP_AS)]
        r#as: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Land a result: record -> propose -> route by the signed policy.
    /// Permit admits canonically (the autonomy lane); Defer parks it in
    /// the human's sign queue; Deny lands nothing. The positional is a
    /// vela.receipt.v1 JSON — the portable contract ANY tool exports.
    Land {
        /// Path to a receipt JSON. Or use --claim/--artifact/--caveat.
        receipt: Option<PathBuf>,
        #[arg(long)]
        frontier: Option<PathBuf>,
        #[arg(long)]
        claim: Option<String>,
        #[arg(long)]
        artifact: Vec<String>,
        #[arg(long)]
        caveat: Vec<String>,
        #[arg(long, help = HELP_AS)]
        r#as: Option<String>,
        /// Publish now: commit locally AND push. Without it, land commits
        /// locally and you publish deliberately with `git push`.
        #[arg(long)]
        push: bool,
        #[arg(long)]
        json: bool,
    },

    /// Submit a frozen-verified witness in ONE step: verify it, land it, bind it
    /// to its finding, drive the exact lane to `machine_verified`, and
    /// materialize. The producer path — no scripts, no manual gate steps.
    Submit {
        /// Path to the witness JSON.
        witness: PathBuf,
        #[arg(long)]
        frontier: Option<PathBuf>,
        #[arg(long, help = HELP_AS)]
        r#as: Option<String>,
        /// Publish now: commit locally AND push. Default: commit locally only.
        #[arg(long)]
        push: bool,
        /// Verify + preview the whole submission; write nothing.
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },

    /// Continuous-integration verbs for a frontier's GitHub Action.
    Ci {
        #[command(subcommand)]
        action: CiAction,
    },

    /// Plain configuration: how YOUR tools behave — never what enters
    /// the record (that is `vela policy`) or who you are (`vela id`).
    /// A closed, validated key set with visible origins; frontier scope
    /// is allowlisted and can only narrow, never widen.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Standing rules: the ceremony that pays compound interest. A
    /// policy you sign ONCE lets agents land whole classes of gated
    /// work with no per-item key ceremony; everything outside policy
    /// waits in `vela sign`. show / draft / test / sign / revoke / log.
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
    /// and default hub. After this, `vela land` / `vela sign` need no
    /// `--key`/`--actor`/`--hub` flags.
    Create {
        /// Your handle, e.g. `alice`. Becomes `reviewer:alice` (or
        /// `agent:alice` with --agent). Defaults to `$USER`.
        #[arg(long)]
        handle: Option<String>,
        /// Register as an agent identity (`agent:<handle>`) instead of a
        /// human reviewer.
        #[arg(long)]
        agent: bool,
        /// Default hub base URL for publish/propose/verify.
        #[arg(long)]
        hub: Option<String>,
        /// Overwrite an existing identity.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show the current identity (actor id, public key, key path, hub).
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
        hub: Option<String>,
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
pub(crate) enum ExperimentAction {
    /// Assemble a content-addressed run-manifest over an experiment's `vac_`
    /// activity turns (ordered, immutable, complete) so a run can be replayed and
    /// no turn can be silently dropped.
    Manifest {
        /// Frontier directory whose `activity/` holds the run's `vac_` envelopes.
        frontier: PathBuf,
        /// Experiment id; filters turns tagged `experiment:<id>`. Use `*` for all.
        #[arg(long, default_value = "*")]
        experiment: String,
        /// Optional path to write the manifest JSON.
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Project the discharge status of a typed cohort (open / discharged /
    /// blocked) over the frontier's accepted findings — mechanical, not asserted.
    Status {
        /// Cohort JSON: an array of obligations, or `{ "obligations": [...] }`.
        cohort: PathBuf,
        /// Frontier directory whose accepted findings discharge obligations.
        frontier: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Author a content-addressed (`vxo_`) cohort obligation from its fields.
    Obligation {
        /// Cohort id this obligation belongs to.
        #[arg(long)]
        cohort: String,
        /// The `vf_` finding id whose acceptance discharges this obligation.
        #[arg(long)]
        target: String,
        /// The exact statement (pins `statement_digest`).
        #[arg(long)]
        statement: String,
        /// Prior accepted judgment ids this obligation depends on (repeatable).
        #[arg(long = "dep")]
        deps: Vec<String>,
        /// How discharge is checked: `lean_kernel` | `vela_verify` | other.
        #[arg(long, default_value = "lean_kernel")]
        discharge_kind: String,
        #[arg(long)]
        json: bool,
    },
}

/// `vela foundry` — one unattended compounding turn over the de-human-gate.
#[derive(Subcommand)]
pub(crate) enum FoundryAction {
    /// Run one turn: produce a candidate (campaign), register its witness, and
    /// run the exact-lane auto-admit. Dry-run by default; `--apply` records the
    /// `policy.auto_admitted` admission when the gate says YES.
    Run {
        /// Frontier directory (e.g. `examples/sidon-sets`).
        frontier: PathBuf,
        /// Witness kind: `sidon`, `golomb`, `cap`, `bh`, …
        #[arg(long)]
        kind: String,
        /// The ambient size parameter `n`.
        #[arg(long)]
        n: usize,
        /// For `bh` witnesses, the order `h`.
        #[arg(long, default_value_t = 2)]
        h: usize,
        /// Secondary order parameter `k` (e.g. `diff_triangle` within-row order J
        /// for a HorizonMath DTS(n,k) target, or `covering`'s block size). Passed
        /// to the campaign only when non-zero.
        #[arg(long, default_value_t = 0)]
        k: usize,
        /// Search restarts.
        #[arg(long, default_value_t = 200)]
        restarts: u64,
        /// Search seed.
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Portfolio size: scan this many consecutive seeds (a diverse-search
        /// portfolio), keep the best-scoring, and propose only that one.
        #[arg(long, default_value_t = 1)]
        seeds: u64,
        /// Gate the turn on the continuous ablation: fail (exit 1) if inherited
        /// frontier state does NOT make this kind compound (treatment <= control).
        #[arg(long)]
        run_ablation: bool,
        /// Record the admission (else dry-run preview the whole turn).
        #[arg(long)]
        apply: bool,
        /// Re-run even if this exact (kind, n, seed, restarts) cell is already in
        /// the attempt ledger. By default the foundry skips a banked cell
        /// (failed-route reuse: don't re-search what a prior turn already did).
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// The foundry's work-list: the attackable target portfolio with its
    /// value-to-beat, read from a substrate-native catalog (the HorizonMath
    /// verifier-attackable subset by default) and cross-referenced against the
    /// live per-family records (e.g. `frontiers/sidon-sets/bounds.json`) so the
    /// gap between the current accepted best and the value-to-beat is legible.
    /// This is what `foundry run` selects from; replaces the web/script JSON
    /// (cohort.json, erdos-wedge.json) as the foundry's portfolio source.
    Targets {
        /// Target catalog (a `HorizonMathCatalog`-shaped JSON with a `problems`
        /// array of `{id, verifier_kind, params, incumbent, status}`).
        #[arg(long, default_value = "frontiers/horizonmath/catalog.json")]
        catalog: PathBuf,
        /// Directory holding live per-family records files (the accepted-best
        /// model, `bounds.json` template). Read to show the current accepted
        /// best against each value-to-beat.
        #[arg(long, default_value = "frontiers")]
        records: PathBuf,
        /// Only show targets a `vela campaign` kind can attack (an engine kind).
        #[arg(long)]
        attackable_only: bool,
        /// Optional typed Erdős bounds sidecar (`examples/erdos-problems/bounds.json`,
        /// the `vela.frontier-bounds.v1` doc emitted by the erdos-deep adapter).
        /// When present, each problem's typed current-best bound is surfaced as a
        /// `value_to_beat` row in the portfolio, so the foundry / attack ranking
        /// sees the Erdős value-to-beat alongside the catalog's incumbents.
        #[arg(long)]
        erdos_bounds: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// The continuous-ablation heartbeat (the plan's hard gate): does inherited
    /// frontier state make the next solver go farther per unit compute? At a
    /// FIXED budget, treatment concentrates it on the boundary (skip-known-work,
    /// enabled by inheriting the frontier's solved targets); control spreads the
    /// same budget across the range it must rediscover. Reports treatment vs
    /// control boundary-success over N seeds; exits 1 if inheritance does not
    /// beat control (so a foundry run can gate on it).
    Ablate {
        /// Frontier directory (its solved targets are the inherited state).
        frontier: PathBuf,
        /// Witness kind to ablate (`sidon`, `golomb`, …).
        #[arg(long)]
        kind: String,
        /// Optional per-family records catalog (`records/<family>.json` or
        /// `bounds.json`): the inherited-state count is read from its accepted,
        /// reproduce-backed bounds instead of the frontier's accepted findings.
        /// Lets the compounding measurement run on a family WITHOUT a key-custody
        /// accept ceremony (the records are already frozen-verified).
        #[arg(long)]
        records: Option<PathBuf>,
        /// The boundary target `n` (the frontier edge being attacked).
        #[arg(long)]
        n: usize,
        /// For `bh`: order `h`.
        #[arg(long, default_value_t = 2)]
        h: usize,
        /// The fixed total search budget (restarts) each arm gets.
        #[arg(long, default_value_t = 200)]
        budget: u64,
        /// Number of seeds to average over.
        #[arg(long, default_value_t = 5)]
        seeds: u64,
        #[arg(long)]
        json: bool,
    },
    /// The prover-in-the-loop work-list: open Lean obligations in a
    /// formal-conjectures corpus, ranked by tractability. Known proved lemmas
    /// compose into proofs of open theorems; this surfaces the tractable
    /// formalization-gap targets (sorry-carrying / `@[category research open]`
    /// decls) the prove loop attacks. Read-only.
    LeanTargets {
        /// The formal-conjectures (or other Lean) corpus root, e.g.
        /// `/Users/.../formal-conjectures`.
        #[arg(long)]
        lean_dir: PathBuf,
        /// Restrict to a sub-path under the corpus (default: the Erdős problems).
        #[arg(long, default_value = "FormalConjectures/ErdosProblems")]
        subdir: String,
        /// Show every open decl, including the headline research-open problems
        /// that are not expected to be subagent-closable (off by default).
        #[arg(long)]
        all: bool,
        /// Cap the number of targets emitted.
        #[arg(long, default_value_t = 40)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// The non-AI verifier half of the prove loop: given a Lean proof already
    /// written into the corpus (the AI producer's output), build it, classify
    /// the target decl's axioms (`#print axioms`, fail-closed on `sorryAx`),
    /// anchor + mint a signed `vlv_`, and — when a frontier is given — draft a
    /// PENDING `verifier.attach`. STOPS there: the truth-bearing accept is a
    /// human key-custody decision (the Lean lane never auto-admits). The Lean
    /// kernel is the trust; the proof's producer is never in the trust path.
    LeanRun {
        /// The Lean corpus root (e.g. the formal-conjectures clone). Its
        /// `lean-toolchain` / `lake-manifest.json` pin the `vlv_`'s provenance.
        #[arg(long)]
        lean_dir: PathBuf,
        /// Module path relative to `--lean-dir`, e.g.
        /// `FormalConjectures/ErdosProblems/828.lean`.
        #[arg(long)]
        module: String,
        /// Fully-qualified decl name for `#print axioms`, e.g.
        /// `Erdos828.erdos_828`.
        #[arg(long)]
        decl: String,
        /// Optional Vela frontier to draft the `verifier.attach` into.
        #[arg(long)]
        frontier: Option<PathBuf>,
        /// The open finding this proof closes (required with `--frontier`).
        #[arg(long)]
        finding: Option<String>,
        /// Reviewer/actor (an `agent:` actor drafts PENDING; a human applies).
        #[arg(long, default_value = "agent:vela-foundry-lean")]
        reviewer: String,
        /// Verifier identity stamped on the `vlv_` (a machine key, not a human).
        #[arg(long, default_value = "agent:vela-foundry-lean")]
        actor: String,
        /// Signing key for the `vlv_` (the verifier's machine key). Resolved
        /// from managed identity when omitted.
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        /// Where to write the `vla_`/`vlv_` artifacts (default: alongside the
        /// frontier under `lean/`, else the current dir).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// The decisive lemma-inheritance measurement (the memo's "Compounding B"):
    /// do accepted Lean lemmas widen the closable boundary? Treatment counts the
    /// open targets that are one-premise-away WITH the inherited lemmas present;
    /// control demotes those lemmas to Open. Δ>0 means inherited verified state
    /// makes the next proof reachable — the formal analogue of skip-known-work.
    LeanAblate {
        /// Frontier directory with Lean findings + inter-problem premise edges.
        frontier: PathBuf,
        /// Explicit inherited-lemma finding ids (comma-separated). Default: every
        /// finding whose assertion_type marks a Lean formalization.
        #[arg(long)]
        lemmas: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Project the typed current-best bounds (value-to-beat) from the erdos-deep
    /// source into a `vela.frontier-bounds.v1` sidecar. ADDITIVE — it reads the
    /// staged source the erdos adapter already ingests and writes a NEW
    /// `bounds.json`; it never touches accepted findings or the frontier
    /// canonical root, so `vela reproduce` is unaffected. Every bound is
    /// unattested (`accepted: false`). Deterministic. `foundry targets
    /// --erdos-bounds <out>` then reads it back as value-to-beat rows.
    ErdosBounds {
        /// The staged erdos-deep source (the `read_erdos_deep` adapter input).
        #[arg(
            long,
            default_value = "examples/erdos-problems/sources/erdos-deep.v1.json"
        )]
        input: PathBuf,
        /// Where to write the typed bounds sidecar.
        #[arg(long, default_value = "examples/erdos-problems/bounds.json")]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// The discovery engine (search -> frozen-verify -> propose).
    Campaign {
        #[command(subcommand)]
        action: CampaignAction,
    },
    /// Lean theorem anchoring + verifier records (vlv_).
    Lean {
        #[command(subcommand)]
        action: LeanAction,
    },
    /// Banked attempts (vat_): verify + list.
    Attempt {
        #[command(subcommand)]
        action: AttemptAction,
    },
    /// Cross-domain transfers (vtr_): verify, mint, registry.
    Transfer {
        #[command(subcommand)]
        action: TransferAction,
    },
    /// Experiment-plane receipts (run manifests, cohort obligations).
    Experiment {
        #[command(subcommand)]
        action: ExperimentAction,
    },
}

/// `vela campaign` — the discovery engine over verifier-gated constructions.
#[derive(Subcommand)]
pub(crate) enum CampaignAction {
    /// Run the engine and report the best verified construction found. Writes
    /// nothing. `--kind` is a verifier kind: gf2_sidon, union_free,
    /// rook_directions, cap, constant_weight (with `--d`/`--w`), covering (with
    /// `--k`/`--t`), sidon, bh (with `--h`), golomb, costas, diff_triangle
    /// (with `--k` as the within-row order J; HorizonMath DTS(I,J) targets).
    Search {
        /// Verifier kind to search.
        kind: String,
        /// Target parameter n (set size domain / order / ground set, kind-dependent).
        #[arg(long)]
        n: usize,
        /// For `bh`: the order h (h=2 is Sidon). Ignored by other kinds.
        #[arg(long, default_value_t = 2)]
        h: usize,
        /// For `constant_weight`: minimum Hamming distance d.
        #[arg(long, default_value_t = 0)]
        d: usize,
        /// For `constant_weight`: codeword weight w.
        #[arg(long, default_value_t = 0)]
        w: usize,
        /// For `covering`: block size k.
        #[arg(long, default_value_t = 0)]
        k: usize,
        /// For `covering`: cover every t-subset.
        #[arg(long, default_value_t = 0)]
        t: usize,
        /// Number of randomized restarts (the work budget).
        #[arg(long, default_value_t = 200)]
        restarts: u64,
        /// RNG seed; the same seed reproduces the same search.
        #[arg(long, default_value_t = 24221)]
        seed: u64,
        #[arg(long)]
        json: bool,
    },
    /// Search, write the verified witness (so `vela reproduce` covers it), and
    /// optionally land a *pending* `finding.add` proposal (no key — a
    /// key-holder accepts).
    Run {
        /// Verifier kind to search (see `search`).
        kind: String,
        #[arg(long)]
        n: usize,
        #[arg(long, default_value_t = 2)]
        h: usize,
        #[arg(long, default_value_t = 0)]
        d: usize,
        #[arg(long, default_value_t = 0)]
        w: usize,
        #[arg(long, default_value_t = 0)]
        k: usize,
        #[arg(long, default_value_t = 0)]
        t: usize,
        #[arg(long, default_value_t = 200)]
        restarts: u64,
        #[arg(long, default_value_t = 24221)]
        seed: u64,
        /// Witness output path. Defaults to
        /// `<frontier>/witnesses/<kind>-n<N>.witness.json`.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Frontier directory (required for `--propose`, or to derive `--out`).
        #[arg(long)]
        frontier: Option<PathBuf>,
        /// Also land a pending `finding.add` proposal for the verified bound.
        #[arg(long)]
        propose: bool,
        /// Reviewer/author identity for the proposal.
        #[arg(long, default_value = "reviewer:will-blair")]
        reviewer: String,
        #[arg(long)]
        json: bool,
    },
}

/// `vela agents` — keep vendor agent-config files generated from `VELA.md`.
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
    /// Backfill frozen-verifier attachments. For each witness artifact in the
    /// frontier, re-run the matching frozen verifier (vela-verify) and, on
    /// pass, land a signed `verifier.attach` recording the check
    /// (ComputationalSearch / vela-verify / Sound). Makes the frozen verifier
    /// legible per finding; the gate still needs >=2 independent attachments to
    /// derive `verified`, so this records the check, it does not flip the gate.
    Backfill {
        /// Frontier directory (e.g. `examples/sidon-sets`).
        frontier: PathBuf,
        /// Reviewer authority landing the attachments (e.g.
        /// `reviewer:will-blair`). Optional: defaults to your configured
        /// identity (`vela id`). A signing key is required to apply.
        #[arg(long = "as", help = HELP_AS)]
        reviewer: Option<String>,
        /// Report the plan without writing.
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Preview the exact-lane auto-admission decision for a finding (Phase 1A,
    /// the de-human-gate). READ-ONLY: it runs the full un-forgeable trust path
    /// over real data — a fresh `vela-verify` re-check of the finding's witness
    /// (reproduce-binding), the frozen `claim_witness_faithful` claim<->witness
    /// binding, and the proposal-level guards + attachment corroboration
    /// predicate — and prints whether the finding WOULD auto-admit to
    /// `machine_verified`, with every guard's verdict. It never writes; the
    /// `policy.auto_admitted` emit is held off pending the acceptance checklist
    /// (see docs/VERIFICATION.md).
    AutoAdmit {
        /// Frontier directory (e.g. `examples/sidon-sets`).
        frontier: PathBuf,
        /// The finding id (`vf_…`) to preview.
        #[arg(long)]
        finding: String,
        /// Record the unsigned `policy.auto_admitted` audit event when (and
        /// only when) the finding WOULD auto-admit. Idempotent: re-running is a
        /// no-op. Never signs, never lands the finding in canonical state. Omit
        /// for a read-only preview.
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Attach an EXTERNAL verifier's output to a finding as a `verifier.attach`.
    /// The first non-frozen source is Inspect-AI (`--from inspect`): an eval
    /// log becomes an `eval_harness` attachment bound to the finding's claim.
    /// Evidence, not a verdict — the attachment is `method_integrity:
    /// unattested` and can NEVER auto-admit; a lone one fails the gate's G1. The
    /// gate (>=2 independent) and the human key still decide.
    Attach {
        /// Frontier directory (e.g. `examples/sidon-sets`).
        frontier: PathBuf,
        /// The finding id (`vf_…`) whose claim the eval checked.
        #[arg(long)]
        finding: String,
        /// The external source. Currently: `inspect` (Inspect-AI eval log).
        #[arg(long, default_value = "inspect")]
        from: String,
        /// Path to the source log (an Inspect `EvalLog` JSON).
        #[arg(long)]
        log: PathBuf,
        /// Headline-metric floor for a passing attachment (exact-match = 1.0).
        #[arg(long, default_value_t = 1.0)]
        threshold: f64,
        /// Reviewer authority landing the attachment. Optional: defaults to your
        /// configured identity (`vela id`). An `agent:` id drafts pending; a
        /// human applies inline (subject to key custody).
        #[arg(long = "as", help = HELP_AS)]
        reviewer: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum ActorAction {
    /// Register an Ed25519 public key for a stable actor identity
    Add {
        frontier: PathBuf,
        /// Stable actor id (e.g. "reviewer:will-blair"). Optional: defaults to
        /// your configured identity (`vela id`).
        id: Option<String>,
        /// Hex-encoded Ed25519 public key (64 hex chars). Optional: defaults to
        /// your configured identity's public key — you should never type it.
        #[arg(long)]
        pubkey: Option<String>,
        /// Optional trust tier (Phase α, v0.6). Currently recognized:
        /// "auto-notes" — permits one-call propose_and_apply_note.
        /// Unknown tier strings load fine but never grant auto-apply.
        #[arg(long)]
        tier: Option<String>,
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
    /// v0.127: Rotate an actor's signing key. Registers a new actor
    /// record under a versioned id, marks the prior actor as revoked
    /// at the current timestamp, and pins a free-form reason. Closes
    /// THREAT_MODEL.md A7 (compromised reviewer key) by giving
    /// reviewers a primitive for retiring a key without inventing
    /// per-frontier ceremony. Historical signatures under the
    /// retired key remain valid (the substrate does not retroactively
    /// invalidate canonical history); new signatures with the retired
    /// key are flagged as `post_revocation_signature` errors by the
    /// signals layer.
    Rotate {
        frontier: PathBuf,
        /// Existing actor id to retire (e.g. `reviewer:will-blair`).
        /// Must be currently registered and not already revoked.
        #[arg(long)]
        id: String,
        /// New actor id to register (e.g.
        /// `reviewer:will-blair-v2-2026-05-10`). Must not collide
        /// with an existing actor id. Convention: append `-v<N>` or
        /// `-v<N>-<date>` to the prior id.
        #[arg(long = "new-id")]
        new_id: String,
        /// New Ed25519 public key (64 hex chars).
        #[arg(long = "new-pubkey")]
        new_pubkey: String,
        /// Free-form reason recorded against the retired actor's
        /// `revoked_reason` field.
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum LeanAction {
    /// Anchor every theorem in the substrate registry. Writes
    /// one `vla_*` anchor JSON per theorem under <output>/.
    AnchorAll {
        /// Path to the `lean/` directory (defaults to repo root).
        #[arg(long)]
        lean_dir: Option<PathBuf>,
        /// Output directory for anchor JSON files. Defaults to
        /// `./theorems/`.
        #[arg(long, default_value = "./theorems")]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Anchor a single theorem by its id (1..=6 in Arc 6 wave 1).
    Anchor {
        /// Theorem id (e.g. 1 for T1).
        id: u32,
        #[arg(long)]
        lean_dir: Option<PathBuf>,
        /// Output path for the anchor record (default: stdout).
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// List the substrate's registered theorems.
    List {
        #[arg(long)]
        json: bool,
    },
    /// v0.170: generate a fresh Ed25519 verifier keypair. Writes
    /// the 32-byte private key (hex) to `--key-out` and the
    /// public-key spec JSON to `--pub-out`.
    Keygen {
        #[arg(long)]
        key_out: PathBuf,
        #[arg(long)]
        pub_out: PathBuf,
        /// Free-form identity to embed in the public-key spec
        /// (e.g. "github-action:constellate-science/vela:verify-lean-bundle").
        #[arg(long = "verifier-actor")]
        actor: String,
    },
    /// v0.170: sign verification records for every anchor in
    /// `--anchors-dir`. Reads `--build-log` and computes its
    /// sha256 as the verifier_output_hash; the lake build that
    /// produced that log must have completed cleanly.
    VerifyAll {
        /// Directory containing T<N>.anchor.json files (default:
        /// `./theorems`).
        #[arg(long, default_value = "./theorems")]
        anchors_dir: PathBuf,
        /// Output directory for T<N>.vlv.json verification records
        /// (default: same as anchors_dir).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Path to a lake build log file. Its sha256 becomes the
        /// verifier_output_hash; the file content is opaque to
        /// the substrate.
        #[arg(long)]
        build_log: PathBuf,
        /// Path to the Ed25519 private key. Optional: defaults to your
        /// configured identity's key (`vela id`).
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        /// Free-form verifier identity (e.g. github-action URL).
        #[arg(long = "verifier-actor")]
        actor: String,
        /// Lean toolchain pin (e.g. `leanprover/lean4:v4.29.1`).
        /// Defaults to the contents of `lean/lean-toolchain` if
        /// present.
        #[arg(long)]
        lean_toolchain: Option<String>,
        /// Mathlib revision (e.g. `v4.29.1`). Defaults to the
        /// `mathlib4.git` pin in `lean/lakefile.lean`.
        #[arg(long)]
        mathlib_revision: Option<String>,
        /// Path to the per-decl axiom report emitted by `Vela/AxiomAudit.lean`
        /// (lines `AXIOMS <decl> | axiom1, axiom2`). When present, each
        /// theorem's axioms are classified against the TCB policy and the
        /// record status is set accordingly. When absent, records are minted
        /// axiom-unknown (legacy behavior).
        #[arg(long)]
        axioms_report: Option<PathBuf>,
        /// Path to the external kernel re-check log (lean4checker/Lean4Lean).
        /// Presence of the marker `KERNEL_RECHECK_FAILED` marks the re-check
        /// failed; an empty/clean log marks it passed; omitting the flag
        /// marks it not-run.
        #[arg(long)]
        kernel_recheck_log: Option<PathBuf>,
        /// External kernel checker name recorded in the TCB policy
        /// (e.g. `lean4checker`). Defaults to `none`.
        #[arg(long, default_value = "none")]
        kernel_checker: String,
        /// External kernel checker version pin (e.g. `lean4checker@v4.29.1`).
        #[arg(long, default_value = "")]
        kernel_checker_version: String,
        /// Comma-separated allowlist of axioms. Defaults to the three
        /// standard classical axioms.
        #[arg(long)]
        allowed_axioms: Option<String>,
        /// Comma-separated forbidden axioms. Defaults to the standard
        /// compiler-trust / `sorry` set.
        #[arg(long)]
        forbidden_axioms: Option<String>,
        /// Output path for the `vtcb_` policy JSON (default:
        /// `<out_dir>/policy.vtcb.json`).
        #[arg(long)]
        out_tcb: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// v0.170: verify a single `vlv_*` record: signature against
    /// declared pubkey + id derivation + anchor cross-check.
    VerifyCheck {
        record: PathBuf,
        /// Path to the matching T<N>.anchor.json. Confirms the
        /// record's anchor_id + module_sha256 still match.
        #[arg(long)]
        anchor: Option<PathBuf>,
        /// Optional path to the `vtcb_` policy JSON. When present,
        /// re-classifies the record's axioms and asserts the stored
        /// `axiom_verdict` and `tcb_id` match.
        #[arg(long)]
        tcb: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum AttemptAction {
    /// Verify a banked attempt file: a single `Attempt` JSON, or a
    /// CanopusAttemptLedger (`{"records": [...]}`, v1 or v2). Each record's
    /// `vat_` id must re-derive, its claim_digest must match, and its Ed25519
    /// signature must verify under the declared pubkey. Unsigned records (no
    /// signature) are reported, not failed.
    Verify {
        /// Path to an Attempt JSON or a ledger with a `records` array.
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// List the banked attempts (`vat_`) in a frontier's event log — the
    /// durable inherited memory (every run's outcome, including failures). The
    /// next portfolio reads this to avoid repeating searched routes. Filter by
    /// `--problem`, `--kind`, or `--status`.
    List {
        /// Frontier directory or repo.
        frontier: PathBuf,
        #[arg(long)]
        problem: Option<u32>,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum TransferAction {
    /// Verify a cross-domain transfer file: a single `Transfer` JSON, or a
    /// `{"records": [...]}` ledger. Each record's `vtr_` id must re-derive and
    /// its Ed25519 signature must verify under the declared pubkey. Unsigned
    /// records are reported, not failed. (This is the structural check; the
    /// T1–T5 admission gate runs in the reducer / `derive_transfer_status`.)
    Verify {
        /// Path to a Transfer JSON or a ledger with a `records` array.
        file: PathBuf,
        /// Re-derive the T1–T5 ADMISSION verdict over real state (the read-time
        /// `derive_transfer_status`), not just the structural signature check.
        /// Resolves A's gate from `--frontier`'s accepted attachments, the
        /// theorem `vlv_` from `--vlv`, and the domain tags.
        #[arg(long)]
        admit: bool,
        /// Source frontier A — its accepted verifier attachments (matching the
        /// transfer's source_claim_digest) resolve A's gate outcome (T1).
        #[arg(long)]
        frontier: Option<PathBuf>,
        /// The transfer theorem's `vlv_` verification file (the LeanHomomorphism
        /// T2 witness). Mint it with `vela foundry lean-run` over the theorem.
        #[arg(long)]
        vlv: Option<PathBuf>,
        /// A's actual domain for the T3 type-match (defaults to the
        /// homomorphism's declared source_type).
        #[arg(long)]
        source_domain: Option<String>,
        /// B's premise domain for the T3 type-match (defaults to the
        /// homomorphism's declared target_type).
        #[arg(long)]
        target_domain: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Mint a signed `vtr_` from a draft JSON (the Transfer body minus
    /// id/signature/signer): source_claim, source_claim_digest, target_claim,
    /// target_premise_digest, homomorphism{...}. Signs with the Ed25519 key
    /// (raw 32-byte hex seed) and writes the content-addressed record.
    Mint {
        /// Path to the draft JSON.
        draft: PathBuf,
        /// Path to the Ed25519 signing key. Optional: defaults to your
        /// configured identity's key (`vela id`).
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        /// Where to write the signed `vtr_` record.
        #[arg(long)]
        out: PathBuf,
    },
    /// Index the cross-domain transfers (`vtr_`) into the transfer registry: a
    /// derived, lane-organized view (certified / target-checked / exploratory)
    /// grouped by domain pair, with each link's proof roots and structural check.
    /// Reads `examples/transfers/*.vtr.json` (or `--dir`); a projection, never a
    /// re-verification or an admission decision.
    Registry {
        /// Directory of `*.vtr.json` transfer records (default examples/transfers).
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Emit the registry as JSON (for the web export) instead of a summary.
        #[arg(long)]
        json: bool,
        /// Write the JSON registry to a file as well.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub(crate) enum FrontierAction {
    /// Scaffold a fresh `frontier.json` stub. The result passes
    /// `vela check --strict` immediately and is ready to accept
    /// findings via `vela finding add`. Prefer `vela init` for new
    /// work: it creates the event-logged `.vela/` repo, and `git push`
    /// is publication (bind once with `vela hub register-git`).
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

#[derive(Subcommand)]
pub(crate) enum HubAction {
    /// Register a frontier's git remote on a hub — the one owner-signed act
    /// in the git-ingestion lane (docs/HUB.md: the hub is an index over
    /// git-replayed state). After this, `git push` IS publication: the hub
    /// re-derives its index from the repo on every ingest sweep, verifying
    /// signatures and hash parity on replay. No further signed publishes.
    RegisterGit {
        /// The frontier to bind (vfr_…)
        vfr_id: String,
        /// The git clone URL (e.g. https://github.com/you/your-frontier.git)
        #[arg(long)]
        remote: String,
        /// Branch or ref the hub ingests
        #[arg(long, default_value = "main")]
        r#ref: String,
        /// Subdirectory holding the frontier (multi-frontier monorepos,
        /// e.g. frontiers/sidon-sets in vela-frontiers). Omit when the
        /// repo root is the frontier.
        #[arg(long, default_value = "")]
        subdir: String,
        /// Hub base URL. Optional: defaults to your configured identity's hub.
        #[arg(long)]
        to: Option<String>,
        /// Path to the owner's Ed25519 private key. Optional: defaults to
        /// your configured identity's key (`vela id`).
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Retire a frontier — the owner-signed deprecation act. When
    /// `--superseded-by` names a successor, the hub redirects the retired
    /// entry's routes to it (a consolidation), so citations still resolve;
    /// without it, the entry is a plain abandonment. Append-only,
    /// earliest-wins; only the entry's owner key may sign it.
    Deprecate {
        /// The frontier to retire (vfr_…).
        vfr_id: String,
        /// The successor frontier (vfr_…) that supersedes it. Omit for a
        /// plain abandonment (no redirect).
        #[arg(long)]
        superseded_by: Option<String>,
        /// Why it is being retired (recorded in the signed deprecation).
        #[arg(long, default_value = "")]
        reason: String,
        /// Hub base URL. Optional: defaults to your configured identity's hub.
        #[arg(long)]
        to: Option<String>,
        /// Path to the owner's Ed25519 private key. Optional: defaults to
        /// your configured identity's key (`vela id`).
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// v0.129: fetch the same registry entry from multiple hubs and
    /// assert byte-identical agreement. Closes part of
    /// THREAT_MODEL.md A11 (compromised hub) by giving operators a
    /// substrate-side cross-hub divergence detector. The
    /// substrate-honest claim: if two or more trustworthy mirrors
    /// agree on the entry's canonical bytes, a third hub's diverging
    /// copy is identifiable.
    WitnessCheck {
        /// Frontier address (`vfr_…`) to fetch from every hub.
        vfr_id: String,
        /// Comma-separated list of hub URLs to query. Requires
        /// at least two; three or more makes the consensus
        /// substrate-honest (a majority can outvote a single
        /// divergent hub).
        #[arg(long, value_delimiter = ',')]
        hubs: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// v0.146: verify a frontier's owner-epoch chain transcript.
    /// Walks each transition, loads the corresponding policy,
    /// proposal, and attestation bundle, and re-runs the v0.145
    /// quorum verification. Surfaces `bootstrap` (chain empty),
    /// `verified` (every transition checks out), `legacy` (no
    /// chain file present; the entry pre-dates v0.144), or
    /// `broken` (at least one transition fails verification).
    VerifyChain {
        /// Frontier path. The chain is read from
        /// `<frontier-dir>/.vela/governance/chain.json`.
        frontier: PathBuf,
        /// Directory holding the `vgp_*.json`, `vop_*.json`,
        /// `vab_*.json` artifacts referenced by the chain. Files
        /// must be named `<id>.json` (e.g. `vop_abc123.json`).
        #[arg(long)]
        artifacts: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Independently verify a hub's RFC 6962 transparency log: fetch the
    /// signed tree head (STH), check its Ed25519 signature against an
    /// externally-pinned pubkey, recompute the Merkle root from the event
    /// content-address preimages, and (with --event) check that event's
    /// inclusion proof. Proves the hub cannot forge or silently drop accepted
    /// state. The Rust sibling of clients/python/vela_verify_log.py.
    VerifyLog {
        /// The frontier (vfr_…) whose log to verify.
        vfr_id: String,
        /// Hub base URL (e.g. https://hub.constellate.science).
        #[arg(long)]
        hub: String,
        /// Optional event id (vev_…) to also prove inclusion of.
        #[arg(long)]
        event: Option<String>,
        /// Expected Ed25519 pubkey (hex), pinned out-of-band. Strongly
        /// recommended; without it the STH's self-advertised key is trusted
        /// (a corruption check only, not authenticity).
        #[arg(long)]
        pubkey: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum LinkAction {
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
pub(crate) enum FindingCommands {
    /// Add a manual finding bundle with an assertion field
    Add {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Assertion text inside the finding bundle
        #[arg(long)]
        assertion: String,
        /// Assertion type. One of: mechanism, observational, computational, theoretical, negative, measurement, exclusion, tension, open_question, hypothesis, candidate_finding
        #[arg(long, default_value = "mechanism")]
        r#type: String,
        /// Source label for the finding
        #[arg(long, default_value = "manual finding")]
        source: String,
        /// Source type. One of: published_paper, preprint, model_output, expert_assertion, database_record, data_release, researcher_notes
        #[arg(long, default_value = "expert_assertion")]
        source_type: String,
        /// Author/reviewer identifier
        #[arg(long)]
        author: String,
        /// Initial confidence score from 0.0 to 1.0
        #[arg(long, default_value = "0.3")]
        confidence: f64,
        /// Evidence type. One of: experimental, observational, computational, theoretical, extracted_from_notes
        #[arg(long, default_value = "theoretical")]
        evidence_type: String,
        /// Evidence span text or JSON. Repeat to attach multiple source spans
        #[arg(long)]
        evidence_span: Vec<String>,
        /// Mark this finding as a candidate gap
        #[arg(long)]
        gap: bool,
        /// Mark this finding as negative-space evidence
        #[arg(long)]
        negative_space: bool,
        /// v0.11: DOI of the source artifact (e.g. "10.1038/s41586-024-...")
        #[arg(long)]
        doi: Option<String>,
        /// v0.11: Publication year
        #[arg(long)]
        year: Option<i32>,
        /// v0.11: Generic source URL when none of the structured identifiers fit
        #[arg(long)]
        url: Option<String>,
        /// v0.11: Source-paper authors as semicolon-separated list (distinct from --author which is the curating Vela actor)
        #[arg(long)]
        source_authors: Option<String>,
        /// v0.11: Conditions/scope text. Replaces the placeholder otherwise written. Should describe scope boundaries (species, dosing, age range, model, etc.)
        #[arg(long)]
        conditions_text: Option<String>,
        /// Output stable JSON
        #[arg(long)]
        json: bool,
        /// Immediately accept and apply the proposal locally
        #[arg(long)]
        apply: bool,
        /// v0.339: path to a replication_attestation JSON (e.g. emitted by
        /// the mechinterp harness for a verified circuit claim). When set,
        /// it rides in the finding.add payload as a sibling of `finding`;
        /// with `--author agent:replicator --apply` the accept gate
        /// auto-accepts the finding iff the attestation passes.
        #[arg(long)]
        replication_attestation: Option<PathBuf>,
    },
    /// v0.327: Read-only projection of one finding: assertion,
    /// evidence atoms, conditions, confidence with basis and
    /// actor-classified reviewed-state, typed links, and provenance.
    /// Deep inspection without raw-JSON spelunking.
    Show {
        /// Frontier JSON file or Vela repo
        frontier: PathBuf,
        /// Finding id (`vf_<hex>`)
        finding_id: String,
        /// Emit stable JSON instead of the human view
        #[arg(long)]
        json: bool,
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
        /// DOI of the source artifact
        #[arg(long)]
        doi: Option<String>,
        /// Publication year
        #[arg(long)]
        year: Option<i32>,
        /// Generic source URL
        #[arg(long)]
        url: Option<String>,
        /// Source-paper authors (semicolon-separated)
        #[arg(long)]
        source_authors: Option<String>,
        /// Conditions/scope text
        #[arg(long)]
        conditions_text: Option<String>,
        json: bool,
        /// Immediately accept and apply the proposal locally
        #[arg(long)]
        apply: bool,
    },
    /// Attach a lightweight note to a finding.
    Note {
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
    /// Attach an explicit caveat to a finding.
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
    /// Revise a finding's confidence interpretation.
    Revise {
        frontier: PathBuf,
        finding_id: String,
        #[arg(long)]
        confidence: f64,
        #[arg(long)]
        reason: String,
        #[arg(long = "as")]
        reviewer: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Mark a finding rejected without deleting it.
    Reject {
        frontier: PathBuf,
        finding_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long = "as")]
        reviewer: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Record a review verdict on a finding. `--status accepted` (the default)
    /// establishes it: it emits a `finding.reviewed` event, and a human
    /// reviewer's accept sets `review_state = Accepted`, which the frontier
    /// state derives to `Established`. Accepts carry no required reason.
    Review {
        frontier: PathBuf,
        finding_id: String,
        /// Verdict: accepted (default) | contested | needs_revision.
        #[arg(long, default_value = "accepted")]
        status: String,
        /// Optional note (accepts default silently; only a downgrade needs one).
        #[arg(long, default_value = "reviewer accepted")]
        reason: String,
        /// Optionally set the finding's confidence in the same command — an
        /// accept at >= 0.6 establishes it (below the fragile floor stays Fragile).
        #[arg(long)]
        confidence: Option<f64>,
        #[arg(long = "as")]
        reviewer: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Record a claim-granularity attribution: which actor originated / derived /
    /// formalized which unit of a finding. Descriptive provenance only — never
    /// touches confidence, review state, or the gate. An agent may draft it; the
    /// one rule is that `--role vouched` must be a human.
    Contribution {
        frontier: PathBuf,
        finding_id: String,
        /// The sub-claim reference: an evidence-span ref, a Lean decl name, or `whole`.
        #[arg(long)]
        unit: String,
        /// evidence_span | lean_decl | step | whole
        #[arg(long = "unit-type", default_value = "whole")]
        unit_type: String,
        /// human | agent | model
        #[arg(long = "agent-kind")]
        agent_kind: String,
        /// ORCID, agent handle, or model id.
        #[arg(long = "agent-id")]
        agent_id: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long = "model-version")]
        model_version: Option<String>,
        /// originated | derived | formalized | extracted | reviewed | vouched
        #[arg(long)]
        role: String,
        /// Free text or an evidence-span reference backing the claim.
        #[arg(long, default_value = "")]
        basis: String,
        #[arg(long = "as")]
        actor: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Retract a finding.
    Retract {
        source: PathBuf,
        finding_id: String,
        #[arg(long)]
        reason: String,
        #[arg(long = "as")]
        reviewer: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        json: bool,
    },
    /// Add typed links between findings.
    Link {
        #[command(subcommand)]
        action: LinkAction,
    },
}

#[derive(Subcommand)]
pub(crate) enum ProposalAction {
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
    /// Preview applying one proposal without mutating the frontier
    Preview {
        frontier: PathBuf,
        proposal_id: String,
        #[arg(long, default_value = "reviewer:preview")]
        reviewer: String,
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
        /// Reviewer actor id. Optional: defaults to your configured identity.
        #[arg(long = "as", help = HELP_AS)]
        reviewer: Option<String>,
        #[arg(long)]
        reason: String,
        /// Path to the reviewer's Ed25519 private key. Optional: defaults to
        /// your configured identity's key.
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Reject one proposal
    Reject {
        /// Leave the signed decision uncommitted (publication stays manual).
        #[arg(long)]
        no_commit: bool,
        /// Publish the commit locally but do not push.
        #[arg(long)]
        no_push: bool,
        frontier: PathBuf,
        proposal_id: String,
        /// Reviewer actor id. Optional: defaults to your configured identity.
        #[arg(long = "as", help = HELP_AS)]
        reviewer: Option<String>,
        #[arg(long)]
        reason: String,
        /// Path to the reviewer's Ed25519 private key. Optional: defaults to
        /// your configured identity's key. A reject is a signed, append-only
        /// event, so key custody is its authority just as for accept.
        #[arg(long, help = HELP_KEY)]
        key: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}
