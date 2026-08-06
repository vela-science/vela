//! Current repository verification, reads, work offers, and review views.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::authority::AuthorityEventV1;
use vela_protocol::authority_history::AuthorityInitializationV1;
use vela_protocol::claim_record::ClaimRecordV1;
use vela_protocol::current_repository::{
    ClaimStandingRefV1, CurrentFrontierProfileV2, CurrentRepositoryV4, RepositoryObjectRefV1,
};
use vela_protocol::events::{EventKind, NULL_HASH};
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::proposal_withdrawal_v1::ProposalWithdrawalV1;
use vela_protocol::repository_origin::RepositoryOriginV1;
use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::VerificationRecordV1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CurrentProposalDecision {
    pub(crate) standing: String,
    pub(crate) event_id: String,
    pub(crate) event_root: String,
    pub(crate) decided_at: String,
    pub(crate) actor: String,
    pub(crate) reason: String,
    pub(crate) applied_event_id: Option<String>,
}

pub(crate) fn cmd_replay_repository(frontier: &Path, json_out: bool) {
    crate::ui::set_mode("replay", json_out);
    let frontier = crate::ui::canonicalize_frontier(frontier);
    let sensitive = sensitive_paths(&frontier);
    if !sensitive.is_empty() {
        let listed = sensitive
            .iter()
            .take(10)
            .map(|path| {
                path.strip_prefix(&frontier)
                    .unwrap_or(path)
                    .display()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(", ");
        crate::cli::fail_return::<()>(&format!(
            "current repository contains sensitive-looking files: {listed}"
        ));
    }
    let repository = verify_current_repository_at(&frontier, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let origin = RepositoryOriginV1::parse(
        &fs::read(frontier.join(".vela/origin.json")).unwrap_or_else(|error| {
            crate::cli::fail_return(&format!("read current repository origin: {error}"))
        }),
    )
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let commit = git_text(&frontier, &["rev-parse", "HEAD^{commit}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let tree = git_text(&frontier, &["rev-parse", "HEAD^{tree}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let payload = json!({
        "schema": "vela.repository-verification.v2",
        "ok": true,
        "command": "replay",
        "frontier": frontier.display().to_string(),
        "frontier_id": repository.frontier_id,
        "git_commit": commit,
        "git_tree": tree,
        "origin_id": origin.origin_id,
        "origin_root": origin.canonical_root()
            .unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "repository_root": repository.canonical_root().unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "authority_keyset_root": repository.authority_keyset_root,
        "authority_policy_root": repository.authority_policy_root,
        "counts": {
            "accepted_claims": repository.accepted_claims.len(),
            "pending_claims": repository.pending_claims.len(),
            "proposals": repository.proposals.len(),
            "proposal_withdrawals": repository.proposal_withdrawals.len(),
            "submissions": repository.submissions.len(),
            "verifications": repository.verifications.len(),
            "artifacts": repository.artifacts.len()
        },
    });
    if json_out {
        crate::cli::print_json(&payload);
    } else {
        /* TERMINOLOGY.md forbids an unqualified "verified", so this line names
        what the replay actually matched instead of asserting a standing. */
        println!("current repository replay matched: signatures, roots, and canonical bytes");
        println!("  frontier: {}", payload["frontier_id"]);
        println!("  origin: {}", payload["origin_id"]);
        println!("  claims: {}", payload["counts"]["accepted_claims"]);
        println!("  repository root: {}", payload["repository_root"]);
    }
}

fn sensitive_paths(root: &Path) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if path.is_dir() {
                if !matches!(name, ".git" | "target" | "node_modules" | "dist" | "build") {
                    stack.push(path);
                }
                continue;
            }
            let lower = name.to_ascii_lowercase();
            if lower == "public.key" || lower.ends_with(".pub") || lower.ends_with(".pubkey") {
                continue;
            }
            let sensitive_extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "key" | "pem" | "p12" | "pfx"
                    )
                });
            if sensitive_extension
                || ["private", "secret", "credential"]
                    .iter()
                    .any(|needle| lower.contains(needle))
            {
                hits.push(path);
            }
        }
    }
    hits.sort();
    hits
}

fn decision_inbox_status_summary(
    projection: &crate::decision_inbox::DecisionInboxProjection,
) -> (Value, usize) {
    let protocol_ready_count = projection
        .entries
        .iter()
        .filter(|entry| {
            entry.readiness.protocol_gate
                == crate::decision_inbox::DecisionInboxProtocolGate::Satisfied
        })
        .count();
    let protocol_blocked_count = projection
        .entries
        .iter()
        .filter(|entry| {
            entry.readiness.protocol_gate
                == crate::decision_inbox::DecisionInboxProtocolGate::Blocked
        })
        .count();
    let pending_count = projection.entries.len();
    (
        json!({
            "pending_count": pending_count,
            "protocol_ready_count": protocol_ready_count,
            "protocol_blocked_count": protocol_blocked_count,
            "projection_root": projection.projection_root,
            "first_entry_root": projection.entries.first().map(|entry| entry.entry_root.clone()),
        }),
        pending_count,
    )
}

