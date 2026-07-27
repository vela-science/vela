use crate::cli::{fail, fail_return, fmt_timestamp, frontier_dir_for_source, print_json};
use colored::Colorize;
use serde_json::json;
use sha2::Digest;
use std::path::Path;
use vela_edge::doctor;
use vela_edge::packet;
use vela_protocol::cli_style as style;
use vela_protocol::repo;

/// Derive only strict-blocking counts from the canonical strict-check payload.
///
/// The strict checker deliberately retains advisory warnings beside failures.
/// Compact status must not relabel those warnings as blockers: readers use
/// `blocker_count` and `blockers_by_code` as the machine contract for the
/// fail-closed bar. The complete warning totals remain available in
/// `integrity.strict_check`.
fn strict_blocker_counts(
    strict_check: &serde_json::Value,
) -> std::collections::BTreeMap<String, usize> {
    let mut blockers_by_code = std::collections::BTreeMap::<String, usize>::new();
    for signal in strict_check
        .get("signals")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|signal| {
            !signal
                .get("id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| id.starts_with("sig_diagnostic_"))
                && signal
                    .get("blocks")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|blocks| {
                        blocks
                            .iter()
                            .any(|block| block.as_str() == Some("strict_check"))
                    })
        })
    {
        let kind = signal
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("strict_signal");
        *blockers_by_code.entry(kind.to_string()).or_default() += 1;
    }
    if let Some(checks) = strict_check
        .get("checks")
        .and_then(serde_json::Value::as_array)
    {
        for check in checks {
            let id = check
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("strict_check");
            let failed = check
                .get("failed")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default() as usize;
            // These categories already retain their more useful product-level
            // codes above. Avoid double-counting them while still deriving the
            // pass/fail result from the canonical strict checker.
            if failed > 0
                && !matches!(
                    id,
                    "signals"
                        | "events"
                        | "active_policy"
                        | "policy_readiness"
                        | "repository_context"
                )
            {
                *blockers_by_code.entry(id.to_string()).or_default() += failed;
            }
        }
    }
    blockers_by_code
}

