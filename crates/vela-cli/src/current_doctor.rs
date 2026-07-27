//! Product diagnostic for Profile v2/current repository epochs.
//!
//! The historical doctor belongs to the Project snapshot runtime. A migrated
//! repository has no Project, frozen policy, proof snapshot, actor registry, or
//! embedded workbench to diagnose. This module checks only current product
//! boundaries and returns one useful next action.

use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use vela_protocol::repository_epoch::RepositoryEpochV1;

fn git_text(frontier: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(frontier)
        .args(args)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| format!("git {} returned non-UTF-8 output", args.join(" ")))
        .map(|value| value.trim().to_string())
}

fn target_index_diagnostic(
    frontier: &Path,
    repository: &vela_protocol::current_repository::CurrentRepositoryV2,
    repository_root: &str,
) -> Result<Value, String> {
    let assessment = vela_edge::target_index::assess_current_target_index(
        frontier,
        &repository.frontier_id,
        &repository.epoch_id,
        repository_root,
    )?;
    let Some(assessment) = assessment else {
        return Ok(json!({
            "status": "not_configured",
            "configured": 0,
            "fresh": 0,
            "issues": [],
        }));
    };
    let mut issues = assessment
        .global_issues
        .iter()
        .map(|issue| issue.code)
        .collect::<Vec<_>>();
    issues.extend(
        assessment
            .target_issues
            .values()
            .flatten()
            .map(|issue| issue.code),
    );
    issues.sort_unstable();
    issues.dedup();
    Ok(json!({
        "status": if issues.is_empty() { "ready" } else { "blocked" },
        "root": assessment.index.index_root,
        "configured": assessment.configured_open(),
        "fresh": assessment.fresh_open_targets().len(),
        "issues": issues,
    }))
}

fn trust_diagnostic(
    frontier: &Path,
    repository: &vela_protocol::current_repository::CurrentRepositoryV2,
    epoch: &RepositoryEpochV1,
) -> Result<Value, String> {
    let authority = crate::cli::load_current_repository_authority(frontier, repository, epoch)?;
    let first_root = authority
        .verification
        .first_authority_record_root
        .as_deref()
        .ok_or_else(|| "current authority history has no sequence-one record".to_string())?;
    let home =
        crate::frontier_txn::operating_system_account_home().map_err(|error| error.to_string())?;
    let loaded = vela_edge::repository_write::load_authority_trust_anchor_from_home(
        &home,
        &repository.frontier_id,
    )?;
    let (status, detail) = match loaded {
        Some(anchor)
            if anchor
                .anchor
                .verify_sequence_one(&repository.frontier_id, first_root)
                .is_ok() =>
        {
            ("pinned", "independent sequence-one root matches")
        }
        Some(anchor)
            if crate::frontier_txn::authority_anchor_selects_current_epoch(
                &anchor.anchor,
                &repository.frontier_id,
                first_root,
                &epoch.predecessor_roots.authority_head,
            ) =>
        {
            (
                "predecessor_pinned",
                "the independently pinned predecessor sequence-one record is also the signed epoch authority head",
            )
        }
        Some(_) => ("blocked", "local trust anchor does not match sequence one"),
        None => (
            "unpinned",
            "authority history verifies but is not independently pinned",
        ),
    };
    Ok(json!({
        "status": status,
        "detail": detail,
        "sequence_one_record_root": first_root,
        "record_count": authority.verification.authority_record_count,
        "head_record_root": authority.verification.final_authority_record_root,
        "event_log_root": authority.verification.final_event_log_root,
    }))
}