pub(crate) fn cmd_current_status(frontier: &Path, json_out: bool) {
    crate::ui::set_mode("status", json_out);
    let frontier = crate::ui::canonicalize_frontier(frontier);
    let profile_source =
        fs::read_to_string(frontier.join("frontier.toml")).unwrap_or_else(|error| {
            crate::cli::fail_return(&format!("read current Frontier Profile: {error}"))
        });
    let profile = CurrentFrontierProfileV2::from_toml_str(&profile_source)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    if !frontier.join(".vela/origin.json").exists()
        && !frontier.join(".vela/repository.json").exists()
    {
        verify_current_bootstrap_at(&frontier)
            .unwrap_or_else(|error| crate::cli::fail_return(&error));
        let commit = git_text(&frontier, &["rev-parse", "HEAD^{commit}"]).ok();
        let tree = git_text(&frontier, &["rev-parse", "HEAD^{tree}"]).ok();
        let next_action = format!("vela init {} --json", frontier.display());
        let payload = json!({
            "schema": "vela.status.v1",
            "ok": true,
            "command": "status",
            "frontier": {
                "id": profile.frontier_id,
                "name": profile.name,
                "profile_root": profile.profile_root()
                    .unwrap_or_else(|error| crate::cli::fail_return(&error))
            },
            "git": {"commit": commit, "tree": tree},
            "integrity": {
                "replay": "not_initialized",
                "strict": "blocked",
                "blocker_count": 1,
                "blockers_by_code": {"repository_authority_uninitialized": 1}
            },
            "roots": {
                "origin": Value::Null,
                "repository": Value::Null,
                "authority_keyset": Value::Null,
                "authority_policy": Value::Null
            },
            "counts": {
                "claims": 0,
                "accepted_claims": 0,
                "pending_claims": 0,
                "pending_review": 0,
                "accepted_review": 0,
                "rejected_review": 0,
                "withdrawn_review": 0,
                "submissions": 0,
                "verifications": 0,
                "artifacts": 0
            },
            "work": {
                "ready_target_count": 0
            },
            "decision_inbox": {
                "pending_count": 0,
                "protocol_ready_count": 0,
                "protocol_blocked_count": 0,
                "projection_root": Value::Null,
                "first_entry_root": Value::Null
            },
            "phase": "authority_uninitialized",
            "next_action": next_action,
        });
        if json_out {
            crate::cli::print_json(&payload);
        } else {
            println!("vela status · {}", payload["frontier"]["name"]);
            println!("  replay    not initialized");
            println!("  strict    blocked · repository authority uninitialized");
            println!("  next      {}", payload["next_action"]);
        }
        return;
    }
    let repository = load_current_repository_at(&frontier, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let repository_root = repository
        .canonical_root()
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let commit = git_text(&frontier, &["rev-parse", "HEAD^{commit}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let tree = git_text(&frontier, &["rev-parse", "HEAD^{tree}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let standings = load_current_proposal_standings(&frontier, &repository)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let pending_proposals = repository
        .proposals
        .iter()
        .filter(|proposal| !standings.contains_key(&proposal.id))
        .collect::<Vec<_>>();
    let pending_review = pending_proposals.len();
    let accepted_review = standings
        .values()
        .filter(|standing| standing.as_str() == "accepted")
        .count();
    let rejected_review = standings
        .values()
        .filter(|standing| standing.as_str() == "rejected")
        .count();
    let withdrawn_review = standings
        .values()
        .filter(|standing| standing.as_str() == "withdrawn")
        .count();
    let target_assessment = vela_edge::target_index::assess_current_target_index(
        &frontier,
        &repository.frontier_id,
        &repository.origin_id,
        &repository_root,
    )
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let target_index_configured = target_assessment.is_some();
    let ready_target_count = target_assessment
        .as_ref()
        .map(|assessment| assessment.fresh_open_targets().len())
        .unwrap_or(0);
    let inbox_projection = crate::decision_inbox::project(&frontier)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let (decision_inbox, pending_decision_count) = decision_inbox_status_summary(&inbox_projection);
    let review_action = (pending_decision_count > 0).then(|| {
        json!({
            "pending_count": pending_decision_count,
            "command": format!("vela review inbox {} --json", frontier.display()),
        })
    });
    let work_action = if target_index_configured {
        json!({
            "mode": "target",
            "ready_target_count": ready_target_count,
            "command": format!("vela next {} --limit 1 --json", frontier.display()),
        })
    } else {
        json!({
            "mode": "direct_submission",
            "ready_target_count": 0,
            "command": format!("vela submit --frontier {} --help", frontier.display()),
            "note": "No Target Index is configured. Submit bounded evidence directly or use a Frontier-owned adapter to generate targets.json."
        })
    };
    let payload = json!({
        "schema": "vela.status.v3",
        "ok": true,
        "command": "status",
        "frontier": {
            "id": repository.frontier_id,
            "name": profile.name,
            "profile_root": repository.profile_root
        },
        "git": {
            "role": "frontier_head",
            "commit": commit,
            "tree": tree
        },
        "integrity": {
            /* The prose below no longer says "verified", which TERMINOLOGY.md
            forbids unqualified. This value keeps the word because it is a wire
            token of vela.status.v3: vela-web pins it as z.literal("verified")
            and its projection builder asserts on it, so retiring it is a
            coordinated schema change, not a wording change. */
            "replay": "verified",
            "strict": "pass",
            "blocker_count": 0,
            "blockers_by_code": {}
        },
        "roots": {
            "origin": repository.origin_root,
            "repository": repository_root,
            "authority_keyset": repository.authority_keyset_root,
            "authority_policy": repository.authority_policy_root
        },
        "counts": {
            "claims": repository.accepted_claims.len() + repository.pending_claims.len(),
            "accepted_claims": repository.accepted_claims.len(),
            "pending_claims": repository.pending_claims.len(),
            "pending_review": pending_review,
            "accepted_review": accepted_review,
            "rejected_review": rejected_review,
            "withdrawn_review": withdrawn_review,
            "submissions": repository.submissions.len(),
            "verifications": repository.verifications.len(),
            "artifacts": repository.artifacts.len()
        },
        "work": {"ready_target_count": ready_target_count},
        "decision_inbox": decision_inbox,
        "actions": {
            "review": review_action,
            "work": work_action,
        },
    });
    if json_out {
        crate::cli::print_json(&payload);
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
        println!("  replay    matched · signatures, roots, canonical bytes");
        println!("  strict    pass");
        println!("  claims    {}", payload["counts"]["claims"]);
        println!(
            "  targets   {} ready",
            payload["work"]["ready_target_count"]
        );
        println!(
            "  inbox     {} pending · {} protocol-ready · {} protocol-blocked",
            payload["decision_inbox"]["pending_count"],
            payload["decision_inbox"]["protocol_ready_count"],
            payload["decision_inbox"]["protocol_blocked_count"]
        );
        println!(
            "  inbox root {}",
            payload["decision_inbox"]["projection_root"]
                .as_str()
                .unwrap_or("unavailable")
        );
        println!(
            "  first card {}",
            payload["decision_inbox"]["first_entry_root"]
                .as_str()
                .unwrap_or("none")
        );
        if let Some(review) = payload["actions"]["review"].as_object() {
            println!(
                "  review    {} pending · {}",
                review["pending_count"],
                review["command"].as_str().unwrap_or("unavailable")
            );
        }
        println!(
            "  work      {}",
            payload["actions"]["work"]["command"]
                .as_str()
                .unwrap_or("unavailable")
        );
    }
}

pub(crate) fn verify_current_profile_at(root: &Path) -> Result<CurrentFrontierProfileV2, String> {
    let profile_source = fs::read_to_string(root.join("frontier.toml"))
        .map_err(|error| format!("read current frontier.toml: {error}"))?;
    CurrentFrontierProfileV2::from_toml_str(&profile_source)
}

pub(crate) fn verify_current_bootstrap_at(root: &Path) -> Result<CurrentFrontierProfileV2, String> {
    let profile = verify_current_profile_at(root)?;
    if root.join(".vela/epoch.json").exists() {
        return Err("repository retains the retired .vela/epoch.json path".into());
    }
    if root.join(".vela/origin.json").exists() || root.join(".vela/repository.json").exists() {
        return Err("current repository bootstrap cannot contain an origin or manifest".into());
    }
    for relative in [
        ".vela/authority",
        ".vela/claims",
        ".vela/proposals",
        ".vela/submissions",
        ".vela/verifications",
        ".vela/artifacts",
        "records",
    ] {
        if root.join(relative).exists() {
            return Err(format!(
                "current repository bootstrap contains canonical object path {relative} before authority initialization"
            ));
        }
    }
    Ok(profile)
}

pub(crate) fn cmd_current_next(frontier: &Path, limit: usize, json_out: bool) {
    crate::ui::set_mode("next", json_out);
    let frontier = crate::ui::canonicalize_frontier(frontier);
    let repository = load_current_repository_at(&frontier, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let repository_root = repository
        .canonical_root()
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let assessment = vela_edge::target_index::assess_current_target_index(
        &frontier,
        &repository.frontier_id,
        &repository.origin_id,
        &repository_root,
    )
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let Some(assessment) = assessment else {
        let payload = json!({
            "schema": "vela.offer.v1",
            "ok": true,
            "command": "next",
            "frontier_id": repository.frontier_id,
            "repository_root": repository_root,
            "availability": {
                "configured": 0,
                "stale": 0,
                "fresh": 0,
                "returned": 0
            },
            "targets": [],
            "next_action": format!("vela submit --frontier {} --help", frontier.display()),
            "note": "No Target Index is configured. Submit bounded evidence directly or use a Frontier-owned adapter to generate targets.json.",
        });
        if json_out {
            crate::cli::print_json(&payload);
        } else {
            println!("next · no configured Target Offers");
            println!(
                "  direct    vela submit --frontier {} --help",
                frontier.display()
            );
            println!("  adapter   generate tracked targets.json for ranked Frontier-owned work");
        }
        return;
    };
    let configured = assessment.configured_open();
    let fresh = assessment.fresh_open_targets();
    let fresh_count = fresh.len();
    let limit = limit.clamp(1, 128);
    let offers = fresh
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(position, target)| {
            json!({
                "queue_position": position + 1,
                "rank": target.rank,
                "lane": "produce",
                "target_id": target.id,
                "title": target.title,
                "objective": target.objective,
                "why": target.why,
                "labels": target.labels,
                "packet": target.packet,
                "verifier_profile": assessment.packet_value(&target.id)
                    .and_then(|packet| packet.get("verifier_profile"))
                    .or_else(|| assessment.packet_value(&target.id)
                        .and_then(|packet| packet.get("verifier"))),
                "next_command": format!(
                    "vela start {} --frontier {} --json",
                    target.id,
                    frontier.display()
                )
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "schema": "vela.offer.v1",
        "ok": true,
        "command": "next",
        "frontier_id": repository.frontier_id,
        "origin_id": repository.origin_id,
        "repository_root": repository_root,
        "target_index_root": assessment.index.index_root,
        "availability": {
            "configured": configured,
            "stale": configured.saturating_sub(fresh_count),
            "fresh": fresh_count,
            "returned": offers.len()
        },
        "targets": offers,
    });
    if json_out {
        crate::cli::print_json(&payload);
    } else {
        let returned = payload["availability"]["returned"].as_u64().unwrap_or(0);
        println!("next · {returned} Target Offer(s)");
        for offer in payload["targets"].as_array().into_iter().flatten() {
            println!(
                "  {}  {}",
                offer["target_id"].as_str().unwrap_or(""),
                offer["title"].as_str().unwrap_or("")
            );
            println!("      {}", offer["why"].as_str().unwrap_or(""));
            println!("      {}", offer["next_command"].as_str().unwrap_or(""));
        }
    }
}

fn authority_event_by_semantic_id<'a>(
    events: &'a [AuthorityEventV1],
    event_id: &str,
) -> Result<Option<&'a AuthorityEventV1>, String> {
    let mut matching = None;
    for event in events {
        if event.semantic_state_event()?.id == event_id {
            if matching.is_some() {
                return Err(format!(
                    "current authority history repeats semantic event {event_id}"
                ));
            }
            matching = Some(event);
        }
    }
    Ok(matching)
}

fn current_proposal_decisions(
    events: &[AuthorityEventV1],
) -> Result<BTreeMap<String, CurrentProposalDecision>, String> {
    let mut decisions = BTreeMap::new();
    for event in events {
        let standing = match event.content.kind {
            EventKind::ReviewAccepted => "accepted",
            EventKind::ReviewRejected => "rejected",
            EventKind::ReviewRevisionRequested => {
                return Err(format!(
                    "current authority event {} uses unsupported revision-request standing",
                    event.id
                ));
            }
            _ => continue,
        };
        if event.content.target.r#type != "proposal" {
            return Err(format!(
                "current review event {} does not target a Proposal",
                event.id
            ));
        }
        let proposal_id = event.content.target.id.clone();
        if event
            .content
            .payload
            .get("proposal_id")
            .and_then(Value::as_str)
            != Some(proposal_id.as_str())
        {
            return Err(format!(
                "current review event {} does not bind its target Proposal",
                event.id
            ));
        }
        let expected_verdict = match event.content.kind {
            EventKind::ReviewAccepted => Some("accepted"),
            EventKind::ReviewRejected => Some("rejected"),
            _ => unreachable!(),
        };
        if expected_verdict.is_some()
            && event.content.payload.get("verdict").and_then(Value::as_str) != expected_verdict
        {
            return Err(format!(
                "current review event {} carries the wrong verdict",
                event.id
            ));
        }
        let applied_event_id = if event.content.kind == EventKind::ReviewAccepted {
            let applied = event
                .content
                .payload
                .get("applied_event_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "current accepted review event {} lacks applied_event_id",
                        event.id
                    )
                })?;
            let applied_event =
                authority_event_by_semantic_id(events, applied)?.ok_or_else(|| {
                    format!(
                        "current accepted review event {} names a missing domain event",
                        event.id
                    )
                })?;
            if applied_event.content.transaction_id != event.content.transaction_id {
                return Err(format!(
                    "current accepted review event {} names a domain event from another transaction",
                    event.id
                ));
            }
            if applied_event.content.target.r#type != "claim"
                || !matches!(
                    applied_event.content.kind,
                    EventKind::FindingAsserted
                        | EventKind::FindingSuperseded
                        | EventKind::FindingRetracted
                )
            {
                return Err(format!(
                    "current accepted review event {} names a non-scientific transition",
                    event.id
                ));
            }
            Some(applied.to_string())
        } else {
            None
        };
        let decision = CurrentProposalDecision {
            standing: standing.into(),
            event_id: event.id.clone(),
            event_root: event.root()?,
            decided_at: event.content.timestamp.clone(),
            actor: event.content.actor.id.clone(),
            reason: event.content.reason.clone(),
            applied_event_id,
        };
        if decisions.insert(proposal_id.clone(), decision).is_some() {
            return Err(format!(
                "current Proposal {proposal_id} has more than one terminal Decision"
            ));
        }
    }
    Ok(decisions)
}

