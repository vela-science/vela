use crate::serve;
use vela_edge::frontier_health;
use vela_edge::lint;
use vela_edge::signals;
use vela_edge::state_integrity;
use vela_edge::validate;
use vela_protocol::events;
use vela_protocol::evidence_ci;
use vela_protocol::frontier_repo;
use vela_protocol::project;
use vela_protocol::proposals;
use vela_protocol::repo;
use vela_protocol::sign;
use vela_protocol::sources;
use vela_protocol::state;

use std::path::{Path, PathBuf};

use clap::Parser;
use colored::Colorize;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::cli_style as style;

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

pub(crate) use crate::claim_view::*;
pub(crate) use crate::cli_admin::*;
pub(crate) use crate::cli_check::*;
use crate::cli_commands::*;
pub(crate) use crate::cli_engine::*;
pub(crate) use crate::cli_frontier::*;
pub(crate) use crate::cli_proof::*;
pub(crate) use crate::cli_read::*;
pub(crate) use crate::cli_write::*;

mod authority;
mod checks;
mod frontier_audit;
pub(crate) mod help_text;
mod identity;
mod lifecycle;
mod output;
pub(crate) mod progress;
pub(crate) mod records;
pub(crate) mod repository_trust;
pub(crate) mod review_decision;
pub(crate) mod safe_text;
mod session;
mod surface;
pub(crate) mod table;
#[cfg(test)]
mod tests;
pub(crate) use authority::*;
pub(crate) use checks::*;
pub(crate) use frontier_audit::*;
pub(crate) use identity::*;
pub(crate) use lifecycle::*;
pub(crate) use output::*;
pub(crate) use records::*;
pub(crate) use session::*;
pub(crate) use surface::*;
// Preserve the crate-public paths these two had when they lived in mod.rs.
pub use checks::scan_for_sensitive_paths;
pub use surface::is_science_subcommand;

