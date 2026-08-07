use std::path::Path;

use crate::style;
use clap::Parser;

#[derive(Parser)]
#[command(name = "vela", version)]
#[command(about = "Portable scientific state")]
struct Cli {
    /// Suppress hint/advice lines (VELA_ADVICE=0 does the same).
    #[arg(long, global = true)]
    quiet: bool,
    #[command(subcommand)]
    command: Commands,
}

use crate::command_handlers::{cmd_reproduce, cmd_verify_evidence};
use crate::command_spec::*;

mod authority;
pub(crate) mod help_text;
mod lifecycle;
mod output;
pub(crate) mod page;
pub(crate) mod progress;
pub(crate) mod records;
pub(crate) mod repo_arg;
pub(crate) mod review_decision;
pub(crate) mod safe_text;
mod surface;
pub(crate) use authority::*;
pub(crate) use lifecycle::*;
pub(crate) use output::*;
pub(crate) use records::*;
pub(crate) use surface::*;
pub fn run_command() {
    // Deliberately NO dotenv here. `dotenvy::dotenv()` walks the working
    // tree upward, and vela runs inside CLONED repository repos — a
    // committed .env could silently inject VELA_ACTOR_ID or other process
    // configuration for anyone who runs vela in it
    // (the attack class git blocks via protected configuration and
    // Codex blocks by refusing base-url keys in project config).
    // Configuration comes from the real environment and ~/.vela only.
    // `style::init()` owns the complete TTY + NO_COLOR contract.

    let cli = Cli::parse();
    crate::ui::set_quiet(cli.quiet);
    match cli.command {
        Commands::Replay {
            repository,
            repo_flag,
            json,
        } => cmd_replay(repository, repo_flag, json),
        Commands::Status {
            repository,
            repo_flag,
            json,
        } => {
            /* Bind after set_mode, never before: argument binding can fail,
            and a `--json` caller is owed the same `{ok, command, error}`
            envelope for a usage error as for a domain one. */
            crate::ui::set_mode("status", json);
            cmd_status_compact(&repo_arg::bind_repo("status", repository, repo_flag), json);
        }
        Commands::Claims {
            repository,
            repo_flag,
            status,
            limit,
            cursor,
            json,
        } => {
            crate::ui::set_mode("claims", json);
            let repository = repo_arg::bind_repo("claims", repository, repo_flag);
            crate::current_claims::cmd_claims(
                &repository,
                status.as_deref(),
                limit,
                cursor.as_deref(),
                json,
            );
        }
        Commands::Log {
            repository,
            object_id,
            repo_flag,
            limit,
            kind,
            as_of,
            json,
        } => {
            crate::ui::set_mode("log", json);
            let (repository, object_id) =
                repo_arg::bind_repo_and_optional_object("log", repository, object_id, repo_flag);
            crate::ui::require_initialized_repo(&repository);
            cmd_log(
                &repository,
                object_id.as_deref(),
                limit,
                kind.as_deref(),
                as_of.as_deref(),
                json,
            );
        }
        Commands::Verification { action } => cmd_verify_evidence(action),
        Commands::Completions { shell } => {
            use clap::CommandFactory;
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            let shell_kind: clap_complete::Shell = match shell.as_str() {
                "bash" => clap_complete::Shell::Bash,
                "zsh" => clap_complete::Shell::Zsh,
                "fish" => clap_complete::Shell::Fish,
                other => fail_kind_return(
                    crate::ui::ErrorKind::Usage,
                    &format!("unsupported shell '{other}'. Valid: bash, zsh, fish"),
                ),
            };
            clap_complete::generate(shell_kind, &mut cmd, name, &mut std::io::stdout());
        }

        Commands::Reproduce {
            path,
            proposal,
            json,
        } => cmd_reproduce(&path, proposal.as_deref(), json),
        Commands::Authority { action } => match action {
            AuthorityAction::Trust { action } => match action {
                AuthorityTrustAction::Pin {
                    repository,
                    repo_flag,
                    record_root,
                    previous_record_root,
                    json,
                } => {
                    crate::ui::set_mode("authority trust pin", json);
                    cmd_authority_trust_pin(
                        &repo_arg::bind_repo("authority trust pin", repository, repo_flag),
                        &record_root,
                        previous_record_root.as_deref(),
                        json,
                    );
                }
            },
        },
        Commands::Init {
            path,
            name,
            scope,
            key,
            reason,
            json,
        } => cmd_init(
            &path,
            name.as_deref(),
            scope.as_deref(),
            key.as_deref(),
            &reason,
            json,
        ),
        Commands::Review { action } => cmd_review(action),
        Commands::Show {
            first,
            second,
            repo_flag,
            json,
        } => {
            crate::ui::set_mode("show", json);
            let (repository, object_id) = repo_arg::bind_repo_and_object(
                "show",
                "an object id",
                "OBJECT_ID",
                first,
                second,
                repo_flag,
            );
            crate::current_read::cmd_show(&repository, &object_id, json);
        }
        Commands::Why {
            first,
            second,
            repo_flag,
            json,
        } => {
            crate::ui::set_mode("why", json);
            let (repository, claim_id) = repo_arg::bind_repo_and_object(
                "why",
                "a full Claim id (vcl_...)",
                "CLAIM_ID",
                first,
                second,
                repo_flag,
            );
            crate::current_read::cmd_why(&repository, &claim_id, json);
        }
        Commands::Next {
            repository,
            repo_flag,
            limit,
            json,
        } => {
            crate::ui::set_mode("next", json);
            let dir = repo_arg::bind_repo("next", repository, repo_flag);
            crate::ui::require_initialized_repo(&dir);
            crate::current_repository::cmd_current_next(&dir, limit, json);
        }
        Commands::Start {
            target,
            repository,
            json,
        } => {
            crate::ui::set_mode("start", json);
            let dir = crate::ui::resolve_repo(repository);
            crate::ui::require_initialized_repo(&dir);
            crate::current_work::cmd_start(&dir, &target, json);
        }
        Commands::Submit {
            submission,
            repository,
            claim,
            claim_type,
            condition,
            replayability,
            artifact,
            caveat,
            producer_check,
            verification_requirement,
            corrects,
            supersedes,
            target_root,
            packet_root,
            profile_root,
            verifier_capsule_root,
            result_contract_root,
            r#as,
            json,
        } => {
            crate::ui::set_mode("submit", json);
            let dir = crate::ui::resolve_repo(repository);
            let authored_actor = submission
                .is_none()
                .then(|| crate::cli_identity::resolve_actor(r#as.as_deref()));
            let preflight_identity =
                vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
                    "schema": "vela.submit-preflight.internal.v1",
                    "repository": dir.display().to_string(),
                    "actor": authored_actor.as_deref().or(r#as.as_deref()).unwrap_or("signed-submission"),
                    "submission": submission.as_ref().map(|path| path.display().to_string()),
                    "claim": claim,
                    "claim_type": claim_type,
                    "conditions": condition,
                    "replayability": replayability,
                    "artifacts": artifact,
                    "caveats": caveat,
                    "producer_checks": producer_check,
                    "verification_requirements": verification_requirement,
                    "corrects": corrects,
                    "supersedes": supersedes,
                    "target_root": target_root,
                    "packet_root": packet_root,
                    "profile_root": profile_root,
                    "verifier_capsule_root": verifier_capsule_root,
                    "result_contract_root": result_contract_root,
                }))
                .unwrap_or_default();
            let preflight_id =
                crate::operation_journal::operation_id("submit-preflight", &preflight_identity);
            let fail_preflight = |kind, message: String| -> ! {
                crate::ui::fail_unchanged(kind, &message, &preflight_id, "vela submit --help")
            };
            let (submission, bundle_root, actor) = if let Some(path) = submission {
                let raw =
                    crate::bounded_file::read_bounded_file(&path, 8 * 1024 * 1024, "Submission v1")
                        .unwrap_or_else(|error| {
                            /* `bounded_file` distinguishes twelve reasons a
                               named file did not produce bytes. This read one
                               of them to pick an exit code and dropped the
                               rest, so a caller was told the Submission file
                               was a domain failure and could not learn whether
                               it was oversized, a symlink, or swapped while
                               being read — three different things to do next. */
                            let kind = if error.code == "missing" {
                                crate::ui::ErrorKind::NotFound
                            } else {
                                crate::ui::ErrorKind::Domain
                            };
                            crate::ui::fail_unchanged_coded(
                                kind,
                                Some(error.published_code()),
                                &error.to_string(),
                                &preflight_id,
                                "vela submit --help",
                            )
                        });
                let parsed = vela_protocol::submission_v1::SubmissionV1::parse(&raw)
                    .unwrap_or_else(|error| {
                        fail_preflight(crate::ui::ErrorKind::Domain, error.to_string())
                    });
                let actor = parsed.authentication.identity_binding.actor_id.clone();
                if let Some(explicit) = r#as.as_deref().map(str::trim)
                    && explicit != actor
                {
                    fail_preflight(
                        crate::ui::ErrorKind::Usage,
                        "--as does not match the signed Submission producer".to_string(),
                    );
                }
                let root = path.parent().map(std::path::Path::to_path_buf);
                (parsed, root, actor)
            } else {
                let actor = authored_actor.expect("locally authored submissions resolve an actor");
                let Some(claim) = claim else {
                    crate::ui::fail_unchanged(
                        crate::ui::ErrorKind::Usage,
                        "submit needs a Submission file or --claim",
                        &preflight_id,
                        "vela submit --help",
                    );
                };
                let claim_type = claim_type.unwrap_or_else(|| {
                    fail_preflight(
                        crate::ui::ErrorKind::Usage,
                        "flag authoring needs --type (computational, theoretical, empirical, negative, or contradiction)".to_string(),
                    )
                });
                let replayability = replayability.unwrap_or_else(|| {
                    fail_preflight(
                        crate::ui::ErrorKind::Usage,
                        "flag authoring needs --replayability (exact, bounded, approximate, unavailable, or unknown)".to_string(),
                    )
                });
                let execution_binding = match (
                    packet_root,
                    profile_root,
                    verifier_capsule_root,
                    result_contract_root,
                ) {
                    (
                        Some(packet_root),
                        Some(profile_root),
                        Some(verifier_capsule_root),
                        Some(result_contract_root),
                    ) => {
                        let binding = vela_protocol::execution_binding::ExecutionBindingV1 {
                            schema: vela_protocol::execution_binding::EXECUTION_BINDING_SCHEMA
                                .to_string(),
                            packet_root,
                            profile_root,
                            verifier_capsule_root,
                            result_contract_root,
                        };
                        binding.validate().unwrap_or_else(|error| {
                            fail_preflight(
                                crate::ui::ErrorKind::Usage,
                                format!("invalid exact execution binding: {error}"),
                            )
                        });
                        Some(binding)
                    }
                    (None, None, None, None) => None,
                    _ => fail_preflight(
                        crate::ui::ErrorKind::Usage,
                        "exact execution binding requires all four full roots".to_string(),
                    ),
                };
                let requested_change = crate::repository_ops::submission_requested_change(
                    corrects,
                    supersedes,
                    target_root,
                )
                .unwrap_or_else(|error| fail_preflight(crate::ui::ErrorKind::Usage, error));
                let authored = crate::repository_ops::author_submission(
                    &dir,
                    &actor,
                    claim,
                    claim_type,
                    condition,
                    replayability,
                    &artifact,
                    caveat,
                    producer_check,
                    verification_requirement,
                    requested_change,
                    execution_binding,
                )
                .unwrap_or_else(|error| {
                    fail_preflight(
                        crate::ui::ErrorKind::Domain,
                        format!("Submission build: {error}"),
                    )
                });
                (authored, None, actor)
            };
            crate::ui::require_initialized_repo(&dir);
            match crate::repository_ops::submit(&dir, &submission, &actor, bundle_root.as_deref()) {
                Ok(outcome) => {
                    if json {
                        let mut payload = serde_json::to_value(&outcome)
                            .expect("SubmitOutcome contains only serializable values");
                        let fields = payload
                            .as_object_mut()
                            .expect("SubmitOutcome serializes as an object");
                        fields.insert("ok".to_string(), serde_json::json!(true));
                        fields.insert("command".to_string(), serde_json::json!("submit"));
                        fields.insert(
                            "request_id".to_string(),
                            serde_json::json!(outcome.operation_id),
                        );
                        print_json(&payload);
                    } else {
                        crate::ui::header(
                            "SUBMISSION",
                            &outcome.submission_id,
                            Some(outcome.route),
                        );
                        println!(
                            "  {:<18} {}",
                            style::dim("proposal"),
                            safe_text::inline(&outcome.proposal_id)
                        );
                        println!(
                            "  {:<18} {}",
                            style::dim("claim"),
                            safe_text::inline(&outcome.claim_id)
                        );
                        /* TERMINOLOGY.md fixes the wording of a successful
                        Submission, so both sentences are quoted from it rather
                        than paraphrased, and they carry no gutter label: the
                        same document says a Submission has no status, so a word
                        like "retained" in the label column would read as one.
                        The second sentence is read off the outcome because
                        TERMINOLOGY.md states the normal case, and a Submission
                        that did move accepted state must not print it. */
                        println!("  Submission retained; review required.");
                        println!(
                            "  Accepted scientific state changed: {}.",
                            if outcome.accepted_state_changed {
                                "yes"
                            } else {
                                "no"
                            }
                        );
                    }
                }
                Err(e) => fail(&e),
            }
        }
    }
}

