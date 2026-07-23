//! `cmd_frontier` and its handler logic, split out of cli.rs.

use crate::cli::{
    cmd_frontier_audit, cmd_frontier_release, cmd_frontier_releases, fail_return, print_json,
};
use crate::cli_commands::FrontierAction;
use colored::Colorize;
use serde_json::json;
use vela_protocol::cli_style as style;
use vela_protocol::frontier_repo;

pub(crate) fn cmd_frontier(action: FrontierAction) {
    use vela_protocol::project::ProjectDependency;
    use vela_protocol::repo;
    match action {
        FrontierAction::Bind {
            frontier,
            reason,
            confirm_root,
            confirm_at,
            json,
        } => crate::cli::repository_bind::cmd_frontier_bind(
            &frontier,
            &reason,
            confirm_root.as_deref(),
            confirm_at.as_deref(),
            json,
        ),
        FrontierAction::Trust { action } => match action {
            crate::cli_commands::FrontierTrustAction::Pin {
                frontier,
                boundary_root,
                confirm_root,
                confirm_at,
                json,
            } => crate::cli::repository_trust::cmd_frontier_trust_pin(
                &frontier,
                &boundary_root,
                confirm_root.as_deref(),
                confirm_at.as_deref(),
                json,
            ),
        },
        FrontierAction::New {
            path,
            name: _,
            description: _,
            force: _,
            json: _,
        } => {
            crate::ui::fail_with(
                crate::ui::ErrorKind::Usage,
                "frontier new is retired; it cannot create or overwrite a legacy v0.1 repository",
                Some(&format!(
                    "use `vela init {} --name <name> --scope <bounded-question>` for a new Profile v1 frontier; use `vela migrate` for an existing repository",
                    path.display()
                )),
            );
        }
        FrontierAction::Materialize { frontier, json } => {
            let spin = (!json).then(|| {
                crate::cli::progress::Spinner::start("materializing derived views from the log")
            });
            let payload =
                materialize_with_write_gate(&frontier).unwrap_or_else(|error| fail_return(&error));
            if let Some(s) = spin {
                s.finish("materialized");
            }
            if json {
                print_json(&payload);
            } else {
                println!(
                    "{} materialized frontier repo at {}",
                    style::ok("frontier"),
                    frontier.display()
                );
            }
        }
        FrontierAction::ListDeps { frontier, json } => {
            let p = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            let deps: Vec<&ProjectDependency> = p.project.dependencies.iter().collect();
            if json {
                let payload = json!({
                    "ok": true,
                    "command": "frontier.list-deps",
                    "frontier": frontier.display().to_string(),
                    "count": deps.len(),
                    "dependencies": deps,
                });
                print_json(&payload);
            } else {
                println!();
                println!(
                    "  {}",
                    format!("VELA · FRONTIER · LIST-DEPS · {}", frontier.display())
                        .to_uppercase()
                        .dimmed()
                );
                println!("  {}", style::tick_row(60));
                if deps.is_empty() {
                    println!("  (no dependencies declared)");
                } else {
                    for d in &deps {
                        let kind = if d.is_cross_frontier() {
                            "cross-frontier"
                        } else {
                            "compile-time"
                        };
                        println!("  · {} [{kind}]", d.name);
                        if let Some(v) = &d.vfr_id {
                            println!("    vfr_id:   {v}");
                        }
                        if let Some(l) = &d.locator {
                            println!("    locator:  {l}");
                        }
                        if let Some(s) = &d.pinned_snapshot_hash {
                            println!("    snapshot: {s}");
                        }
                    }
                }
            }
        }
        FrontierAction::Diff {
            left,
            right,
            json,
            quiet,
        } => vela_protocol::diff::run(&left, &right, json, quiet),
        FrontierAction::RecoverPublication {
            operation,
            frontier,
            push,
            json,
        } => {
            crate::ui::set_mode("frontier.recover-publication", json);
            let dir = crate::ui::resolve_frontier(frontier);
            let opts = if push {
                crate::config::git_publish::PublishOptions::pushing()
            } else {
                crate::config::git_publish::PublishOptions::new(false)
            };
            let publication =
                match crate::decision_plan::recover_decision_operation(&dir, &operation) {
                    Ok(Some(outcome)) => crate::cli::sign_session::publish_exact_decision(
                        &dir,
                        &format!("recover decision: {operation}"),
                        &outcome,
                        &opts,
                    ),
                    Ok(None) => {
                        crate::config::git_publish::recover_publication(&dir, &operation, &opts)
                    }
                    Err(error) => crate::config::git_publish::PublicationOutcome {
                        state: crate::config::git_publish::PublicationState::Unknown {
                            reason: error.to_string(),
                        },
                        recovery_command: None,
                    },
                };
            let ok = matches!(
                &publication.state,
                crate::config::git_publish::PublicationState::Unchanged { .. }
                    | crate::config::git_publish::PublicationState::CommittedLocal { .. }
                    | crate::config::git_publish::PublicationState::Pushed { .. }
            );
            let payload = json!({
                "ok": ok,
                "command": "frontier.recover-publication",
                "operation_id": operation,
                "publication": publication,
            });
            if json {
                print_json(&payload);
            } else {
                println!(
                    "frontier publication recovery · {}",
                    if ok { "recovered" } else { "blocked" }
                );
                println!("  operation: {operation}");
                println!("  publication: {}", payload["publication"]);
            }
            if !ok {
                std::process::exit(1);
            }
        }
        FrontierAction::Release {
            frontier,
            name,
            notes,
            previous,
            json,
        } => cmd_frontier_release(frontier, name, notes, previous, json),
        FrontierAction::Releases { frontier, json } => cmd_frontier_releases(frontier, json),
        FrontierAction::Audit { frontier, json } => cmd_frontier_audit(frontier, json),
        FrontierAction::Rank {
            frontier,
            limit,
            json,
        } => cmd_frontier_rank(&frontier, limit, json),
    }
}

