use crate::cli::{fail, fail_return, fmt_timestamp, print_json};
use colored::Colorize;
use serde_json::json;
use std::path::Path;
use vela_edge::doctor;
use vela_edge::packet;
use vela_protocol::cli_style as style;
use vela_protocol::repo;

/// Stable compact status contract for the 0.9 product surface.
pub(crate) fn cmd_status_compact(path: &Path, json_out: bool) {
    crate::repository_upgrade::cmd_current_status(path, json_out);
}

/// Signed-but-uncommitted store state: the worst state a decision can
/// be in (it exists on one machine, invisible to CI, the hub, and every
/// collaborator). Counts changed/untracked files under the frontier's
/// store paths; 0 when not a git repo.
pub(crate) fn unpublished_store_files(path: &Path) -> usize {
    let Ok(out) = crate::git_hardened::output(
        path,
        &[
            "status",
            "--porcelain",
            "--",
            ".vela",
            "frontier.json",
            "vela.lock",
            "proof",
        ],
    ) else {
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
    if path.join(".vela/epoch.json").is_file() {
        let payload = crate::current_read::log_payload(path, limit, kind_filter)
            .unwrap_or_else(|error| fail_return(&error));
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).expect("serialize current log")
        );
        return;
    }
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
    if let Some(frontier) = frontier
        && frontier.join(".vela/epoch.json").is_file()
    {
        crate::current_doctor::cmd_current_doctor(frontier, all, json_output);
        return;
    }
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
