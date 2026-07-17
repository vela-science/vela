use crate::cli::{fail, fail_return, fmt_timestamp, frontier_dir_for_source, print_json};
use colored::Colorize;
use serde_json::json;
use sha2::Digest;
use std::path::Path;
use vela_edge::doctor;
use vela_edge::packet;
use vela_protocol::cli_style as style;
use vela_protocol::repo;

/// Stable compact status contract for the 0.9 product surface.
pub(crate) fn cmd_status_compact(path: &Path, json_out: bool) {
    crate::ui::set_mode("status", json_out);
    let frontier_dir = frontier_dir_for_source(path);
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier_dir)
        .unwrap_or_else(|error| fail_return(&error));
    let _barrier =
        crate::frontier_txn::FrontierTxn::acquire_recovery_barrier(frontier_dir, &journal_dir)
            .unwrap_or_else(|error| fail_return(&error.to_string()));
    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let project = vela_protocol::repo::load_from_path(frontier_dir)
        .unwrap_or_else(|error| fail_return(&error));
    let active_policy = vela_protocol::acceptance_policy::load_active_policy_snapshot(frontier_dir);
    let replay = vela_protocol::reducer::verify_replay(&project);
    let policy = vela_protocol::proposals::policy_accept::assess_policy_readiness(
        &project,
        active_policy.as_ref().map_err(String::as_str),
        &observed_at,
    );
    let policy_ok = policy.permit_readiness()
        != vela_protocol::proposals::policy_accept::PermitReadiness::Blocked;
    let status_ok = replay.ok && policy_ok;
    let signals = vela_edge::signals::analyze_at(&project, &[], Some(frontier_dir));
    let mut blockers_by_code = std::collections::BTreeMap::<String, usize>::new();
    for signal in &signals.signals {
        if signal.blocks.iter().any(|block| block == "strict_check") {
            *blockers_by_code.entry(signal.kind.clone()).or_default() += 1;
        }
    }
    let blocker_count = blockers_by_code.values().sum::<usize>();
    let pending_review = project
        .proposals
        .iter()
        .filter(|proposal| proposal.status == "pending_review")
        .count();
    let open_work = std::fs::read(frontier_dir.join("targets.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| {
            value
                .get("targets")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .map(|targets| {
            targets
                .iter()
                .filter(|target| {
                    target.get("state").and_then(serde_json::Value::as_str) == Some("open")
                })
                .count()
        })
        .unwrap_or(0);
    let git_text = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(frontier_dir)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    let git_commit = git_text(&["rev-parse", "HEAD^{commit}"]);
    let git_tree = git_text(&["rev-parse", "HEAD^{tree}"]);
    let git_clean =
        git_text(&["status", "--porcelain=v1", "--untracked-files=all"]).is_some_and(|status| {
            status.lines().all(|line| {
                line.get(3..)
                    .is_some_and(|path| path.starts_with(".vela/operation-journals/"))
            })
        });
    let actor_registry_path = frontier_dir.join(".vela/actors.json");
    let actor_registry_root = std::fs::read(&actor_registry_path)
        .map(|bytes| format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes))))
        .unwrap_or_else(|_| {
            vela_protocol::canonical::to_canonical_bytes(&project.actors)
                .map(|bytes| format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes))))
                .unwrap_or_else(|error| {
                    fail_return(&format!("canonicalize actor registry: {error}"))
                })
        });
    let artifact_root = vela_protocol::canonical::to_canonical_bytes(&project.artifacts)
        .map(|bytes| format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes))))
        .unwrap_or_else(|error| fail_return(&format!("canonicalize artifacts: {error}")));
    let next_action = if !replay.ok {
        "vela check . --strict"
    } else if !policy_ok {
        "vela doctor . --all --json"
    } else if open_work > 0 {
        "vela next . --json"
    } else if pending_review > 0 {
        "vela review list . --json"
    } else {
        "none"
    };
    let payload = json!({
        "ok": status_ok,
        "command": "status",
        "schema": "vela.status.v1",
        "frontier": {
            "id": project.frontier_id(),
            "name": project.project.name,
        },
        "git": {
            "commit": git_commit,
            "tree": git_tree,
            "clean": git_clean,
        },
        "roots": {
            "event_log": format!("sha256:{}", vela_protocol::events::event_log_hash(&project.events)),
            "snapshot": format!("sha256:{}", vela_protocol::events::snapshot_hash(&project)),
            "proposals": format!("sha256:{}", vela_protocol::proposals::proposal_state_hash(&project.proposals)),
            "actor_registry": actor_registry_root,
            "artifacts": artifact_root,
        },
        "integrity": {
            "replay": if replay.ok { "reproduced" } else { "diverged" },
            "replay_diffs": replay.diffs.len(),
            "strict": if blocker_count == 0 { "pass" } else { "blocked" },
            "blocker_count": blocker_count,
            "blockers_by_code": blockers_by_code,
        },
        "counts": {
            "events": project.events.len(),
            "findings": project.findings.len(),
            "open_work": open_work,
            "pending_review": pending_review,
        },
        "policy": {
            "state": policy.state().as_str(),
            "permit_readiness": policy.permit_readiness().as_str(),
            "reason_codes": policy.reason_codes(),
            "error": policy.detail(),
        },
        "next_action": next_action,
    });
    if json_out {
        print_json(&payload);
    } else {
        println!(
            "vela status · {}",
            payload["frontier"]["name"].as_str().unwrap_or("frontier")
        );
        println!("  frontier  {}", project.frontier_id());
        println!(
            "  commit    {}",
            payload["git"]["commit"].as_str().unwrap_or("unavailable")
        );
        println!(
            "  replay    {}",
            payload["integrity"]["replay"].as_str().unwrap_or("unknown")
        );
        println!("  strict    {} blocker(s)", blocker_count);
        for (code, count) in payload["integrity"]["blockers_by_code"]
            .as_object()
            .into_iter()
            .flatten()
        {
            println!("            {} {}", count, code);
        }
        println!("  events    {}", project.events.len());
        println!("  findings  {}", project.findings.len());
        println!("  work      {} open", open_work);
        println!("  review    {} pending", pending_review);
        println!(
            "  policy    {} · Permit {}",
            policy.state().as_str(),
            policy.permit_readiness().as_str()
        );
        if let Some(detail) = policy.detail() {
            println!("            {detail}");
        }
        println!("  next      {next_action}");
    }
    if !status_ok {
        std::process::exit(1);
    }
}

