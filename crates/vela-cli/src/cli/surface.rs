//! The compact product help and advanced command reference.

pub(crate) fn print_product_help() {
    println!(
        "Vela {}\nVersion control for scientific state.\n",
        env!("CARGO_PKG_VERSION")
    );
    println!("Usage: vela <COMMAND>\n");
    println!("  init       status     next       start");
    println!("  submit     show       why        review");
    println!("  replay     reproduce  log\n");
    println!("Run `vela help advanced` for setup and verification commands.");
}

pub(crate) fn print_advanced_help() {
    print!("{}", advanced_help_text());
}

/// The curated, grouped command reference (`vela help advanced`).
pub(crate) fn advanced_help_text() -> String {
    format!(
        r#"Vela {}
Version control for scientific state.
Agents submit evidence. Verifiers establish scoped results. Authorized
Decisions change Standing. Git preserves and publishes bytes.

Usage:
  vela <COMMAND>

Daily product:
  init          Create a signed, replayable Git-native Frontier
  status        Compact frontier identity, roots, counts, and next action
  next          Ranked Target Offers
  start         Inspect one exact Target and its bounded completion contract
  submit        Retain authenticated producer input for review
  show          One exact object, its root, era, and authority effect
  why           Root-bound explanation of one Claim's Standing
  review        Inspect or perform one exact Proposal lifecycle action
  replay        Replay, signatures, parity, and repository integrity
  reproduce     Re-run stored witnesses with frozen verifiers
  log           Recent signed events or one finding's history

Advanced verification and integration:
  verification  Retain non-authorizing scoped Verification Records

Advanced setup:
  authority     pin an independently published repository trust root
Hidden utility:
  completions   generate shell completion scripts
"#,
        env!("CARGO_PKG_VERSION"),
    )
}