pub fn run_from_args() {
    style::init();
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(String::as_str) {
        None => {
            print_product_help();
            return;
        }
        Some("-h" | "--help" | "help") => {
            // v0.47: top-level help shows the daily flow. The full
            // 30+ subcommand list lives behind `vela help advanced`.
            if args.get(2).map(String::as_str) == Some("advanced") {
                print_advanced_help();
            } else {
                print_product_help();
            }
            return;
        }
        Some("-V" | "--version" | "version") => {
            println!("vela {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some(_) => {}
    }
    run_command();
}

fn cmd_status_compact(path: &Path, json_out: bool) {
    crate::current_repository::cmd_current_status(path, json_out);
}

fn cmd_log(
    path: &Path,
    object_id: Option<&str>,
    limit: usize,
    kind_filter: Option<&str>,
    as_of: Option<&str>,
    json: bool,
) {
    crate::ui::set_mode("log", json);
    let payload = crate::current_read::log_payload(path, object_id, limit, kind_filter, as_of)
        .unwrap_or_else(|error| fail_return(&error));
    if json {
        crate::cli::print_json(&payload);
        return;
    }
    /* `--json` was bound, passed to set_mode, and then ignored: both branches
    printed the same pretty JSON. The reasons in this log are full sentences
    written by the deciding human and are the most readable thing the CLI
    holds; they were the hardest to read. */
    let events = payload["events"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    println!("log · {} event(s), newest first", events.len());
    for event in events {
        let field = |key: &str| event[key].as_str().unwrap_or("not recorded").to_string();
        println!(
            "  {}  {}  {}",
            field("timestamp"),
            field("kind"),
            field("target")
        );
        if let Some(reason) = event["reason"].as_str() {
            println!("      {reason}");
        }
    }
}

fn cmd_review(action: ReviewAction) {
    /// The one Proposal-shaped object every `review` subcommand but `inbox`
    /// and `list` names, so the missing-argument error is written once.
    const PROPOSAL: (&str, &str) = ("a Proposal id (vpr_...)", "PROPOSAL_ID");
    match action {
        ReviewAction::Inbox {
            repository,
            repo_flag,
            json,
        } => {
            crate::ui::set_mode("review.inbox", json);
            let repository = repo_arg::bind_repo("review inbox", repository, repo_flag);
            crate::decision_inbox::cmd_decision_inbox(&repository, json);
        }
        ReviewAction::List {
            repository,
            repo_flag,
            status,
            limit,
            cursor,
            json,
        } => {
            crate::ui::set_mode("review list", json);
            let repository = repo_arg::bind_repo("review list", repository, repo_flag);
            crate::current_repository::cmd_current_review_list(
                &repository,
                status.as_deref(),
                limit,
                cursor.as_deref(),
                json,
            );
        }
        ReviewAction::Show {
            first,
            second,
            repo_flag,
            json,
        } => {
            crate::ui::set_mode("review show", json);
            let (repository, proposal_id) = repo_arg::bind_repo_and_object(
                "review show",
                PROPOSAL.0,
                PROPOSAL.1,
                first,
                second,
                repo_flag,
            );
            crate::current_repository::cmd_current_review_show(&repository, &proposal_id, json);
        }
        ReviewAction::Accept {
            first,
            second,
            repo_flag,
            if_entry_root,
            reason,
            json,
        } => {
            crate::ui::set_mode("review.accept", json);
            let (repository, proposal_id) = repo_arg::bind_repo_and_object(
                "review accept",
                PROPOSAL.0,
                PROPOSAL.1,
                first,
                second,
                repo_flag,
            );
            review_decision::cmd_review_decide(
                repository,
                &proposal_id,
                crate::current_repository_decision::DecisionAction::Accept,
                if_entry_root.as_deref(),
                reason,
                json,
            );
        }
        ReviewAction::Reject {
            first,
            second,
            repo_flag,
            if_entry_root,
            reason,
            json,
        } => {
            crate::ui::set_mode("review.reject", json);
            let (repository, proposal_id) = repo_arg::bind_repo_and_object(
                "review reject",
                PROPOSAL.0,
                PROPOSAL.1,
                first,
                second,
                repo_flag,
            );
            review_decision::cmd_review_decide(
                repository,
                &proposal_id,
                crate::current_repository_decision::DecisionAction::Reject,
                if_entry_root.as_deref(),
                reason,
                json,
            );
        }
        ReviewAction::Withdraw {
            first,
            second,
            repo_flag,
            actor,
            reason,
            json,
        } => {
            crate::ui::set_mode("review.withdraw", json);
            let (repository, proposal_id) = repo_arg::bind_repo_and_object(
                "review withdraw",
                PROPOSAL.0,
                PROPOSAL.1,
                first,
                second,
                repo_flag,
            );
            crate::current_withdrawal::cmd_withdraw(
                &repository,
                &proposal_id,
                &actor,
                &reason,
                json,
            );
        }
    }
}

fn cmd_replay(
    repository: Option<std::path::PathBuf>,
    repo_flag: Option<std::path::PathBuf>,
    json_output: bool,
) {
    crate::ui::set_mode("replay", json_output);
    let repository = repo_arg::bind_repo("replay", repository, repo_flag);
    crate::ui::require_initialized_repo(&repository);
    crate::current_repository::cmd_replay_repository(&repository, json_output);
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    /// The repository convention `command_spec.rs` states, asserted against the
    /// parsed surface rather than against prose, so the module doc cannot go
    /// back to describing a convention the surface does not have. A verb added
    /// later must either accept both spellings or be named here as one of the
    /// two arguments that is deliberately not a repository.
    #[test]
    fn every_repository_verb_accepts_both_spellings() {
        /// `init <path>` is a destination to create and `reproduce <path>` is a
        /// reproduction scope; neither takes discovery, so neither takes the
        /// flag. `completions` touches no repository at all.
        const NOT_REPOSITORY_VERBS: [&str; 3] = ["init", "reproduce", "completions"];

        fn walk(command: &clap::Command, path: &str) {
            let leaf = command.get_subcommands().count() == 0;
            let name = command.get_name();
            let path = if path.is_empty() {
                name.to_string()
            } else {
                format!("{path} {name}")
            };
            if leaf && !NOT_REPOSITORY_VERBS.contains(&name) {
                let flag = command
                    .get_arguments()
                    .find(|arg| arg.get_long() == Some("repo"));
                assert!(
                    flag.is_some(),
                    "`{path}` acts on a repository but does not accept --repo"
                );
                assert_eq!(
                    flag.and_then(clap::Arg::get_help)
                        .map(|help| help.to_string()),
                    Some(crate::command_spec::HELP_REPO.to_string()),
                    "`{path} --repo` must state the one repository contract"
                );
                assert!(
                    command
                        .get_positionals()
                        .any(|arg| arg.get_id() == "repository"
                            || arg.get_value_names().is_some_and(|names| names
                                .iter()
                                .any(|name| name.as_str() == "REPO")))
                        || matches!(name, "start" | "submit"),
                    "`{path}` accepts --repo but has no positional repository, and only start and submit may omit one"
                );
            }
            if leaf && NOT_REPOSITORY_VERBS.contains(&name) {
                assert!(
                    !command
                        .get_arguments()
                        .any(|arg| arg.get_long() == Some("repo")),
                    "`{path}` is documented as taking no repository argument"
                );
            }
            for child in command.get_subcommands() {
                walk(child, &path);
            }
        }

        walk(&Cli::command(), "");
    }
}