/// Signed-but-uncommitted store state: the worst state a decision can
/// be in (it exists on one machine, invisible to CI, the hub, and every
/// collaborator). Counts changed/untracked files under the frontier's
/// store paths; 0 when not a git repo.
pub(crate) fn unpublished_store_files(path: &Path) -> usize {
    let Ok(out) = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "status",
            "--porcelain",
            "--",
            ".vela",
            "frontier.json",
            "vela.lock",
            "proof",
        ])
        .output()
    else {
        return 0;
    };
    if !out.status.success() {
        return 0;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// v0.42: Recent canonical events. The `git log` analogue.
pub(crate) fn cmd_log(path: &Path, limit: usize, kind_filter: Option<&str>, json: bool) {
    crate::ui::set_mode("log", json);
    let project = repo::load_from_path(path).unwrap_or_else(|e| fail_return(&e));
    let mut events: Vec<&vela_protocol::events::StateEvent> = project
        .events
        .iter()
        .filter(|e| match kind_filter {
            Some(k) => e.kind.as_str().contains(k),
            None => true,
        })
        .collect();
    events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    events.truncate(limit);

    if json {
        let payload: Vec<_> = events
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "kind": e.kind,
                    "actor": e.actor.id,
                    "target": &e.target.id,
                    "target_type": &e.target.r#type,
                    "timestamp": e.timestamp,
                    "reason": e.reason,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "log",
                "events": payload,
            }))
            .expect("serialize log")
        );
        return;
    }

    println!();
    println!(
        "  {}",
        format!("VELA · LOG · {}  (latest {})", path.display(), events.len())
            .to_uppercase()
            .dimmed()
    );
    println!("  {}", style::tick_row(60));
    if events.is_empty() {
        println!("  (no events)");
        return;
    }
    // The width-aware table computes column widths from content and, on a
    // narrow terminal, truncates the widest column with `…`; piped output
    // stays full-width and byte-stable (reason is the flexible last column,
    // pre-capped so a novel-length reason can't blow up the row).
    let clip = |s: &str, w: usize| -> String {
        if s.chars().count() > w {
            format!(
                "{}…",
                s.chars().take(w.saturating_sub(1)).collect::<String>()
            )
        } else {
            s.to_string()
        }
    };
    let mut table = crate::cli::table::Table::new();
    for e in &events {
        table.row([
            fmt_timestamp(&e.timestamp),
            e.kind.as_str().to_string(),
            clip(&e.actor.id, 24),
            clip(&e.target.id, 22),
            e.reason.chars().take(80).collect(),
        ]);
    }
    println!("{}", table.render());
    println!();
}

/// `vela verify <packet_dir>` — same code path as
/// `vela packet validate`, surfaced under a friendlier top-level name.
/// Reads every file in the manifest, recomputes SHA-256, validates the
/// proof-trace chain. Exit 0 on all-match, 1 on any mismatch.
pub(crate) fn cmd_verify(path: &Path, json_output: bool) {
    let result = packet::validate(path);
    match result {
        Ok(output) if json_output => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "command": "verify",
                    "result": output,
                }))
                .expect("failed to serialize verify response")
            );
        }
        Ok(output) => {
            println!("{output}");
            println!(
                "\nverify: ok\n  every file in the manifest matched its claimed sha256.\n  pull this packet on another machine, run the same command, see the same line."
            );
        }
        Err(e) => fail(&e),
    }
}