pub async fn run_command() {
    // Deliberately NO dotenv here. `dotenvy::dotenv()` walks the working
    // tree upward, and vela runs inside CLONED frontier repos — a
    // committed .env could silently inject VELA_ACTOR_ID / VELA_KEY_PATH /
    // VELA_NO_PUBLISH for anyone who runs vela in it
    // (the attack class git blocks via protected configuration and
    // Codex blocks by refusing base-url keys in project config).
    // Configuration comes from the real environment and ~/.vela only.

    // Color contract: NO_COLOR always wins; ui.color=never/always
    // overrides the tty heuristic.
    if std::env::var_os("NO_COLOR").is_some() {
        colored::control::set_override(false);
    } else {
        match crate::config::settings::resolve("ui.color", None)
            .0
            .as_str()
        {
            "never" => colored::control::set_override(false),
            "always" => colored::control::set_override(true),
            _ => {}
        }
    }

    let cli = Cli::parse();
    crate::ui::set_quiet(cli.quiet);
    match cli.command {
        Commands::Repository { action } => match action {
            RepositoryAction::Verify { frontier, json } => {
                crate::repository_upgrade::cmd_repository_verify(&frontier, json)
            }
            RepositoryAction::Upgrade {
                frontier,
                to,
                archive_dir,
                reason,
                confirm_root,
                json,
            } => crate::repository_upgrade::cmd_repository_upgrade(
                &frontier,
                &to,
                &archive_dir,
                &reason,
                confirm_root.as_deref(),
                json,
            ),
        },
        Commands::Check {
            source,
            schema,
            stats,
            evidence,
            conformance,
            conformance_dir,
            all,
            schema_only,
            strict,
            fix,
            json,
        } => {
            if evidence {
                // `check --evidence` folds in the standalone `evidence-ci` verb,
                // routing to the same handler. A source/frontier is required.
                let frontier = source.unwrap_or_else(|| {
                    fail_return("check --evidence needs a frontier path (e.g. `vela check <frontier> --evidence`)")
                });
                cmd_evidence_ci(&frontier, json);
            } else {
                cmd_check(
                    source.as_deref(),
                    schema,
                    stats,
                    conformance,
                    &conformance_dir,
                    all,
                    schema_only,
                    strict,
                    fix,
                    json,
                );
            }
        }
        Commands::Doctor {
            frontier,
            port,
            all,
            json,
        } => cmd_doctor(frontier.as_deref(), port, all, json).await,
        Commands::Proof {
            frontier,
            out,
            template,
            record_proof_state,
            json,
        } => cmd_proof(&frontier, &out, &template, record_proof_state, json),
        Commands::Serve {
            frontier,
            frontiers,
            backend,
            http,
            setup,
            check_tools,
            adoption,
            profile,
            json,
        } => {
            if setup {
                cmd_mcp_setup(frontier.as_deref(), frontiers.as_deref());
            } else if check_tools {
                let source =
                    serve::ProjectSource::from_args(frontier.as_deref(), frontiers.as_deref());
                match serve::check_tools(source, adoption) {
                    Ok(report) => {
                        if json {
                            print_json(&report);
                        } else {
                            print_tool_check_report(&report);
                        }
                    }
                    Err(e) => fail(&format!("Tool check failed: {e}")),
                }
            } else {
                let profile = profile.unwrap_or_else(|| {
                    crate::config::settings::try_resolve("mcp.profile", frontier.as_deref())
                        .unwrap_or_else(|error| fail_return(&error))
                        .0
                });
                let mcp_profile = vela_edge::tool_registry::McpProfile::parse(&profile)
                    .unwrap_or_else(|e| fail_return(&e));
                let source =
                    serve::ProjectSource::from_args(frontier.as_deref(), frontiers.as_deref());
                if let Some(port) = http {
                    serve::run_http(source, backend.as_deref(), port, mcp_profile).await;
                } else {
                    serve::run(source, backend.as_deref(), mcp_profile).await;
                }
            }
        }
        Commands::Status { frontier, json } => {
            cmd_status_compact(&crate::ui::resolve_frontier(frontier), json)
        }
        Commands::Log {
            frontier,
            finding_id,
            limit,
            kind,
            as_of,
            json,
        } => {
            let (frontier, finding_id) =
                crate::ui::resolve_frontier_with_id(frontier, finding_id, &["vf_"]);
            if let Some(vf) = finding_id {
                let payload = state::history_as_of(&frontier, &vf, as_of.as_deref())
                    .unwrap_or_else(|e| fail_return(&e));
                if json {
                    print_json(&payload);
                } else {
                    print_history(&payload);
                }
            } else {
                cmd_log(&frontier, limit, kind.as_deref(), json);
            }
        }
        Commands::Gate { action } => cmd_gate(action),
        Commands::Verification { action } => cmd_verify_evidence(action),
        Commands::Agents { action } => crate::cli_agents::cmd_agents(action),
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
        Commands::Id { action } => cmd_id(action),
        Commands::Actor { action } => cmd_actor(action),
        Commands::Authority { action } => match action {
            AuthorityAction::Init {
                frontier,
                key,
                reason,
                json,
            } => cmd_authority_init(&frontier, key.as_deref(), &reason, json),
            AuthorityAction::Upgrade {
                frontier,
                reason,
                json,
            } => cmd_authority_upgrade(&frontier, &reason, json),
            AuthorityAction::Trust { action } => match action {
                AuthorityTrustAction::Pin {
                    frontier,
                    record_root,
                    json,
                } => cmd_authority_trust_pin(&frontier, &record_root, json),
            },
        },
        Commands::Frontier { action } => cmd_frontier(action),
        Commands::Init {
            path,
            name,
            scope,
            json,
        } => cmd_init(&path, name.as_deref(), scope.as_deref(), json),
        Commands::Review { action } => cmd_review(action),
        Commands::Proposal { action } => cmd_proposal(action),
        Commands::TargetIndex { action } => crate::target_index::cmd_target_index(action),
        Commands::Claim {
            command:
                ClaimCommands::Show {
                    frontier,
                    claim_id,
                    view,
                    json,
                },
        } => cmd_claim_show(&frontier, &claim_id, &view, json),
        Commands::Show {
            frontier,
            object_id,
            json,
        } => crate::cli_object::cmd_show(&frontier, &object_id, json),
        Commands::Why {
            frontier,
            claim_id,
            json,
        } => crate::cli_object::cmd_why(&frontier, &claim_id, json),

        Commands::Artifact { command } => match command {
            ArtifactCommands::Retract {
                frontier,
                artifact_id,
                reason,
                actor,
                json,
            } => {
                crate::ui::set_mode("artifact.retract", json);
                cmd_artifact_retract(frontier, artifact_id, reason, actor, json)
            }
        },

        Commands::Next {
            frontier,
            limit,
            json,
        } => {
            crate::ui::set_mode("next", json);
            let dir = crate::ui::resolve_frontier(frontier);
            crate::repository_upgrade::cmd_current_next(&dir, limit, json);
        }
        Commands::Start {
            target,
            frontier,
            ttl,
            drop: drop_it,
            reason,
            r#as,
            json,
        } => {
            crate::ui::set_mode("start", json);
            let dir = crate::ui::resolve_frontier(frontier);
            let actor = crate::cli_identity::resolve_actor(r#as.as_deref());
            crate::current_work::cmd_start(
                &dir,
                target.as_deref(),
                ttl,
                drop_it,
                reason.as_deref(),
                &actor,
                json,
            );
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
            packet_root,
            profile_root,
            verifier_capsule_root,
            result_contract_root,
            attempt,
            r#as,
            push,
            json,
        } => {
            crate::ui::set_mode("submit", json);
            let dir = crate::ui::resolve_frontier(frontier);
            let actor = crate::cli_identity::resolve_actor(r#as.as_deref());
            let preflight_identity =
                vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
                    "schema": "vela.submit-preflight.internal.v1",
                    "frontier": dir.display().to_string(),
                    "actor": actor,
                    "submission": submission.as_ref().map(|path| path.display().to_string()),
                    "claim": claim,
                    "claim_type": claim_type,
                    "conditions": condition,
                    "replayability": replayability,
                    "artifacts": artifact,
                    "caveats": caveat,
                    "producer_checks": producer_check,
                    "verification_requirements": verification_requirement,
                    "packet_root": packet_root,
                    "profile_root": profile_root,
                    "verifier_capsule_root": verifier_capsule_root,
                    "result_contract_root": result_contract_root,
                    "attempt": attempt,
                    "push": push,
                }))
                .unwrap_or_default();
            let preflight_id =
                crate::operation_journal::operation_id("submit-preflight", &preflight_identity);
            let fail_preflight = |kind, message: String| -> ! {
                crate::ui::fail_unchanged(kind, &message, &preflight_id, "vela submit --help")
            };
            let (submission, bundle_root) = if let Some(path) = submission {
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
                let root = path.parent().map(std::path::Path::to_path_buf);
                (parsed, root)
            } else {
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
                        let binding = vela_protocol::receipt_v1::ExecutionBindingV1 {
                            schema: vela_protocol::receipt_v1::EXECUTION_BINDING_SCHEMA.to_string(),
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
                let authored = crate::workflow::author_submission(
                    &dir,
                    &actor,
                    attempt.as_deref(),
                    claim,
                    claim_type,
                    condition,
                    replayability,
                    &artifact,
                    caveat,
                    producer_check,
                    verification_requirement,
                    execution_binding,
                )
                .unwrap_or_else(|error| {
                    fail_preflight(
                        crate::ui::ErrorKind::Domain,
                        format!("Submission build: {error}"),
                    )
                });
                (authored, None)
            };
            match crate::workflow::submit(
                &dir,
                &submission,
                &actor,
                attempt.as_deref(),
                bundle_root.as_deref(),
                push,
            ) {
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
                            vela_protocol::cli_style::dim("proposal"),
                            safe_text::inline(&outcome.proposal_id)
                        );
                        println!(
                            "  {:<18} {}",
                            vela_protocol::cli_style::dim("registration"),
                            safe_text::inline(&outcome.registration_record_id)
                        );
                        println!(
                            "  {:<18} {}",
                            vela_protocol::cli_style::dim("accepted delta"),
                            outcome.accepted_event_delta
                        );
                    }
                }
                Err(e) => fail(&e),
            }
        }
        Commands::Config { action } => match action {
            ConfigAction::Get {
                key,
                frontier,
                json,
            } => {
                crate::ui::set_mode("config", json);
                crate::config::settings::cmd_config_get(&key, frontier.as_deref(), json)
            }
            ConfigAction::Set {
                key,
                value,
                frontier,
                json,
            } => {
                crate::ui::set_mode("config", json);
                crate::config::settings::cmd_config_set(&key, &value, frontier.as_deref(), json)
            }
            ConfigAction::Unset {
                key,
                frontier,
                json,
            } => {
                crate::ui::set_mode("config", json);
                crate::config::settings::cmd_config_unset(&key, frontier.as_deref(), json)
            }
            ConfigAction::List { frontier, json } => {
                crate::ui::set_mode("config", json);
                crate::config::settings::cmd_config_list(frontier.as_deref(), json)
            }
        },
        Commands::Policy { action } => match action {
            PolicyAction::Show { frontier, json } => {
                crate::ui::set_mode("policy", json);
                crate::config::cli_policy::cmd_policy_show(
                    &crate::ui::resolve_frontier(frontier),
                    json,
                )
            }
            PolicyAction::Test { frontier, json } => {
                crate::ui::set_mode("policy", json);
                crate::config::cli_policy::cmd_policy_test(
                    &crate::ui::resolve_frontier(frontier),
                    json,
                )
            }
            PolicyAction::EvaluateProposal { operands, json } => {
                crate::ui::set_mode("policy", json);
                let proposal_id = operands
                    .iter()
                    .find(|value| value.starts_with("vpr_"))
                    .cloned()
                    .unwrap_or_else(|| {
                        fail_return("policy evaluate-proposal needs one vpr_ proposal id")
                    });
                let frontier = operands
                    .iter()
                    .find(|value| !value.starts_with("vpr_"))
                    .map(PathBuf::from);
                crate::config::cli_policy::cmd_policy_evaluate_proposal(
                    &crate::ui::resolve_frontier(frontier),
                    &proposal_id,
                    json,
                )
            }
            PolicyAction::Log { frontier, json } => {
                crate::ui::set_mode("policy", json);
                crate::config::cli_policy::cmd_policy_log(
                    &crate::ui::resolve_frontier(frontier),
                    json,
                )
            }
        },
    }
}