/// Build the stable status projection without printing or mutating state.
///
/// `trusted_home` exists only for deterministic tests. Production passes
/// `None`, which resolves the operating-system account home through the same
/// hardened boundary as canonical writes. Profile v1 work projection receives
/// the exact independently retained repository trust anchor only after the
/// complete repository context has verified.
pub(crate) fn compact_status_payload_with_home(
    path: &Path,
    observed_at: &str,
    trusted_home: Option<&Path>,
) -> Result<serde_json::Value, String> {
    let frontier_dir = frontier_dir_for_source(path);
    let project = vela_protocol::repo::load_from_path(frontier_dir)?;
    let repository_context = crate::cli::repository_context_assessment_with_project_and_home(
        frontier_dir,
        Some(&project),
        trusted_home,
    );
    let repository_context_valid = repository_context
        .payload
        .get("valid")
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    let repository_not_applicable = repository_context
        .payload
        .get("status")
        .and_then(serde_json::Value::as_str)
        == Some("not_applicable");
    // A successfully loaded standalone v0.1 snapshot has no repository
    // boundary to verify. Keep that historical read surface deliberately,
    // while every Profile v1 repository still requires a valid context.
    let repository_acceptable = repository_context_valid || repository_not_applicable;
    let repository_generation = repository_context
        .payload
        .get("generation")
        .and_then(serde_json::Value::as_str)
        .or(repository_not_applicable.then_some("standalone_v0_1"));

    let active_policy = vela_protocol::acceptance_policy::load_active_policy_snapshot(frontier_dir);
    let replay = vela_protocol::reducer::verify_replay(&project);
    let policy = vela_protocol::proposals::policy_accept::assess_policy_readiness(
        &project,
        active_policy.as_ref().map_err(String::as_str),
        observed_at,
    );
    let policy_ok = policy.permit_readiness()
        != vela_protocol::proposals::policy_accept::PermitReadiness::Blocked;
    // Strict verification and Target Index assessment are independent,
    // read-only projections over the same already verified repository
    // context. Run them together: a large Frontier should pay for both
    // boundaries, but it need not pay their wall times serially.
    let (strict_check, work_projection_result) = std::thread::scope(|scope| {
        let strict = scope.spawn(|| {
            // `status` is the compact product projection of the same
            // fail-closed bar as `check --strict`. Do not independently
            // approximate that bar here.
            crate::cli::check_json_payload_with_preloaded(
                frontier_dir,
                false,
                true,
                Some(&project),
                repository_context.payload.clone(),
            )
        });
        let work = scope.spawn(|| {
            if repository_acceptable {
                vela_edge::frontier_next::try_frontier_next_projection_with_trust_anchor_and_authority(
                    &project,
                    Some(frontier_dir),
                    observed_at,
                    1,
                    repository_context.target_index_trust_anchor.as_ref(),
                    &repository_context.authority_events,
                )
                .map(Some)
            } else {
                Ok(None)
            }
        });
        let strict_check = strict
            .join()
            .map_err(|_| "strict status projection panicked".to_string())?;
        let work_projection_result = work
            .join()
            .map_err(|_| "work status projection panicked".to_string())?;
        Ok::<_, String>((strict_check, work_projection_result))
    })?;
    let strict_check_ok = strict_check.get("ok").and_then(serde_json::Value::as_bool) == Some(true);
    // Compact status counts the intrinsic signals already produced by the
    // canonical check and adds check categories below. Diagnostic-derived
    // signals have a reserved id prefix and are excluded here so repository,
    // schema, and replay failures are not double-counted.
    let mut blockers_by_code = strict_blocker_counts(&strict_check);
    if !replay.ok {
        *blockers_by_code
            .entry("reducer_replay_failed".to_string())
            .or_default() += 1;
    }
    if !policy_ok {
        *blockers_by_code
            .entry("policy_readiness_blocked".to_string())
            .or_default() += 1;
    }
    if !repository_acceptable {
        let code = repository_context
            .payload
            .get("code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("repository_context_invalid");
        *blockers_by_code.entry(code.to_string()).or_default() += 1;
    }
    if !strict_check_ok && blockers_by_code.is_empty() {
        // Defensive fallback for any future strict-check category that has
        // not yet gained a compact status code. Never turn an unknown strict
        // failure into a pass.
        blockers_by_code.insert("strict_check_failed".into(), 1);
    }

    let pending_review = project
        .proposals
        .iter()
        .filter(|proposal| proposal.status == "pending_review")
        .count();

    // A failed repository context grants no Target Index v2 projection. The
    // same independently retained boundary pin that validated the repository
    // is threaded into the projection; no pin is inferred from targets.json
    // or any other repository-controlled bytes.
    let mut target_index_error = None;
    let work_projection = match work_projection_result {
        Ok(projection) => projection,
        Err(error) => {
            *blockers_by_code
                .entry("target_index_projection_invalid".to_string())
                .or_default() += 1;
            target_index_error = Some(error);
            None
        }
    };
    let open_work = work_projection
        .as_ref()
        .map_or(0, |projection| projection.producer_work.configured_open);
    let available_work = work_projection
        .as_ref()
        .map_or(0, |projection| projection.producer_work.available);
    let leased_work = work_projection
        .as_ref()
        .map_or(0, |projection| projection.producer_work.leased);
    let stale_work = work_projection
        .as_ref()
        .map_or(0, |projection| projection.producer_work.stale);

    let git_text = |args: &[&str]| crate::git_hardened::text(frontier_dir, args).ok();
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
    let actor_registry_root = match std::fs::read(&actor_registry_path) {
        Ok(bytes) => format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes))),
        Err(_) => vela_protocol::canonical::to_canonical_bytes(&project.actors)
            .map(|bytes| format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes))))
            .map_err(|error| format!("canonicalize actor registry: {error}"))?,
    };
    let artifact_root = vela_protocol::canonical::to_canonical_bytes(&project.artifacts)
        .map(|bytes| format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes))))?;
    let legacy_snapshot_root =
        status_legacy_snapshot_root(frontier_dir, &project, repository_generation)?;
    let scientific_state_root =
        if repository_generation == Some("profile_v1") && repository_context_valid {
            repository_context
                .payload
                .get("scientific_state_root")
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };

    let blocker_count = blockers_by_code.values().sum::<usize>();
    let status_ok = strict_check_ok
        && replay.ok
        && policy_ok
        && repository_acceptable
        && target_index_error.is_none();
    let next_action = if !strict_check_ok || !replay.ok || !repository_acceptable {
        "vela check . --strict"
    } else if !policy_ok {
        "vela doctor . --all --json"
    } else if target_index_error.is_some() {
        "vela target-index repair . --json"
    } else if available_work > 0 || leased_work > 0 {
        "vela next . --json"
    } else if pending_review > 0 {
        "vela review list . --json"
    } else {
        "none"
    };

    Ok(json!({
        "ok": status_ok,
        "command": "status",
        "schema": "vela.status.v2",
        "frontier": {
            "id": project.frontier_id(),
            "name": project.project.name,
            "profile_generation": repository_generation,
        },
        "git": {
            "commit": git_commit,
            "tree": git_tree,
            "clean": git_clean,
        },
        "roots": {
            "event_log": format!("sha256:{}", vela_protocol::events::event_log_hash(&project.events)),
            "scientific_state_root": scientific_state_root,
            "legacy_snapshot_root": legacy_snapshot_root,
            "proposals": format!("sha256:{}", vela_protocol::proposals::proposal_state_hash(&project.proposals)),
            "actor_registry": actor_registry_root,
            "artifacts": artifact_root,
        },
        "integrity": {
            "replay": if replay.ok { "reproduced" } else { "diverged" },
            "replay_diffs": replay.diffs.len(),
            "strict": if strict_check_ok { "pass" } else { "blocked" },
            "blocker_count": blocker_count,
            "blockers_by_code": blockers_by_code,
            "strict_check": strict_check.get("summary").cloned().unwrap_or(serde_json::Value::Null),
            "repository_context": repository_context.payload,
            "target_index_error": target_index_error,
        },
        "counts": {
            "events": project.events.len(),
            "claims": project.findings.len(),
            "open_work": open_work,
            "available_work": available_work,
            "leased_work": leased_work,
            "stale_work": stale_work,
            "pending_review": pending_review,
        },
        "policy": {
            "state": policy.state().as_str(),
            "permit_readiness": policy.permit_readiness().as_str(),
            "reason_codes": policy.reason_codes(),
            "error": policy.detail(),
        },
        "next_action": next_action,
    }))
}

