//! The compact product help and advanced command reference.

pub(crate) fn print_product_help() {
    println!(
        "Vela {}\nVersion control for scientific state.\n",
        env!("CARGO_PKG_VERSION")
    );
    println!("Usage: vela <COMMAND>\n");
    println!("  init       status     next       start");
    println!("  submit     show       why        review");
    println!("  check      reproduce  log        doctor\n");
    println!("Run `vela help advanced` for setup and verification commands.");
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
  claim         record, standing, evidence, and attribution views
  id            optional file-backed producer identity
  agents        regenerate agent adapters from VELA.md
  config        closed local/frontier configuration

Advanced verification and integration:
  verification  Retain non-authorizing scoped Verification Records

Advanced setup:
  authority     initialize standard repository authority for a fresh Frontier
  target-index  inspect, diagnose, or seal derived producer targets
  repository    verify or perform the one-time current epoch upgrade

Hidden utility:
  completions   generate shell completion scripts
"#,
        env!("CARGO_PKG_VERSION"),
    )
}
