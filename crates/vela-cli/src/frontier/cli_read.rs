use crate::cli::{fail, fail_return, fmt_timestamp, frontier_label, print_json};
use colored::Colorize;
use serde_json::json;
use std::path::Path;
use vela_edge::doctor;
use vela_edge::packet;
use vela_protocol::cli_style as style;
use vela_protocol::repo;

/// v0.42: One-screen status. The `git status` analogue.
pub(crate) fn cmd_status(path: &Path, json: bool) {
    crate::ui::set_mode("status", json);
    let project = repo::load_from_path(path).unwrap_or_else(|e| fail_return(&e));

    // Replay integrity: the one-line truth a stranger checks first.
    let replay = vela_protocol::reducer::verify_replay(&project);
    let replay_line = if replay.ok {
        "reproduced".to_string()
    } else {
        format!("DIVERGED ({} diff(s))", replay.diffs.len())
    };

    // Production state: live leases, attestations, registrations.
    let now_iso = chrono::Utc::now().to_rfc3339();
    let live_leases: Vec<&vela_protocol::project::AttemptClaim> = project
        .attempt_claims
        .iter()
        .filter(|c| {
            chrono::DateTime::parse_from_rfc3339(&c.claimed_at)
                .map(|t| {
                    (t + chrono::Duration::seconds(c.lease_ttl_seconds as i64)).to_rfc3339()
                        > now_iso
                })
                .unwrap_or(false)
        })
        .collect();
    let attestation_count = project.statement_attestations.len();
    let registration_count = project.statement_registrations.len();
    let last_event_ts = project.events.iter().map(|e| e.timestamp.as_str()).max();

    // Inbox counts.
    let mut pending_total = 0usize;
    let mut pending_by_kind: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for p in &project.proposals {
        if p.status == "pending_review" {
            pending_total += 1;
            *pending_by_kind.entry(p.kind.clone()).or_insert(0) += 1;
        }
    }
    let review_page = crate::review_material::ReviewProjection::page(
        path,
        crate::review_material::ReviewRequest {
            limit: Some(5),
            ..crate::review_material::ReviewRequest::default()
        },
    );
    let review_projection = match &review_page {
        Ok(page) => json!({
            "snapshot_root": page.snapshot_root,
            "event_log_root": page.event_log_root,
            "observed_at": page.observed_at,
            "total": page.total,
            "returned": page.returned,
            "items": page.items,
            "next_cursor": page.next_cursor,
        }),
        Err(error) => json!({
            "error": {"code": error.code, "message": error.message},
        }),
    };

    // The memo's epistemic vector: never collapse into one green check.
    // claimed / evidence-attached / contested / refuted / retracted / stale
    // are DIFFERENT states, and an agent reading --json gets each count.
    let mut by_status: std::collections::BTreeMap<String, usize> = Default::default();
    let mut with_evidence = 0usize;
    for f in &project.findings {
        let s = if f.flags.retracted {
            "retracted"
        } else if f.flags.contested {
            "contested"
        } else if f.flags.superseded {
            "superseded"
        } else {
            "accepted"
        };
        *by_status.entry(s.to_string()).or_default() += 1;
        if !f.evidence.evidence_spans.is_empty()
            || f.provenance.url.as_deref().is_some_and(|u| !u.is_empty())
            || f.provenance.doi.as_deref().is_some_and(|d| !d.is_empty())
        {
            with_evidence += 1;
        }
    }
    let verdicts: std::collections::BTreeMap<String, usize> = {
        let mut m: std::collections::BTreeMap<String, usize> = Default::default();
        for a in &project.statement_attestations {
            *m.entry(format!("{:?}", a.verdict).to_lowercase())
                .or_default() += 1;
        }
        m
    };

    // Compounding block (v0.736): is accepted state running on policy rails,
    // is failure landing channel-attributed, is context being reused — plus
    // the curated channel map when a channels.yaml sits next to the frontier.
    let compounding = vela_edge::frontier_health::compounding_metrics(&project);
    let (channels_cold, channels_total) =
        match vela_edge::channel_map::ChannelTaxonomy::load_for_frontier(path) {
            Some(taxonomy) => {
                let map = vela_edge::channel_map::channel_map(&project, &taxonomy);
                let cold = map
                    .iter()
                    .filter(|c| c.status == vela_edge::channel_map::ChannelState::Cold)
                    .count();
                (cold, map.len())
            }
            None => (0, 0),
        };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "command": "status",
                "frontier": frontier_label(&project),
                "vfr_id": project.frontier_id(),
                "replay": {"ok": replay.ok, "diffs": replay.diffs.len()},
                "findings": {
                    "total": project.findings.len(),
                    "by_status": by_status,
                    "with_evidence": with_evidence,
                },
                "judgment": {
                    "statement_attestations": project.statement_attestations.len(),
                    "by_verdict": verdicts,
                },
                "proof": {
                    "status": project.proof_state.latest_packet.status,
                },
                "policy": match vela_protocol::acceptance_policy::load_active_policy_snapshot(path) {
                    Ok(snapshot) => match snapshot.mode {
                        vela_protocol::acceptance_policy::ActivePolicyMode::Active => {
                            let vp = snapshot.verified.expect("active mode carries VerifiedPolicy");
                            json!({"id": vp.policy.id, "mode": "live"})
                        }
                        vela_protocol::acceptance_policy::ActivePolicyMode::StagedUnsigned => {
                            json!({"mode": "staged", "next": "vela policy sign"})
                        }
                        vela_protocol::acceptance_policy::ActivePolicyMode::Absent => {
                            json!({"mode": "shadow"})
                        }
                    },
                    Err(e) => json!({"mode": "BROKEN", "error": e}),
                },
                "events": project.events.len(),
                "actors": project.actors.len(),
                "compounding": {
                    "autonomy_ratio": compounding.autonomy_ratio,
                    "dead_channel_coverage": compounding.dead_channel_coverage,
                    "unlock_yield_last": compounding.unlock_yield_last,
                    "context_reuse_ratio": compounding.context_reuse_ratio,
                    "attempts_avoided": compounding.attempts_avoided,
                    "channels": {"cold": channels_cold, "total": channels_total},
                },
                "inbox": {
                    "pending_total": pending_total,
                    "pending_by_kind": pending_by_kind,
                    "review": review_projection,
                },
                "unpublished_store_files": unpublished_store_files(path),
                "next": if pending_total > 0 {
                    json!(format!(
                        "{pending_total} pending proposal(s) await a human key: `vela sign`"
                    ))
                } else if !replay.ok {
                    json!("replay DIVERGED: run `vela check .` and inspect")
                } else if project.proof_state.latest_packet.status == "stale" {
                    json!("proof packet stale: `vela frontier materialize .`")
                } else {
                    json!(null)
                },
            }))
            .expect("serialize status")
        );
        return;
    }

    println!();
    println!(
        "  {}",
        format!("VELA · STATUS · {}", path.display())
            .to_uppercase()
            .dimmed()
    );
    println!("  {}", style::tick_row(60));
    println!();
    println!("  frontier:    {}", frontier_label(&project));
    println!("  vfr_id:      {}", project.frontier_id());
    println!(
        "  replay:      {}",
        if replay.ok {
            style::ok(&replay_line)
        } else {
            style::warn(&replay_line)
        }
    );
    println!("  last event:  {}", last_event_ts.unwrap_or("none"));
    if !live_leases.is_empty() {
        println!("  leases:      {} live", live_leases.len());
        for l in live_leases.iter().take(5) {
            let remaining = chrono::DateTime::parse_from_rfc3339(&l.claimed_at)
                .ok()
                .map(|t| {
                    (t + chrono::Duration::seconds(l.lease_ttl_seconds as i64))
                        .signed_duration_since(chrono::Utc::now())
                })
                .map(|d| {
                    let m = d.num_minutes().max(0);
                    if m >= 60 {
                        format!("expires in {}h{:02}m", m / 60, m % 60)
                    } else {
                        format!("expires in {m}m")
                    }
                })
                .unwrap_or_else(|| format!("ttl {}s", l.lease_ttl_seconds));
            println!(
                "    · {}  {}  ({remaining})",
                l.obligation_id, l.claimant_actor
            );
        }
        if live_leases.len() > 5 {
            println!("    … +{} more", live_leases.len() - 5);
        }
    }
    if attestation_count + registration_count > 0 {
        println!(
            "  judgment:    {attestation_count} statement attestation(s), {registration_count} registration(s)"
        );
    }
    {
        let vec_line: Vec<String> = by_status.iter().map(|(k, v)| format!("{v} {k}")).collect();
        if !vec_line.is_empty() {
            println!("  state:       {}", vec_line.join(" · "));
        }
    }
    println!(
        "  findings:    {}    events: {}    actors: {}",
        project.findings.len(),
        project.events.len(),
        project.actors.len(),
    );
    let pct = |r: f64| (r * 100.0).round() as i64;
    println!(
        "  compounding: autonomy {}% · channels {channels_cold}/{channels_total} cold · reuse {}%",
        pct(compounding.autonomy_ratio),
        pct(compounding.context_reuse_ratio),
    );
    println!();
    match vela_protocol::acceptance_policy::load_active_policy_snapshot(path) {
        Ok(snapshot) => match snapshot.mode {
            vela_protocol::acceptance_policy::ActivePolicyMode::Active => println!(
                "  policy:      live ({})",
                snapshot
                    .verified
                    .expect("active mode carries VerifiedPolicy")
                    .policy
                    .id
            ),
            vela_protocol::acceptance_policy::ActivePolicyMode::StagedUnsigned => println!(
                "  policy:      {}",
                style::warn("staged — one signature activates it")
            ),
            vela_protocol::acceptance_policy::ActivePolicyMode::Absent => {}
        },
        Err(error) => println!(
            "  policy:      {}",
            style::lost(&format!("BROKEN — {error}"))
        ),
    }
    let unpublished = unpublished_store_files(path);
    if unpublished > 0 {
        println!(
            "  {}  {unpublished} store file(s) changed but not committed — signed state that exists only on this machine",
            style::warn("unpublished")
        );
        println!("             publish: git add -A && git commit && git push");
    }
    if pending_total > 0 {
        println!(
            "  {}  {pending_total} pending proposals",
            style::warn("sign queue")
        );
        for (k, n) in &pending_by_kind {
            println!("    · {n:>3}  {k}");
        }
        match &review_page {
            Ok(page) => {
                for item in &page.items {
                    let accept = item
                        .brief
                        .action("accept")
                        .map(|action| action.eligibility.as_str())
                        .unwrap_or("unavailable");
                    let reject = item
                        .brief
                        .action("reject")
                        .map(|action| action.eligibility.as_str())
                        .unwrap_or("unavailable");
                    println!(
                        "    · {}  {}",
                        item.brief.audit.proposal_id,
                        crate::cli::safe_text::inline(&item.brief.change.claim)
                    );
                    println!(
                        "      {} · accept {accept} · reject {reject} · facts {}",
                        item.brief.authority.route, item.brief.audit.decision_facts_root
                    );
                }
                if page.next_cursor.is_some() {
                    println!(
                        "    … {} more available through `vela sign`",
                        page.total - page.returned
                    );
                }
            }
            Err(error) => println!(
                "    · review projection unavailable: {}",
                crate::cli::safe_text::inline(&error.to_string())
            ),
        }
        println!();
        println!("  next:  vela sign   (one session, one confirm, one key read)");
    } else if !replay.ok {
        println!("  {}  replay diverged", style::warn("!!"));
        println!();
        println!("  next:  vela check . --strict");
    } else {
        println!("  {}  sign queue clean", style::ok("ok"));
    }
    println!();
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

pub(crate) async fn cmd_doctor(frontier: Option<&Path>, port: u16, json_output: bool) {
    let report = doctor::run(frontier, port);
    // The local setup/ceremony lane lives crate-side (identity, pin,
    // policy freshness, adapters, registry) and merges into the report.
    let frontier_dir = if report.frontier_load_ok {
        Some(std::path::PathBuf::from(&report.frontier_path))
    } else {
        None
    };
    let setup = crate::config::doctor_setup::run(frontier_dir.as_deref());
    if json_output {
        let mut merged = serde_json::to_value(&report).unwrap_or_default();
        if let Some(obj) = merged.as_object_mut() {
            obj.insert(
                "setup".to_string(),
                serde_json::to_value(&setup).unwrap_or_default(),
            );
        }
        print_json(&merged);
    } else {
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
    }
    if !report.blocking.is_empty() {
        std::process::exit(1);
    }
}