pub(crate) fn load_current_proposal_decisions(
    frontier: &Path,
    repository: &CurrentRepositoryV4,
) -> Result<BTreeMap<String, CurrentProposalDecision>, String> {
    let origin_bytes = fs::read(frontier.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    let authority = crate::cli::load_current_repository_authority(frontier, repository, &origin)?;
    current_proposal_decisions(&authority.history.authority_events)
}

pub(crate) fn load_current_proposal_standings(
    frontier: &Path,
    repository: &CurrentRepositoryV4,
) -> Result<BTreeMap<String, String>, String> {
    let decisions = load_current_proposal_decisions(frontier, repository)?;
    let mut standings = decisions
        .into_iter()
        .map(|(proposal_id, decision)| (proposal_id, decision.standing))
        .collect::<BTreeMap<_, _>>();
    for proposal_id in load_current_proposal_withdrawals(frontier, repository)?.into_keys() {
        if standings
            .insert(proposal_id.clone(), "withdrawn".into())
            .is_some()
        {
            return Err(format!(
                "current Proposal {proposal_id} has both a producer Withdrawal and an authority Decision"
            ));
        }
    }
    Ok(standings)
}

pub(crate) fn load_current_proposal_withdrawals(
    frontier: &Path,
    repository: &CurrentRepositoryV4,
) -> Result<BTreeMap<String, ProposalWithdrawalV1>, String> {
    let mut withdrawals = BTreeMap::new();
    for reference in &repository.proposal_withdrawals {
        let bytes = read_rooted_object(frontier, &reference.path, &reference.root)?;
        let withdrawal = ProposalWithdrawalV1::parse(&bytes)?;
        if withdrawal.withdrawal_id != reference.id
            || withdrawal.canonical_root()? != reference.root
        {
            return Err(format!(
                "current Proposal Withdrawal {} differs from its repository reference",
                reference.id
            ));
        }
        let proposal_reference = repository
            .proposals
            .iter()
            .find(|candidate| {
                candidate.id == withdrawal.proposal_id && candidate.root == withdrawal.proposal_root
            })
            .ok_or_else(|| {
                format!(
                    "Proposal Withdrawal {} does not bind one exact retained Proposal",
                    withdrawal.withdrawal_id
                )
            })?;
        let proposal = ProposalV1::parse(&read_rooted_object(
            frontier,
            &proposal_reference.path,
            &proposal_reference.root,
        )?)?;
        let submission_reference = repository
            .submissions
            .iter()
            .find(|candidate| {
                candidate.id == withdrawal.submission_id
                    && candidate.root == withdrawal.submission_root
            })
            .ok_or_else(|| {
                format!(
                    "Proposal Withdrawal {} does not bind one exact retained Submission",
                    withdrawal.withdrawal_id
                )
            })?;
        let submission = SubmissionV1::parse(&read_rooted_object(
            frontier,
            &submission_reference.path,
            &submission_reference.root,
        )?)?;
        withdrawal.verify_with(&proposal, &submission)?;
        if withdrawals
            .insert(withdrawal.proposal_id.clone(), withdrawal)
            .is_some()
        {
            return Err(format!(
                "current Proposal {} has more than one Withdrawal",
                proposal.proposal_id
            ));
        }
    }
    Ok(withdrawals)
}

fn validate_current_proposal_standing(
    root: &Path,
    repository: &CurrentRepositoryV4,
    events: &[AuthorityEventV1],
) -> Result<(), String> {
    let withdrawals = load_current_proposal_withdrawals(root, repository)?;
    let decisions = current_proposal_decisions(events)?;
    let mut standings = decisions
        .iter()
        .map(|(proposal_id, decision)| (proposal_id.clone(), decision.standing.clone()))
        .collect::<BTreeMap<_, _>>();
    for proposal_id in withdrawals.keys() {
        if standings
            .insert(proposal_id.clone(), "withdrawn".into())
            .is_some()
        {
            return Err(format!(
                "current Proposal {proposal_id} has both a producer Withdrawal and an authority Decision"
            ));
        }
    }
    for proposal_id in standings.keys() {
        if !repository
            .proposals
            .iter()
            .any(|proposal| proposal.id == *proposal_id)
        {
            return Err(format!(
                "current Decision targets Proposal {proposal_id} outside the repository"
            ));
        }
    }
    for reference in &repository.proposals {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let proposal = ProposalV1::parse(&bytes)?;
        let claim = rooted_claim_for_proposal(root, &proposal)?;
        let pending = repository.pending_claims.iter().any(|candidate| {
            candidate.claim_id == proposal.subject.id
                && candidate.claim_root == proposal.subject.root
        });
        let accepted = repository.accepted_claims.iter().any(|candidate| {
            candidate.claim_id == proposal.subject.id
                && candidate.claim_root == proposal.subject.root
        });
        let decision = decisions.get(&proposal.proposal_id);
        let standing = standings
            .get(&proposal.proposal_id)
            .map(String::as_str)
            .unwrap_or("pending_review");
        let expected = match (proposal.action.as_str(), standing) {
            ("claim.add" | "claim.revise", "pending_review") => (true, false),
            ("claim.add" | "claim.revise", "accepted") => (false, true),
            ("claim.add" | "claim.revise", "rejected") => (false, false),
            ("claim.add" | "claim.revise", "withdrawn") => (false, false),
            ("claim.withdraw", "pending_review" | "rejected") => (false, true),
            ("claim.withdraw", "withdrawn") => (false, true),
            ("claim.withdraw", "accepted") => (false, false),
            (action, standing) => {
                return Err(format!(
                    "current Proposal {} has unsupported action/standing {action}/{standing}",
                    proposal.proposal_id
                ));
            }
        };
        if (pending, accepted) != expected {
            return Err(format!(
                "current Proposal {} standing disagrees with the repository Claim indexes",
                proposal.proposal_id
            ));
        }
        let Some(decision) = decision else {
            continue;
        };
        if decision.standing != "accepted" {
            continue;
        }
        let applied_id = decision
            .applied_event_id
            .as_deref()
            .ok_or_else(|| "accepted Decision lacks its applied event".to_string())?;
        let applied = authority_event_by_semantic_id(events, applied_id)?
            .ok_or_else(|| "accepted Decision applied event is missing".to_string())?;
        if applied.content.actor.id != decision.actor
            || applied
                .content
                .payload
                .get("proposal_id")
                .and_then(Value::as_str)
                != Some(proposal.proposal_id.as_str())
            || applied
                .content
                .payload
                .get("claim_id")
                .and_then(Value::as_str)
                != Some(proposal.subject.id.as_str())
        {
            return Err(format!(
                "current Proposal {} applied event has the wrong actor or object binding",
                proposal.proposal_id
            ));
        }
        let transition_matches = match proposal.action.as_str() {
            "claim.add" => {
                applied.content.kind == EventKind::FindingAsserted
                    && applied.content.target.id == proposal.subject.id
                    && applied.content.before_hash == NULL_HASH
                    && applied.content.after_hash == proposal.subject.root
            }
            "claim.revise" => {
                let predecessors = claim
                    .relations
                    .iter()
                    .filter(|relation| matches!(relation.kind.as_str(), "corrects" | "supersedes"))
                    .collect::<Vec<_>>();
                predecessors.len() == 1
                    && applied.content.kind == EventKind::FindingSuperseded
                    && applied.content.target.id == predecessors[0].target_claim_id
                    && applied.content.before_hash != NULL_HASH
                    && applied.content.after_hash == proposal.subject.root
            }
            "claim.withdraw" => {
                applied.content.kind == EventKind::FindingRetracted
                    && applied.content.target.id == proposal.subject.id
                    && applied.content.before_hash == proposal.subject.root
                    && applied.content.after_hash == NULL_HASH
            }
            _ => false,
        };
        if !transition_matches {
            return Err(format!(
                "current Proposal {} applied event does not match its exact transition",
                proposal.proposal_id
            ));
        }
    }
    Ok(())
}

pub(crate) fn cmd_current_review_list(
    frontier: &Path,
    status: Option<&str>,
    limit: usize,
    cursor: Option<&str>,
    json_out: bool,
) {
    crate::ui::set_mode("review list", json_out);
    crate::ui::require_initialized_frontier(frontier);
    let status = status.unwrap_or("pending_review");
    if !["pending_review", "accepted", "rejected", "withdrawn", "all"].contains(&status) {
        crate::cli::fail_kind(
            crate::ui::ErrorKind::Usage,
            "current review status must be pending_review, accepted, rejected, withdrawn, or all",
        );
    }
    let frontier = crate::ui::canonicalize_frontier(frontier);
    let repository = load_current_repository_at(&frontier, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let decisions = load_current_proposal_decisions(&frontier, &repository)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let withdrawals = load_current_proposal_withdrawals(&frontier, &repository)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let mut items = repository
        .proposals
        .iter()
        .filter_map(|reference| {
            let bytes = read_rooted_object(&frontier, &reference.path, &reference.root)
                .unwrap_or_else(|error| crate::cli::fail_return(&error));
            let proposal =
                ProposalV1::parse(&bytes).unwrap_or_else(|error| crate::cli::fail_return(&error));
            let decision = decisions.get(&proposal.proposal_id);
            let withdrawal = withdrawals.get(&proposal.proposal_id);
            let standing = decision.map_or_else(
                || {
                    if withdrawal.is_some() {
                        "withdrawn"
                    } else {
                        "pending_review"
                    }
                },
                |decision| decision.standing.as_str(),
            );
            if status != "all" && standing != status {
                return None;
            }
            Some((
                proposal.created_at.clone(),
                json!({
                    "proposal_id": proposal.proposal_id,
                    "proposal_root": reference.root,
                    "created_at": proposal.created_at,
                    "action": proposal.action,
                    "status": standing,
                    "claim_id": proposal.subject.id,
                    "claim_root": proposal.subject.root,
                    "actor": proposal.actor,
                    "submission_id": proposal.producer_package.id,
                    "reason": proposal.reason,
                    "caveats": proposal.caveats,
                    "decision": decision,
                    "withdrawal": withdrawal
                }),
            ))
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right.0.cmp(&left.0).then_with(|| {
            left.1["proposal_id"]
                .as_str()
                .cmp(&right.1["proposal_id"].as_str())
        })
    });
    let items = items.into_iter().map(|(_, item)| item).collect::<Vec<_>>();
    let page = crate::cli::page::paginate("review", "Proposal", items, limit, cursor, |item| {
        item["proposal_id"].as_str()
    });
    let total = page.total;
    let payload = json!({
        "schema": "vela.review.v1",
        "ok": true,
        "command": "review.list",
        "frontier_id": repository.frontier_id,
        "repository_root": repository.canonical_root().unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "status": status,
        "order": "created_at_desc_then_proposal_id",
        "total": total,
        "returned": page.items.len(),
        "next_cursor": page.next_cursor,
        "items": page.items,
    });
    if json_out {
        crate::cli::print_json(&payload);
    } else {
        println!("review · {total} {status} proposal(s)");
        for item in payload["items"].as_array().into_iter().flatten() {
            /* A decided Proposal shows the Decision's reason, not the
            Submission's retention reason. The retention line is the same
            boilerplate on every row and says nothing about why this one
            was settled the way it was. */
            let reason = item["decision"]["reason"]
                .as_str()
                .or_else(|| item["reason"].as_str())
                .unwrap_or("");
            println!(
                "  {}  {}  {}  {}",
                item["proposal_id"].as_str().unwrap_or(""),
                item["created_at"].as_str().unwrap_or(""),
                item["action"].as_str().unwrap_or(""),
                reason
            );
        }
    }
}

pub(crate) fn cmd_current_review_show(frontier: &Path, proposal_id: &str, json_out: bool) {
    crate::ui::set_mode("review show", json_out);
    crate::ui::require_initialized_frontier(frontier);
    let frontier = crate::ui::canonicalize_frontier(frontier);
    let repository = load_current_repository_at(&frontier, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let decisions = load_current_proposal_decisions(&frontier, &repository)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let withdrawals = load_current_proposal_withdrawals(&frontier, &repository)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let reference = repository
        .proposals
        .iter()
        .find(|reference| reference.id == proposal_id)
        .unwrap_or_else(|| {
            crate::cli::fail_kind_return(
                crate::ui::ErrorKind::NotFound,
                "current repository has no exact Proposal with that ID",
            )
        });
    let proposal_bytes = read_rooted_object(&frontier, &reference.path, &reference.root)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let proposal =
        ProposalV1::parse(&proposal_bytes).unwrap_or_else(|error| crate::cli::fail_return(&error));
    let standing = decisions.get(proposal_id).map_or_else(
        || {
            if withdrawals.contains_key(proposal_id) {
                "withdrawn"
            } else {
                "pending_review"
            }
        },
        |decision| decision.standing.as_str(),
    );
    let claim_path =
        crate::current_submission::rooted_path("records/claims/sha256", &proposal.subject.root)
            .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let claim_bytes = read_rooted_object(&frontier, &claim_path, &proposal.subject.root)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let claim =
        ClaimRecordV1::parse(&claim_bytes).unwrap_or_else(|error| crate::cli::fail_return(&error));
    if claim.claim_id != proposal.subject.id {
        crate::cli::fail_return::<()>("current Proposal Claim bytes have the wrong identity");
    }
    let submission_reference = repository
        .submissions
        .iter()
        .find(|submission| {
            submission.id == proposal.producer_package.id
                && submission.root == proposal.producer_package.root
        })
        .unwrap_or_else(|| {
            crate::cli::fail_return("current Proposal has no exact retained Submission")
        });
    let submission_bytes = read_rooted_object(
        &frontier,
        &submission_reference.path,
        &submission_reference.root,
    )
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let submission = vela_protocol::submission_v1::SubmissionV1::parse(&submission_bytes)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let verifications = repository
        .verifications
        .iter()
        .filter_map(|verification| {
            let bytes = read_rooted_object(&frontier, &verification.path, &verification.root)
                .unwrap_or_else(|error| crate::cli::fail_return(&error));
            let record = VerificationRecordV1::parse(&bytes)
                .unwrap_or_else(|error| crate::cli::fail_return(&error));
            verification_targets_proposal(&proposal, &claim, &record).then_some(json!({
                "verification_record_root": verification.root,
                "record": record
            }))
        })
        .collect::<Vec<_>>();
    let decision = decisions.get(proposal_id);
    let withdrawal = withdrawals.get(proposal_id);
    let decision_inbox = crate::decision_inbox::review_context(&frontier, proposal_id)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let payload = json!({
        "schema": "vela.review.v1",
        "ok": true,
        "command": "review.show",
        "frontier_id": repository.frontier_id,
        "repository_root": repository.canonical_root().unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "proposal_id": proposal.proposal_id,
        "proposal_root": reference.root,
        "standing": standing,
        "proposal": proposal,
        "claim": claim,
        "submission": submission,
        "verification_records": verifications,
        "decision": decision,
        "withdrawal": withdrawal,
        "decision_inbox": decision_inbox,
        "authority_boundary": "Verification records report bounded checks. A producer may close its own pending Proposal; only a repository-authority Decision can change accepted scientific Standing.",
    });
    if json_out {
        crate::cli::print_json(&payload);
    } else {
        println!("review · {proposal_id} · {standing}");
        println!(
            "  action: {}",
            payload["proposal"]["action"].as_str().unwrap_or("")
        );
        println!(
            "  claim: {}",
            payload["proposal"]["subject"]["id"].as_str().unwrap_or("")
        );
        /* Two reasons exist and they are not interchangeable. The Proposal's
        reason says why the Submission was retained for review; the
        Decision's reason is what an authorized human wrote when they
        changed Standing. Printing the first under a heading that says
        "accepted" attributes the retention boilerplate to the decider and
        drops the only sentence in the record that carries their judgment. */
        println!(
            "  submitted: {}",
            payload["proposal"]["reason"].as_str().unwrap_or("")
        );
        if let Some(decision) = payload["decision"].as_object() {
            let actor = decision
                .get("actor")
                .and_then(Value::as_str)
                .unwrap_or("actor not recorded");
            let at = decision
                .get("decided_at")
                .and_then(Value::as_str)
                .unwrap_or("time not recorded");
            println!("  decided: {standing} by {actor} at {at}");
            if let Some(reason) = decision.get("reason").and_then(Value::as_str) {
                println!("  decision reason: {reason}");
            }
        }
        println!(
            "  verification records: {}",
            payload["verification_records"]
                .as_array()
                .map_or(0, Vec::len)
        );
        println!(
            "  authority: {}",
            payload["authority_boundary"].as_str().unwrap_or("")
        );
    }
}

/// Return whether one authenticated Verification Record observes this exact
/// Proposal, Claim, and retained Submission.
pub(crate) fn verification_targets_proposal(
    proposal: &ProposalV1,
    claim: &ClaimRecordV1,
    record: &VerificationRecordV1,
) -> bool {
    if claim.claim_id != proposal.subject.id {
        return false;
    }
    record.subject.proposal_id == proposal.proposal_id
        && record.subject.claim_id == proposal.subject.id
        && record.subject.submission_id == proposal.producer_package.id
        && record.subject.submission_root == proposal.producer_package.root
}

fn rooted_claim_for_proposal(root: &Path, proposal: &ProposalV1) -> Result<ClaimRecordV1, String> {
    let claim_path =
        crate::current_submission::rooted_path("records/claims/sha256", &proposal.subject.root)?;
    let claim_bytes = read_rooted_object(root, &claim_path, &proposal.subject.root)?;
    let claim = ClaimRecordV1::parse(&claim_bytes)?;
    if claim.canonical_bytes()? != claim_bytes || claim.claim_id != proposal.subject.id {
        return Err(format!(
            "current Proposal {} has the wrong canonical Claim bytes",
            proposal.proposal_id
        ));
    }
    Ok(claim)
}

fn verification_targets_rooted_proposal(
    root: &Path,
    proposal: &ProposalV1,
    record: &VerificationRecordV1,
) -> Result<bool, String> {
    let claim = rooted_claim_for_proposal(root, proposal)?;
    Ok(verification_targets_proposal(proposal, &claim, record))
}

fn proposal_matches_signed_submission(
    proposal: &ProposalV1,
    claim: &ClaimRecordV1,
    submission: &SubmissionV1,
) -> Result<(), String> {
    if proposal.actor != submission.provenance.producer || proposal.caveats != submission.caveats {
        return Err("Proposal actor or caveats disagree with its signed Submission".into());
    }

    let expected_action = match submission.requested_change.kind.as_str() {
        "add_claim" => "claim.add",
        "correct_claim" | "supersede_claim" => "claim.revise",
        "retract_claim" => "claim.withdraw",
        kind => return Err(format!("unsupported signed Submission change {kind}")),
    };
    if proposal.action != expected_action {
        return Err("Proposal action disagrees with its signed Submission".into());
    }

    if proposal.action == "claim.withdraw" {
        let target = submission
            .requested_change
            .target
            .as_ref()
            .ok_or_else(|| "withdrawal Submission has no exact Claim target".to_string())?;
        if proposal.subject.id != target.claim_id || proposal.subject.root != target.claim_root {
            return Err("withdrawal Proposal does not bind its signed Submission target".into());
        }
        return Ok(());
    }

    if claim.assertion.text != submission.claim.assertion
        || claim.assertion.kind != submission.claim.claim_type
        || claim.created_at != submission.provenance.emitted_at
        || !claim.extensions.is_empty()
    {
        return Err("Proposal Claim body disagrees with its signed Submission".into());
    }

    let mut expected_conditions = submission.claim.conditions.clone();
    expected_conditions.extend(
        submission
            .caveats
            .iter()
            .map(|caveat| format!("Caveat: {caveat}")),
    );
    if claim.conditions != expected_conditions {
        return Err("Proposal Claim conditions disagree with its signed Submission".into());
    }

    let expected_evidence = submission
        .artifacts
        .iter()
        .map(|artifact| {
            let digest = artifact
                .digest
                .strip_prefix("sha256:")
                .expect("verified Submission Artifact digest is sha256");
            (
                "supports",
                None,
                artifact.digest.as_str(),
                Some(format!("records/artifacts/sha256/{digest}")),
            )
        })
        .collect::<Vec<_>>();
    let observed_evidence = claim
        .evidence
        .iter()
        .map(|evidence| {
            (
                evidence.relation.as_str(),
                evidence.artifact_id.as_deref(),
                evidence.artifact_root.as_str(),
                evidence.artifact_path.clone(),
            )
        })
        .collect::<Vec<_>>();
    if observed_evidence != expected_evidence {
        return Err("Proposal Claim evidence disagrees with its signed Submission".into());
    }

    let relation_matches = match submission.requested_change.kind.as_str() {
        "add_claim" => claim.revision == 1 && claim.relations.is_empty(),
        "correct_claim" | "supersede_claim" => {
            let target = submission.requested_change.target.as_ref();
            claim.revision > 1
                && claim.relations.len() == 1
                && target.is_some_and(|target| {
                    claim.relations[0].target_claim_id == target.claim_id
                        && claim.relations[0].kind
                            == if submission.requested_change.kind == "correct_claim" {
                                "corrects"
                            } else {
                                "supersedes"
                            }
                })
        }
        _ => false,
    };
    if !relation_matches {
        return Err("Proposal Claim relation disagrees with its signed Submission".into());
    }

    if claim.provenance.len() != 1
        || claim.provenance[0].kind != "submission"
        || claim.provenance[0].title
            != format!("Authenticated Submission {}", submission.submission_id)
        || claim.provenance[0].authors != [submission.provenance.producer.clone()]
    {
        return Err("Proposal Claim provenance disagrees with its signed Submission".into());
    }
    Ok(())
}

/// Load and validate the current repository identity and authority chain.
pub(crate) fn load_current_repository_at(
    root: &Path,
    require_authority_record: bool,
) -> Result<CurrentRepositoryV4, String> {
    let profile_source = fs::read_to_string(root.join("frontier.toml"))
        .map_err(|error| format!("read current frontier.toml: {error}"))?;
    let profile = CurrentFrontierProfileV2::from_toml_str(&profile_source)?;
    let profile_root = profile.profile_root()?;
    if root.join(".vela/epoch.json").exists() {
        return Err("current repository cannot retain an epoch boundary".into());
    }
    let origin_bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    let origin_root = origin.canonical_root()?;
    let repository_bytes = fs::read(root.join(".vela/repository.json"))
        .map_err(|error| format!("read current repository manifest: {error}"))?;
    let repository = CurrentRepositoryV4::parse(&repository_bytes)?;
    if repository.frontier_id != profile.frontier_id
        || repository.frontier_id != origin.frontier_id
        || repository.profile_root != profile_root
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin.origin_id
        || repository.origin_root != origin_root
    {
        return Err(
            "current Profile, repository manifest, and origin do not bind the same identity".into(),
        );
    }
    if require_authority_record {
        let loaded = crate::cli::load_current_repository_authority(root, &repository, &origin)?;
        validate_current_proposal_standing(root, &repository, &loaded.history.authority_events)?;
        let mut records = Vec::with_capacity(loaded.history.authority_envelopes.len());
        for envelope in &loaded.history.authority_envelopes {
            records.push(crate::cli::authority_record_from_envelope(envelope)?);
        }
        let signed_repository_transitions =
            verify_repository_manifest_delta_chain(records.iter().map(|record| {
                (
                    record.content.sequence,
                    record.content.object_delta.as_slice(),
                )
            }))?;
        verify_routine_evidence_ancestry(root, &signed_repository_transitions, &repository)?;
    }
    Ok(repository)
}

pub(crate) fn verify_current_repository_at(
    root: &Path,
    require_authority_record: bool,
) -> Result<CurrentRepositoryV4, String> {
    let repository = load_current_repository_at(root, false)?;
    let origin_bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    let repository_root = repository.canonical_root()?;
    if root.join("targets.json").is_file() {
        let bytes = fs::read(root.join("targets.json"))
            .map_err(|error| format!("read current Target Index: {error}"))?;
        let index: vela_edge::target_index::TargetIndexV5 = serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse current Target Index: {error}"))?;
        index.validate()?;
        if index.canonical_bytes()?.as_slice() != bytes.as_slice()
            || index.frontier_id != repository.frontier_id
            || index.repository.origin_id != repository.origin_id
            || index.repository.repository_root != repository_root
        {
            return Err(
                "current Target Index does not bind the exact current repository".to_string(),
            );
        }
        if require_authority_record {
            let assessment = vela_edge::target_index::assess_current_target_index(
                root,
                &repository.frontier_id,
                &repository.origin_id,
                &repository_root,
            )?
            .ok_or_else(|| "current Target Index disappeared during verification".to_string())?;
            let mut codes = assessment
                .global_issues
                .iter()
                .map(|issue| issue.code)
                .collect::<Vec<_>>();
            codes.extend(
                assessment
                    .target_issues
                    .values()
                    .flatten()
                    .map(|issue| issue.code),
            );
            codes.sort_unstable();
            codes.dedup();
            if !codes.is_empty() {
                return Err(format!(
                    "current Target Index fails closed: {}",
                    codes.join(", ")
                ));
            }
        }
    }

    let object_bytes = read_current_object_set(root, &repository)?;
    for reference in repository
        .accepted_claims
        .iter()
        .chain(&repository.pending_claims)
    {
        let bytes = object_bytes
            .get(&reference.path)
            .ok_or_else(|| format!("current object {} was not loaded", reference.path))?;
        let claim = ClaimRecordV1::parse(bytes)?;
        if claim.canonical_bytes()?.as_slice() != bytes.as_slice()
            || claim.claim_id != reference.claim_id
            || claim
                .evidence
                .iter()
                .filter_map(|evidence| evidence.artifact_id.as_deref())
                .any(|artifact_id| artifact_id.starts_with("va_"))
        {
            return Err(format!(
                "{} does not contain one current Claim",
                reference.path
            ));
        }
    }
    let mut proposal_by_submission = BTreeMap::new();
    for reference in &repository.proposals {
        let bytes = object_bytes
            .get(&reference.path)
            .ok_or_else(|| format!("current object {} was not loaded", reference.path))?;
        let proposal = ProposalV1::parse(bytes)?;
        if proposal.canonical_bytes()?.as_slice() != bytes.as_slice()
            || proposal.proposal_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Proposal",
                reference.path
            ));
        }
        let claim = rooted_claim_for_proposal(root, &proposal)?;
        let submission_reference = repository
            .submissions
            .iter()
            .find(|candidate| candidate.id == proposal.producer_package.id)
            .ok_or_else(|| {
                format!(
                    "{} targets Submission {} outside the current repository",
                    reference.path, proposal.producer_package.id
                )
            })?;
        if submission_reference.root != proposal.producer_package.root
            || submission_reference.path != proposal.producer_package.path
        {
            return Err(format!(
                "{} does not bind the current Submission reference",
                reference.path
            ));
        }
        let submission =
            SubmissionV1::parse(object_bytes.get(&submission_reference.path).ok_or_else(
                || {
                    format!(
                        "current object {} was not loaded",
                        submission_reference.path
                    )
                },
            )?)?;
        proposal_matches_signed_submission(&proposal, &claim, &submission).map_err(|error| {
            format!(
                "{} has an invalid producer package: {error}",
                reference.path
            )
        })?;
        if let Some(previous) = proposal_by_submission.insert(
            proposal.producer_package.id.clone(),
            proposal.proposal_id.clone(),
        ) {
            return Err(format!(
                "Submission {} is retained by multiple Proposals: {previous} and {}",
                proposal.producer_package.id, proposal.proposal_id
            ));
        }
    }
    for reference in &repository.submissions {
        let bytes = object_bytes
            .get(&reference.path)
            .ok_or_else(|| format!("current object {} was not loaded", reference.path))?;
        let submission = SubmissionV1::parse(bytes)?;
        if submission.canonical_bytes()?.as_slice() != bytes.as_slice()
            || submission.submission_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Submission",
                reference.path
            ));
        }
        if !proposal_by_submission.contains_key(&submission.submission_id) {
            return Err(format!("{} has no exact retained Proposal", reference.path));
        }
    }
    for reference in &repository.verifications {
        let bytes = object_bytes
            .get(&reference.path)
            .ok_or_else(|| format!("current object {} was not loaded", reference.path))?;
        let verification = VerificationRecordV1::parse(bytes)?;
        if verification.canonical_bytes()?.as_slice() != bytes.as_slice()
            || verification.verification_record_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Verification Record",
                reference.path
            ));
        }
        let proposal_reference = repository
            .proposals
            .iter()
            .find(|candidate| candidate.id == verification.subject.proposal_id)
            .ok_or_else(|| {
                format!(
                    "{} targets Proposal {} outside the current repository",
                    reference.path, verification.subject.proposal_id
                )
            })?;
        let proposal =
            ProposalV1::parse(object_bytes.get(&proposal_reference.path).ok_or_else(|| {
                format!("current object {} was not loaded", proposal_reference.path)
            })?)?;
        if !verification_targets_rooted_proposal(root, &proposal, &verification)? {
            return Err(format!(
                "{} does not bind its exact Proposal subject and producer package",
                reference.path
            ));
        }
        let submission_reference = repository
            .submissions
            .iter()
            .find(|candidate| candidate.id == verification.subject.submission_id)
            .ok_or_else(|| {
                format!(
                    "{} targets Submission {} outside the current repository",
                    reference.path, verification.subject.submission_id
                )
            })?;
        if submission_reference.root != verification.subject.submission_root
            || submission_reference.path != proposal.producer_package.path
        {
            return Err(format!(
                "{} does not bind the current Submission reference",
                reference.path
            ));
        }
        for artifact_id in verification
            .subject
            .artifact_ids
            .iter()
            .chain(&verification.output_artifact_ids)
        {
            if !repository
                .artifacts
                .iter()
                .any(|artifact| artifact.id == *artifact_id)
            {
                return Err(format!(
                    "{} names Artifact {} outside the current repository",
                    reference.path, artifact_id
                ));
            }
        }
    }
    for reference in &repository.artifacts {
        if reference.schema != "content-addressed-artifact"
            || reference.id.starts_with("va_")
            || reference.root != format!("sha256:{}", reference.id)
        {
            return Err(format!(
                "Artifact {} is not identified by its full content root",
                reference.id
            ));
        }
        let bytes = object_bytes
            .get(&reference.path)
            .ok_or_else(|| format!("current object {} was not loaded", reference.path))?;
        if root_bytes(bytes) != reference.root {
            return Err(format!(
                "{} does not contain the declared content-addressed Artifact",
                reference.path
            ));
        }
    }

    for path in files_recursive(root)? {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "current repository path escaped its root".to_string())?
            .to_string_lossy();
        if is_retired_current_path(&relative) {
            return Err(format!(
                "current repository retains retired protocol path {relative}"
            ));
        }
    }
    if require_authority_record {
        verify_current_repository_authority(root, &repository, &origin)?;
    }
    Ok(repository)
}

