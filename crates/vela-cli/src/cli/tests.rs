#[cfg(test)]
mod surface_tests {
    //! Pins the released command surface to the clap enum, so the
    //! drift that silently broke `id` and `publish` this cycle (a real
    //! command rejected as "unknown or non-release") can never recur, and
    //! so the curated `help advanced` reference can never omit a command.
    use crate::cli::*;
    use clap::{CommandFactory, Parser};

    /// Building the ~226-node clap tree needs more than a default test
    /// thread's 2 MiB stack (it is fine on the 8 MiB main thread, where the
    /// CLI actually runs), so each test runs its body on a roomy stack.
    fn on_big_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(f)
            .unwrap()
            .join()
            .unwrap();
    }

    fn released_names() -> Vec<String> {
        Cli::command()
            .get_subcommands()
            .map(|c| c.get_name().to_string())
            .collect()
    }

    #[test]
    fn every_clap_subcommand_is_released() {
        on_big_stack(|| {
            for name in released_names() {
                assert!(
                    is_science_subcommand(&name),
                    "clap exposes subcommand `{name}` but is_science_subcommand rejects it \
                     (a RELEASE_DENY entry, or a derivation bug) — it would 404 at dispatch"
                );
            }
        });
    }

    #[test]
    fn every_subcommand_is_documented_in_advanced_help() {
        on_big_stack(|| {
            let help = strict_help_text();
            for name in released_names() {
                // Commands curated out of the menu (HIDDEN_FROM_ADVANCED_HELP) stay
                // callable but are intentionally not listed; the guard applies
                // only to the canonical surface.
                if HIDDEN_FROM_ADVANCED_HELP.contains(&name.as_str()) {
                    continue;
                }
                let listed = help.lines().any(|l| {
                    let t = l.trim_start();
                    t == name || t.starts_with(&format!("{name} "))
                });
                assert!(
                    listed,
                    "subcommand `{name}` is not listed in `vela help advanced` \
                     (strict_help_text) — add a row so the reference stays complete, \
                     or add it to HIDDEN_FROM_ADVANCED_HELP if it is intentionally off-menu"
                );
            }
        });
    }

    /// Every visible command leads its `--help` with an EXAMPLES block
    /// (gh/clig.dev). The block is an `after_long_help` const in
    /// `cli/help_text.rs`; a new verb without one fails here. `policy` is
    /// intercepted before clap and carries its examples in the hand-rolled
    /// typed command tree and carries the same `after_long_help` contract.
    #[test]
    fn every_visible_command_has_examples() {
        on_big_stack(|| {
            let cmd = Cli::command();
            let mut missing = Vec::new();
            for c in cmd.get_subcommands() {
                if c.is_hide_set() {
                    continue; // hidden (completions) is off-menu
                }
                let has = c
                    .get_after_long_help()
                    .or_else(|| c.get_after_help())
                    .map(|s| s.to_string().contains("EXAMPLES"))
                    .unwrap_or(false);
                if !has {
                    missing.push(c.get_name().to_string());
                }
            }
            assert!(
                missing.is_empty(),
                "these visible commands lack an EXAMPLES block (add a const in \
                 cli/help_text.rs and wire `#[command(after_long_help = …)]`): {missing:?}"
            );
        });
    }

    #[test]
    fn policy_has_one_typed_parser_for_every_released_verb() {
        on_big_stack(|| {
            for args in [
                vec!["vela", "policy", "show", ".", "--json"],
                vec!["vela", "policy", "suggest", ".", "--json"],
                vec![
                    "vela",
                    "policy",
                    "draft",
                    "witness-rederivation",
                    ".",
                    "--json",
                ],
                vec!["vela", "policy", "draft", ".", "--from-suggest", "--json"],
                vec!["vela", "policy", "test", ".", "--json"],
                vec![
                    "vela",
                    "policy",
                    "evaluate-proposal",
                    ".",
                    "vpr_example",
                    "--json",
                ],
                vec![
                    "vela",
                    "policy",
                    "decide",
                    ".",
                    "--activate",
                    "vap_example",
                    "--reason",
                    "bounded rules",
                    "--json",
                ],
                vec!["vela", "policy", "sign", ".", "--yes", "--json"],
                vec![
                    "vela", "policy", "revoke", ".", "--reason", "rotate", "--yes", "--json",
                ],
                vec![
                    "vela",
                    "policy",
                    "retire-legacy",
                    ".",
                    "--reason",
                    "retire",
                    "--as",
                    "agent:test",
                    "--json",
                ],
                vec!["vela", "policy", "log", ".", "--json"],
            ] {
                Cli::try_parse_from(&args).unwrap_or_else(|error| {
                    panic!("typed policy parse failed for {args:?}: {error}")
                });
            }
            assert!(
                Cli::try_parse_from([
                    "vela",
                    "policy",
                    "draft",
                    "witness-rederivation",
                    ".",
                    "unexpected-third-operand",
                ])
                .is_err(),
                "draft must not accept more than its typed operand budget"
            );
        });
    }

    /// The v0.900 product surface, guarded in both directions. A dropped
    /// command and an unreviewed addition both fail this test.
    const V0900_VISIBLE: &[&str] = &[
        "actor",
        "agents",
        "artifact",
        "check",
        "ci",
        "config",
        "doctor",
        "finding",
        "frontier",
        "gate",
        "id",
        "init",
        "land",
        "log",
        "migrate",
        "next",
        "policy",
        "proof",
        "reproduce",
        "review",
        "serve",
        "sign",
        "status",
        "work",
    ];
    const V0900_HIDDEN: &[&str] = &["completions"];

    #[test]
    fn v0900_surface_is_exact_both_directions() {
        on_big_stack(|| {
            let cmd = Cli::command();
            let mut visible: Vec<String> = Vec::new();
            let mut hidden: Vec<String> = Vec::new();
            for c in cmd.get_subcommands() {
                if c.is_hide_set() {
                    hidden.push(c.get_name().to_string());
                } else {
                    visible.push(c.get_name().to_string());
                }
            }
            visible.sort();
            hidden.sort();
            let want_visible: Vec<String> = V0900_VISIBLE.iter().map(|s| s.to_string()).collect();
            let want_hidden: Vec<String> = V0900_HIDDEN.iter().map(|s| s.to_string()).collect();
            assert_eq!(
                visible, want_visible,
                "the VISIBLE surface drifted — a removal broke the porcelain, or an \
                 addition must be a deliberate baseline change"
            );
            assert_eq!(
                hidden, want_hidden,
                "the HIDDEN surface drifted — hiding/unhiding is a deliberate act"
            );
        });
    }

    #[test]
    fn retired_verbs_are_not_reachable() {
        on_big_stack(|| {
            for name in [
                "verify",
                "history",
                "accept-batch",
                "normalize",
                "ingest",
                "claim",
                "campaign",
                "lean",
                "attempt",
                "transfer",
                "experiment",
                "registry",
                "publish",
                "clone",
                "workspace",
                "attest",
                "receipt",
                "submit",
                // the v0.738 hard cut: ten verbs retired into the loop
                "inbox",
                "propose",
                "accept",
                "record",
                "pack",
                "attach",
                "queue",
                // the v0.900 product cut: replaced or moved out of core
                "atlas",
                "credit",
                "diff",
                "foundry",
                "hub",
                "proposals",
                "publication",
                "reproduce-external",
                "state",
            ] {
                assert!(
                    !is_science_subcommand(name),
                    "retired verb `{name}` is still reachable — the cut regressed"
                );
            }
        });
    }
}
