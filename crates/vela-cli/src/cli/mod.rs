use crate::serve;
use vela_edge::frontier_health;
use vela_edge::lint;
use vela_edge::signals;
use vela_edge::state_integrity;
use vela_edge::validate;
use vela_protocol::bundle;
use vela_protocol::diff;
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

pub(crate) use crate::cli_admin::*;
pub(crate) use crate::cli_check::*;
use crate::cli_commands::*;
pub(crate) use crate::cli_engine::*;
pub(crate) use crate::cli_finding::*;
pub(crate) use crate::cli_frontier::*;
pub(crate) use crate::cli_proof::*;
pub(crate) use crate::cli_read::*;
pub(crate) use crate::cli_registry::*;
pub(crate) use crate::cli_write::*;

mod checks;
mod frontier_audit;
mod frontier_diff;
mod governance;
pub(crate) mod help_text;
mod identity;
mod json_edit;
mod lifecycle;
mod links;
mod output;
pub(crate) mod progress;
pub(crate) mod prompt;
pub(crate) mod records;
pub(crate) mod safe_text;
mod session;
pub(crate) mod sign_session;
mod surface;
pub(crate) mod table;
#[cfg(test)]
mod tests;
pub(crate) use checks::*;
pub(crate) use frontier_audit::*;
pub(crate) use frontier_diff::*;
pub(crate) use governance::*;
pub(crate) use identity::*;
pub(crate) use json_edit::*;
pub(crate) use lifecycle::*;
pub(crate) use links::*;
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
    // committed .env could silently inject VELA_HUB_URL / VELA_ACTOR_ID /
    // VELA_KEY_PATH / VELA_NO_PUBLISH for anyone who runs vela in it
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
            json,
        } => cmd_doctor(frontier.as_deref(), port, json),
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
            cmd_status(&crate::ui::resolve_frontier(frontier), json)
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
        Commands::Agents { action } => crate::cli_agents::cmd_agents(action),
        Commands::Foundry { action } => crate::cli_engine::cmd_foundry(action),
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

        Commands::Reproduce { path, json } => cmd_reproduce(&path, json),
        Commands::Submit {
            witness,
            frontier,
            r#as,
            push,
            dry_run,
            json,
        } => {
            let dir = crate::ui::resolve_frontier(frontier);
            // A solver produced the witness, so it lands under an agent lane (a
            // reviewer land would queue for the human sign ceremony instead).
            let actor = crate::cli_identity::resolve_actor(r#as.as_deref());
            let actor = if actor.starts_with("agent:") || actor.starts_with("ci:") {
                actor
            } else {
                format!(
                    "agent:{}",
                    actor.split(':').next_back().unwrap_or("producer")
                )
            };
            crate::cli_engine::cmd_submit(&dir, &witness, &actor, push, dry_run, json);
        }
        Commands::Ci { action } => match action {
            crate::server::cli_commands::CiAction::Verdict {
                frontier,
                base,
                json,
            } => {
                crate::config::cli_policy::cmd_ci_verdict(&frontier, &base, json);
            }
        },
        Commands::Credit {
            finding_id,
            frontier,
            json,
        } => cmd_credit(&frontier, &finding_id, json),
        Commands::Id { action } => cmd_id(action),
        Commands::Actor { action } => cmd_actor(action),
        Commands::Frontier { action } => cmd_frontier(action),
        Commands::Hub { action } => cmd_hub(action),
        Commands::Publication { action } => match action {
            PublicationAction::Recover {
                operation,
                frontier,
                push,
                json,
            } => {
                crate::ui::set_mode("publication recover", json);
                let dir = crate::ui::resolve_frontier(frontier);
                let opts = if push {
                    crate::config::git_publish::PublishOptions::pushing()
                } else {
                    crate::config::git_publish::PublishOptions::new(false, false)
                };
                let publication =
                    crate::config::git_publish::recover_publication(&dir, &operation, &opts);
                let ok = matches!(
                    &publication.state,
                    crate::config::git_publish::PublicationState::CommittedLocal { .. }
                        | crate::config::git_publish::PublicationState::Pushed { .. }
                );
                if json {
                    print_json(&serde_json::json!({
                        "ok": ok,
                        "command": "publication.recover",
                        "operation_id": operation,
                        "publication": publication,
                    }));
                } else {
                    crate::ui::header(
                        "PUBLICATION RECOVER",
                        &operation,
                        Some(if ok { "recovered" } else { "not recovered" }),
                    );
                    println!(
                        "  {:<16} {}",
                        style::dim("publication"),
                        safe_text::inline(
                            &serde_json::to_string(&publication)
                                .unwrap_or_else(|_| "unknown".to_string())
                        )
                    );
                    println!(
                        "  {:<16} {}",
                        style::dim("next"),
                        safe_text::inline(
                            publication
                                .recovery_command
                                .as_deref()
                                .unwrap_or("git status --short")
                        )
                    );
                }
                if !ok {
                    std::process::exit(1);
                }
            }
        },
        Commands::Init {
            path,
            name,
            template,
            no_git,
            json,
        } => cmd_init(&path, &name, &template, !no_git, json),
        Commands::Diff {
            target,
            frontier_b,
            frontier,
            reviewer,
            json,
            quiet,
        } => {
            // v0.701: arg-order-insensitive. A `vpr_*` id in EITHER positional
            // routes to proposal preview; the other positional (or `--frontier`,
            // else `.`) is the frontier. So `vela diff <frontier> <vpr_>`,
            // `vela diff <vpr_> <frontier>`, and `vela diff <vpr_>` all work — no
            // more "Path does not exist" when the args are transposed.
            let first = target.clone();
            let vpr = if target.starts_with("vpr_") {
                Some(target.clone())
            } else if frontier_b.as_deref().is_some_and(|s| s.starts_with("vpr_")) {
                frontier_b.clone()
            } else {
                None
            };
            if let Some(target) = vpr {
                let frontier_root = frontier
                    .clone()
                    .or_else(|| {
                        // the positional that is NOT the proposal id, if any
                        if first.starts_with("vpr_") {
                            frontier_b.clone().map(std::path::PathBuf::from)
                        } else {
                            Some(std::path::PathBuf::from(&first))
                        }
                    })
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                let preview = proposals::preview_at_path(&frontier_root, &target, &reviewer)
                    .unwrap_or_else(|e| fail_return(&e));
                // The Engine's prospective read: what Evidence CI would say if
                // this proposal were accepted. Best-effort — a hiccup here must
                // never break the diff itself.
                let verdict = proposals::preview_engine_verdict(&frontier_root, &target).ok();
                let engine_json = verdict.as_ref().map(|v| {
                    json!({
                        "verdict": v.status,
                        "new_blocking": v.new_blocking,
                        "new_warnings": v.new_warnings,
                        "release_blocking_failed": v.release_blocking_failed,
                        "warnings": v.warnings,
                    })
                });
                let payload = json!({
                    "ok": true,
                    "command": "diff.proposal",
                    "frontier": frontier_root.display().to_string(),
                    "proposal_id": target,
                    "preview": preview,
                    "engine": engine_json,
                });
                if json {
                    print_json(&payload);
                } else {
                    // The reviewer's one-screen answer to "what would this
                    // change, and what does it actually SAY": the proposal's
                    // own text, the shape delta, the engine's prospective
                    // verdict with the warnings NAMED, and the decision verb.
                    let proposal = repo::load_from_path(&frontier_root).ok().and_then(|proj| {
                        let found = proj.proposals.iter().find(|p| p.id == target).cloned();
                        let pack = proj
                            .released_diff_packs
                            .iter()
                            .filter(|r| r.verdict.is_none())
                            .find(|r| r.member_proposals.iter().any(|m| m == &target))
                            .map(|r| r.pack_id.clone());
                        found.map(|p| (p, pack))
                    });
                    println!();
                    println!(
                        "  {}",
                        format!("VELA · DIFF · {}", safe_text::inline(&target))
                            .to_uppercase()
                            .dimmed()
                    );
                    println!("  {}", vela_protocol::cli_style::tick_row(60));
                    let pack_id = match &proposal {
                        Some((p, pack)) => {
                            println!(
                                "  kind:      {}   by {}",
                                safe_text::inline(&p.kind),
                                safe_text::inline(&p.actor.id)
                            );
                            let reason = safe_text::inline(&p.reason);
                            if !reason.is_empty() {
                                println!("  reason:    {reason}");
                            }
                            if let Some(text) = p
                                .payload
                                .pointer("/finding/assertion/text")
                                .and_then(serde_json::Value::as_str)
                            {
                                println!(
                                    "  proposes:  {}",
                                    wrap_line(&safe_text::inline(text), 78)
                                );
                            }
                            pack.clone()
                        }
                        None => {
                            println!("  kind:      {}", safe_text::inline(&preview.kind));
                            None
                        }
                    };
                    println!(
                        "  shape:     findings {} -> {} · events {} -> {} · artifacts {} -> {}",
                        preview.findings_before,
                        preview.findings_after,
                        preview.events_before,
                        preview.events_after,
                        preview.artifacts_before,
                        preview.artifacts_after,
                    );
                    if !preview.changed_findings.is_empty() {
                        println!(
                            "  changes:   {}",
                            safe_text::inline(&preview.changed_findings.join(", "))
                        );
                    }
                    if let Some(v) = &verdict {
                        match v.status.as_str() {
                            "pass" => println!("  engine:    evidence-ci clean if accepted"),
                            "warn" => {
                                println!(
                                    "  engine:    {} new review warning(s) if accepted",
                                    v.new_warnings.len()
                                );
                                for w in v.new_warnings.iter().take(5) {
                                    println!("    · {}", safe_text::inline(w));
                                }
                                if v.new_warnings.len() > 5 {
                                    println!("    … +{} more", v.new_warnings.len() - 5);
                                }
                            }
                            "blocked" => {
                                println!(
                                    "  engine:    WOULD BLOCK — {} new release-blocking failure(s)",
                                    v.new_blocking.len()
                                );
                                for b in v.new_blocking.iter().take(5) {
                                    println!("    · {}", safe_text::inline(b));
                                }
                            }
                            other => println!("  engine:    {}", safe_text::inline(other)),
                        }
                    }
                    println!();
                    match pack_id {
                        Some(pack) => {
                            println!(
                                "  decide:    vela sign    (this proposal rides pack {})",
                                safe_text::inline(&pack)
                            )
                        }
                        None => println!(
                            "  decide:    vela sign {} --yes    (or: vela proposals reject . {} --reason \"…\")",
                            safe_text::inline(&target),
                            safe_text::inline(&target)
                        ),
                    }
                    println!();
                }
            } else {
                let b_str = frontier_b.unwrap_or_else(|| {
                    fail_return(
                        "diff: two-frontier mode needs a second positional (filesystem path or `vfr_*` id); for proposal preview pass a `vpr_*` id",
                    )
                });
                // v0.140: when either side is a `vfr_*` id, pull
                // the frontier through the registry into a temp
                // dir and run the diff against the pulled path.
                // The tempdir lives for the duration of the diff
                // and is reclaimed on drop.
                let _tmp = if target.starts_with("vfr_") || b_str.starts_with("vfr_") {
                    Some(
                        tempfile::Builder::new()
                            .prefix("vela-diff-")
                            .tempdir()
                            .unwrap_or_else(|e| {
                                fail_return(&format!("tempdir for vfr resolve: {e}"))
                            }),
                    )
                } else {
                    None
                };
                let resolve_side = |side: &str, _slot: &str| -> std::path::PathBuf {
                    if side.starts_with("vfr_") {
                        fail_return(
                            "diff by vfr_ id used the retired hub transport; `git clone` the \
                             frontier repo and pass its path instead",
                        )
                    } else {
                        std::path::PathBuf::from(side)
                    }
                };
                let frontier_a = resolve_side(&target, "a");
                let frontier_b_path = resolve_side(&b_str, "b");
                diff::run(&frontier_a, &frontier_b_path, json, quiet);
            }
        }
        Commands::Proposals { action } => cmd_proposals(action),
        Commands::Finding { command } => match command {
            FindingCommands::Add {
                frontier,
                assertion,
                r#type,
                source,
                source_type,
                author,
                confidence,
                evidence_type,
                evidence_span,
                gap,
                negative_space,
                doi,
                year,
                url,
                source_authors,
                conditions_text,
                json,
                apply,
                replication_attestation,
            } => {
                validate_enum_arg("--type", &r#type, bundle::VALID_ASSERTION_TYPES);
                let replication_attestation = if let Some(p) = replication_attestation {
                    let raw = std::fs::read_to_string(&p).unwrap_or_else(|e| {
                        fail_return(&format!("--replication-attestation {}: {e}", p.display()))
                    });
                    Some(
                        serde_json::from_str::<serde_json::Value>(&raw).unwrap_or_else(|e| {
                            fail_return(&format!("--replication-attestation parse: {e}"))
                        }),
                    )
                } else {
                    None
                };
                validate_enum_arg(
                    "--evidence-type",
                    &evidence_type,
                    bundle::VALID_EVIDENCE_TYPES,
                );
                validate_enum_arg(
                    "--source-type",
                    &source_type,
                    bundle::VALID_PROVENANCE_SOURCE_TYPES,
                );
                let parsed_evidence_spans = parse_evidence_spans(&evidence_span);
                let parsed_source_authors = source_authors
                    .map(|s| {
                        s.split(';')
                            .map(|a| a.trim().to_string())
                            .filter(|a| !a.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                let report = state::add_finding(
                    &frontier,
                    state::FindingDraftOptions {
                        text: assertion,
                        assertion_type: r#type,
                        source,
                        source_type,
                        author,
                        confidence,
                        evidence_type,
                        doi,
                        year,
                        url,
                        source_authors: parsed_source_authors,
                        source_refs: Vec::new(),
                        conditions_text,
                        evidence_spans: parsed_evidence_spans,
                        gap,
                        negative_space,
                        replication_attestation,
                    },
                    apply,
                )
                .unwrap_or_else(|e| fail_return(&e));
                print_state_report(&report, json);
            }
            FindingCommands::Show {
                frontier,
                finding_id,
                json,
            } => cmd_finding_show(&frontier, &finding_id, json),
            FindingCommands::Supersede {
                frontier,
                old_id,
                assertion,
                r#type,
                source,
                source_type,
                author,
                reason,
                confidence,
                evidence_type,
                doi,
                year,
                url,
                source_authors,
                conditions_text,
                json,
                apply,
            } => {
                validate_enum_arg("--type", &r#type, bundle::VALID_ASSERTION_TYPES);
                validate_enum_arg(
                    "--evidence-type",
                    &evidence_type,
                    bundle::VALID_EVIDENCE_TYPES,
                );
                validate_enum_arg(
                    "--source-type",
                    &source_type,
                    bundle::VALID_PROVENANCE_SOURCE_TYPES,
                );
                let parsed_source_authors = source_authors
                    .map(|s| {
                        s.split(';')
                            .map(|a| a.trim().to_string())
                            .filter(|a| !a.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                let report = state::supersede_finding(
                    &frontier,
                    &old_id,
                    &reason,
                    state::FindingDraftOptions {
                        text: assertion,
                        assertion_type: r#type,
                        source,
                        source_type,
                        author,
                        confidence,
                        evidence_type,
                        doi,
                        year,
                        url,
                        source_authors: parsed_source_authors,
                        source_refs: Vec::new(),
                        conditions_text,
                        evidence_spans: Vec::new(),
                        gap: false,
                        negative_space: false,
                        replication_attestation: None,
                    },
                    apply,
                )
                .unwrap_or_else(|e| fail_return(&e));
                print_state_report(&report, json);
            }
            FindingCommands::Note {
                frontier,
                finding_id,
                text,
                author,
                apply,
                json,
            } => cmd_finding_note(frontier, finding_id, text, author, apply, json),
            FindingCommands::Caveat {
                frontier,
                finding_id,
                text,
                author,
                apply,
                json,
            } => cmd_finding_caveat(frontier, finding_id, text, author, apply, json),
            FindingCommands::Revise {
                frontier,
                finding_id,
                confidence,
                reason,
                reviewer,
                apply,
                json,
            } => cmd_finding_revise(
                frontier, finding_id, confidence, reason, reviewer, apply, json,
            ),
            FindingCommands::Reject {
                frontier,
                finding_id,
                reason,
                reviewer,
                apply,
                json,
            } => cmd_finding_reject(frontier, finding_id, reason, reviewer, apply, json),
            FindingCommands::Review {
                frontier,
                finding_id,
                status,
                reason,
                confidence,
                reviewer,
                apply,
                json,
            } => cmd_finding_review(
                frontier, finding_id, status, reason, confidence, reviewer, apply, json,
            ),
            FindingCommands::Contribution {
                frontier,
                finding_id,
                unit,
                unit_type,
                agent_kind,
                agent_id,
                model,
                model_version,
                role,
                basis,
                actor,
                apply,
                json,
            } => cmd_finding_contribution(
                frontier,
                finding_id,
                unit,
                unit_type,
                agent_kind,
                agent_id,
                model,
                model_version,
                role,
                basis,
                actor,
                apply,
                json,
            ),
            FindingCommands::Retract {
                source,
                finding_id,
                reason,
                reviewer,
                apply,
                json,
            } => cmd_finding_retract(source, finding_id, reason, reviewer, apply, json),
            FindingCommands::Link { action } => cmd_link(action),
        },

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

        Commands::Sign {
            target,
            frontier,
            yes,
            reason,
            batch,
            reset,
            sk,
            key,
            json,
        } => {
            crate::ui::set_mode("sign", json);
            if sk {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Usage,
                    "--sk (hardware touch-to-sign) is designed but not yet wired: the recommended path is a PKCS#11/OpenPGP Ed25519 token (raw Ed25519, zero verifier change); see docs/HARDWARE_SIGNING_PROPOSAL.md",
                    Some(
                        "run the ceremony with your file key; pin the binary with `vela id pin-binary`",
                    ),
                );
            }
            if let Some(batch) = batch {
                let dir = crate::ui::resolve_frontier(frontier);
                cmd_review_fidelity_batch(dir, batch, None, key, json);
            } else if let Some(target) = target {
                let as_path = std::path::Path::new(&target);
                if as_path.exists() {
                    sign_session::cmd_sign_detached(as_path, key.as_deref(), json);
                } else if yes {
                    sign_session::cmd_sign_one(frontier, &target, reason, key, json);
                } else {
                    crate::ui::fail_with(
                        crate::ui::ErrorKind::Usage,
                        &format!("`vela sign {target}` needs --yes for a scripted decision"),
                        Some("or run bare `vela sign` for the interactive session"),
                    );
                }
            } else {
                sign_session::cmd_sign_session(frontier, key, json, reset);
            }
        }
        Commands::Next {
            frontier,
            limit,
            json,
        } => {
            crate::ui::set_mode("next", json);
            let dir = crate::ui::resolve_frontier(frontier);
            let project =
                vela_protocol::repo::load_from_path(&dir).unwrap_or_else(|e| fail_return(&e));
            let targets = vela_edge::frontier_next::frontier_next(&project, Some(&dir), limit);
            if json {
                print_json(&serde_json::json!({
                    "ok": true, "command": "next",
                    "targets": targets.iter().map(|t| serde_json::json!({
                        "lane": t.lane, "id": t.id, "title": t.title,
                        "why": t.why, "next_command": t.next_command,
                    })).collect::<Vec<_>>(),
                }));
            } else {
                let tg = if targets.len() == 1 {
                    "target"
                } else {
                    "targets"
                };
                crate::ui::header("NEXT", ".", Some(&format!("{} {tg}", targets.len())));
                for t in &targets {
                    let chip = match t.lane.as_str() {
                        "attack" => vela_protocol::cli_style::brass("attack"),
                        "verify" => vela_protocol::cli_style::moss("verify"),
                        other => vela_protocol::cli_style::dim(&safe_text::inline(other)),
                    };
                    println!("  {chip}  {}", safe_text::inline(&t.title));
                    println!(
                        "      {}",
                        vela_protocol::cli_style::dim(&safe_text::inline(&format!(
                            "{} · {}",
                            t.id, t.why
                        )))
                    );
                    println!(
                        "      {}",
                        vela_protocol::cli_style::dim(&safe_text::inline(&t.next_command))
                    );
                }
                if targets.is_empty() {
                    println!("  · nothing open — the frontier is waiting on new seeds");
                }
            }
        }
        Commands::Work {
            target,
            frontier,
            ttl,
            drop: drop_it,
            r#as,
            json,
        } => {
            crate::ui::set_mode("work", json);
            let dir = crate::ui::resolve_frontier(frontier);
            let Some(target) = target else {
                let base = crate::workflow::session_dir(&dir, "");
                let sessions = base
                    .parent()
                    .map(|p| std::fs::read_dir(p).map(|d| d.count()).unwrap_or(0))
                    .unwrap_or(0);
                println!("  {} open session dir(s) under .vela/work/", sessions);
                return;
            };
            let actor = crate::cli_identity::resolve_actor(r#as.as_deref());
            if drop_it {
                let sdir = crate::workflow::session_dir(&dir, &target);
                let _ = std::fs::remove_dir_all(&sdir);
                println!(
                    "  · dropped session {} (lease expires by TTL)",
                    safe_text::inline(&target)
                );
                return;
            }
            let ttl = ttl.unwrap_or_else(|| {
                crate::config::settings::resolve("work.lease_ttl_seconds", Some(&dir))
                    .0
                    .parse()
                    .unwrap_or(86400)
            });
            match crate::workflow::claim(&dir, &target, &actor, Some(ttl)) {
                Ok(claim) => {
                    let briefing = crate::workflow::briefing(&dir, &target)
                        .unwrap_or_else(|e| fail_return(&e));
                    let sdir = crate::workflow::session_dir(&dir, &target);
                    std::fs::create_dir_all(&sdir).ok();
                    std::fs::write(
                        sdir.join("offer.json"),
                        serde_json::to_string_pretty(&briefing).unwrap_or_default(),
                    )
                    .ok();
                    if json {
                        print_json(&serde_json::json!({
                            "ok": true, "command": "work", "target": target,
                            "claim": claim, "briefing": briefing,
                            "session_dir": sdir.display().to_string(),
                        }));
                    } else {
                        crate::ui::header("WORK", &target, Some("lease claimed, briefing loaded"));
                        let b = briefing.get("briefing").unwrap_or(&briefing);
                        if let Some(s) = b.get("statement").and_then(|v| v.as_str()) {
                            println!("  {}", safe_text::multiline(s));
                        }
                        for (label, key) in [
                            ("gate", "gate"),
                            ("value to beat", "value_to_beat"),
                            ("attempts", "attempt_count"),
                        ] {
                            if let Some(v) = b.get(key) {
                                let vs = v
                                    .as_str()
                                    .map(str::to_string)
                                    .unwrap_or_else(|| v.to_string());
                                if vs != "null" {
                                    println!(
                                        "  {:<14} {}",
                                        vela_protocol::cli_style::dim(label),
                                        safe_text::inline(&vs)
                                    );
                                }
                            }
                        }
                        println!(
                            "  {:<14} {}",
                            vela_protocol::cli_style::dim("full offer"),
                            safe_text::inline(&sdir.join("offer.json").display().to_string())
                        );
                        println!(
                            "  {:<14} vela land <receipt.json>",
                            vela_protocol::cli_style::dim("when done")
                        );
                    }
                }
                Err(e) => fail(&e),
            }
        }
        Commands::Land {
            receipt,
            frontier,
            claim,
            claim_type,
            replayability,
            artifact,
            caveat,
            r#as,
            push,
            json,
        } => {
            crate::ui::set_mode("land", json);
            let dir = crate::ui::resolve_frontier(frontier);
            let actor = crate::cli_identity::resolve_actor(r#as.as_deref());
            let preflight_identity =
                vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
                    "schema": "vela.land-preflight.internal.v1",
                    "frontier": dir.display().to_string(),
                    "actor": actor,
                    "receipt": receipt.as_ref().map(|path| path.display().to_string()),
                    "claim": claim,
                    "claim_type": claim_type,
                    "replayability": replayability,
                    "artifacts": artifact,
                    "caveats": caveat,
                    "push": push,
                }))
                .unwrap_or_default();
            let preflight_id =
                crate::operation_journal::operation_id("land-preflight", &preflight_identity);
            let fail_preflight = |kind, message: String| -> ! {
                crate::ui::fail_unchanged(kind, &message, &preflight_id, "vela land --help")
            };
            let receipt = if let Some(path) = receipt {
                let raw = std::fs::read(&path).unwrap_or_else(|error| {
                    fail_preflight(
                        crate::ui::ErrorKind::NotFound,
                        format!("read {}: {error}", path.display()),
                    )
                });
                vela_protocol::receipt_v1::ReceiptV1::parse(&raw).unwrap_or_else(|error| {
                    fail_preflight(crate::ui::ErrorKind::Domain, error.to_string())
                })
            } else {
                let Some(claim) = claim else {
                    crate::ui::fail_unchanged(
                        crate::ui::ErrorKind::Usage,
                        "land needs a receipt file or --claim",
                        &preflight_id,
                        "vela land --help",
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
                crate::workflow::author_receipt(
                    &dir,
                    &actor,
                    claim,
                    claim_type,
                    replayability,
                    &artifact,
                    caveat,
                )
                .unwrap_or_else(|error| {
                    fail_preflight(
                        crate::ui::ErrorKind::Domain,
                        format!("receipt build: {error}"),
                    )
                })
            };
            match crate::workflow::land(&dir, &receipt, &actor, push) {
                Ok(outcome) => {
                    let (route, detail) = outcome.route.summary();
                    if json {
                        print_json(&serde_json::json!({
                            "ok": true, "command": "land",
                            "request_id": outcome.operation_id,
                            "operation_id": outcome.operation_id,
                            "receipt_root": outcome.receipt_root,
                            "record_id": outcome.record_id,
                            "proposal_id": outcome.proposal_id,
                            "finding_id": outcome.finding_id,
                            "route": route, "detail": detail,
                            "publication": outcome.publication,
                        }));
                    } else {
                        render_land_outcome(
                            &outcome.proposal_id,
                            route,
                            &detail,
                            &outcome.operation_id,
                            &outcome.publication,
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
            PolicyAction::Draft {
                frontier,
                template,
                replace,
                json,
            } => {
                crate::ui::set_mode("policy", json);
                crate::config::cli_policy::cmd_policy_draft(
                    &crate::ui::resolve_frontier(frontier),
                    &template,
                    replace,
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
            // NB: `policy` is intercepted before clap (cli_policy.rs owns the
            // live dispatch); these arms are shadowed. `json=false` keeps them
            // type-checking without pretending to route JSON they never see.
            PolicyAction::Sign { frontier, key, yes } => {
                crate::ui::set_mode("policy", false);
                crate::config::cli_policy::cmd_policy_sign(
                    &crate::ui::resolve_frontier(frontier),
                    key.as_deref(),
                    yes,
                    false,
                )
            }
            PolicyAction::Revoke {
                frontier,
                key,
                reason,
                yes,
            } => {
                crate::ui::set_mode("policy", false);
                crate::config::cli_policy::cmd_policy_revoke(
                    &crate::ui::resolve_frontier(frontier),
                    key.as_deref(),
                    &reason,
                    yes,
                    false,
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

fn render_land_outcome(
    proposal_id: &str,
    route: &str,
    detail: &str,
    operation_id: &str,
    publication: &crate::config::git_publish::PublicationOutcome,
) {
    use crate::config::git_publish::PublicationState;

    let (state, retained, changed_git, unchanged, remote, fallback_next) = match &publication.state
    {
        PublicationState::Uncommitted { candidate, reason } => (
            format!("uncommitted: {reason}"),
            candidate
                .as_ref()
                .map(|oid| format!("candidate {oid} is not on the target ref"))
                .unwrap_or_else(|| "no publication commit on the target ref".to_string()),
            "no Git ref moved".to_string(),
            "unrelated index/worktree entries".to_string(),
            "not contacted by this publication attempt".to_string(),
            "git status --short".to_string(),
        ),
        PublicationState::Stale {
            candidate,
            expected,
            actual,
        } => (
            format!("stale: candidate {candidate} planned from {expected}; target is now {actual}"),
            format!("candidate {candidate} was not installed on the target ref"),
            "the competing Git ref update won; Vela did not merge or overwrite it".to_string(),
            "caller index/worktree entries".to_string(),
            "not contacted by this publication attempt".to_string(),
            "git status --short".to_string(),
        ),
        PublicationState::CommittedLocal { commit } => (
            "committed_local".to_string(),
            format!("local commit {commit}"),
            format!("local Git ref now retains commit {commit}"),
            "unrelated index/worktree entries".to_string(),
            "unverified; only the local commit is proven".to_string(),
            "git push".to_string(),
        ),
        PublicationState::Pushed { commit, remote } => (
            format!("pushed to {remote}"),
            format!("commit {commit}"),
            format!("commit {commit} is verified on {remote}"),
            "unrelated index/worktree entries".to_string(),
            format!("verified on {remote}"),
            "vela status --json".to_string(),
        ),
        PublicationState::Unknown { reason } => (
            format!("unknown: {reason}"),
            "publication state is not proven; inspect the recovery result".to_string(),
            "publication could not be proven".to_string(),
            "unrelated index/worktree entries".to_string(),
            "unverified".to_string(),
            "git status --short".to_string(),
        ),
    };
    let next = publication
        .recovery_command
        .as_deref()
        .unwrap_or(&fallback_next);

    crate::ui::header("LAND", proposal_id, Some(route));
    println!(
        "  {:<16} proposal {} routed {} ({})",
        style::dim("changed"),
        safe_text::inline(proposal_id),
        safe_text::inline(route),
        safe_text::inline(detail)
    );
    println!(
        "  {:<16} {}",
        style::dim("git"),
        safe_text::inline(&changed_git)
    );
    println!(
        "  {:<16} {}",
        style::dim("unchanged"),
        safe_text::inline(&unchanged)
    );
    println!(
        "  {:<16} {}",
        style::dim("remote"),
        safe_text::inline(&remote)
    );
    println!(
        "  {:<16} {}",
        style::dim("publication"),
        safe_text::inline(&state)
    );
    println!(
        "  {:<16} {}",
        style::dim("request"),
        safe_text::inline(operation_id)
    );
    println!(
        "  {:<16} {}",
        style::dim("retained"),
        safe_text::inline(&retained)
    );
    println!("  {:<16} {}", style::dim("next"), safe_text::inline(next));
}

pub(crate) fn parse_evidence_spans(inputs: &[String]) -> Vec<Value> {
    inputs
        .iter()
        .filter_map(|input| {
            let trimmed = input.trim();
            if trimmed.is_empty() {
                return None;
            }
            if trimmed.starts_with('{') {
                match serde_json::from_str::<Value>(trimmed) {
                    Ok(value @ Value::Object(_)) => return Some(value),
                    Ok(_) | Err(_) => {
                        eprintln!(
                            "{} evidence span JSON should be an object; storing as text",
                            style::warn("warn")
                        );
                    }
                }
            }
            Some(json!({
                "section": "curator_source",
                "text": trimmed,
            }))
        })
        .collect()
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
        Some("policy") => {
            crate::cli_policy::run(&args);
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
        // The state projections: `vela state <frontier> [vf_]` (claim-state),
        // `vela state trust|pack|diff …` (trust vector, claim pack, Evidence
        // Diff), and the math-atlas anchor links `vela state
        // anchor|anchors|unanchor`. Intercepted ahead of the clap dispatcher
        // (mirroring `proof verify`). The internal parsers still speak the
        // historical `claim <mode>` argv shape, so the argv is rewritten:
        // bare `vela state X` becomes `claim state X`.
        Some("state") => {
            let mode = args.get(2).map(String::as_str);
            let mut rewritten: Vec<String> = vec![args[0].clone(), "claim".to_string()];
            match mode {
                Some("trust" | "pack" | "diff") => {
                    rewritten.extend(args[2..].iter().cloned());
                    crate::cli_claim::run(&rewritten);
                }
                Some("anchor" | "anchors" | "unanchor") => {
                    rewritten.extend(args[2..].iter().cloned());
                    crate::cli_claim::run_anchor(&rewritten);
                }
                _ => {
                    rewritten.push("state".to_string());
                    rewritten.extend(args[2..].iter().cloned());
                    crate::cli_claim::run(&rewritten);
                }
            }
            return;
        }
        // Math Atlas projection: `vela atlas <frontier>...`. Read-only,
        // cross-frontier; unions claims into cells by HardIdentity anchors.
        Some("atlas") => {
            crate::cli_atlas::run(&args);
            return;
        }
        Some(cmd) if !is_science_subcommand(cmd) => {
            eprintln!(
                "{} unknown or non-release command: {cmd}",
                style::err_prefix()
            );
            eprintln!("run `vela --help` for the strict v0 command surface.");
            std::process::exit(2);
        }
        Some(_) => {}
    }
    let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    runtime.block_on(run_command());
}