fn read_current_object_set(
    root: &Path,
    repository: &CurrentRepositoryV4,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut references = repository
        .accepted_claims
        .iter()
        .chain(&repository.pending_claims)
        .map(|reference| (reference.path.as_str(), reference.claim_root.as_str()))
        .chain(
            repository
                .proposals
                .iter()
                .chain(&repository.proposal_withdrawals)
                .chain(&repository.submissions)
                .chain(&repository.verifications)
                .chain(&repository.artifacts)
                .map(|reference| (reference.path.as_str(), reference.root.as_str())),
        )
        .collect::<Vec<_>>();
    references.sort_unstable();
    references.dedup();
    if references.is_empty() {
        return Ok(BTreeMap::new());
    }

    let parallelism = std::thread::available_parallelism()
        .map(|value| usize::from(value).saturating_mul(8))
        .unwrap_or(32)
        .clamp(1, 64);
    let chunk_size = references.len().div_ceil(parallelism);
    let batches = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in references.chunks(chunk_size) {
            handles.push(scope.spawn(move || {
                chunk
                    .iter()
                    .map(|(path, expected_root)| {
                        read_rooted_object(root, path, expected_root)
                            .map(|bytes| ((*path).to_string(), bytes))
                    })
                    .collect::<Vec<_>>()
            }));
        }
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| "current object reader thread panicked".to_string())
            })
            .collect::<Result<Vec<_>, _>>()
    })?;

    let mut loaded = BTreeMap::new();
    for batch in batches {
        for result in batch {
            let (path, bytes) = result?;
            loaded.insert(path, bytes);
        }
    }
    Ok(loaded)
}

