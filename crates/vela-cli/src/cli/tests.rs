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
    fn policy_parser_exposes_only_frozen_era0_read_verbs() {
        on_big_stack(|| {
            for args in [
                vec!["vela", "policy", "show", ".", "--json"],
                vec!["vela", "policy", "test", ".", "--json"],
                vec![
                    "vela",
                    "policy",
                    "evaluate-proposal",
                    ".",
                    "vpr_example",
                    "--json",
                ],
                vec!["vela", "policy", "log", ".", "--json"],
            ] {
                Cli::try_parse_from(&args).unwrap_or_else(|error| {
                    panic!("typed policy parse failed for {args:?}: {error}")
                });
            }
            for retired in [
                "suggest",
                "draft",
                "decide",
                "sign",
                "revoke",
                "retire-legacy",
            ] {
                assert!(
                    Cli::try_parse_from(["vela", "policy", retired, "."]).is_err(),
                    "retired policy writer {retired} must not parse"
                );
            }
        });
    }

    #[test]
    fn submit_execution_binding_is_all_or_nothing_and_file_exclusive() {
        on_big_stack(|| {
            let roots = [
                "--packet-root",
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "--profile-root",
                "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                "--verifier-capsule-root",
                "sha256:3333333333333333333333333333333333333333333333333333333333333333",
                "--result-contract-root",
                "sha256:4444444444444444444444444444444444444444444444444444444444444444",
            ];
            let mut complete = vec![
                "vela",
                "submit",
                "--claim",
                "bounded result",
                "--type",
                "computational",
                "--replayability",
                "exact",
            ];
            complete.extend(roots);
            assert!(Cli::try_parse_from(complete).is_ok());
            assert!(
                Cli::try_parse_from([
                    "vela",
                    "submit",
                    "--claim",
                    "bounded result",
                    "--type",
                    "computational",
                    "--replayability",
                    "exact",
                    "--packet-root",
                    "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                ])
                .is_err()
            );
            let mut foreign = vec!["vela", "submit", "submission.json"];
            foreign.extend(roots);
            assert!(Cli::try_parse_from(foreign).is_err());
        });
    }

    #[test]
    fn direct_review_actions_are_visible_and_flag_hidden_decide_is_retired() {
        on_big_stack(|| {
            for action in ["accept", "reject"] {
                Cli::try_parse_from([
                    "vela",
                    "review",
                    action,
                    ".",
                    "vpr_0123456789abcdef",
                    "--reason",
                    "exact scoped reason",
                    "--json",
                ])
                .unwrap_or_else(|error| panic!("review {action} must parse: {error}"));
            }
            Cli::try_parse_from([
                "vela",
                "review",
                "diff",
                ".",
                "vpr_0123456789abcdef",
                "--json",
            ])
            .unwrap_or_else(|error| panic!("review diff must parse: {error}"));

            assert!(
                Cli::try_parse_from([
                    "vela",
                    "review",
                    "decide",
                    ".",
                    "vpr_0123456789abcdef",
                    "--accept",
                    "--reason",
                    "legacy path",
                ])
                .is_err(),
                "flag-hidden review decide must not remain a writable alias"
            );
            assert!(
                Cli::try_parse_from(["vela", "review", "preview", ".", "vpr_0123456789abcdef",])
                    .is_err(),
                "review preview must retire in favor of review diff"
            );
            Cli::try_parse_from([
                "vela",
                "proposal",
                "withdraw",
                ".",
                "vpr_0123456789abcdef",
                "--as",
                "agent:producer",
                "--reason",
                "superseded",
            ])
            .unwrap_or_else(|error| panic!("proposal withdraw must parse: {error}"));
            assert!(
                Cli::try_parse_from([
                    "vela",
                    "review",
                    "withdraw",
                    ".",
                    "vpr_0123456789abcdef",
                    "--as",
                    "agent:producer",
                    "--reason",
                    "legacy path",
                ])
                .is_err(),
                "producer withdrawal must not remain under review"
            );
        });
    }

    #[test]
    fn retired_migration_is_absent_and_target_sealing_modes_are_exact() {
        on_big_stack(|| {
            let migrate = [
                "vela",
                "migrate",
                ".",
                "--profile",
                "../frontier-profile.yaml",
                "--target-candidate",
                "../target-index-candidate.json",
                "--as",
                "reviewer:repository-administrator",
                "--reason",
                "bind the exact repository",
            ];
            assert!(Cli::try_parse_from(migrate).is_err());
            for mode in ["--check", "--apply"] {
                let mut args = migrate.to_vec();
                args.push(mode);
                assert!(
                    Cli::try_parse_from(args).is_err(),
                    "retired migration must not parse with {mode}"
                );
            }

            let seal = [
                "vela",
                "target-index",
                "seal",
                ".",
                "--candidate",
                "../target-index-candidate.json",
            ];
            assert!(Cli::try_parse_from(seal).is_err());
            for mode in ["--check", "--apply"] {
                let mut args = seal.to_vec();
                args.push(mode);
                assert!(Cli::try_parse_from(args).is_ok(), "{mode} must parse alone");
            }
            let mut both = seal.to_vec();
            both.extend(["--check", "--apply"]);
            assert!(Cli::try_parse_from(both).is_err());

            assert!(crate::cli::help_text::SERVE.contains("--http 3741"));
            assert!(!crate::cli::help_text::SERVE.contains("hub.constellate.science"));
        });
    }

    /// The v0.930 product surface, guarded in both directions. A dropped
    /// command and an unreviewed addition both fail this test.
    const V0930_VISIBLE: &[&str] = &[
        "actor",
        "agents",
        "artifact",
        "check",
        "config",
        "doctor",
        "finding",
        "frontier",
        "gate",
        "id",
        "init",
        "log",
        "next",
        "policy",
        "proof",
        "proposal",
        "reproduce",
        "review",
        "serve",
        "show",
        "start",
        "status",
        "submit",
        "verification",
        "why",
    ];
    const V0900_HIDDEN: &[&str] = &["authority", "completions", "target-index"];

    #[test]
    fn v0930_surface_is_exact_both_directions() {
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
            let want_visible: Vec<String> = V0930_VISIBLE.iter().map(|s| s.to_string()).collect();
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

    #[test]
    fn era_zero_writers_and_one_time_migration_writers_are_absent() {
        on_big_stack(|| {
            for args in [
                vec!["vela", "sign"],
                vec!["vela", "migrate", "."],
                vec!["vela", "authority", "enable-work", "."],
                vec!["vela", "frontier", "bind", "."],
                vec!["vela", "id", "protect"],
                vec!["vela", "id", "lock"],
                vec!["vela", "id", "pin-binary"],
                vec!["vela", "actor", "add", "."],
                vec!["vela", "actor", "activate", "."],
            ] {
                assert!(
                    Cli::try_parse_from(&args).is_err(),
                    "retired writer unexpectedly parses: {args:?}"
                );
            }
            assert!(Cli::try_parse_from(["vela", "actor", "list", "."]).is_ok());
            assert!(Cli::try_parse_from(["vela", "frontier", "materialize", "."]).is_ok());
        });
    }

    #[test]
    fn proposal_scoped_verification_commands_parse_exactly() {
        on_big_stack(|| {
            assert!(
                Cli::try_parse_from([
                    "vela",
                    "verification",
                    "import",
                    ".",
                    "verification.json",
                    "--as",
                    "verifier:independent",
                    "--json",
                ])
                .is_ok()
            );
            assert!(
                Cli::try_parse_from([
                    "vela",
                    "reproduce",
                    ".",
                    "--proposal",
                    "vpr_fixture",
                    "--json",
                ])
                .is_ok()
            );
        });
    }
}