fn status_legacy_snapshot_root(
    frontier_dir: &Path,
    project: &vela_protocol::project::Project,
    repository_generation: Option<&str>,
) -> Result<String, String> {
    if repository_generation == Some("profile_v1")
        && let Some(vela_protocol::frontier_repo::FrontierLockFile::V1(lock)) =
            vela_protocol::frontier_repo::read_repository_lock(frontier_dir)?
    {
        // This is the exact compatibility projection tracked by the checkout.
        // A compatible newer reader may derive a different broad Project hash
        // from its own display-only compiler metadata while still validating
        // the lock-pinned historical view. Report the retained root that
        // `check --strict` actually verified, not the reader-local derivative.
        return Ok(lock.legacy_snapshot_root);
    }
    Ok(format!(
        "sha256:{}",
        vela_protocol::events::snapshot_hash(project)
    ))
}

/// Stable compact status contract for the 0.9 product surface.
pub(crate) fn cmd_status_compact(path: &Path, json_out: bool) {
    if path.join(".vela/epoch.json").is_file() {
        crate::repository_upgrade::cmd_current_status(path, json_out);
        return;
    }
    crate::ui::set_mode("status", json_out);
    let frontier_dir = frontier_dir_for_source(path);
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier_dir)
        .unwrap_or_else(|error| fail_return(&error));
    crate::frontier_txn::FrontierTxn::verify_recovery_barrier_read_only(frontier_dir, &journal_dir)
        .unwrap_or_else(|error| fail_return(&error.to_string()));
    let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let payload = compact_status_payload_with_home(path, &observed_at, None)
        .unwrap_or_else(|error| fail_return(&error));
    crate::frontier_txn::FrontierTxn::verify_recovery_barrier_read_only(frontier_dir, &journal_dir)
        .unwrap_or_else(|error| fail_return(&error.to_string()));
    if json_out {
        print_json(&payload);
    } else {
        println!(
            "vela status · {}",
            payload["frontier"]["name"].as_str().unwrap_or("frontier")
        );
        println!(
            "  frontier  {}",
            payload["frontier"]["id"].as_str().unwrap_or("unavailable")
        );
        println!(
            "  commit    {}",
            payload["git"]["commit"].as_str().unwrap_or("unavailable")
        );
        println!(
            "  replay    {}",
            payload["integrity"]["replay"].as_str().unwrap_or("unknown")
        );
        println!(
            "  strict    {} blocker(s)",
            payload["integrity"]["blocker_count"]
                .as_u64()
                .unwrap_or_default()
        );
        for (code, count) in payload["integrity"]["blockers_by_code"]
            .as_object()
            .into_iter()
            .flatten()
        {
            println!("            {} {}", count, code);
        }
        println!("  events    {}", payload["counts"]["events"]);
        println!("  claims    {}", payload["counts"]["claims"]);
        println!(
            "  work      {} available · {} leased · {} configured open",
            payload["counts"]["available_work"],
            payload["counts"]["leased_work"],
            payload["counts"]["open_work"],
        );
        println!(
            "  review    {} pending",
            payload["counts"]["pending_review"]
        );
        println!(
            "  policy    {} · Permit {}",
            payload["policy"]["state"].as_str().unwrap_or("unknown"),
            payload["policy"]["permit_readiness"]
                .as_str()
                .unwrap_or("unknown")
        );
        if let Some(detail) = payload["policy"]["error"].as_str() {
            println!("            {detail}");
        }
        println!(
            "  next      {}",
            payload["next_action"].as_str().unwrap_or("none")
        );
    }
    if payload.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
        std::process::exit(1);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_status_never_relabels_strict_check_warnings_as_blockers() {
        let strict_check = json!({
            "signals": [
                {
                    "id": "sig_missing_condition",
                    "kind": "missing_conditions",
                    "blocks": ["strict_check"]
                },
                {
                    "id": "sig_advisory",
                    "kind": "signals_warning",
                    "blocks": []
                },
                {
                    "id": "sig_diagnostic_state_integrity",
                    "kind": "state_integrity",
                    "blocks": ["strict_check"]
                }
            ],
            "checks": [
                {"id": "signals", "failed": 1, "warnings": 3},
                {"id": "state_integrity", "failed": 2, "warnings": 7},
                {"id": "frontier_graph", "failed": 0, "warnings": 9}
            ]
        });

        let blockers = strict_blocker_counts(&strict_check);
        assert_eq!(blockers.get("missing_conditions"), Some(&1));
        assert_eq!(blockers.get("state_integrity"), Some(&2));
        assert_eq!(blockers.values().sum::<usize>(), 3);
        assert!(!blockers.keys().any(|code| code.ends_with("_warning")));
        assert!(!blockers.contains_key("signals_warning"));
    }

    #[test]
    fn compact_status_reports_the_profile_lock_legacy_snapshot_root() {
        let directory = tempfile::tempdir().expect("tempdir");
        vela_protocol::frontier_repo::initialize_profile_v1_minimal(
            directory.path(),
            vela_protocol::frontier_repo::ProfileV1InitOptions {
                name: "Status root fixture",
                scope: "Does status report the exact compatibility root retained by this checkout?",
                initialize_git: false,
            },
        )
        .expect("initialize fixture");
        let Some(vela_protocol::frontier_repo::FrontierLockFile::V1(lock)) =
            vela_protocol::frontier_repo::read_repository_lock(directory.path())
                .expect("read lock")
        else {
            panic!("expected Profile v1 lock");
        };
        let mut project =
            vela_protocol::repo::load_from_path(directory.path()).expect("load fixture");
        project.project.compiler = "vela/a-different-compatible-reader".to_string();
        let reader_local_root =
            format!("sha256:{}", vela_protocol::events::snapshot_hash(&project));
        assert_ne!(reader_local_root, lock.legacy_snapshot_root);

        let observed = status_legacy_snapshot_root(directory.path(), &project, Some("profile_v1"))
            .expect("derive status root");
        assert_eq!(observed, lock.legacy_snapshot_root);
    }
}
