//! The compact product help and advanced command reference.
//!
//! Both grids are hand-set text, because the order a reader meets the verbs in
//! is an editorial decision clap cannot make and the one-line glosses are
//! shorter than the `about` strings the parser carries. What they are not
//! allowed to be is a second opinion about which verbs exist: the grids are
//! held to `Cli::command()` by the tests at the bottom of this file, so a verb
//! added, removed, or renamed on the parser fails here until the grid follows.
//!
//! `docs/CLI.md` carries the same surface a third time, as a published
//! reference that `vela-web` vendors and serves. It was bound to nothing, so a
//! renamed verb left the site advertising a command the binary would not run,
//! and nothing said so. Those tests are here too, reaching the same
//! `Cli::command()` — the doc is downstream of the grids, and the grids are
//! downstream of the parser, in one direction with no second opinion anywhere
//! along it.

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

    /// Is this line an entry in the advanced reference — a two-space-indented
    /// line whose first field is followed by a column gap? `  vela <COMMAND>`
    /// under Usage carries a single space and is not one.
    fn advanced_entry_verb(line: &str) -> Option<String> {
        let entry = line.strip_prefix("  ")?;
        let (name, rest) = entry.split_once(' ')?;
        (!name.is_empty() && rest.starts_with(' ')).then(|| name.to_string())
    }

    /// Every entry in the advanced reference.
    fn advanced_grid_verbs() -> BTreeSet<String> {
        super::advanced_help_text()
            .lines()
            .filter_map(advanced_entry_verb)
            .collect()
    }

    /// The advanced reference's entries outside its `Hidden utility:` group.
    ///
    /// This is the published surface, and it is derived rather than listed:
    /// `completions` is a shell-integration utility that the reference groups
    /// as hidden and `docs/CLI.md` does not document, and an allow-list naming
    /// it would be a fourth place the partition is stated. The grouping the
    /// reference already draws is the partition.
    fn documented_verbs() -> BTreeSet<String> {
        let text = super::advanced_help_text();
        let mut group = String::new();
        let mut verbs = BTreeSet::new();
        for line in text.lines() {
            if !line.starts_with(' ') && line.ends_with(':') {
                group = line.to_string();
                continue;
            }
            if group == "Hidden utility:" {
                continue;
            }
            if let Some(verb) = advanced_entry_verb(line) {
                verbs.insert(verb);
            }
        }
        verbs
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

    /// The published reference, read from the repository root.
    fn cli_reference() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/CLI.md");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
    }

    /// The words inside the first fenced `text` block after `anchor`.
    ///
    /// Both verb blocks in `docs/CLI.md` are transcripts — one of what default
    /// help prints, one of the advanced grouping — so they are whitespace-
    /// separated names and nothing else. An anchor that no longer appears is a
    /// failure, not an empty set: a heading rewritten around the block would
    /// otherwise silently switch this check off.
    fn reference_block_verbs(anchor: &str) -> BTreeSet<String> {
        let document = cli_reference();
        let after = document
            .split_once(anchor)
            .unwrap_or_else(|| panic!("docs/CLI.md no longer contains `{anchor}`"))
            .1;
        let body = after
            .split_once("```text\n")
            .unwrap_or_else(|| panic!("docs/CLI.md has no text block after `{anchor}`"))
            .1;
        let body = body
            .split_once("```")
            .unwrap_or_else(|| panic!("docs/CLI.md leaves the block after `{anchor}` unclosed"))
            .0;
        body.split_whitespace().map(str::to_string).collect()
    }

    /// The first column of the daily-command table: `| \`verb\` | gloss |`.
    fn reference_table_verbs() -> BTreeSet<String> {
        cli_reference()
            .lines()
            .filter_map(|line| {
                let cell = line.strip_prefix("| `")?;
                let (name, rest) = cell.split_once('`')?;
                rest.starts_with(" |").then(|| name.to_string())
            })
            .collect()
    }

    /// `docs/CLI.md` says default help exposes *exactly* this list, so it is
    /// held to exactly that and not to membership.
    #[test]
    fn the_reference_quotes_the_compact_grid_the_binary_prints() {
        assert_eq!(
            reference_block_verbs("Default help exposes exactly:"),
            product_grid_verbs(),
            "docs/CLI.md and `vela help` disagree about the daily flow"
        );
    }

    /// The table is the same verbs with their contracts. A verb renamed
    /// in the block and not the table leaves a row describing nothing.
    #[test]
    fn the_reference_table_covers_the_daily_grid_it_prints() {
        assert_eq!(
            reference_table_verbs(),
            reference_block_verbs("Default help exposes exactly:"),
            "docs/CLI.md's daily table and daily grid name different verbs"
        );
    }

    /// The whole published reference, against the whole published surface.
    ///
    /// This is the assertion a rename fails. `vela help advanced` is already
    /// held to the parser above, so binding the document to the reference's
    /// non-hidden groups binds it to `Cli::command()` through one chain rather
    /// than by reading clap a second time.
    #[test]
    fn the_reference_documents_exactly_the_published_verbs() {
        let mut documented = reference_block_verbs("Default help exposes exactly:");
        documented.extend(reference_block_verbs("## Advanced commands"));
        assert_eq!(
            documented,
            documented_verbs(),
            "docs/CLI.md and `vela help advanced` disagree about the published surface"
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