fn current_doctor_payload(frontier: &Path, all: bool) -> Result<Value, String> {
    let frontier = frontier
        .canonicalize()
        .map_err(|error| format!("resolve current Frontier {}: {error}", frontier.display()))?;
    let repository = crate::repository_upgrade::verify_current_repository_at(&frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let epoch_bytes = std::fs::read(frontier.join(".vela/epoch.json"))
        .map_err(|error| format!("read current repository epoch: {error}"))?;
    let epoch = RepositoryEpochV1::parse(&epoch_bytes)?;
    let epoch_root = epoch.canonical_root()?;
    let commit = git_text(&frontier, &["rev-parse", "HEAD^{commit}"])?;
    let tree = git_text(&frontier, &["rev-parse", "HEAD^{tree}"])?;
    let tracked_dirt = git_text(
        &frontier,
        &["status", "--porcelain", "--untracked-files=no"],
    )?;
    let git_status = if tracked_dirt.is_empty() {
        "clean"
    } else {
        "tracked_dirt"
    };
    let decisions =
        crate::repository_upgrade::load_current_proposal_decisions(&frontier, &repository)?;
    let pending = repository
        .proposals
        .iter()
        .filter(|proposal| !decisions.contains_key(&proposal.id))
        .collect::<Vec<_>>();
    let target_index = target_index_diagnostic(&frontier, &repository, &repository_root)?;
    let authority = trust_diagnostic(&frontier, &repository, &epoch)?;
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve Vela executable: {error}"))?;
    let binary_sha256 = crate::authority_transaction::execution_binary_sha256(&executable)?;

    let mut blockers = Vec::new();
    if git_status != "clean" {
        blockers.push("tracked_worktree_dirt");
    }
    if authority["status"] == "blocked" {
        blockers.push("authority_trust_mismatch");
    }
    if target_index["status"] == "blocked" {
        blockers.push("target_index_invalid");
    }
    let next_action = if blockers.contains(&"authority_trust_mismatch") {
        format!(
            "vela authority trust pin {} --record-root {} --json",
            frontier.display(),
            authority["sequence_one_record_root"]
                .as_str()
                .unwrap_or("<full-sequence-one-root>")
        )
    } else if !blockers.is_empty() {
        format!("vela check {} --json", frontier.display())
    } else if let Some(proposal) = pending.first() {
        format!(
            "vela review show {} {} --json",
            frontier.display(),
            proposal.id
        )
    } else if target_index["fresh"].as_u64().unwrap_or_default() > 0 {
        format!("vela next {} --limit 1 --json", frontier.display())
    } else {
        format!("vela check {} --json", frontier.display())
    };

    let mut payload = json!({
        "schema": "vela.doctor.v2",
        "ok": blockers.is_empty(),
        "command": "doctor",
        "binary": {
            "version": env!("CARGO_PKG_VERSION"),
            "path": executable,
            "sha256": binary_sha256,
        },
        "frontier": {
            "path": frontier,
            "id": repository.frontier_id,
            "profile": "current",
            "load": "verified",
        },
        "git": {
            "commit": commit,
            "tree": tree,
            "status": git_status,
        },
        "roots": {
            "epoch": epoch_root,
            "repository": repository_root,
            "authority_keyset": repository.authority_keyset_root,
            "authority_policy": repository.authority_policy_root,
        },
        "authority": authority,
        "target_index": target_index,
        "counts": {
            "accepted_claims": repository.accepted_claims.len(),
            "pending_claims": repository.pending_claims.len(),
            "pending_review": pending.len(),
            "submissions": repository.submissions.len(),
            "registrations": repository.registrations.len(),
            "verifications": repository.verifications.len(),
            "artifacts": repository.artifacts.len(),
        },
        "blockers": blockers,
        "next_action": next_action,
        "legacy_runtime_used": false,
    });
    if all {
        payload["details"] = json!({
            "epoch": epoch,
            "decisions": decisions,
            "serve": "retired_from_current_product",
            "historical_policy": "available_only_through_the_pinned_predecessor_release",
            "historical_actor_registry": "available_only_through_the_pinned_predecessor_release",
        });
    }
    Ok(payload)
}

pub(crate) fn cmd_current_doctor(frontier: &Path, all: bool, json_out: bool) {
    crate::ui::set_mode("doctor", json_out);
    let payload = current_doctor_payload(frontier, all)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    if json_out {
        crate::cli::print_json(&payload);
    } else {
        println!(
            "vela doctor · {}",
            payload["frontier"]["id"].as_str().unwrap_or("frontier")
        );
        println!("  repository  verified");
        println!(
            "  git         {}",
            payload["git"]["status"].as_str().unwrap_or("unknown")
        );
        println!(
            "  authority   {}",
            payload["authority"]["status"].as_str().unwrap_or("unknown")
        );
        println!(
            "  targets     {}",
            payload["target_index"]["status"]
                .as_str()
                .unwrap_or("unknown")
        );
        println!(
            "  blockers    {}",
            payload["blockers"]
                .as_array()
                .filter(|items| !items.is_empty())
                .map(|items| items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", "))
                .unwrap_or_else(|| "none".into())
        );
        println!(
            "  next        {}",
            payload["next_action"].as_str().unwrap_or("none")
        );
    }
    if payload["ok"] != true {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_output_is_trimmed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let top = git_text(root, &["rev-parse", "--show-toplevel"]).unwrap();
        assert!(!top.ends_with('\n'));
        assert!(Path::new(&top).is_absolute());
    }
}
