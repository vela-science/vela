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
    CURRENT_ARTIFACT_RECORD_SCHEMA_V1, CurrentArtifactRecordV1, CurrentFrontierProfileV2,
    CurrentRepositoryV2, CurrentRepositoryV3, RepositoryObjectRefV1,
};
use vela_protocol::events::{EventKind, NULL_HASH};
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::repository_epoch::RepositoryBoundaryV1;
use vela_protocol::repository_origin::RepositoryOriginV1;
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

pub(crate) fn cmd_repository_verify(frontier: &Path, json_out: bool) {
    crate::ui::set_mode("repository verify", json_out);
    let frontier = frontier.canonicalize().unwrap_or_else(|error| {
        crate::cli::fail_return(&format!(
            "resolve current Frontier {}: {error}",
            frontier.display()
        ))
    });
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
    if frontier.join(".vela/origin.json").is_file() {
        let repository = verify_compacted_repository_at(&frontier, true)
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
            "command": "repository verify",
            "frontier": frontier.display().to_string(),
            "frontier_id": repository.frontier_id,
            "git_commit": commit,
            "git_tree": tree,
            "origin_id": origin.origin_id,
            "origin_root": origin.canonical_root()
                .unwrap_or_else(|error| crate::cli::fail_return(&error)),
            "repository_root": repository.canonical_root()
                .unwrap_or_else(|error| crate::cli::fail_return(&error)),
            "authority_keyset_root": repository.authority_keyset_root,
            "authority_policy_root": repository.authority_policy_root,
            "counts": {
                "accepted_claims": repository.accepted_claims.len(),
                "pending_claims": repository.pending_claims.len(),
                "proposals": repository.proposals.len(),
                "submissions": repository.submissions.len(),
                "registrations": repository.registrations.len(),
                "verifications": repository.verifications.len(),
                "artifacts": repository.artifacts.len()
            },
        });
        if json_out {
            crate::cli::print_json(&payload);
        } else {
            println!("current repository verified");
            println!("  frontier: {}", payload["frontier_id"]);
            println!("  origin: {}", payload["origin_id"]);
            println!("  claims: {}", payload["counts"]["accepted_claims"]);
            println!("  repository root: {}", payload["repository_root"]);
        }
        return;
    }
    let repository = verify_predecessor_repository_at(&frontier, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let epoch_bytes = fs::read(frontier.join(".vela/epoch.json")).unwrap_or_else(|error| {
        crate::cli::fail_return(&format!("read current repository epoch: {error}"))
    });
    let epoch = RepositoryBoundaryV1::parse(&epoch_bytes)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let commit = git_text(&frontier, &["rev-parse", "HEAD^{commit}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let tree = git_text(&frontier, &["rev-parse", "HEAD^{tree}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let payload = json!({
        "schema": "vela.repository-verification.v1",
        "ok": true,
        "command": "repository verify",
        "frontier": frontier.display().to_string(),
        "frontier_id": repository.frontier_id,
        "git_commit": commit,
        "git_tree": tree,
        "epoch_id": epoch.epoch_id(),
        "epoch_root": epoch.canonical_root().unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "repository_root": repository.canonical_root().unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "authority_keyset_root": repository.authority_keyset_root,
        "authority_policy_root": repository.authority_policy_root,
        "counts": {
            "accepted_claims": repository.accepted_claims.len(),
            "pending_claims": repository.pending_claims.len(),
            "proposals": repository.proposals.len(),
            "submissions": repository.submissions.len(),
            "registrations": repository.registrations.len(),
            "verifications": repository.verifications.len(),
            "artifacts": repository.artifacts.len()
        },
    });
    if json_out {
        crate::cli::print_json(&payload);
    } else {
        println!("current repository verified");
        println!("  frontier: {}", payload["frontier_id"]);
        println!("  epoch: {}", payload["epoch_id"]);
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

pub(crate) fn cmd_current_status(frontier: &Path, json_out: bool) {
    crate::ui::set_mode("status", json_out);
    let frontier = frontier.canonicalize().unwrap_or_else(|error| {
        crate::cli::fail_return(&format!(
            "resolve current Frontier {}: {error}",
            frontier.display()
        ))
    });
    let profile_source =
        fs::read_to_string(frontier.join("frontier.yaml")).unwrap_or_else(|error| {
            crate::cli::fail_return(&format!("read current Frontier Profile: {error}"))
        });
    let profile = CurrentFrontierProfileV2::from_yaml_str(&profile_source)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    if !frontier.join(".vela/epoch.json").exists()
        && !frontier.join(".vela/repository.json").exists()
    {
        verify_current_bootstrap_at(&frontier)
            .unwrap_or_else(|error| crate::cli::fail_return(&error));
        let commit = git_text(&frontier, &["rev-parse", "HEAD^{commit}"]).ok();
        let tree = git_text(&frontier, &["rev-parse", "HEAD^{tree}"]).ok();
        let next_action = format!(
            "vela authority init {} --reason 'Establish repository authority.' --json",
            frontier.display()
        );
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
                "epoch": Value::Null,
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
                "submissions": 0,
                "registrations": 0,
                "verifications": 0,
                "artifacts": 0
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
    let repository = load_compacted_repository_at(&frontier, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let commit = git_text(&frontier, &["rev-parse", "HEAD^{commit}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let tree = git_text(&frontier, &["rev-parse", "HEAD^{tree}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let decisions = load_current_proposal_decisions(&frontier, &repository)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let pending_proposals = repository
        .proposals
        .iter()
        .filter(|proposal| !decisions.contains_key(&proposal.id))
        .collect::<Vec<_>>();
    let pending_review = pending_proposals.len();
    let accepted_review = decisions
        .values()
        .filter(|decision| decision.standing == "accepted")
        .count();
    let rejected_review = decisions
        .values()
        .filter(|decision| decision.standing == "rejected")
        .count();
    let next_action = if let Some(proposal) = pending_proposals.first() {
        format!(
            "vela review show {} {} --json",
            frontier.display(),
            proposal.id
        )
    } else {
        format!("vela repository verify {} --json", frontier.display())
    };
    let payload = json!({
        "schema": "vela.status.v1",
        "ok": true,
        "command": "status",
        "frontier": {
            "id": repository.frontier_id,
            "name": profile.name,
            "profile_root": repository.profile_root
        },
        "git": {
            "commit": commit,
            "tree": tree
        },
        "integrity": {
            "replay": "verified",
            "strict": "pass",
            "blocker_count": 0,
            "blockers_by_code": {}
        },
        "roots": {
            "origin": repository.origin_root,
            "repository": repository.canonical_root().unwrap_or_else(|error| crate::cli::fail_return(&error)),
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
            "submissions": repository.submissions.len(),
            "registrations": repository.registrations.len(),
            "verifications": repository.verifications.len(),
            "artifacts": repository.artifacts.len()
        },
        "next_action": next_action,
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
        println!("  replay    verified");
        println!("  strict    pass");
        println!("  claims    {}", payload["counts"]["claims"]);
        println!(
            "  review    {} pending",
            payload["counts"]["pending_review"]
        );
        println!(
            "  next      {}",
            payload["next_action"].as_str().unwrap_or("none")
        );
    }
}

pub(crate) fn verify_current_profile_at(root: &Path) -> Result<CurrentFrontierProfileV2, String> {
    let profile_source = fs::read_to_string(root.join("frontier.yaml"))
        .map_err(|error| format!("read current frontier.yaml: {error}"))?;
    CurrentFrontierProfileV2::from_yaml_str(&profile_source)
}

pub(crate) fn verify_current_bootstrap_at(root: &Path) -> Result<CurrentFrontierProfileV2, String> {
    let profile = verify_current_profile_at(root)?;
    if root.join(".vela/epoch.json").exists()
        || root.join(".vela/origin.json").exists()
        || root.join(".vela/repository.json").exists()
    {
        return Err(
            "current repository bootstrap cannot contain an epoch, origin, or repository manifest"
                .into(),
        );
    }
    for relative in [
        ".vela/authority",
        ".vela/claims",
        ".vela/proposals",
        ".vela/submissions",
        ".vela/registrations",
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

pub(crate) fn verify_compaction_bootstrap_at(
    root: &Path,
    expected_objects: &[RepositoryObjectRefV1],
) -> Result<CurrentFrontierProfileV2, String> {
    let profile = verify_current_profile_at(root)?;
    if root.join(".vela/epoch.json").exists()
        || root.join(".vela/origin.json").exists()
        || root.join(".vela/repository.json").exists()
        || root.join(".vela/authority").exists()
    {
        return Err(
            "repository compaction bootstrap cannot contain an authority boundary or manifest"
                .into(),
        );
    }
    for relative in [
        ".vela/claims",
        ".vela/proposals",
        ".vela/submissions",
        ".vela/registrations",
        ".vela/verifications",
        ".vela/artifacts",
    ] {
        if root.join(relative).exists() {
            return Err(format!(
                "repository compaction bootstrap contains retired canonical object path {relative}"
            ));
        }
    }

    let mut expected_paths = BTreeMap::new();
    for reference in expected_objects {
        let path = crate::frontier_txn::RepoPath::parse(reference.path.clone())
            .map_err(|error| format!("invalid compaction object path: {error}"))?;
        if !path.as_str().starts_with("records/") {
            return Err(format!(
                "repository compaction object {} is outside records/",
                path.as_str()
            ));
        }
        if expected_paths
            .insert(path.as_str().to_string(), reference.root.clone())
            .is_some()
        {
            return Err(format!(
                "repository compaction repeats object path {}",
                path.as_str()
            ));
        }
        let bytes = fs::read(root.join(path.as_str()))
            .map_err(|error| format!("read compaction object {}: {error}", path.as_str()))?;
        if root_bytes(&bytes) != reference.root {
            return Err(format!(
                "repository compaction object {} does not match {}",
                path.as_str(),
                reference.root
            ));
        }
    }
    let records = root.join("records");
    let observed_paths = if records.exists() {
        files_recursive(&records)?
            .into_iter()
            .map(|path| {
                path.strip_prefix(root)
                    .map(|path| {
                        path.to_string_lossy()
                            .replace(std::path::MAIN_SEPARATOR, "/")
                    })
                    .map_err(|_| "repository compaction object escaped its Frontier".to_string())
            })
            .collect::<Result<BTreeSet<_>, _>>()?
    } else {
        BTreeSet::new()
    };
    if observed_paths != expected_paths.keys().cloned().collect::<BTreeSet<_>>() {
        return Err(
            "repository compaction bootstrap contains missing or unexplained record files".into(),
        );
    }
    Ok(profile)
}

pub(crate) fn cmd_current_next(frontier: &Path, limit: usize, json_out: bool) {
    crate::ui::set_mode("next", json_out);
    let frontier = frontier.canonicalize().unwrap_or_else(|error| {
        crate::cli::fail_return(&format!(
            "resolve current Frontier {}: {error}",
            frontier.display()
        ))
    });
    let repository = load_compacted_repository_at(&frontier, true)
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
                "available": 0,
                "leased": 0,
                "returned": 0
            },
            "targets": [],
            "next_action": "No Target Index is configured; inspect the Frontier before inventing work.",
        });
        if json_out {
            crate::cli::print_json(&payload);
        } else {
            println!("next · no configured Target Offers");
        }
        return;
    };
    let configured = assessment.configured_open();
    let fresh = assessment.fresh_open_targets();
    let available = fresh.len();
    let limit = limit.clamp(1, 128);
    let offers = fresh
        .into_iter()
        .take(limit)
        .map(|target| {
            json!({
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
                "lease_state": "available",
                "next_command": format!(
                    "vela start {} --frontier {} --as agent:<name> --json",
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
            "stale": configured.saturating_sub(available),
            "available": available,
            "leased": 0,
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
    repository: &CurrentRepositoryV3,
) -> Result<BTreeMap<String, CurrentProposalDecision>, String> {
    let origin_bytes = fs::read(frontier.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    let authority = crate::cli::load_compacted_repository_authority(frontier, repository, &origin)?;
    current_proposal_decisions(&authority.history.authority_events)
}

fn validate_current_proposal_standing(
    root: &Path,
    accepted_claims: &[vela_protocol::current_repository::ClaimStandingRefV1],
    pending_claims: &[vela_protocol::current_repository::ClaimStandingRefV1],
    proposals: &[RepositoryObjectRefV1],
    events: &[AuthorityEventV1],
) -> Result<(), String> {
    let decisions = current_proposal_decisions(events)?;
    for proposal_id in decisions.keys() {
        if !proposals.iter().any(|proposal| proposal.id == *proposal_id) {
            return Err(format!(
                "current Decision targets Proposal {proposal_id} outside the repository"
            ));
        }
    }
    for reference in proposals {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let proposal = ProposalV1::parse(&bytes)?;
        let claim = rooted_claim_for_proposal(root, &proposal)?;
        let pending = pending_claims.iter().any(|candidate| {
            candidate.claim_id == proposal.subject.id
                && candidate.claim_root == proposal.subject.root
        });
        let accepted = accepted_claims.iter().any(|candidate| {
            candidate.claim_id == proposal.subject.id
                && candidate.claim_root == proposal.subject.root
        });
        let decision = decisions.get(&proposal.proposal_id);
        let standing = decision
            .map(|decision| decision.standing.as_str())
            .unwrap_or("pending_review");
        let expected = match (proposal.action.as_str(), standing) {
            ("claim.add" | "claim.revise", "pending_review") => (true, false),
            ("claim.add" | "claim.revise", "accepted") => (false, true),
            ("claim.add" | "claim.revise", "rejected") => (false, false),
            ("claim.withdraw", "pending_review" | "rejected") => (false, true),
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
    let status = status.unwrap_or("pending_review");
    if !["pending_review", "accepted", "rejected", "all"].contains(&status) {
        crate::cli::fail_return::<()>(
            "current review status must be pending_review, accepted, rejected, or all",
        );
    }
    let frontier = frontier.canonicalize().unwrap_or_else(|error| {
        crate::cli::fail_return(&format!(
            "resolve current Frontier {}: {error}",
            frontier.display()
        ))
    });
    let repository = load_compacted_repository_at(&frontier, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let decisions = load_current_proposal_decisions(&frontier, &repository)
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
            let standing = decision
                .map(|decision| decision.standing.as_str())
                .unwrap_or("pending_review");
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
                    "decision": decision
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
    let total = items.len();
    let limit = limit.clamp(1, 100);
    let start = match cursor {
        None => 0,
        Some(cursor) => items
            .iter()
            .position(|item| item["proposal_id"].as_str() == Some(cursor))
            .map(|index| index + 1)
            .unwrap_or_else(|| {
                crate::cli::fail_return("review cursor does not name an exact current Proposal")
            }),
    };
    let mut page = items
        .into_iter()
        .skip(start)
        .take(limit + 1)
        .collect::<Vec<_>>();
    let has_more = page.len() > limit;
    page.truncate(limit);
    let next_cursor = has_more
        .then(|| page.last().and_then(|item| item["proposal_id"].as_str()))
        .flatten();
    let payload = json!({
        "schema": "vela.review.v1",
        "ok": true,
        "command": "review.list",
        "frontier_id": repository.frontier_id,
        "repository_root": repository.canonical_root().unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "status": status,
        "order": "created_at_desc_then_proposal_id",
        "total": total,
        "returned": page.len(),
        "next_cursor": next_cursor,
        "items": page,
    });
    if json_out {
        crate::cli::print_json(&payload);
    } else {
        println!("review · {total} {status} proposal(s)");
        for item in payload["items"].as_array().into_iter().flatten() {
            println!(
                "  {}  {}  {}  {}",
                item["proposal_id"].as_str().unwrap_or(""),
                item["created_at"].as_str().unwrap_or(""),
                item["action"].as_str().unwrap_or(""),
                item["reason"].as_str().unwrap_or("")
            );
        }
    }
}

pub(crate) fn cmd_current_review_show(frontier: &Path, proposal_id: &str, json_out: bool) {
    crate::ui::set_mode("review show", json_out);
    let frontier = frontier.canonicalize().unwrap_or_else(|error| {
        crate::cli::fail_return(&format!(
            "resolve current Frontier {}: {error}",
            frontier.display()
        ))
    });
    let repository = load_compacted_repository_at(&frontier, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let decisions = load_current_proposal_decisions(&frontier, &repository)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let reference = repository
        .proposals
        .iter()
        .find(|reference| reference.id == proposal_id)
        .unwrap_or_else(|| {
            crate::cli::fail_return("current repository has no exact Proposal with that ID")
        });
    let proposal_bytes = read_rooted_object(&frontier, &reference.path, &reference.root)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let proposal =
        ProposalV1::parse(&proposal_bytes).unwrap_or_else(|error| crate::cli::fail_return(&error));
    let standing = decisions
        .get(proposal_id)
        .map(|decision| decision.standing.as_str())
        .unwrap_or("pending_review");
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
        "authority_boundary": "Verification records report bounded checks. Only a repository-authority Decision can change standing.",
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
        println!(
            "  reason: {}",
            payload["proposal"]["reason"].as_str().unwrap_or("")
        );
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
/// current Proposal lineage and retained Submission.
///
/// Records created before a repository-epoch transition remain signed over
/// their predecessor Proposal and Claim identities. ADR 0022 retains those
/// bytes as usable observations only when both identities map through the
/// current objects' exact `imported_from` blocks. New records bind the current
/// identities directly.
pub(crate) fn verification_targets_proposal(
    proposal: &ProposalV1,
    claim: &ClaimRecordV1,
    record: &VerificationRecordV1,
) -> bool {
    if claim.claim_id != proposal.subject.id {
        return false;
    }
    let direct_subject = record.subject.proposal_id == proposal.proposal_id
        && record.subject.claim_id == proposal.subject.id;
    let imported_subject = proposal.imported_from.as_ref().is_some_and(|source| {
        source.proposal_id == record.subject.proposal_id
            && claim
                .imported_from
                .as_ref()
                .is_some_and(|claim_source| claim_source.object_id == record.subject.claim_id)
    });
    (direct_subject || imported_subject)
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

pub(crate) fn verify_predecessor_repository_at(
    root: &Path,
    require_authority_record: bool,
) -> Result<CurrentRepositoryV2, String> {
    let profile_source = fs::read_to_string(root.join("frontier.yaml"))
        .map_err(|error| format!("read current frontier.yaml: {error}"))?;
    let profile = CurrentFrontierProfileV2::from_yaml_str(&profile_source)?;
    let profile_root = profile.profile_root()?;

    let epoch_bytes = fs::read(root.join(".vela/epoch.json"))
        .map_err(|error| format!("read current repository epoch: {error}"))?;
    let epoch = RepositoryBoundaryV1::parse(&epoch_bytes)?;
    let epoch_root = epoch.canonical_root()?;

    let repository_bytes = fs::read(root.join(".vela/repository.json"))
        .map_err(|error| format!("read current repository manifest: {error}"))?;
    let repository = CurrentRepositoryV2::parse(&repository_bytes)?;
    if repository.frontier_id != profile.frontier_id
        || repository.frontier_id != epoch.frontier_id()
        || repository.profile_root != profile_root
        || repository.epoch_id != epoch.epoch_id()
        || repository.epoch_root != epoch_root
    {
        return Err(
            "current Profile, repository manifest, and epoch do not bind the same identity".into(),
        );
    }
    let repository_root = repository.canonical_root()?;
    if root.join("targets.json").is_file() {
        let bytes = fs::read(root.join("targets.json"))
            .map_err(|error| format!("read current Target Index: {error}"))?;
        let index: vela_edge::target_index::PredecessorTargetIndexV3 =
            serde_json::from_slice(&bytes)
                .map_err(|error| format!("parse current Target Index: {error}"))?;
        index.validate()?;
        if index.canonical_bytes()?.as_slice() != bytes.as_slice()
            || index.frontier_id != repository.frontier_id
            || index.repository.epoch_id != repository.epoch_id
            || index.repository.repository_root != repository_root
        {
            return Err(
                "current Target Index does not bind the exact current repository".to_string(),
            );
        }
        // The predecessor index is inspected only while finalizing the
        // one-time compaction. Its exact canonical bytes and repository
        // binding are load-bearing here; source, packet, and worktree
        // preconditions are separately bound by the signed compaction plan.
    }

    for reference in &repository.accepted_claims {
        let bytes = read_rooted_object(root, &reference.path, &reference.claim_root)?;
        let claim = ClaimRecordV1::parse(&bytes)?;
        if claim.canonical_bytes()?.as_slice() != bytes.as_slice()
            || claim.claim_id != reference.claim_id
        {
            return Err(format!(
                "{} does not contain the declared canonical Claim",
                reference.path
            ));
        }
    }
    for reference in &repository.pending_claims {
        let bytes = read_rooted_object(root, &reference.path, &reference.claim_root)?;
        let claim = ClaimRecordV1::parse(&bytes)?;
        if claim.canonical_bytes()?.as_slice() != bytes.as_slice()
            || claim.claim_id != reference.claim_id
        {
            return Err(format!(
                "{} does not contain the declared canonical pending Claim",
                reference.path
            ));
        }
    }
    for reference in &repository.proposals {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let proposal = ProposalV1::parse(&bytes)?;
        if proposal.canonical_bytes()?.as_slice() != bytes.as_slice()
            || proposal.proposal_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Proposal",
                reference.path
            ));
        }
    }
    for reference in &repository.submissions {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let submission = vela_protocol::submission_v1::SubmissionV1::parse(&bytes)?;
        if submission.canonical_bytes()?.as_slice() != bytes.as_slice()
            || submission.submission_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Submission",
                reference.path
            ));
        }
    }
    for reference in &repository.registrations {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let registration = vela_protocol::registration_record::RegistrationRecordV1::parse(&bytes)?;
        if registration.canonical_bytes()?.as_slice() != bytes.as_slice()
            || registration.registration_record_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Registration Record",
                reference.path
            ));
        }
    }
    for reference in &repository.verifications {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let verification = vela_protocol::verification_record::VerificationRecordV1::parse(&bytes)?;
        if verification.canonical_bytes()?.as_slice() != bytes.as_slice()
            || verification.verification_record_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Verification Record",
                reference.path
            ));
        }
        let mut matching_proposals = Vec::new();
        for proposal_reference in &repository.proposals {
            let proposal_bytes =
                read_rooted_object(root, &proposal_reference.path, &proposal_reference.root)?;
            let proposal = ProposalV1::parse(&proposal_bytes)?;
            if proposal.proposal_id == verification.subject.proposal_id
                || proposal
                    .imported_from
                    .as_ref()
                    .is_some_and(|source| source.proposal_id == verification.subject.proposal_id)
            {
                matching_proposals.push((proposal_reference, proposal));
            }
        }
        let [(_, proposal)] = matching_proposals.as_slice() else {
            return Err(format!(
                "{} targets Proposal {} with {} current or imported matches",
                reference.path,
                verification.subject.proposal_id,
                matching_proposals.len()
            ));
        };
        let claim_matches = verification_targets_rooted_proposal(root, proposal, &verification)?;
        if !claim_matches {
            return Err(format!(
                "{} does not bind its exact current/imported Proposal subject and producer package",
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
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        if reference.schema == "content-addressed-artifact" {
            continue;
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{} is not JSON: {error}", reference.path))?;
        if value.get("schema").and_then(Value::as_str) != Some(reference.schema.as_str()) {
            return Err(format!("{} has the wrong Artifact schema", reference.path));
        }
        if reference.schema == CURRENT_ARTIFACT_RECORD_SCHEMA_V1 {
            let artifact = CurrentArtifactRecordV1::parse(&bytes)?;
            if artifact.artifact_id != reference.id {
                return Err(format!(
                    "{} does not contain the declared canonical Artifact",
                    reference.path
                ));
            }
        }
    }

    let keyset_path = root.join(format!(
        ".vela/authority/keysets/{}.json",
        repository
            .authority_keyset_root
            .trim_start_matches("sha256:")
    ));
    let keyset_bytes = fs::read(&keyset_path)
        .map_err(|error| format!("read current authority keyset: {error}"))?;
    let keyset: vela_protocol::authority::AuthorityKeysetV1 = serde_json::from_slice(&keyset_bytes)
        .map_err(|error| format!("parse current authority keyset: {error}"))?;
    keyset.validate()?;
    if keyset.frontier_id != repository.frontier_id
        || keyset.root()? != repository.authority_keyset_root
        || vela_protocol::canonical::to_canonical_bytes(&keyset)? != keyset_bytes
    {
        return Err("current repository authority keyset binding is invalid".into());
    }

    let policy_path = root.join(format!(
        ".vela/authority/policies/{}.json",
        repository
            .authority_policy_root
            .trim_start_matches("sha256:")
    ));
    let policy_bytes = fs::read(&policy_path)
        .map_err(|error| format!("read current authority policy: {error}"))?;
    let policy: vela_protocol::authority::PolicyBundleV1 = serde_json::from_slice(&policy_bytes)
        .map_err(|error| format!("parse current authority policy: {error}"))?;
    policy.validate()?;
    if policy.frontier_id != repository.frontier_id
        || policy.root()? != repository.authority_policy_root
        || vela_protocol::canonical::to_canonical_bytes(&policy)? != policy_bytes
    {
        return Err("current repository policy binding is invalid".into());
    }
    let material_paths = crate::authority_transaction::authority_policy_material_paths(&policy)
        .map_err(|error| error.to_string())?;
    let material = vela_authority::CedarPolicyMaterial {
        schema: fs::read_to_string(root.join(&material_paths[0]))
            .map_err(|error| format!("read current Cedar schema: {error}"))?,
        policies: fs::read_to_string(root.join(&material_paths[1]))
            .map_err(|error| format!("read current Cedar policies: {error}"))?,
        entities: serde_json::from_slice(
            &fs::read(root.join(&material_paths[2]))
                .map_err(|error| format!("read current Cedar entities: {error}"))?,
        )
        .map_err(|error| format!("parse current Cedar entities: {error}"))?,
    };
    material.validate_against(&policy)?;

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
        verify_current_epoch_authority(root, &repository, &epoch)?;
    }
    Ok(repository)
}

/// Verify the final current-origin repository produced by the one-time
/// pre-release compaction.
///
/// This verifier intentionally does not fall back to the predecessor epoch
/// reader. It is used to prove the replacement repository before the bridge
/// and all predecessor-only types are deleted.
pub(crate) fn load_compacted_repository_at(
    root: &Path,
    require_authority_record: bool,
) -> Result<CurrentRepositoryV3, String> {
    let profile_source = fs::read_to_string(root.join("frontier.yaml"))
        .map_err(|error| format!("read current frontier.yaml: {error}"))?;
    let profile = CurrentFrontierProfileV2::from_yaml_str(&profile_source)?;
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
    let repository = CurrentRepositoryV3::parse(&repository_bytes)?;
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
        let loaded = crate::cli::load_compacted_repository_authority(root, &repository, &origin)?;
        validate_current_proposal_standing(
            root,
            &repository.accepted_claims,
            &repository.pending_claims,
            &repository.proposals,
            &loaded.history.authority_events,
        )?;
        let mut records = Vec::with_capacity(loaded.history.authority_envelopes.len());
        for envelope in &loaded.history.authority_envelopes {
            records.push(crate::cli::authority_record_from_envelope(envelope)?);
        }
        verify_repository_manifest_delta_chain(
            records.iter().map(|record| {
                (
                    record.content.sequence,
                    record.content.object_delta.as_slice(),
                )
            }),
            &repository.canonical_root()?,
        )?;
    }
    Ok(repository)
}

pub(crate) fn verify_compacted_repository_at(
    root: &Path,
    require_authority_record: bool,
) -> Result<CurrentRepositoryV3, String> {
    let repository = load_compacted_repository_at(root, false)?;
    let origin_bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    let repository_root = repository.canonical_root()?;
    if root.join("targets.json").is_file() {
        let bytes = fs::read(root.join("targets.json"))
            .map_err(|error| format!("read current Target Index: {error}"))?;
        let index: vela_edge::target_index::TargetIndexV4 = serde_json::from_slice(&bytes)
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
    }
    for reference in &repository.submissions {
        let bytes = object_bytes
            .get(&reference.path)
            .ok_or_else(|| format!("current object {} was not loaded", reference.path))?;
        let submission = vela_protocol::submission_v1::SubmissionV1::parse(bytes)?;
        if submission.canonical_bytes()?.as_slice() != bytes.as_slice()
            || submission.submission_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Submission",
                reference.path
            ));
        }
    }
    for reference in &repository.registrations {
        let bytes = object_bytes
            .get(&reference.path)
            .ok_or_else(|| format!("current object {} was not loaded", reference.path))?;
        let registration = vela_protocol::registration_record::RegistrationRecordV1::parse(bytes)?;
        if registration.canonical_bytes()?.as_slice() != bytes.as_slice()
            || registration.registration_record_id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Registration Record",
                reference.path
            ));
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
                "compacted Artifact {} is not identified by its full content root",
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
                "compacted repository retains retired protocol path {relative}"
            ));
        }
    }
    if require_authority_record {
        verify_compacted_repository_authority(root, &repository, &origin)?;
    }
    Ok(repository)
}

fn read_current_object_set(
    root: &Path,
    repository: &CurrentRepositoryV3,
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
                .chain(&repository.submissions)
                .chain(&repository.registrations)
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
) -> Result<CurrentRepositoryV3, String> {
    let repository = verify_compacted_repository_at(root, false)?;
    let origin_bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    verify_compacted_repository_authority(root, &repository, &origin)?;
    Ok(repository)
}

fn compacted_initial_repository(
    root: &Path,
    origin: &RepositoryOriginV1,
) -> Result<CurrentRepositoryV3, String> {
    let commits = git_text(
        root,
        &[
            "log",
            "--format=%H",
            "--diff-filter=A",
            "--",
            ".vela/origin.json",
        ],
    )?
    .lines()
    .filter(|line| !line.is_empty())
    .map(ToString::to_string)
    .collect::<Vec<_>>();
    let [commit] = commits.as_slice() else {
        return Err(format!(
            "current repository must introduce its origin exactly once; found {} commits",
            commits.len()
        ));
    };
    let read_blob = |path: &str| -> Result<Vec<u8>, String> {
        let spec = format!("{commit}:{path}");
        let output = crate::git_hardened::output(root, &["show", &spec])?;
        if !output.status.success() {
            return Err(format!(
                "read compacted origin blob {spec}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(output.stdout)
    };
    let initial_origin_bytes = read_blob(".vela/origin.json")?;
    let initial_origin = RepositoryOriginV1::parse(&initial_origin_bytes)?;
    if initial_origin != *origin {
        return Err("current origin differs from its introducing Git commit".into());
    }
    let repository_bytes = read_blob(".vela/repository.json")?;
    let repository = CurrentRepositoryV3::parse(&repository_bytes)?;
    if repository.frontier_id != origin.frontier_id
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin.origin_id
        || repository.origin_root != origin.canonical_root()?
        || !repository.pending_claims.is_empty()
        || !repository.proposals.is_empty()
        || !repository.submissions.is_empty()
        || !repository.registrations.is_empty()
        || !repository.verifications.is_empty()
    {
        return Err(
            "origin-introducing commit is not one exact compacted repository bootstrap".into(),
        );
    }
    Ok(repository)
}

fn read_rooted_object(root: &Path, path: &str, expected_root: &str) -> Result<Vec<u8>, String> {
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

fn verify_current_epoch_authority(
    root: &Path,
    repository: &CurrentRepositoryV2,
    epoch: &RepositoryBoundaryV1,
) -> Result<(), String> {
    let loaded = crate::cli::load_current_repository_authority(root, repository, epoch)?;
    validate_current_proposal_standing(
        root,
        &repository.accepted_claims,
        &repository.pending_claims,
        &repository.proposals,
        &loaded.history.authority_events,
    )?;
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
            .map_err(|error| format!("parse current epoch initialization payload: {error}"))?;
    initialization.validate()?;
    let (initial_event_log_root, initial_actor_registry_root) = match epoch.predecessor_roots() {
        Some(roots) => (roots.event_log.as_str(), roots.actor_registry.as_str()),
        None => {
            let genesis = epoch
                .genesis()
                .ok_or_else(|| "repository boundary has no origin".to_string())?;
            (
                genesis.initial_event_log_root.as_str(),
                genesis.initial_actor_registry_root.as_str(),
            )
        }
    };
    if initialization.frontier_id != repository.frontier_id
        || initialization.initial_event_log_root != initial_event_log_root
        || initialization.initial_actor_registry_root != initial_actor_registry_root
        || initialization.new_authority_keyset_root != repository.authority_keyset_root
        || initialization.new_policy_bundle_root != repository.authority_policy_root
        || initialization.new_principal_id != event.content.principal_id
        || initialization.reason != epoch.reason()
    {
        return Err(
            "current epoch authority initialization does not bind the exact predecessor and current roots"
                .into(),
        );
    }

    let event_paths = authority_store_files(&root.join(".vela/authority/events"), ".json")?;
    if event_paths.len() != loaded.history.authority_events.len() {
        return Err("current authority event store contains unverified objects".into());
    }
    for path in event_paths {
        let bytes =
            fs::read(&path).map_err(|error| format!("read current authority event: {error}"))?;
        let stored: vela_protocol::authority::AuthorityEventV1 = serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse current authority event: {error}"))?;
        stored.validate()?;
        if vela_protocol::canonical::to_canonical_bytes(&stored)? != bytes
            || path.file_name().and_then(|value| value.to_str())
                != Some(format!("{}.json", stored.id).as_str())
        {
            return Err(
                "current authority event store contains a non-canonical or misnamed object".into(),
            );
        }
    }

    let record_paths = authority_store_files(&root.join(".vela/authority/records"), ".dsse.json")?;
    if record_paths.len() != loaded.history.authority_envelopes.len() {
        return Err("current authority record store contains unverified objects".into());
    }
    for path in record_paths {
        let bytes =
            fs::read(&path).map_err(|error| format!("read current authority record: {error}"))?;
        let envelope: vela_protocol::authority::AuthorityEnvelopeV1 =
            serde_json::from_slice(&bytes)
                .map_err(|error| format!("parse current authority envelope: {error}"))?;
        if vela_protocol::canonical::to_canonical_bytes(&envelope)? != bytes {
            return Err("current authority envelope is not canonical JSON".into());
        }
        let record = crate::cli::authority_record_from_envelope(&envelope)?;
        if path.file_name().and_then(|value| value.to_str())
            != Some(format!("{}.dsse.json", record.record_id).as_str())
        {
            return Err("current authority record filename does not match its identity".into());
        }
    }

    let first_envelope =
        loaded.history.authority_envelopes.first().ok_or_else(|| {
            "current repository authority has no initialization record".to_string()
        })?;
    let first = crate::cli::authority_record_from_envelope(first_envelope)?;
    if first.content.sequence != 1
        || first.content.event_ids != vec![event.id.clone()]
        || first.content.before_event_log_root != initial_event_log_root
        || first.content.principal.principal_id != event.content.principal_id
        || first.content.authorization.policy_bundle_root != initialization.new_policy_bundle_root
    {
        return Err(
            "current epoch authority record does not bind its exact event and roots".into(),
        );
    }
    let expected_after = vela_protocol::authority_history::authority_event_log_root(
        initial_event_log_root,
        &[event],
    )?;
    if first.content.after_event_log_root != expected_after {
        return Err("current epoch authority record has the wrong after-event root".into());
    }
    let required_delta = [
        (
            crate::authority_transaction::authority_event_path(&event.id),
            event.root()?,
        ),
        (".vela/epoch.json".into(), epoch.canonical_root()?),
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
                "current epoch authority record does not cover exact postimage {path}"
            ));
        }
    }
    let mut repository_records = Vec::with_capacity(loaded.history.authority_envelopes.len());
    for envelope in &loaded.history.authority_envelopes {
        repository_records.push(crate::cli::authority_record_from_envelope(envelope)?);
    }
    verify_repository_manifest_delta_chain(
        repository_records.iter().map(|record| {
            (
                record.content.sequence,
                record.content.object_delta.as_slice(),
            )
        }),
        &repository.canonical_root()?,
    )?;
    Ok(())
}

fn verify_compacted_repository_authority(
    root: &Path,
    repository: &CurrentRepositoryV3,
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
    let loaded = crate::cli::load_compacted_repository_authority(root, repository, origin)?;
    validate_current_proposal_standing(
        root,
        &repository.accepted_claims,
        &repository.pending_claims,
        &repository.proposals,
        &loaded.history.authority_events,
    )?;
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
    let initial_repository = compacted_initial_repository(root, origin)?;
    let mut initial_objects = initial_repository
        .accepted_claims
        .iter()
        .map(|reference| RepositoryObjectRefV1 {
            schema: vela_protocol::claim_record::CLAIM_RECORD_V1_SCHEMA.into(),
            id: reference.claim_id.clone(),
            root: reference.claim_root.clone(),
            path: reference.path.clone(),
        })
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
    verify_repository_manifest_delta_chain(
        repository_records.iter().map(|record| {
            (
                record.content.sequence,
                record.content.object_delta.as_slice(),
            )
        }),
        &repository.canonical_root()?,
    )?;
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
    let observed_record_paths = if root.join("records").exists() {
        files_recursive(&root.join("records"))?
    } else {
        Vec::new()
    }
    .into_iter()
    .map(|path| {
        path.strip_prefix(root)
            .map(|path| path.to_string_lossy().to_string())
            .map_err(|_| "current record path escaped its repository".to_string())
    })
    .collect::<Result<BTreeSet<_>, _>>()?;
    if observed_record_paths
        != covered_record_paths
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
    {
        return Err("current repository contains missing or unexplained record files".into());
    }
    let current_paths = repository
        .accepted_claims
        .iter()
        .chain(&repository.pending_claims)
        .map(|reference| reference.path.as_str())
        .chain(
            repository
                .proposals
                .iter()
                .chain(&repository.submissions)
                .chain(&repository.registrations)
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

fn verify_repository_manifest_delta_chain<'a>(
    records: impl IntoIterator<Item = (u64, &'a [vela_protocol::authority::ObjectDeltaV1])>,
    current_repository_root: &str,
) -> Result<(), String> {
    let mut active_root: Option<String> = None;
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
                    "current epoch authority record does not cover initial repository manifest"
                        .into(),
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
        } else if delta.before_root != active_root {
            return Err(format!(
                "authority record {sequence} breaks repository manifest root continuity"
            ));
        }
        active_root.clone_from(&delta.after_root);
    }
    if !saw_initial {
        return Err(
            "current epoch authority history lacks its initial repository manifest delta".into(),
        );
    }
    if active_root.as_deref() != Some(current_repository_root) {
        return Err(
            "current repository manifest root is not the final signed authority postimage".into(),
        );
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
    crate::git_hardened::text(frontier, args)
}

fn is_retired_current_path(path: &str) -> bool {
    path == ".vela/actors.json"
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
    use vela_protocol::claim_record::{ClaimAssertion, ImportedClaimSource};
    use vela_protocol::events::{NULL_HASH, StateActor, StateTarget};
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::proposal_v1::{
        ImportedProposalSource, ProposalProducerPackage, ProposalSubject,
    };
    use vela_protocol::verification_record::{
        IndependenceDisclosure, VerificationMethod, VerificationRecordDraft, VerificationScope,
        VerificationSubject,
    };

    use super::*;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn imported_review_lineage() -> (ProposalV1, ClaimRecordV1, VerificationRecordV1) {
        let predecessor_commit = "1".repeat(40);
        let legacy_claim_id = "vf_6e8f08edac62ff26".to_string();
        let legacy_proposal_id = "vpr_a983c9305332b0a8".to_string();
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
            Some(ImportedClaimSource {
                era: "era0".into(),
                object_id: legacy_claim_id.clone(),
                object_root: root('3'),
                predecessor_commit: predecessor_commit.clone(),
            }),
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
            "Register the exact bounded result.".into(),
            ProposalProducerPackage {
                kind: "submission_v1".into(),
                id: submission_id.clone(),
                root: submission_root.clone(),
                path: "records/submissions/sha256/fixture.json".into(),
            },
            vec!["The bounded result is not universal.".into()],
            Some(ImportedProposalSource {
                proposal_id: legacy_proposal_id.clone(),
                proposal_root: root('4'),
                predecessor_commit,
            }),
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
                    claim_id: legacy_claim_id,
                    artifact_ids: vec!["va_fixture".into()],
                    submission_id,
                    submission_root,
                    proposal_id: legacy_proposal_id,
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
        verify_repository_manifest_delta_chain(
            [(1, first.as_slice()), (2, second.as_slice())],
            &submitted,
        )
        .unwrap();

        assert_eq!(
            verify_repository_manifest_delta_chain(
                [(1, first.as_slice()), (2, second.as_slice())],
                &root('3'),
            )
            .unwrap_err(),
            "current repository manifest root is not the final signed authority postimage"
        );

        let broken = vec![ObjectDeltaV1 {
            path: ".vela/repository.json".into(),
            before_root: Some(root('4')),
            after_root: Some(submitted),
            object_kind: "repository_manifest".into(),
        }];
        assert_eq!(
            verify_repository_manifest_delta_chain(
                [(1, first.as_slice()), (2, broken.as_slice())],
                &root('3'),
            )
            .unwrap_err(),
            "authority record 2 breaks repository manifest root continuity"
        );
    }

    #[test]
    fn migrated_verification_targets_exact_imported_lineage() {
        let (proposal, claim, verification) = imported_review_lineage();
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
        unrelated_claim.imported_from = None;
        assert!(!verification_targets_proposal(
            &proposal,
            &unrelated_claim,
            &verification
        ));
    }

    #[test]
    fn terminal_imported_verification_uses_retained_rooted_claim() {
        let temporary = tempfile::tempdir().unwrap();
        let (proposal, claim, verification) = imported_review_lineage();
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