fn materialize_with_write_gate(frontier: &std::path::Path) -> Result<serde_json::Value, String> {
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)?;
    let write_barrier =
        crate::frontier_txn::FrontierTxn::acquire_write_barrier(frontier, &journal_dir)
            .map_err(|error| error.to_string())?;
    let project = vela_protocol::repo::load_from_path(frontier)?;
    let visible = frontier_repo::render_visible_repo_files(frontier, &project)?;
    let managed = vela_protocol::repo::ManagedFileSet {
        writes: visible,
        deletes: Default::default(),
    };
    let writes = crate::frontier_txn::PlannedWrite::from_managed_files(managed)
        .map_err(|error| error.to_string())?;
    let request = vela_protocol::canonical::to_canonical_bytes(&serde_json::json!({
        "schema": "vela.frontier-materialize-request.internal.v1",
        "frontier_id": project.frontier_id(),
        "event_log_root": format!("sha256:{}", vela_protocol::events::event_log_hash(&project.events)),
    }))?;
    crate::frontier_txn::execute_no_event_transaction(
        write_barrier,
        frontier,
        "frontier-materialize",
        crate::frontier_txn::ContentDigest::hash(request),
        &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        &project,
        writes,
        Vec::new(),
        serde_json::json!({"schema": "vela.frontier-materialize-result.internal.v1"}),
    )
    .map_err(|error| error.to_string())?;
    let lock = frontier_repo::read_repository_lock(frontier)?
        .ok_or_else(|| "materialization completed without vela.lock".to_string())?;
    Ok(match lock {
        frontier_repo::FrontierLockFile::V1(lock) => serde_json::json!({
            "schema": frontier_repo::FRONTIER_MATERIALIZE_SCHEMA,
            "ok": true,
            "path": frontier.display().to_string(),
            "wrote_frontier": "frontier.json",
            "wrote_lock": "vela.lock",
            "wrote_proof": "proof/latest.json",
            "wrote_events_manifest": "proof/events.manifest.jsonl",
            "profile_root": lock.profile_root,
            "identity_root": lock.identity_root,
            "dependency_root": lock.dependency_root,
            "scientific_state_root": lock.scientific_state_root,
            "legacy_snapshot_root": lock.legacy_snapshot_root,
            "event_log_root": lock.event_log_root,
            "proposal_root": lock.proposal_root,
        }),
        frontier_repo::FrontierLockFile::LegacyV0_1(lock) => serde_json::json!({
            "schema": frontier_repo::FRONTIER_MATERIALIZE_SCHEMA,
            "ok": true,
            "path": frontier.display().to_string(),
            "wrote_frontier": "frontier.json",
            "wrote_lock": "vela.lock",
            "wrote_proof": "proof/latest.json",
            "wrote_events_manifest": "proof/events.manifest.jsonl",
            "snapshot_hash": lock.snapshot_hash,
            "event_log_hash": lock.event_log_hash,
            "proposal_state_hash": lock.proposal_state_hash,
        }),
    })
}