/// Verify the complete current repository and its authority history while
/// allowing a derived Target Index to report its own staleness.
///
/// Target-index inspect, repair, and reseal must remain available precisely
/// when tracked source or packet bytes drift. Canonical repository and
/// authority objects still fail closed.
pub(crate) fn verify_current_repository_allow_derived_drift_at(
    root: &Path,
) -> Result<CurrentRepositoryV4, String> {
    let repository = verify_current_repository_at(root, false)?;
    let origin_bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    verify_current_repository_authority(root, &repository, &origin)?;
    Ok(repository)
}

/// The repository manifest exactly as the Frontier's origin commit retains it.
///
/// This is the boundary `vela claims` reads a Claim's origin era from: a Claim
/// the origin manifest already bound came through the last compaction, and
/// everything else was admitted by the current authority chain since.
pub(crate) fn initial_repository(
    root: &Path,
    origin: &RepositoryOriginV1,
) -> Result<CurrentRepositoryV4, String> {
    let commit = current_origin_commit(root, origin)?;
    let read_blob = |path: &str| -> Result<Vec<u8>, String> {
        let spec = format!("{commit}:{path}");
        let output = vela_edge::git::output(root, &["show", &spec])?;
        if !output.status.success() {
            return Err(format!(
                "read origin blob {spec}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(output.stdout)
    };
    let initial_origin_bytes = read_blob(".vela/origin.json")?;
    let initial_origin = RepositoryOriginV1::parse(&initial_origin_bytes)?;
    if initial_origin != *origin {
        return Err("current origin differs from its exact boundary commit".into());
    }
    let repository_bytes = read_blob(".vela/repository.json")?;
    let repository = CurrentRepositoryV4::parse(&repository_bytes)?;
    if repository.frontier_id != origin.frontier_id
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin.origin_id
        || repository.origin_root != origin.canonical_root()?
    {
        return Err("current origin commit does not bind its exact repository manifest".into());
    }
    Ok(repository)
}

fn current_origin_commit(root: &Path, origin: &RepositoryOriginV1) -> Result<String, String> {
    let expected = origin.canonical_bytes()?;
    let commits = git_text(root, &["log", "--format=%H", "--", ".vela/origin.json"])?
        .lines()
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut matching = Vec::new();
    for commit in commits {
        let spec = format!("{commit}:.vela/origin.json");
        let output = vela_edge::git::output(root, &["show", &spec])?;
        if output.status.success() && output.stdout == expected {
            matching.push(commit);
        }
    }
    let [commit] = matching.as_slice() else {
        return Err(format!(
            "current repository must commit its exact origin once; found {} matching commits",
            matching.len()
        ));
    };
    Ok(commit.clone())
}

pub(crate) fn read_rooted_object(
    root: &Path,
    path: &str,
    expected_root: &str,
) -> Result<Vec<u8>, String> {
    let bytes = fs::read(root.join(path))
        .map_err(|error| format!("read current object {path}: {error}"))?;
    if root_bytes(&bytes) != expected_root {
        return Err(format!(
            "current object {path} does not match its declared root"
        ));
    }
    let expected_name = expected_root.trim_start_matches("sha256:");
    if Path::new(path).file_stem().and_then(|value| value.to_str()) != Some(expected_name) {
        return Err(format!(
            "current object {path} filename does not match its declared root"
        ));
    }
    Ok(bytes)
}

fn verify_current_repository_authority(
    root: &Path,
    repository: &CurrentRepositoryV4,
    origin: &RepositoryOriginV1,
) -> Result<(), String> {
    let genesis_event_log_root = format!("sha256:{}", vela_protocol::events::event_log_hash(&[]));
    let genesis_actor_registry_root = format!("sha256:{}", hex::encode(Sha256::digest([])));
    let initial_event_log_root = origin
        .predecessor
        .as_ref()
        .map_or(genesis_event_log_root.as_str(), |predecessor| {
            predecessor.archived_event_log_root.as_str()
        });
    let initial_actor_registry_root = origin
        .predecessor
        .as_ref()
        .map_or(genesis_actor_registry_root.as_str(), |predecessor| {
            predecessor.archived_actor_registry_root.as_str()
        });
    let loaded = crate::cli::load_current_repository_authority(root, repository, origin)?;
    validate_current_proposal_standing(root, repository, &loaded.history.authority_events)?;
    let initialization_event_id = loaded
        .verification
        .initialization_event_id
        .as_deref()
        .ok_or_else(|| "current repository authority lacks its initialization event".to_string())?;
    let event = loaded
        .history
        .authority_events
        .iter()
        .find(|event| event.id == initialization_event_id)
        .ok_or_else(|| {
            "current repository authority initialization event is not retained".to_string()
        })?;
    let initialization: AuthorityInitializationV1 =
        serde_json::from_value(event.content.payload.clone())
            .map_err(|error| format!("parse current initialization payload: {error}"))?;
    initialization.validate()?;
    if initialization.frontier_id != repository.frontier_id
        || initialization.initial_event_log_root != initial_event_log_root
        || initialization.initial_actor_registry_root != initial_actor_registry_root
        || initialization.new_authority_keyset_root != repository.authority_keyset_root
        || initialization.new_policy_bundle_root != repository.authority_policy_root
        || initialization.new_principal_id != event.content.principal_id
        || initialization.reason != origin.reason
    {
        return Err(
            "current authority initialization does not bind the exact origin and current roots"
                .into(),
        );
    }
    let event_paths = authority_store_files(&root.join(".vela/authority/events"), ".json")?;
    if event_paths.len() != loaded.history.authority_events.len() {
        return Err("current authority event store contains unverified objects".into());
    }
    let record_paths = authority_store_files(&root.join(".vela/authority/records"), ".dsse.json")?;
    if record_paths.len() != loaded.history.authority_envelopes.len() {
        return Err("current authority record store contains unverified objects".into());
    }
    let first_envelope =
        loaded.history.authority_envelopes.first().ok_or_else(|| {
            "current repository authority has no initialization record".to_string()
        })?;
    let first = crate::cli::authority_record_from_envelope(first_envelope)?;
    if first.content.sequence != 1
        || first.content.previous_authority_record_root.is_some()
        || first.content.event_ids != vec![event.id.clone()]
        || first.content.before_event_log_root != initial_event_log_root
        || first.content.principal.principal_id != event.content.principal_id
        || first.content.authorization.policy_bundle_root != initialization.new_policy_bundle_root
    {
        return Err("current authority record does not bind its exact event and origin".into());
    }
    let expected_after = vela_protocol::authority_history::authority_event_log_root(
        initial_event_log_root,
        &[event],
    )?;
    if first.content.after_event_log_root != expected_after {
        return Err("current authority record has the wrong after-event root".into());
    }
    let initial_repository = initial_repository(root, origin)?;
    let mut initial_objects = initial_repository
        .accepted_claims
        .iter()
        .chain(&initial_repository.pending_claims)
        .map(|reference| RepositoryObjectRefV1 {
            schema: vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA.into(),
            id: reference.claim_id.clone(),
            root: reference.claim_root.clone(),
            path: reference.path.clone(),
        })
        .chain(initial_repository.proposals.iter().cloned())
        .chain(initial_repository.submissions.iter().cloned())
        .chain(initial_repository.verifications.iter().cloned())
        .chain(initial_repository.artifacts.iter().cloned())
        .collect::<Vec<_>>();
    initial_objects.sort_by(|left, right| {
        (&left.schema, &left.id, &left.root, &left.path).cmp(&(
            &right.schema,
            &right.id,
            &right.root,
            &right.path,
        ))
    });
    initial_objects.dedup();
    let initial_object_set_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&initial_objects)?
    );
    if initial_object_set_root != origin.initial_object_set_root {
        return Err("sequence-one object set disagrees with the repository origin".into());
    }
    let required_delta = [
        (
            crate::authority_transaction::authority_event_path(&event.id),
            event.root()?,
        ),
        (".vela/origin.json".into(), origin.canonical_root()?),
        (
            ".vela/repository.json".into(),
            initial_repository.canonical_root()?,
        ),
    ];
    for (path, after_root) in required_delta {
        let matching = first
            .content
            .object_delta
            .iter()
            .filter(|delta| delta.path == path && delta.after_root.as_deref() == Some(&after_root))
            .count();
        if matching != 1 {
            return Err(format!(
                "current authority record does not cover exact postimage {path}"
            ));
        }
    }
    let mut repository_records = Vec::with_capacity(loaded.history.authority_envelopes.len());
    for envelope in &loaded.history.authority_envelopes {
        repository_records.push(crate::cli::authority_record_from_envelope(envelope)?);
    }
    let signed_repository_transitions =
        verify_repository_manifest_delta_chain(repository_records.iter().map(|record| {
            (
                record.content.sequence,
                record.content.object_delta.as_slice(),
            )
        }))?;
    verify_routine_evidence_ancestry(root, &signed_repository_transitions, repository)?;
    let mut covered_record_paths = initial_objects
        .iter()
        .map(|object| (object.path.clone(), object.root.clone()))
        .collect::<BTreeMap<_, _>>();
    for record in &repository_records {
        for delta in &record.content.object_delta {
            if !delta.path.starts_with("records/") {
                continue;
            }
            let Some(after_root) = delta.after_root.as_deref() else {
                return Err(format!(
                    "current record {} cannot be deleted from canonical history",
                    delta.path
                ));
            };
            if let Some(previous) =
                covered_record_paths.insert(delta.path.clone(), after_root.to_string())
                && previous != after_root
            {
                return Err(format!(
                    "authority history changes immutable current object {}",
                    delta.path
                ));
            }
        }
    }
    let observed_record_files = if root.join("records").exists() {
        files_recursive(&root.join("records"))?
    } else {
        Vec::new()
    };
    let mut observed_record_paths = BTreeMap::new();
    for path in observed_record_files {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "current record path escaped its repository".to_string())?
            .to_string_lossy()
            .to_string();
        observed_record_paths.insert(
            relative,
            root_bytes(
                &fs::read(&path)
                    .map_err(|error| format!("read current record {}: {error}", path.display()))?,
            ),
        );
    }
    let mut current_record_paths = repository
        .accepted_claims
        .iter()
        .chain(&repository.pending_claims)
        .map(|reference| (reference.path.clone(), reference.claim_root.clone()))
        .chain(
            repository
                .proposals
                .iter()
                .chain(&repository.proposal_withdrawals)
                .chain(&repository.submissions)
                .chain(&repository.verifications)
                .chain(&repository.artifacts)
                .map(|reference| (reference.path.clone(), reference.root.clone())),
        )
        .collect::<BTreeMap<_, _>>();
    // A rejected Claim or an accepted withdrawal is historical rather than a
    // current Claim index entry, but the retained Proposal still binds its
    // exact immutable Claim bytes.
    for reference in &repository.proposals {
        let proposal_bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let proposal = ProposalV1::parse(&proposal_bytes)?;
        let path = crate::current_submission::rooted_path(
            "records/claims/sha256",
            &proposal.subject.root,
        )?;
        if let Some(previous) =
            current_record_paths.insert(path.clone(), proposal.subject.root.clone())
            && previous != proposal.subject.root
        {
            return Err(format!(
                "current Proposal Claim reference disagrees with retained bytes at {path}"
            ));
        }
    }
    verify_current_record_coverage(
        &covered_record_paths,
        &current_record_paths,
        &observed_record_paths,
    )?;
    let current_paths = repository
        .accepted_claims
        .iter()
        .chain(&repository.pending_claims)
        .map(|reference| reference.path.as_str())
        .chain(
            repository
                .proposals
                .iter()
                .chain(&repository.proposal_withdrawals)
                .chain(&repository.submissions)
                .chain(&repository.verifications)
                .chain(&repository.artifacts)
                .map(|reference| reference.path.as_str()),
        )
        .collect::<BTreeSet<_>>();
    for (path, expected_root) in covered_record_paths {
        if !current_paths.contains(path.as_str()) {
            read_rooted_object(root, &path, &expected_root)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SignedRepositoryManifestTransition {
    sequence: u64,
    before_root: Option<String>,
    after_root: String,
}

fn verify_repository_manifest_delta_chain<'a>(
    records: impl IntoIterator<Item = (u64, &'a [vela_protocol::authority::ObjectDeltaV1])>,
) -> Result<Vec<SignedRepositoryManifestTransition>, String> {
    let mut transitions = Vec::new();
    let mut saw_initial = false;
    for (sequence, deltas) in records {
        let matching = deltas
            .iter()
            .filter(|delta| delta.path == ".vela/repository.json")
            .collect::<Vec<_>>();
        if matching.len() > 1 {
            return Err(format!(
                "authority record {sequence} repeats the repository manifest delta"
            ));
        }
        let Some(delta) = matching.first() else {
            if sequence == 1 {
                return Err(
                    "initial authority record does not cover the repository manifest".into(),
                );
            }
            continue;
        };
        let recognized_kind = delta.object_kind == "repository_manifest"
            || (sequence == 1 && delta.object_kind == "canonical_evidence");
        if !recognized_kind || delta.after_root.is_none() {
            return Err(format!(
                "authority record {sequence} breaks repository manifest root continuity"
            ));
        }
        if sequence == 1 {
            saw_initial = true;
            if delta.before_root.is_some() {
                return Err(
                    "initial authority record breaks repository manifest root continuity".into(),
                );
            }
        } else if delta.before_root.is_none() {
            return Err(format!(
                "authority record {sequence} breaks repository manifest root continuity"
            ));
        }
        transitions.push(SignedRepositoryManifestTransition {
            sequence,
            before_root: delta.before_root.clone(),
            after_root: delta.after_root.clone().expect("validated above"),
        });
    }
    if !saw_initial {
        return Err("current authority history lacks its initial repository manifest delta".into());
    }
    Ok(transitions)
}

fn repository_manifest_at_commit(root: &Path, commit: &str) -> Result<CurrentRepositoryV4, String> {
    let spec = format!("{commit}:.vela/repository.json");
    let output = vela_edge::git::output(root, &["show", &spec])?;
    if !output.status.success() {
        return Err(format!(
            "read repository manifest at {commit}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    CurrentRepositoryV4::parse(&output.stdout)
}

/// Replay unsigned evidence-manifest commits from the last exact signed
/// repository checkpoint. Between authority checkpoints the record store may
/// only add immutable evidence; a Decision creates the next signed checkpoint.
fn verify_routine_evidence_ancestry(
    root: &Path,
    signed_transitions: &[SignedRepositoryManifestTransition],
    current: &CurrentRepositoryV4,
) -> Result<(), String> {
    let Some(first) = signed_transitions.first() else {
        return Err("authority history has no signed repository checkpoint".into());
    };
    // Fresh initialization verifies its exact signed postimage before the
    // first Git commit exists. There is no intervening routine overlay to
    // replay in the one-record case.
    if signed_transitions.len() == 1 && current.canonical_root()? == first.after_root {
        return Ok(());
    }
    let commits = git_text(
        root,
        &[
            "rev-list",
            "--reverse",
            "HEAD",
            "--",
            ".vela/repository.json",
        ],
    )?;
    let origin = RepositoryOriginV1::parse(
        &fs::read(root.join(".vela/origin.json"))
            .map_err(|error| format!("read current repository origin: {error}"))?,
    )?;
    let origin_commit = current_origin_commit(root, &origin)?;
    let mut versions = Vec::new();
    let mut in_current_history = false;
    for commit in commits.lines().filter(|line| !line.is_empty()) {
        if commit == origin_commit {
            in_current_history = true;
        }
        if !in_current_history {
            continue;
        }
        let repository = repository_manifest_at_commit(root, commit)?;
        versions.push((commit.to_string(), repository.canonical_root()?, repository));
    }
    if !in_current_history {
        return Err("current origin commit is not retained in repository ancestry".into());
    }
    let current_root = current.canonical_root()?;
    if versions
        .last()
        .is_none_or(|(_, retained_root, _)| retained_root != &current_root)
    {
        // Postcondition verification runs after the transaction is installed
        // but before its exact Git publication. Treat that installed manifest
        // as the final in-flight version; publication still performs its own
        // compare-and-swap and clean-clone verification.
        versions.push(("<working-tree>".into(), current_root, current.clone()));
    }
    let find_after = |root_value: &str, start: usize| {
        versions
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, (_, candidate_root, _))| candidate_root == root_value)
            .map(|(index, _)| index)
    };
    let Some(mut checkpoint_index) = find_after(&first.after_root, 0) else {
        return Err(format!(
            "signed repository manifest {} is not retained in Git ancestry",
            first.after_root
        ));
    };
    let first_commit = versions[checkpoint_index].0.clone();

    for transition in signed_transitions.iter().skip(1) {
        let before_root = transition.before_root.as_deref().ok_or_else(|| {
            format!(
                "authority record {} has no repository preimage",
                transition.sequence
            )
        })?;
        let Some(before_index) = find_after(before_root, checkpoint_index) else {
            return Err(format!(
                "authority record {} repository preimage {before_root} is not retained after its prior signed checkpoint",
                transition.sequence
            ));
        };
        let mut prior = versions[checkpoint_index].2.clone();
        for (_, _, candidate) in &versions[checkpoint_index + 1..=before_index] {
            verify_routine_evidence_overlay(&prior, candidate)?;
            prior = candidate.clone();
        }
        let Some(after_index) = find_after(&transition.after_root, before_index.saturating_add(1))
        else {
            return Err(format!(
                "authority record {} repository postimage {} is not retained after its exact preimage",
                transition.sequence, transition.after_root
            ));
        };
        if after_index != before_index + 1 {
            return Err(format!(
                "authority record {} does not immediately follow its exact repository preimage",
                transition.sequence
            ));
        }
        checkpoint_index = after_index;
    }

    let range = format!("{first_commit}..HEAD");
    let changed_records = git_text(
        root,
        &[
            "log",
            "--format=",
            "--name-only",
            "--diff-filter=DMR",
            &range,
            "--",
            "records",
        ],
    )?;
    if let Some(path) = changed_records.lines().find(|line| !line.is_empty()) {
        return Err(format!(
            "routine evidence Git ancestry deletes, rewrites, or renames retained record {path}"
        ));
    }

    let mut prior = versions[checkpoint_index].2.clone();
    for (_, _, candidate) in &versions[checkpoint_index + 1..] {
        verify_routine_evidence_overlay(&prior, candidate)?;
        prior = candidate.clone();
    }
    if prior != *current {
        verify_routine_evidence_overlay(&prior, current)?;
    }
    Ok(())
}

/// Verify the only repository-manifest drift that routine, self-authenticated
/// evidence may introduce after the last authority checkpoint.
///
/// This deliberately does not authenticate object bytes. Callers must first
/// run the ordinary current-object verifier, which validates producer and
/// verifier signatures, canonical roots, and all Proposal/Submission/
/// Verification links. This helper owns the smaller authority boundary:
/// routine evidence may append pending-review material, but it may not alter
/// identity, authority configuration, accepted Standing, or any retained
/// reference from the authority checkpoint.
pub(crate) fn verify_routine_evidence_overlay(
    authority_checkpoint: &CurrentRepositoryV4,
    current: &CurrentRepositoryV4,
) -> Result<(), String> {
    authority_checkpoint.verify()?;
    current.verify()?;

    if authority_checkpoint.frontier_id != current.frontier_id
        || authority_checkpoint.profile_root != current.profile_root
        || authority_checkpoint.origin_id != current.origin_id
        || authority_checkpoint.origin_root != current.origin_root
    {
        return Err("routine evidence changes repository identity".into());
    }
    if authority_checkpoint.authority_keyset_root != current.authority_keyset_root
        || authority_checkpoint.authority_policy_root != current.authority_policy_root
    {
        return Err("routine evidence changes repository authority configuration".into());
    }
    if authority_checkpoint.accepted_claims != current.accepted_claims {
        return Err("routine evidence changes accepted scientific Standing".into());
    }

    let removed_pending = authority_checkpoint
        .pending_claims
        .iter()
        .filter(|retained| {
            !current
                .pending_claims
                .iter()
                .any(|candidate| candidate.claim_id == retained.claim_id && candidate == *retained)
        })
        .count();
    let added_withdrawals = current
        .proposal_withdrawals
        .iter()
        .filter(|candidate| {
            !authority_checkpoint
                .proposal_withdrawals
                .iter()
                .any(|retained| retained.id == candidate.id && retained == *candidate)
        })
        .count();
    if removed_pending > added_withdrawals {
        return Err(
            "routine evidence removes pending Claims without one appended Proposal Withdrawal per removal"
                .into(),
        );
    }
    require_unchanged_or_removed_claim_refs(
        "pending Claim",
        &authority_checkpoint.pending_claims,
        &current.pending_claims,
    )?;
    require_retained_object_refs(
        "Proposal",
        &authority_checkpoint.proposals,
        &current.proposals,
    )?;
    require_retained_object_refs(
        "Proposal Withdrawal",
        &authority_checkpoint.proposal_withdrawals,
        &current.proposal_withdrawals,
    )?;
    require_retained_object_refs(
        "Submission",
        &authority_checkpoint.submissions,
        &current.submissions,
    )?;
    require_retained_object_refs(
        "Verification Record",
        &authority_checkpoint.verifications,
        &current.verifications,
    )?;
    require_retained_object_refs(
        "Artifact",
        &authority_checkpoint.artifacts,
        &current.artifacts,
    )?;
    Ok(())
}

fn require_unchanged_or_removed_claim_refs(
    label: &str,
    checkpoint: &[ClaimStandingRefV1],
    current: &[ClaimStandingRefV1],
) -> Result<(), String> {
    for retained in checkpoint {
        if let Some(candidate) = current
            .iter()
            .find(|candidate| candidate.claim_id == retained.claim_id)
            && candidate != retained
        {
            return Err(format!("routine evidence rewrites retained {label}"));
        }
    }
    Ok(())
}

fn require_retained_object_refs(
    label: &str,
    checkpoint: &[RepositoryObjectRefV1],
    current: &[RepositoryObjectRefV1],
) -> Result<(), String> {
    for retained in checkpoint {
        match current.iter().find(|candidate| candidate.id == retained.id) {
            Some(candidate) if candidate == retained => {}
            Some(_) => return Err(format!("routine evidence rewrites retained {label}")),
            None => return Err(format!("routine evidence removes retained {label}")),
        }
    }
    Ok(())
}

/// Require every retained `records/**` byte to be explained exactly once by
/// either signed authority history or the validated current evidence graph.
///
/// `evidence_references` includes direct manifest references plus transitive
/// Claim references held by Proposals. Paths and roots are supplied only after
/// their canonical object parsers have succeeded. This is intentionally a
/// coverage check, not a second record format or transaction log.
pub(crate) fn verify_current_record_coverage(
    authority_covered: &BTreeMap<String, String>,
    evidence_references: &BTreeMap<String, String>,
    observed: &BTreeMap<String, String>,
) -> Result<(), String> {
    let mut expected = authority_covered.clone();
    for (path, root) in evidence_references {
        if let Some(previous) = expected.insert(path.clone(), root.clone())
            && previous != *root
        {
            return Err(format!(
                "current evidence reference disagrees with retained authority bytes at {path}"
            ));
        }
    }
    for (path, root) in &expected {
        match observed.get(path) {
            Some(observed_root) if observed_root == root => {}
            Some(_) => return Err(format!("current record bytes disagree at {path}")),
            None => {
                return Err(format!(
                    "current repository is missing retained record {path}"
                ));
            }
        }
    }
    if let Some(path) = observed.keys().find(|path| !expected.contains_key(*path)) {
        return Err(format!(
            "current repository contains unexplained record {path}"
        ));
    }
    Ok(())
}

fn authority_store_files(directory: &Path, suffix: &str) -> Result<Vec<PathBuf>, String> {
    let mut files = fs::read_dir(directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.ends_with(suffix))
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn root_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn git_text(frontier: &Path, args: &[&str]) -> Result<String, String> {
    vela_edge::git::text(frontier, args)
}

/// The set of paths a current repository must no longer carry.
///
/// This is the verifier's own answer, and it is the only one. The authority
/// writer asks it before admitting an object draft, so a repository that
/// replay would refuse cannot be written in the first place.
pub(crate) fn is_retired_current_path(path: &str) -> bool {
    path == ".vela/actors.json"
        || path == "frontier.yaml"
        || path == "frontier.json"
        || path.starts_with(".vela/events/")
        || path.starts_with(".vela/findings/")
        || path.starts_with(".vela/proposals/")
        || path.starts_with(".vela/artifacts/")
        || path.starts_with(".vela/policies/")
        || path.starts_with("records/receipts/")
        || path.starts_with("records/review/")
        || path.starts_with("records/decision-evidence/")
        || (path.starts_with("records/vrc_") && path.ends_with(".json"))
}

fn files_recursive(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("read {}: {error}", directory.display()))?
        {
            let path = entry
                .map_err(|error| format!("read {} entry: {error}", directory.display()))?
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use vela_protocol::authority::{AUTHORITY_MODE, AuthorityEventContentV1};
    use vela_protocol::claim_record::ClaimAssertion;
    use vela_protocol::events::{NULL_HASH, StateActor, StateTarget};
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::proposal_v1::{ProposalProducerPackage, ProposalSubject};
    use vela_protocol::submission_v1::{
        RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    };
    use vela_protocol::verification_record::{
        IndependenceDisclosure, VerificationMethod, VerificationRecordDraft, VerificationScope,
        VerificationSubject,
    };

    use super::*;

    #[test]
    fn current_repository_rejects_retired_profile_paths() {
        assert!(is_retired_current_path("frontier.yaml"));
        assert!(is_retired_current_path("frontier.json"));
        assert!(!is_retired_current_path("frontier.toml"));
    }

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn submission_artifact(byte: char) -> SubmissionArtifact {
        SubmissionArtifact {
            kind: "fixture".into(),
            path: format!("artifacts/{byte}.json"),
            digest: root(byte),
        }
    }

    fn signed_submission_and_claim() -> (SubmissionV1, ClaimRecordV1) {
        let key = SigningKey::from_bytes(&[71_u8; 32]);
        let producer = "agent:proposal-binding-fixture";
        let emitted_at = "2026-07-27T00:00:00Z";
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: producer.into(),
                actor_class: ActorClass::Agent,
                created_at: emitted_at.into(),
            },
            &key,
        )
        .unwrap();
        let submission = SubmissionV1::build(
            SubmissionDraft {
                claim: SubmissionClaim {
                    assertion: "An exact bounded search completed.".into(),
                    claim_type: "computational".into(),
                    conditions: vec!["The exact retained range was replayed.".into()],
                },
                artifacts: vec![submission_artifact('a')],
                caveats: vec!["The bounded result is not universal.".into()],
                replayability: "exact".into(),
                producer_checks: Vec::new(),
                verification_requirements: vec!["Replay the exact Artifact.".into()],
                requested_change: RequestedChange {
                    kind: "add_claim".into(),
                    target: None,
                },
                provenance: SubmissionProvenance {
                    producer: producer.into(),
                    source_system: "fixture".into(),
                    source_attempt: None,
                    source_run: None,
                    emitted_at: emitted_at.into(),
                },
                execution_binding: None,
            },
            identity,
            &key,
        )
        .unwrap();
        let claim = ClaimRecordV1::build(
            1,
            ClaimAssertion {
                text: submission.claim.assertion.clone(),
                kind: submission.claim.claim_type.clone(),
            },
            vec![
                submission.claim.conditions[0].clone(),
                format!("Caveat: {}", submission.caveats[0]),
            ],
            vec![vela_protocol::claim_record::ClaimEvidenceRef {
                relation: "supports".into(),
                artifact_id: None,
                artifact_root: submission.artifacts[0].digest.clone(),
                artifact_path: Some(format!(
                    "records/artifacts/sha256/{}",
                    submission.artifacts[0]
                        .digest
                        .strip_prefix("sha256:")
                        .unwrap()
                )),
            }],
            vec![vela_protocol::claim_record::ClaimSource {
                kind: "submission".into(),
                title: format!("Authenticated Submission {}", submission.submission_id),
                locator: None,
                authors: vec![producer.into()],
                year: Some(2026),
            }],
            Vec::new(),
            emitted_at.into(),
            BTreeMap::new(),
        )
        .unwrap();
        (submission, claim)
    }

    #[test]
    fn proposal_directly_binds_its_signed_submission() {
        let (submission, claim) = signed_submission_and_claim();
        let proposal = ProposalV1::build(
            "claim.add".into(),
            ProposalSubject {
                kind: "claim".into(),
                id: claim.claim_id.clone(),
                root: claim.canonical_root().unwrap(),
            },
            submission.provenance.producer.clone(),
            "2026-07-27T00:00:01Z".into(),
            "Review the exact signed Submission.".into(),
            ProposalProducerPackage {
                kind: "submission_v1".into(),
                id: submission.submission_id.clone(),
                root: submission.canonical_root().unwrap(),
                path: format!(
                    "records/submissions/sha256/{}.json",
                    submission
                        .canonical_root()
                        .unwrap()
                        .strip_prefix("sha256:")
                        .unwrap()
                ),
            },
            submission.caveats.clone(),
        )
        .unwrap();
        proposal_matches_signed_submission(&proposal, &claim, &submission).unwrap();

        let wrong_action = ProposalV1::build(
            "claim.revise".into(),
            proposal.subject.clone(),
            proposal.actor.clone(),
            proposal.created_at.clone(),
            proposal.reason.clone(),
            proposal.producer_package.clone(),
            proposal.caveats.clone(),
        )
        .unwrap();
        assert!(
            proposal_matches_signed_submission(&wrong_action, &claim, &submission)
                .unwrap_err()
                .contains("action disagrees")
        );
    }

    fn current_review_lineage() -> (ProposalV1, ClaimRecordV1, VerificationRecordV1) {
        let submission_id = "vsb_ce7f0f4d4b6a4c40".to_string();
        let submission_root = root('2');
        let claim = ClaimRecordV1::build(
            1,
            ClaimAssertion {
                text: "An exact bounded search completed.".into(),
                kind: "computational".into(),
            },
            vec!["The frozen verifier replays the exact range.".into()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            "2026-07-27T00:00:00Z".into(),
            BTreeMap::new(),
        )
        .unwrap();
        let proposal = ProposalV1::build(
            "claim.add".into(),
            ProposalSubject {
                kind: "claim".into(),
                id: claim.claim_id.clone(),
                root: claim.canonical_root().unwrap(),
            },
            "agent:producer-fixture".into(),
            "2026-07-27T00:00:01Z".into(),
            "Submit the exact bounded result.".into(),
            ProposalProducerPackage {
                kind: "submission_v1".into(),
                id: submission_id.clone(),
                root: submission_root.clone(),
                path: "records/submissions/sha256/fixture.json".into(),
            },
            vec!["The bounded result is not universal.".into()],
        )
        .unwrap();
        let key = SigningKey::from_bytes(&[73_u8; 32]);
        let verifier = "verifier:fixture";
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: verifier.into(),
                actor_class: ActorClass::Org,
                created_at: "2026-07-27T00:00:02Z".into(),
            },
            &key,
        )
        .unwrap();
        let verification = VerificationRecordV1::build(
            VerificationRecordDraft {
                subject: VerificationSubject {
                    claim_id: claim.claim_id.clone(),
                    artifact_ids: vec!["a".repeat(64)],
                    submission_id,
                    submission_root,
                    proposal_id: proposal.proposal_id.clone(),
                },
                method: VerificationMethod {
                    profile: "fixture-v1".into(),
                    implementation: "fixture-verifier".into(),
                    environment_root: root('5'),
                },
                scope: VerificationScope {
                    property: "Replay the frozen verifier.".into(),
                    does_not_establish: vec!["Scientific acceptance.".into()],
                },
                outcome: "pass".into(),
                verifier: verifier.into(),
                independence: IndependenceDisclosure {
                    declared_independent_of: vec!["agent:producer-fixture".into()],
                    shared_dependencies: Vec::new(),
                },
                output_artifact_ids: Vec::new(),
                started_at: "2026-07-27T00:00:03Z".into(),
                completed_at: "2026-07-27T00:00:04Z".into(),
            },
            identity,
            &key,
        )
        .unwrap();
        (proposal, claim, verification)
    }

    fn review_event(
        transaction_id: &str,
        kind: EventKind,
        proposal_id: &str,
        applied_event_id: Option<&str>,
    ) -> AuthorityEventV1 {
        let verdict = match kind {
            EventKind::ReviewAccepted => "accepted",
            EventKind::ReviewRejected => "rejected",
            _ => panic!("review fixture requires a terminal review kind"),
        };
        let mut payload = json!({
            "proposal_id": proposal_id,
            "proposal_kind": "claim.add",
            "verdict": verdict,
            "repository_before": root('1'),
            "repository_after": root('2'),
        });
        if let Some(applied_event_id) = applied_event_id {
            payload["applied_event_id"] = Value::String(applied_event_id.into());
        }
        AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: transaction_id.into(),
            principal_id: "local:fixture|uid:501".into(),
            authority_mode: AUTHORITY_MODE.into(),
            kind,
            target: StateTarget {
                r#type: "proposal".into(),
                id: proposal_id.into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: "local:fixture|uid:501".into(),
            },
            timestamp: "2026-07-27T00:00:01Z".into(),
            reason: "Decide the exact fixture Proposal.".into(),
            before_hash: NULL_HASH.into(),
            after_hash: NULL_HASH.into(),
            payload,
            caveats: Vec::new(),
        })
        .unwrap()
    }

    fn applied_event(proposal_id: &str, claim_id: &str) -> AuthorityEventV1 {
        AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "vtx_fixture_accept".into(),
            principal_id: "local:fixture|uid:501".into(),
            authority_mode: AUTHORITY_MODE.into(),
            kind: EventKind::FindingAsserted,
            target: StateTarget {
                r#type: "claim".into(),
                id: claim_id.into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: "local:fixture|uid:501".into(),
            },
            timestamp: "2026-07-27T00:00:01Z".into(),
            reason: "Accept the exact fixture Claim.".into(),
            before_hash: NULL_HASH.into(),
            after_hash: root('3'),
            payload: json!({
                "claim_id": claim_id,
                "claim_root": root('3'),
                "proposal_id": proposal_id,
                "repository_before": root('1'),
                "repository_after": root('2'),
            }),
            caveats: Vec::new(),
        })
        .unwrap()
    }

    #[test]
    fn current_proposal_standing_uses_the_linked_semantic_event() {
        let proposal_id = "vpr_0123456789abcdef";
        let domain = applied_event(proposal_id, &format!("vcl_{}", "3".repeat(64)));
        let applied_id = domain.semantic_event_id().unwrap();
        let review = review_event(
            "vtx_fixture_accept",
            EventKind::ReviewAccepted,
            proposal_id,
            Some(&applied_id),
        );
        let decisions = current_proposal_decisions(&[domain.clone(), review.clone()]).unwrap();
        let decision = decisions.get(proposal_id).unwrap();
        assert_eq!(decision.standing, "accepted");
        assert_eq!(
            decision.applied_event_id.as_deref(),
            Some(applied_id.as_str())
        );

        let decisions = current_proposal_decisions(&[review, domain]).unwrap();
        let decision = decisions.get(proposal_id).unwrap();
        assert_eq!(decision.standing, "accepted");
        assert_eq!(
            decision.applied_event_id.as_deref(),
            Some(applied_id.as_str())
        );
    }

    #[test]
    fn current_proposal_standing_requires_one_transaction() {
        let proposal_id = "vpr_0123456789abcdef";
        let domain = applied_event(proposal_id, &format!("vcl_{}", "3".repeat(64)));
        let applied_id = domain.semantic_event_id().unwrap();
        let review = review_event(
            "vtx_fixture_other",
            EventKind::ReviewAccepted,
            proposal_id,
            Some(&applied_id),
        );
        let review_id = review.id.clone();
        assert_eq!(
            current_proposal_decisions(&[review, domain]).unwrap_err(),
            format!(
                "current accepted review event {} names a domain event from another transaction",
                review_id
            )
        );
    }

    #[test]
    fn current_proposal_standing_rejects_missing_and_duplicate_decisions() {
        let proposal_id = "vpr_0123456789abcdef";
        let missing = review_event(
            "vtx_fixture_accept",
            EventKind::ReviewAccepted,
            proposal_id,
            Some("vev_0000000000000000"),
        );
        assert!(current_proposal_decisions(&[missing]).is_err());

        let first = review_event(
            "vtx_fixture_reject_one",
            EventKind::ReviewRejected,
            proposal_id,
            None,
        );
        let second = review_event(
            "vtx_fixture_reject_two",
            EventKind::ReviewRejected,
            proposal_id,
            None,
        );
        assert!(current_proposal_decisions(&[first, second]).is_err());
    }

    #[test]
    fn repository_manifest_root_follows_the_signed_delta_chain() {
        use vela_protocol::authority::ObjectDeltaV1;

        let initial = root('1');
        let submitted = root('2');
        let first = vec![ObjectDeltaV1 {
            path: ".vela/repository.json".into(),
            before_root: None,
            after_root: Some(initial.clone()),
            object_kind: "canonical_evidence".into(),
        }];
        let second = vec![ObjectDeltaV1 {
            path: ".vela/repository.json".into(),
            before_root: Some(initial.clone()),
            after_root: Some(submitted.clone()),
            object_kind: "repository_manifest".into(),
        }];
        let transitions =
            verify_repository_manifest_delta_chain([(1, first.as_slice()), (2, second.as_slice())])
                .unwrap();
        assert_eq!(transitions.last().unwrap().after_root, submitted);
        assert_eq!(
            transitions[1].before_root.as_deref(),
            Some(initial.as_str())
        );

        let adopted_overlay = vec![ObjectDeltaV1 {
            path: ".vela/repository.json".into(),
            before_root: Some(root('4')),
            after_root: Some(submitted.clone()),
            object_kind: "repository_manifest".into(),
        }];
        let transitions = verify_repository_manifest_delta_chain([
            (1, first.as_slice()),
            (2, adopted_overlay.as_slice()),
        ])
        .unwrap();
        assert_eq!(transitions.last().unwrap().after_root, submitted);
        assert_eq!(
            transitions[1].before_root.as_deref(),
            Some(root('4').as_str())
        );

        let missing_preimage = vec![ObjectDeltaV1 {
            path: ".vela/repository.json".into(),
            before_root: None,
            after_root: Some(root('5')),
            object_kind: "repository_manifest".into(),
        }];
        assert_eq!(
            verify_repository_manifest_delta_chain([
                (1, first.as_slice()),
                (2, missing_preimage.as_slice()),
            ])
            .unwrap_err(),
            "authority record 2 breaks repository manifest root continuity"
        );
    }

    fn repository_fixture() -> CurrentRepositoryV4 {
        CurrentRepositoryV4 {
            schema: vela_protocol::current_repository::CURRENT_REPOSITORY_SCHEMA_V4.into(),
            frontier_id: "vfr_0123456789abcdef".into(),
            profile_root: root('1'),
            origin_id: "vro_0123456789abcdef".into(),
            origin_root: root('2'),
            accepted_claims: Vec::new(),
            pending_claims: Vec::new(),
            proposals: Vec::new(),
            proposal_withdrawals: Vec::new(),
            submissions: Vec::new(),
            verifications: Vec::new(),
            artifacts: Vec::new(),
            authority_keyset_root: root('3'),
            authority_policy_root: root('4'),
        }
    }

    fn object_reference(kind: &str, id: &str, byte: char) -> RepositoryObjectRefV1 {
        let digest = byte.to_string().repeat(64);
        RepositoryObjectRefV1 {
            schema: format!("vela.{kind}.v1"),
            id: id.into(),
            root: format!("sha256:{digest}"),
            path: format!("records/{kind}/sha256/{digest}.json"),
        }
    }

    #[test]
    fn routine_evidence_overlay_is_append_only_and_cannot_change_standing() {
        let mut checkpoint = repository_fixture();
        checkpoint
            .proposals
            .push(object_reference("proposal", "vpr_0000000000000001", '5'));
        checkpoint.verify().unwrap();

        let mut current = checkpoint.clone();
        current.pending_claims.push(ClaimStandingRefV1 {
            claim_id: format!("vcl_{}", "6".repeat(64)),
            claim_root: root('6'),
            standing: "pending_review".into(),
            path: format!("records/claims/sha256/{}.json", "6".repeat(64)),
        });
        current
            .submissions
            .push(object_reference("submissions", "vsb_0000000000000001", '7'));
        verify_routine_evidence_overlay(&checkpoint, &current).unwrap();

        let mut withdrawn = current.clone();
        withdrawn.pending_claims.clear();
        withdrawn.proposal_withdrawals.push(object_reference(
            "proposal-withdrawals",
            "vpw_0000000000000001",
            'a',
        ));
        verify_routine_evidence_overlay(&current, &withdrawn).unwrap();

        let mut unbound_removal = current.clone();
        unbound_removal.pending_claims.clear();
        assert_eq!(
            verify_routine_evidence_overlay(&current, &unbound_removal).unwrap_err(),
            "routine evidence removes pending Claims without one appended Proposal Withdrawal per removal"
        );

        let mut accepted = current.clone();
        accepted.accepted_claims = vec![ClaimStandingRefV1 {
            claim_id: format!("vcl_{}", "8".repeat(64)),
            claim_root: root('8'),
            standing: "accepted".into(),
            path: format!("records/claims/sha256/{}.json", "8".repeat(64)),
        }];
        assert_eq!(
            verify_routine_evidence_overlay(&checkpoint, &accepted).unwrap_err(),
            "routine evidence changes accepted scientific Standing"
        );

        let mut removed = current.clone();
        removed.proposals.clear();
        assert_eq!(
            verify_routine_evidence_overlay(&checkpoint, &removed).unwrap_err(),
            "routine evidence removes retained Proposal"
        );

        let mut rewritten = current;
        rewritten.proposals[0].root = root('9');
        rewritten.proposals[0].path = format!("records/proposal/sha256/{}.json", "9".repeat(64));
        assert_eq!(
            verify_routine_evidence_overlay(&checkpoint, &rewritten).unwrap_err(),
            "routine evidence rewrites retained Proposal"
        );
    }

    #[test]
    fn current_record_coverage_unifies_authority_and_self_authenticated_evidence() {
        let authority = BTreeMap::from([("records/claims/sha256/a.json".into(), root('a'))]);
        let evidence = BTreeMap::from([("records/submissions/sha256/b.json".into(), root('b'))]);
        let observed = BTreeMap::from([
            ("records/claims/sha256/a.json".into(), root('a')),
            ("records/submissions/sha256/b.json".into(), root('b')),
        ]);
        verify_current_record_coverage(&authority, &evidence, &observed).unwrap();

        let mut unexplained = observed.clone();
        unexplained.insert("records/artifacts/sha256/c".into(), root('c'));
        assert_eq!(
            verify_current_record_coverage(&authority, &evidence, &unexplained).unwrap_err(),
            "current repository contains unexplained record records/artifacts/sha256/c"
        );

        let mut missing = observed.clone();
        missing.remove("records/claims/sha256/a.json");
        assert_eq!(
            verify_current_record_coverage(&authority, &evidence, &missing).unwrap_err(),
            "current repository is missing retained record records/claims/sha256/a.json"
        );

        let conflicting = BTreeMap::from([("records/claims/sha256/a.json".into(), root('d'))]);
        assert_eq!(
            verify_current_record_coverage(&authority, &conflicting, &observed).unwrap_err(),
            "current evidence reference disagrees with retained authority bytes at records/claims/sha256/a.json"
        );
    }

    #[test]
    fn verification_targets_exact_current_lineage() {
        let (proposal, claim, verification) = current_review_lineage();
        assert!(verification_targets_proposal(
            &proposal,
            &claim,
            &verification
        ));

        let mut direct = verification.clone();
        direct.subject.proposal_id = proposal.proposal_id.clone();
        direct.subject.claim_id = claim.claim_id.clone();
        assert!(verification_targets_proposal(&proposal, &claim, &direct));

        let mut wrong_submission_root = verification.clone();
        wrong_submission_root.subject.submission_root = root('9');
        assert!(!verification_targets_proposal(
            &proposal,
            &claim,
            &wrong_submission_root
        ));

        let mut wrong_proposal = verification.clone();
        wrong_proposal.subject.proposal_id = "vpr_0000000000000000".into();
        assert!(!verification_targets_proposal(
            &proposal,
            &claim,
            &wrong_proposal
        ));

        let mut wrong_claim = verification.clone();
        wrong_claim.subject.claim_id = "vf_0000000000000000".into();
        assert!(!verification_targets_proposal(
            &proposal,
            &claim,
            &wrong_claim
        ));

        let mut unrelated_claim = claim.clone();
        unrelated_claim.claim_id = format!("vcl_{}", "9".repeat(64));
        assert!(!verification_targets_proposal(
            &proposal,
            &unrelated_claim,
            &verification
        ));
    }

    #[test]
    fn terminal_verification_uses_retained_rooted_claim() {
        let temporary = tempfile::tempdir().unwrap();
        let (proposal, claim, verification) = current_review_lineage();
        let claim_root = claim.canonical_root().unwrap();
        let claim_path =
            crate::current_submission::rooted_path("records/claims/sha256", &claim_root).unwrap();
        let absolute = temporary.path().join(claim_path);
        fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        fs::write(&absolute, claim.canonical_bytes().unwrap()).unwrap();

        assert!(
            verification_targets_rooted_proposal(temporary.path(), &proposal, &verification)
                .unwrap()
        );

        fs::write(&absolute, b"{}\n").unwrap();
        assert!(
            verification_targets_rooted_proposal(temporary.path(), &proposal, &verification)
                .is_err()
        );
    }
}
