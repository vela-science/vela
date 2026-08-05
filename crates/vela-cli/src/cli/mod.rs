use std::path::Path;

use crate::style;
use clap::Parser;

#[derive(Parser)]
#[command(name = "vela", version)]
#[command(about = "Portable frontier state for science")]
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
pub(crate) mod progress;
pub(crate) mod records;
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
    // tree upward, and vela runs inside CLONED frontier repos — a
    // committed .env could silently inject VELA_ACTOR_ID or other process
    // configuration for anyone who runs vela in it
    // (the attack class git blocks via protected configuration and
    // Codex blocks by refusing base-url keys in project config).
    // Configuration comes from the real environment and ~/.vela only.
    // `style::init()` owns the complete TTY + NO_COLOR contract.

    let cli = Cli::parse();
    crate::ui::set_quiet(cli.quiet);
    match cli.command {
        Commands::Replay { source, json } => cmd_replay(source.as_deref(), json),
        Commands::Status { frontier, json } => {
            cmd_status_compact(&crate::ui::resolve_frontier(frontier), json)
        }
        Commands::Log {
            frontier,
            object_id,
            limit,
            kind,
            as_of,
            json,
        } => {
            let frontier = crate::ui::resolve_frontier(frontier);
            crate::ui::set_mode("log", json);
            crate::ui::require_initialized_frontier(&frontier);
            cmd_log(
                &frontier,
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
                other => fail_return(&format!(
                    "unsupported shell '{other}'. Valid: bash, zsh, fish"
                )),
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
                    frontier,
                    record_root,
                    previous_record_root,
                    json,
                } => cmd_authority_trust_pin(
                    &frontier,
                    &record_root,
                    previous_record_root.as_deref(),
                    json,
                ),
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
            frontier,
            object_id,
            json,
        } => crate::current_read::cmd_show(&frontier, &object_id, json),
        Commands::Why {
            frontier,
            claim_id,
            json,
        } => crate::current_read::cmd_why(&frontier, &claim_id, json),
        Commands::Next {
            frontier,
            limit,
            json,
        } => {
            crate::ui::set_mode("next", json);
            let dir = crate::ui::resolve_frontier(frontier);
            crate::ui::require_initialized_frontier(&dir);
            crate::current_repository::cmd_current_next(&dir, limit, json);
        }
        Commands::Start {
            target,
            frontier,
            json,
        } => {
            crate::ui::set_mode("start", json);
            let dir = crate::ui::resolve_frontier(frontier);
            crate::ui::require_initialized_frontier(&dir);
            crate::current_work::cmd_start(&dir, &target, json);
        }
        Commands::Submit {
            submission,
            frontier,
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
            let dir = crate::ui::resolve_frontier(frontier);
            let authored_actor = submission
                .is_none()
                .then(|| crate::cli_identity::resolve_actor(r#as.as_deref()));
            let preflight_identity =
                vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
                    "schema": "vela.submit-preflight.internal.v1",
                    "frontier": dir.display().to_string(),
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
                            let kind = if error.code == "missing" {
                                crate::ui::ErrorKind::NotFound
                            } else {
                                crate::ui::ErrorKind::Domain
                            };
                            fail_preflight(kind, error.to_string())
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
            crate::ui::require_initialized_frontier(&dir);
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
                            style::dim("accepted delta"),
                            outcome.accepted_event_delta
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
    println!(
        "{}",
        serde_json::to_string_pretty(&payload).expect("serialize current log")
    );
}

fn cmd_review(action: ReviewAction) {
    match action {
        ReviewAction::Inbox { frontier, json } => {
            crate::decision_inbox::cmd_decision_inbox(&frontier, json)
        }
        ReviewAction::List {
            frontier,
            status,
            limit,
            cursor,
            json,
        } => crate::current_repository::cmd_current_review_list(
            &frontier,
            status.as_deref(),
            limit,
            cursor.as_deref(),
            json,
        ),
        ReviewAction::Show {
            frontier,
            proposal_id,
            json,
        } => crate::current_repository::cmd_current_review_show(&frontier, &proposal_id, json),
        ReviewAction::Accept {
            frontier,
            proposal_id,
            if_entry_root,
            reason,
            json,
        } => review_decision::cmd_review_decide(
            frontier,
            &proposal_id,
            crate::current_repository_decision::DecisionAction::Accept,
            if_entry_root.as_deref(),
            reason,
            json,
        ),
        ReviewAction::Reject {
            frontier,
            proposal_id,
            if_entry_root,
            reason,
            json,
        } => review_decision::cmd_review_decide(
            frontier,
            &proposal_id,
            crate::current_repository_decision::DecisionAction::Reject,
            if_entry_root.as_deref(),
            reason,
            json,
        ),
        ReviewAction::Withdraw {
            frontier,
            proposal_id,
            actor,
            reason,
            json,
        } => {
            crate::current_withdrawal::cmd_withdraw(&frontier, &proposal_id, &actor, &reason, json)
        }
    }
}

fn cmd_replay(source: Option<&Path>, json_output: bool) {
    crate::ui::set_mode("replay", json_output);
    let frontier = crate::ui::resolve_frontier(source.map(Path::to_path_buf));
    crate::ui::require_initialized_frontier(&frontier);
    crate::current_repository::cmd_replay_repository(&frontier, json_output);
}