pub(crate) async fn cmd_doctor(frontier: Option<&Path>, port: u16, all: bool, json_output: bool) {
    let report = doctor::run(frontier, port);
    // The local setup/ceremony lane lives crate-side (identity, pin,
    // policy freshness, adapters, registry) and merges into the report.
    let frontier_dir = if report.frontier_load_ok {
        Some(std::path::PathBuf::from(&report.frontier_path))
    } else {
        None
    };
    let setup = crate::config::doctor_setup::run(frontier_dir.as_deref());
    let setup_blockers = setup
        .iter()
        .filter(|check| check.status == crate::config::doctor_setup::SetupStatus::Fail)
        .map(|check| check.name.to_string())
        .collect::<Vec<_>>();
    let mut blockers = report.blocking.clone();
    blockers.extend(setup_blockers);
    blockers.sort();
    blockers.dedup();
    let next_action = setup
        .iter()
        .find(|check| {
            check.status == crate::config::doctor_setup::SetupStatus::Fail && !check.next.is_empty()
        })
        .map(|check| check.next.clone())
        .or_else(|| report.next_commands.first().cloned());
    if json_output && !all {
        print_json(&json!({
            "schema": "vela.doctor.v1",
            "ok": blockers.is_empty(),
            "command": "doctor",
            "binary_version": report.binary_version,
            "frontier": {
                "path": report.frontier_path,
                "kind": report.frontier_kind,
                "load": if report.frontier_load_ok { "ok" } else { "blocked" },
            },
            "policy": if report.policy_ok { "ready" } else { "needs_attention" },
            "proof": report.proof_status,
            "evidence_ci": if report.evidence_ci_ok { "ok" } else { "blocked" },
            "blockers": blockers,
            "next_action": next_action,
        }));
    } else if json_output {
        let mut merged = serde_json::to_value(&report).unwrap_or_default();
        if let Some(obj) = merged.as_object_mut() {
            obj.insert(
                "setup".to_string(),
                serde_json::to_value(&setup).unwrap_or_default(),
            );
        }
        print_json(&merged);
    } else if all {
        println!("vela doctor");
        println!("  binary:      {}", report.binary_version);
        println!("  frontier:    {}", report.frontier_path);
        println!("  kind:        {}", report.frontier_kind);
        println!(
            "  policy:      {}",
            if report.policy_ok {
                "ok"
            } else {
                "needs attention"
            }
        );
        println!("  proof:       {}", report.proof_status);
        println!(
            "  evidence ci: {}",
            if report.evidence_ci_ok {
                "ok"
            } else {
                "needs attention"
            }
        );
        println!(
            "  serve:       port {} {}",
            report.workbench_port,
            if report.workbench_port_available {
                "available"
            } else {
                "unavailable"
            }
        );
        println!();
        println!("setup:");
        for c in &setup {
            let mark = match c.status {
                crate::config::doctor_setup::SetupStatus::Ok => "ok  ",
                crate::config::doctor_setup::SetupStatus::Warn => "warn",
                crate::config::doctor_setup::SetupStatus::Fail => "FAIL",
            };
            println!("  {mark}  {:<11} {}", c.name, c.detail);
            if !c.next.is_empty() {
                println!("        {:<11} → {}", "", c.next);
            }
        }
        if !report.blocking.is_empty() {
            println!("  blocking:    {}", report.blocking.join(", "));
        }
        if !report.warnings.is_empty() {
            println!("  warnings:    {}", report.warnings.join(", "));
        }
        println!();
        println!("next:");
        for command in &report.next_commands {
            println!("  {command}");
        }
        if let Some(config) = &report.mcp_config {
            println!();
            println!("mcp:");
            println!(
                "  {}",
                serde_json::to_string(config).expect("serialize mcp config")
            );
        }
    } else {
        println!("vela doctor · {}", report.frontier_path);
        println!("  binary    {}", report.binary_version);
        println!(
            "  frontier  {}",
            if report.frontier_load_ok {
                "ok"
            } else {
                "blocked"
            }
        );
        println!(
            "  policy    {}",
            if report.policy_ok {
                "ready"
            } else {
                "needs attention"
            }
        );
        println!("  proof     {}", report.proof_status);
        println!(
            "  blockers  {}",
            if blockers.is_empty() {
                "none".to_string()
            } else {
                blockers.join(", ")
            }
        );
        if let Some(next) = &next_action {
            println!("  next      {next}");
        }
        println!("  details   vela doctor . --all");
    }
    if !blockers.is_empty() {
        std::process::exit(1);
    }
}
