//! The compact product help and advanced command reference.
//!
//! Both grids are hand-set text, because the order a reader meets the verbs in
//! is an editorial decision clap cannot make and the one-line glosses are
//! shorter than the `about` strings the parser carries. What they are not
//! allowed to be is a second opinion about which verbs exist: the grids are
//! held to `Cli::command()` by the tests at the bottom of this file, so a verb
//! added, removed, or renamed on the parser fails here until the grid follows.

pub(crate) fn print_product_help() {
    print!("{}", product_help_text());
}

/// The daily-flow grid (`vela`, `vela help`).
pub(crate) fn product_help_text() -> String {
    format!(
        r#"Vela {}
Version control for scientific state.

Usage: vela <COMMAND>

  init       status     claims     next
  start      submit     show       why
  review     replay     reproduce  log

Run `vela help advanced` for setup and verification commands.
"#,
        env!("CARGO_PKG_VERSION")
    )
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
  claims        What the Frontier holds: id, assertion, Standing, origin era
  next          Ranked Target Offers
  start         Inspect one exact Target and its bounded completion contract
  submit        Retain authenticated producer input for review
  show          One exact object, its root, era, and authority effect
  why           Root-bound explanation of one Claim's Standing
  review        Inspect or perform one exact Proposal lifecycle action
  replay        Replay, signatures, parity, and repository integrity
  reproduce     Re-run stored witnesses with frozen verifiers
  log           Recent signed events, or the covered history of one object

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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use clap::CommandFactory;

    /// The verbs clap really accepts, hidden ones included.
    ///
    /// `help` is clap's own and has no dispatch arm, so it is not part of the
    /// surface either grid describes.
    fn parser_verbs() -> BTreeSet<String> {
        super::super::Cli::command()
            .get_subcommands()
            .map(|command| command.get_name().to_string())
            .filter(|name| name != "help")
            .collect()
    }

    /// Every entry in the advanced reference: a two-space-indented line whose
    /// first field is followed by a column gap. `  vela <COMMAND>` under Usage
    /// carries a single space and is not one.
    fn advanced_grid_verbs() -> BTreeSet<String> {
        super::advanced_help_text()
            .lines()
            .filter_map(|line| {
                let entry = line.strip_prefix("  ")?;
                let (name, rest) = entry.split_once(' ')?;
                (!name.is_empty() && rest.starts_with(' ')).then(|| name.to_string())
            })
            .collect()
    }

    /// The compact grid holds names alone, several to a line.
    fn product_grid_verbs() -> BTreeSet<String> {
        super::product_help_text()
            .lines()
            .filter_map(|line| line.strip_prefix("  "))
            .flat_map(str::split_whitespace)
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn the_advanced_reference_lists_exactly_the_verbs_the_parser_accepts() {
        assert_eq!(
            advanced_grid_verbs(),
            parser_verbs(),
            "`vela help advanced` and the clap surface disagree about which verbs exist"
        );
    }

    /// The compact grid is a chosen subset — the daily flow — so it is checked
    /// for membership rather than equality. What it may not do is print a verb
    /// the binary cannot run.
    #[test]
    fn the_compact_grid_offers_only_verbs_the_parser_accepts() {
        let parser = parser_verbs();
        let missing = product_grid_verbs()
            .difference(&parser)
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "`vela help` offers verbs the parser does not accept: {missing:?}"
        );
    }

    /// Both grids are one surface, so the compact one may not quietly become a
    /// place a verb lives that the reference never mentions.
    #[test]
    fn the_compact_grid_is_drawn_from_the_advanced_reference() {
        let advanced = advanced_grid_verbs();
        let orphans = product_grid_verbs()
            .difference(&advanced)
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            orphans.is_empty(),
            "`vela help` offers verbs `vela help advanced` never lists: {orphans:?}"
        );
    }
}