/// `vela frontier rank` — rank OPEN findings by accumulating structural support
/// (which is closest to a verifier-run from done). A read-only projection over
/// the typed frontier graph; carries the popularity baseline and the evidence
/// behind each score so the suggestion is inspectable.
fn cmd_frontier_rank(frontier: &std::path::Path, limit: usize, json: bool) {
    use vela_protocol::frontier_identification::{
        frontier_identification, heterogeneity_surfacing,
    };
    use vela_protocol::repo;
    let source = repo::detect(frontier).unwrap_or_else(|e| fail_return(&e));
    let proj = repo::load(&source).unwrap_or_else(|e| fail_return(&e));
    let ranked = frontier_identification(&proj);
    // The companion query: relations in live disagreement (Garg's heterogeneity
    // surfacing). Never auto-adjudicated — a lead for the human review queue.
    let contested = heterogeneity_surfacing(&proj);
    let shown: Vec<_> = ranked.iter().take(limit).collect();
    if json {
        print_json(&json!({
            "command": "frontier rank",
            "schema": "vela.frontier_rank.v0.1",
            "ranking_kind": "structural_opportunity",
            "authority": "advice_only",
            "work_queue": false,
            "producer_work_command": "vela next . --json",
            "frontier_id": proj.frontier_id(),
            "open_total": ranked.len(),
            "candidates": shown,
            "contested": contested,
        }));
        return;
    }
    if ranked.is_empty() {
        println!(
            "no structural opportunities are currently surfaced; use `vela next . --json` for producer work."
        );
        return;
    }
    println!(
        "frontier rank: {} structural opportunity candidate(s) (advice only, not the producer work queue)",
        ranked.len()
    );
    for (i, c) in shown.iter().enumerate() {
        println!(
            "{:>3}. {}  {}",
            i + 1,
            c.id,
            style::dim(&c.label.chars().take(70).collect::<String>())
        );
        println!(
            "      score {:.2} (popularity baseline {:.0}) — {}",
            c.score, c.baseline, c.why
        );
        if !c.evidence.is_empty() {
            let ev: Vec<&str> = c.evidence.iter().take(4).map(String::as_str).collect();
            println!("      evidence: {}", ev.join(", "));
        }
    }
    if !contested.is_empty() {
        println!(
            "\n  contested ({} relation(s) in live disagreement — for human review, never auto-resolved):",
            contested.len()
        );
        for h in contested.iter().take(limit) {
            println!(
                "    {} <-> {}  {}",
                h.finding,
                h.partner,
                style::dim(&h.label.chars().take(56).collect::<String>())
            );
        }
    }
    println!(
        "\n  structural advice, not authority. Use `vela next . --json` for canonical producer offers; verification is not acceptance."
    );
}

#[cfg(test)]
mod write_gate_tests {
    use super::*;

    fn snapshot(root: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        fn walk(
            root: &std::path::Path,
            directory: &std::path::Path,
            files: &mut std::collections::BTreeMap<String, Vec<u8>>,
        ) {
            let Ok(entries) = std::fs::read_dir(directory) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(root, &path, files);
                } else if path.is_file() {
                    files.insert(
                        path.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        std::fs::read(path).unwrap(),
                    );
                }
            }
        }
        let mut files = std::collections::BTreeMap::new();
        walk(root, root, &mut files);
        files
    }

    #[test]
    fn materialize_requires_profile_v1_without_touching_legacy_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let project = vela_protocol::project::assemble("legacy", Vec::new(), 0, 0, "fixture");
        vela_protocol::repo::init_repo(temp.path(), &project).unwrap();
        let before = snapshot(temp.path());

        let error = materialize_with_write_gate(temp.path()).unwrap_err();
        assert!(
            error.contains("frontier_profile_upgrade_required"),
            "{error}"
        );
        assert_eq!(snapshot(temp.path()), before);
        assert!(!temp.path().join(".vela/operation-journals").exists());
    }
}