// Bare `vela` (no args) opens a session against the nearest `.vela/`
// repo, walking up from cwd. The session prints a one-screen
// dashboard, then accepts single-letter verb shortcuts or
// natural-language questions routed through `cmd_ask`.
//
// Doctrine: this is the daily-driver entry, not a kitchen-sink IDE.
// Single screen, no scroll, no full TUI redraw. Each verb spawns the
// existing kernel command and prints its output inline. The session
// stays out of the user's way: type something, get an answer, type
// again. OpenCode/Claude Code shape.

pub fn run_from_args() {
    style::init();
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(String::as_str) {
        // v0.47: bare `vela` opens a session against the nearest
        // `.vela/` repo. The 30+ subcommand list is still there for
        // direct invocation; the session is the daily-driver entry.
        None => {
            run_session();
            return;
        }
        Some("-h" | "--help" | "help") => {
            // v0.47: top-level help shows the daily flow. The full
            // 30+ subcommand list lives behind `vela help advanced`.
            if args.get(2).map(String::as_str) == Some("advanced") {
                print_strict_help();
            } else {
                print_session_help();
            }
            return;
        }
        Some("-V" | "--version" | "version") => {
            println!("vela {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("proof") if args.get(2).map(String::as_str) == Some("verify") => {
            let json = args.iter().any(|arg| arg == "--json");
            let frontier = args
                .iter()
                .skip(3)
                .find(|arg| !arg.starts_with('-'))
                .map(PathBuf::from)
                .inspect(|p| {
                    // An exported proof-packet DIR (has manifest.json but no
                    // .vela/) verifies via the packet validator — the path
                    // packets themselves stamp into their receipts.
                    if p.join("manifest.json").exists() && !p.join(".vela").is_dir() {
                        crate::cli_read::cmd_verify(p, json);
                        std::process::exit(0);
                    }
                })
                .unwrap_or_else(|| {
                    eprintln!(
                        "{} proof verify requires a frontier repo",
                        style::err_prefix()
                    );
                    std::process::exit(2);
                });
            cmd_proof_verify(&frontier, json);
            return;
        }
        Some("proof") if args.get(2).map(String::as_str) == Some("explain") => {
            let frontier = args
                .iter()
                .skip(3)
                .find(|arg| !arg.starts_with('-'))
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    eprintln!(
                        "{} proof explain requires a frontier repo",
                        style::err_prefix()
                    );
                    std::process::exit(2);
                });
            cmd_proof_explain(&frontier);
            return;
        }
        // Historical projection namespaces were consolidated under `claim`.
        Some("state") => {
            eprintln!("{} `vela state` retired in 0.900", style::err_prefix());
            eprintln!(
                "use `vela claim show <frontier> <claim_id> --view record|standing|evidence`"
            );
            std::process::exit(2);
        }
        Some("finding") => {
            eprintln!(
                "{} `vela finding` retired from the current product language",
                style::err_prefix()
            );
            eprintln!(
                "use `vela claim show <frontier> <claim_id> --view record|standing|evidence`"
            );
            std::process::exit(2);
        }
        Some("atlas") => {
            eprintln!(
                "{} `vela atlas` retired from the core binary in 0.900",
                style::err_prefix()
            );
            eprintln!("use the campaign reader or a Canopus verifier profile");
            std::process::exit(2);
        }
        Some(cmd) if !is_science_subcommand(cmd) => {
            let replacement = match cmd {
                "proposals" => Some(
                    "vela review list <frontier> --json, then vela review show <frontier> <vpr_id> --json",
                ),
                "diff" => Some(
                    "vela review diff <frontier> <vpr_id>, or vela frontier diff <left> <right>",
                ),
                "credit" => Some("vela claim show <frontier> <claim_id> --view attribution"),
                "publication" => Some("vela frontier recover-publication"),
                "hub" => Some("vela serve for a local read surface, or the optional Observatory"),
                "land" => Some(
                    "vela submit <submission.json>, or vela submit --attempt <attempt-id> --claim <scoped-result> ...",
                ),
                "verify" => Some(
                    "vela verification import <frontier> <verification.json> --as verifier:<actor>",
                ),
                "foundry" | "reproduce-external" => {
                    Some("a Canopus verifier profile or parent campaign script")
                }
                _ => None,
            };
            if let Some(replacement) = replacement {
                eprintln!("{} `vela {cmd}` retired in 0.900", style::err_prefix());
                eprintln!("use `{replacement}`");
            } else {
                eprintln!("{} unknown command: {cmd}", style::err_prefix());
                eprintln!("run `vela --help` for the product surface.");
            }
            std::process::exit(2);
        }
        Some(_) => {}
    }
    let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    runtime.block_on(run_command());
}
