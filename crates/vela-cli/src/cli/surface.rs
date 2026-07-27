//! The released command surface: the deny/curation lists, the derived
//! `is_science_subcommand` gate, and the curated `vela help advanced`
//! text.

use super::*;

/// Names retired from the core binary in v0.900. They remain here so the
/// dispatch gate cannot accidentally revive a stale alias.
const RELEASE_DENY: &[&str] = &[
    "atlas",
    "credit",
    "diff",
    "foundry",
    "hub",
    "land",
    "proposals",
    "publication",
    "reproduce-external",
    "verify",
];

/// Names omitted from the advanced menu. `completions` remains callable but
/// hidden; the rest are retired names retained for concise migration guidance.
#[cfg(test)]
pub(crate) const HIDDEN_FROM_ADVANCED_HELP: &[&str] = &[
    "atlas",
    "completions",
    "credit",
    "diff",
    "foundry",
    "hub",
    "land",
    "proposals",
    "publication",
    "reproduce-external",
    "verify",
];

/// Whether `name` is a released top-level command the dispatcher will hand
/// to clap. Derived from the clap command tree (`Cli::command()`), not a
/// hand-maintained list, so a newly-added subcommand — or any of its
/// aliases — is accepted the instant it exists. `surface.rs`'s unit tests
/// pin this to the enum so the derivation can never silently drop a
/// command. (Pre-clap intercepts like `claim state` / `proof verify` are
/// matched in `run_from_args` before this gate, so they need no entry.)
/// The released top-level command names + aliases, derived once from the
/// clap tree and memoized. Building the full command tree is not free, so
/// caching keeps `is_science_subcommand` O(1) per dispatch instead of
/// rebuilding ~226 nodes every call.
fn released_command_names() -> &'static std::collections::HashSet<String> {
    use std::sync::OnceLock;
    static NAMES: OnceLock<std::collections::HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        use clap::CommandFactory;
        let mut set = std::collections::HashSet::new();
        for c in Cli::command().get_subcommands() {
            set.insert(c.get_name().to_string());
            for a in c.get_all_aliases() {
                set.insert(a.to_string());
            }
        }
        set
    })
}

pub fn is_science_subcommand(name: &str) -> bool {
    if RELEASE_DENY.contains(&name) {
        return false;
    }
    released_command_names().contains(name)
}

pub(crate) fn print_strict_help() {
    print!("{}", strict_help_text());
}

/// The curated, grouped command reference (`vela help advanced`). Kept
/// hand-curated for legibility — clap's flat alphabetical dump is worse UX —
/// but `mod surface_tests` asserts every released subcommand appears here,
/// so it can never silently omit a newly-added command (the drift the old
/// hand-maintained allowlist suffered, now caught at the help layer too).
pub(crate) fn strict_help_text() -> String {
    let retired_line = RELEASE_DENY.join(", ");
    format!(
        r#"Vela {}
Version control for scientific state.
Agents submit evidence. Verifiers establish scoped results. Authorized
Decisions change Standing. Git preserves and publishes bytes.

Usage:
  vela <COMMAND>

Daily product:
  init          Create a minimal Git-native frontier
  status        Compact frontier identity, roots, counts, and next action
  next          Ranked Target Offers
  start         Start one bounded Attempt against an exact Target
  submit        Register authenticated producer input for review
  show          One exact object, its root, era, and authority effect
  why           Root-bound explanation of one Claim's Standing
  review        Inspect or perform one exact authorized Proposal action
  check         Replay, signatures, parity, and strict signals
  reproduce     Re-run stored witnesses with frozen verifiers
  log           Recent signed events or one finding's history
  doctor        Blockers plus one safe next action

Nouns and setup:
  finding       record, standing, evidence, and attribution views
  artifact      content-addressed evidence lifecycle
  frontier      materialize, compare, recover publication, release, audit
  policy        frozen Era-0 policy inspection and admission history
  proposal      producer lifecycle for one exact pending Proposal
  actor         inspect the frozen Era-0 actor registry
  id            optional file-backed producer identity
  agents        regenerate agent adapters from VELA.md
  config        closed local/frontier configuration

Advanced verification and integration:
  verification  Retain non-authorizing scoped Verification Records
  gate          claim-level verification projections
  proof         proof packet export, verify, and explain
  serve         read-only or nonfinalizing draft MCP/HTTP surface

Advanced setup:
  target-index  inspect, diagnose, or seal derived producer targets

Hidden utility:
  completions   generate shell completion scripts

Retired from the core product: {}
"#,
        env!("CARGO_PKG_VERSION"),
        retired_line,
    )
}
