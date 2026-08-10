//! Repository verification, reads, work offers, and review views.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::authority::AuthorityEventV1;
use vela_protocol::authority_history::AuthorityInitializationV1;
use vela_protocol::claim_record::ClaimRecordV1;
use vela_protocol::events::{EventKind, NULL_HASH};
use vela_protocol::proposal::ProposalV1;
use vela_protocol::proposal_withdrawal::ProposalWithdrawalEnvelopeV2;
use vela_protocol::repository::{
    ClaimStandingRefV1, RepositoryObjectRefV1, RepositoryProfileV1, RepositoryV4,
};
use vela_protocol::repository_origin::RepositoryOriginV1;
use vela_protocol::status::{
    REPOSITORY_HEAD_ROLE, ReplayState, StatusActions, StatusCounts, StatusDecisionInbox, StatusGit,
    StatusIntegrity, StatusRepository, StatusReviewAction, StatusRoots, StatusV4, StatusWorkAction,
    StrictState,
};
use vela_protocol::submission::SubmissionRecordV2;
use vela_protocol::verification_record::VerificationRecordEnvelopeV2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProposalDecision {
    pub(crate) standing: String,
    pub(crate) event_id: String,
    pub(crate) event_root: String,
    pub(crate) decided_at: String,
    pub(crate) actor: String,
    pub(crate) reason: String,
    pub(crate) applied_event_id: Option<String>,
}

pub(crate) fn cmd_replay_repository(repository_path: &Path, json_out: bool) {
    crate::ui::set_mode("replay", json_out);
    let repository_path = crate::ui::canonicalize_repo(repository_path);
    let sensitive = sensitive_paths(&repository_path);
    if !sensitive.is_empty() {
        let listed = sensitive
            .iter()
            .take(10)
            .map(|path| {
                path.strip_prefix(&repository_path)
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
    let repository = verify_repository_at(&repository_path, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let origin = RepositoryOriginV1::parse(
        &fs::read(repository_path.join(".vela/origin.json")).unwrap_or_else(|error| {
            crate::cli::fail_return(&format!("read current repository origin: {error}"))
        }),
    )
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let commit = git_text(&repository_path, &["rev-parse", "HEAD^{commit}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let tree = git_text(&repository_path, &["rev-parse", "HEAD^{tree}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let payload = json!({
        "schema": "vela.repository-verification.v3",
        "ok": true,
        "command": "replay",
        /* v2 spelled this `frontier`, which ADR 0039 left naming a derived
        query with no directory. What replay was always reporting is the
        directory it read, so v3 names it beside the other `repository_*`
        facts it is one of. The key moved with a version rather than in place
        because a caller reads it. */
        "repository_path": repository_path.display().to_string(),
        "repository_id": repository.repository_id,
        "git_commit": commit,
        "git_tree": tree,
        "origin_id": origin.id().unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "origin_root": origin.canonical_root()
            .unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "repository_root": repository.canonical_root().unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "authority_keyset_root": repository.authority_keyset_root,
        "authority_model_root": repository.authority_model_root,
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
        println!("  repository: {}", payload["repository_id"]);
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
) -> (StatusDecisionInbox, usize) {
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
        StatusDecisionInbox {
            pending_count: pending_count as u64,
            protocol_ready_count: protocol_ready_count as u64,
            protocol_blocked_count: protocol_blocked_count as u64,
            projection_root: Some(projection.projection_root.clone()),
            first_entry_root: projection
                .entries
                .first()
                .map(|entry| entry.entry_root.clone()),
        },
        pending_count,
    )
}

pub(crate) fn cmd_status(repository_path: &Path, json_out: bool) {
    crate::ui::set_mode("status", json_out);
    let repository_path = crate::ui::canonicalize_repo(repository_path);
    let profile_source =
        fs::read_to_string(repository_path.join("vela.toml")).unwrap_or_else(|error| {
            crate::cli::fail_return(&format!("read repository profile: {error}"))
        });
    let profile = RepositoryProfileV1::from_toml_str(&profile_source)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    if !repository_path.join(".vela/origin.json").exists()
        && !repository_path.join(".vela/repository.json").exists()
    {
        verify_bootstrap_at(&repository_path)
            .unwrap_or_else(|error| crate::cli::fail_return(&error));
        let commit = git_text(&repository_path, &["rev-parse", "HEAD^{commit}"]).ok();
        let tree = git_text(&repository_path, &["rev-parse", "HEAD^{tree}"]).ok();
        /* One command reports one document. This branch answered `status` with
        `vela.status.v1` while the initialized branch below answered with
        `vela.status.v4`: not two versions of one contract but one contract and
        one literal that never moved when the contract did, so a caller keying
        on `schema` saw a version it had no reader for and could not tell a
        cold repository from a stale release. The phase is a value, not a schema:
        it is `integrity`, and it is `actions.work.mode`, which names the one
        command that clears it. `phase` and `next_action` said the same two
        things in a shape only this branch had, and are gone with them.

        The two branches are now one type, `StatusV4`, which is what makes a
        per-branch `schema` literal unwritable rather than merely wrong. The
        nulls below stay explicit `None`s of always-present fields: see the
        type's header on why absence is a different document. */
        let payload = StatusV4::new(
            StatusRepository {
                id: profile.repository_id.clone(),
                name: profile.name.clone(),
                profile_root: profile
                    .profile_root()
                    .unwrap_or_else(|error| crate::cli::fail_return(&error)),
            },
            /* The role is what this Git pointer means, not whether it has
            reached a commit yet; a bootstrap has the role and null anchors. */
            StatusGit {
                role: REPOSITORY_HEAD_ROLE.into(),
                commit,
                tree,
            },
            StatusIntegrity {
                replay: ReplayState::NotInitialized,
                strict: StrictState::Blocked,
                blocker_count: 1,
                blockers_by_code: BTreeMap::from([(
                    "repository_authority_uninitialized".to_string(),
                    1,
                )]),
            },
            StatusRoots {
                origin: None,
                repository: None,
                authority_keyset: None,
                authority_policy: None,
            },
            StatusCounts::default(),
            StatusDecisionInbox {
                pending_count: 0,
                protocol_ready_count: 0,
                protocol_blocked_count: 0,
                projection_root: None,
                first_entry_root: None,
            },
            StatusActions {
                review: None,
                work: StatusWorkAction::AuthorityUninitialized {
                    command: format!("vela init {} --json", repository_path.display()),
                    note: "The retained repository profile has no repository authority yet. Resume `vela init`; nothing else can produce, verify, or decide until it completes.".into(),
                },
            },
        );
        if json_out {
            crate::cli::print_json(&payload);
        } else {
            println!("{}", payload.repository.name);
            if let Some(remote) = human_remote(&repository_path) {
                println!("{remote}");
            }
            println!();
            println!("  replay    not initialized");
            println!("  strict    blocked · repository authority uninitialized");
            println!("  next      {}", payload.actions.work.command());
        }
        return;
    }
    let repository = load_repository_at(&repository_path, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let repository_root = repository
        .canonical_root()
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let commit = git_text(&repository_path, &["rev-parse", "HEAD^{commit}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let tree = git_text(&repository_path, &["rev-parse", "HEAD^{tree}"])
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let standings = load_current_proposal_standings(&repository_path, &repository)
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
    let inbox_projection = crate::decision_inbox::project(&repository_path)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let (decision_inbox, pending_decision_count) = decision_inbox_status_summary(&inbox_projection);
    let review_action = (pending_decision_count > 0).then(|| StatusReviewAction {
        pending_count: pending_decision_count as u64,
        command: format!("vela review inbox {} --json", repository_path.display()),
    });
    let work_action = StatusWorkAction::DirectSubmission {
        command: format!("vela submit --repo {} --help", repository_path.display()),
        note: "Submit bounded evidence directly.".into(),
    };
    let payload = StatusV4::new(
        StatusRepository {
            id: repository.repository_id.clone(),
            name: profile.name,
            profile_root: repository.profile_root.clone(),
        },
        StatusGit {
            role: REPOSITORY_HEAD_ROLE.into(),
            commit: Some(commit),
            tree: Some(tree),
        },
        StatusIntegrity {
            /* The prose below no longer says "verified", which TERMINOLOGY.md
            forbids unqualified. This value keeps the word because it is a wire
            token of vela.status.v4: vela-web pins it as z.literal("verified")
            and its projection builder asserts on it, so retiring it is a
            coordinated schema change, not a wording change. */
            replay: ReplayState::Verified,
            strict: StrictState::Pass,
            blocker_count: 0,
            blockers_by_code: BTreeMap::new(),
        },
        StatusRoots {
            origin: Some(repository.origin_root.clone()),
            repository: Some(repository_root),
            authority_keyset: Some(repository.authority_keyset_root.clone()),
            authority_policy: Some(repository.authority_model_root.clone()),
        },
        StatusCounts {
            claims: (repository.accepted_claims.len() + repository.pending_claims.len()) as u64,
            accepted_claims: repository.accepted_claims.len() as u64,
            pending_claims: repository.pending_claims.len() as u64,
            pending_review: pending_review as u64,
            accepted_review: accepted_review as u64,
            rejected_review: rejected_review as u64,
            withdrawn_review: withdrawn_review as u64,
            submissions: repository.submissions.len() as u64,
            verifications: repository.verifications.len() as u64,
            artifacts: repository.artifacts.len() as u64,
        },
        decision_inbox,
        StatusActions {
            review: review_action,
            work: work_action,
        },
    );
    if json_out {
        crate::cli::print_json(&payload);
    } else {
        /* Human identity first, machine identity underneath.

        `repository_id` is what the protocol names a repository and what the
        trust store keys on, and it is the wrong thing to lead with: nobody
        says "open 01234567-89ab-4def-8123-456789abcdef", they say
        `vela-science/math`. This
        is the same split Git already draws between `main` and the commit it
        points at. The id has not moved and is not less important; it is one
        line down, where an identity, trust or debugging question finds it. */
        println!("{}", payload.repository.name);
        if let Some(remote) = human_remote(&repository_path) {
            println!("{remote}");
        }
        println!();
        println!("  repository {}", payload.repository.id);
        println!(
            "  state      {}",
            payload.roots.repository.as_deref().unwrap_or("unavailable")
        );
        println!(
            "  commit     {}",
            payload.git.commit.as_deref().unwrap_or("unavailable")
        );
        println!("  replay    matched · signatures, roots, canonical bytes");
        println!("  strict    pass");
        println!("  claims    {}", payload.counts.claims);
        println!(
            "  inbox     {} pending · {} protocol-ready · {} protocol-blocked",
            payload.decision_inbox.pending_count,
            payload.decision_inbox.protocol_ready_count,
            payload.decision_inbox.protocol_blocked_count
        );
        println!(
            "  inbox root {}",
            payload
                .decision_inbox
                .projection_root
                .as_deref()
                .unwrap_or("unavailable")
        );
        println!(
            "  first card {}",
            payload
                .decision_inbox
                .first_entry_root
                .as_deref()
                .unwrap_or("none")
        );
        if let Some(review) = &payload.actions.review {
            println!(
                "  review    {} pending · {}",
                review.pending_count, review.command
            );
        }
        println!("  work      {}", payload.actions.work.command());
    }
}

pub(crate) fn verify_profile_at(root: &Path) -> Result<RepositoryProfileV1, String> {
    let profile_source = fs::read_to_string(root.join("vela.toml"))
        .map_err(|error| format!("read vela.toml: {error}"))?;
    RepositoryProfileV1::from_toml_str(&profile_source)
}

pub(crate) fn verify_bootstrap_at(root: &Path) -> Result<RepositoryProfileV1, String> {
    let profile = verify_profile_at(root)?;
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
) -> Result<BTreeMap<String, ProposalDecision>, String> {
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
                    EventKind::ClaimAsserted
                        | EventKind::ClaimSuperseded
                        | EventKind::ClaimRetracted
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
        let decision = ProposalDecision {
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
    repository_path: &Path,
    repository: &RepositoryV4,
) -> Result<BTreeMap<String, ProposalDecision>, String> {
    let origin_bytes = fs::read(repository_path.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    let authority = crate::cli::load_repository_authority(repository_path, repository, &origin)?;
    current_proposal_decisions(&authority.history.authority_events)
}

pub(crate) fn load_current_proposal_standings(
    repository_path: &Path,
    repository: &RepositoryV4,
) -> Result<BTreeMap<String, String>, String> {
    let decisions = load_current_proposal_decisions(repository_path, repository)?;
    let mut standings = decisions
        .into_iter()
        .map(|(proposal_id, decision)| (proposal_id, decision.standing))
        .collect::<BTreeMap<_, _>>();
    for proposal_id in load_current_proposal_withdrawals(repository_path, repository)?.into_keys() {
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
    repository_path: &Path,
    repository: &RepositoryV4,
) -> Result<BTreeMap<String, ProposalWithdrawalEnvelopeV2>, String> {
    let mut withdrawals = BTreeMap::new();
    for reference in &repository.proposal_withdrawals {
        let bytes = read_rooted_object(repository_path, &reference.path, &reference.root)?;
        /* A withdrawal declares no key of its own: it is signed by whoever
        signed the Submission behind it, so the retained Submission has to be
        found before the signature can be checked at all. */
        let (declared_id, declared_root) =
            ProposalWithdrawalEnvelopeV2::declared_submission(&bytes)?;
        let submission_reference = repository
            .submissions
            .iter()
            .find(|candidate| candidate.id == declared_id && candidate.root == declared_root)
            .ok_or_else(|| {
                format!(
                    "Proposal Withdrawal {} does not bind one exact retained Submission",
                    reference.id
                )
            })?;
        let submission = SubmissionRecordV2::parse(&read_rooted_object(
            repository_path,
            &submission_reference.path,
            &submission_reference.root,
        )?)?;
        let withdrawal = ProposalWithdrawalEnvelopeV2::parse(&bytes, &submission)?;
        if withdrawal.id != reference.id || withdrawal.root != reference.root {
            return Err(format!(
                "current Proposal Withdrawal {} differs from its repository reference",
                reference.id
            ));
        }
        let proposal_reference = repository
            .proposals
            .iter()
            .find(|candidate| {
                candidate.id == withdrawal.withdrawal.proposal_id
                    && candidate.root == withdrawal.withdrawal.proposal_root
            })
            .ok_or_else(|| {
                format!(
                    "Proposal Withdrawal {} does not bind one exact retained Proposal",
                    withdrawal.id
                )
            })?;
        let proposal = ProposalV1::parse(&read_rooted_object(
            repository_path,
            &proposal_reference.path,
            &proposal_reference.root,
        )?)?;
        withdrawal.verify_with(&proposal, &submission)?;
        if withdrawals
            .insert(withdrawal.withdrawal.proposal_id.clone(), withdrawal)
            .is_some()
        {
            return Err(format!(
                "current Proposal {} has more than one Withdrawal",
                proposal.id()
            ));
        }
    }
    Ok(withdrawals)
}

fn validate_current_proposal_standing(
    root: &Path,
    repository: &RepositoryV4,
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
    /* Which Claims an accepted correction has already retired.

    Acceptance of a Claim carrying `corrects` or `supersedes` retires the
    predecessor, which leaves the accepted index — and the predecessor's own
    Proposal is still retained saying `accepted`. Without this set the two
    statements read as a contradiction and the repository stops loading: every
    read verb, `status` and `replay` included, fails on a repository that did
    nothing but accept a correction. That is what the first correction driven
    end to end through the CLI actually produced.

    The predecessor is identified from the successor's own retained Claim
    Record rather than from a manifest field, so this reads the same bytes the
    Decision acted on. `moves_standing` is the protocol's own test for which
    relation kinds acceptance acts on; a descriptive relation retires
    nothing. */
    let mut retired_by_correction = BTreeSet::new();
    for reference in &repository.proposals {
        let bytes = read_rooted_object(root, &reference.path, &reference.root)?;
        let proposal = ProposalV1::parse(&bytes)?;
        if standings.get(&proposal.id()).map(String::as_str) != Some("accepted") {
            continue;
        }
        let claim = rooted_claim_for_proposal(root, &proposal)?;
        for relation in &claim.relations {
            if relation.moves_standing() {
                retired_by_correction.insert(relation.target_claim_id.clone());
            }
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
        let decision = decisions.get(&proposal.id());
        let standing = standings
            .get(&proposal.id())
            .map(String::as_str)
            .unwrap_or("pending_review");
        let expected = match (proposal.action.as_str(), standing) {
            ("claim.add" | "claim.revise", "pending_review") => (true, false),
            /* An accepted Proposal whose Claim a later accepted correction has
            since retired is correctly in neither index. It is not pending —
            no ruling is owed — and it is not accepted, because the correction
            replaced it. */
            ("claim.add" | "claim.revise", "accepted")
                if retired_by_correction.contains(&proposal.subject.id) =>
            {
                (false, false)
            }
            ("claim.add" | "claim.revise", "accepted") => (false, true),
            ("claim.add" | "claim.revise", "rejected") => (false, false),
            ("claim.add" | "claim.revise", "withdrawn") => (false, false),
            ("claim.withdraw", "pending_review" | "rejected") => (false, true),
            ("claim.withdraw", "withdrawn") => (false, true),
            ("claim.withdraw", "accepted") => (false, false),
            (action, standing) => {
                return Err(format!(
                    "current Proposal {} has unsupported action/standing {action}/{standing}",
                    proposal.id()
                ));
            }
        };
        if (pending, accepted) != expected {
            return Err(format!(
                "current Proposal {} standing disagrees with the repository Claim indexes",
                proposal.id()
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
                != Some(proposal.id().as_str())
            || applied
                .content
                .payload
                .get("claim_id")
                .and_then(Value::as_str)
                != Some(proposal.subject.id.as_str())
        {
            return Err(format!(
                "current Proposal {} applied event has the wrong actor or object binding",
                proposal.id()
            ));
        }
        let transition_matches = match proposal.action.as_str() {
            "claim.add" => {
                applied.content.kind == EventKind::ClaimAsserted
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
                    && applied.content.kind == EventKind::ClaimSuperseded
                    && applied.content.target.id == predecessors[0].target_claim_id
                    && applied.content.before_hash != NULL_HASH
                    && applied.content.after_hash == proposal.subject.root
            }
            "claim.withdraw" => {
                applied.content.kind == EventKind::ClaimRetracted
                    && applied.content.target.id == proposal.subject.id
                    && applied.content.before_hash == proposal.subject.root
                    && applied.content.after_hash == NULL_HASH
            }
            _ => false,
        };
        if !transition_matches {
            return Err(format!(
                "current Proposal {} applied event does not match its exact transition",
                proposal.id()
            ));
        }
    }
    Ok(())
}

pub(crate) fn cmd_review_list(
    repository_path: &Path,
    status: Option<&str>,
    limit: usize,
    cursor: Option<&str>,
    json_out: bool,
) {
    crate::ui::set_mode("review list", json_out);
    crate::ui::require_initialized_repo(repository_path);
    let status = status.unwrap_or("pending_review");
    if !["pending_review", "accepted", "rejected", "withdrawn", "all"].contains(&status) {
        crate::cli::fail_kind(
            crate::ui::ErrorKind::Usage,
            "current review status must be pending_review, accepted, rejected, withdrawn, or all",
        );
    }
    let repository_path = crate::ui::canonicalize_repo(repository_path);
    let repository = load_repository_at(&repository_path, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let decisions = load_current_proposal_decisions(&repository_path, &repository)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let withdrawals = load_current_proposal_withdrawals(&repository_path, &repository)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let mut items = repository
        .proposals
        .iter()
        .filter_map(|reference| {
            let bytes = read_rooted_object(&repository_path, &reference.path, &reference.root)
                .unwrap_or_else(|error| crate::cli::fail_return(&error));
            let proposal =
                ProposalV1::parse(&bytes).unwrap_or_else(|error| crate::cli::fail_return(&error));
            let decision = decisions.get(&proposal.id());
            let withdrawal = withdrawals.get(&proposal.id());
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
                    "proposal_id": proposal.id(),
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
                    "withdrawal": withdrawal.map(|value| &value.withdrawal)
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
        "repository_id": repository.repository_id,
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

pub(crate) fn cmd_review_show(repository_path: &Path, proposal_id: &str, json_out: bool) {
    crate::ui::set_mode("review show", json_out);
    crate::ui::require_initialized_repo(repository_path);
    let repository_path = crate::ui::canonicalize_repo(repository_path);
    let repository = load_repository_at(&repository_path, true)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let decisions = load_current_proposal_decisions(&repository_path, &repository)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let withdrawals = load_current_proposal_withdrawals(&repository_path, &repository)
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
    let proposal_bytes = read_rooted_object(&repository_path, &reference.path, &reference.root)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let proposal =
        ProposalV1::parse(&proposal_bytes).unwrap_or_else(|error| crate::cli::fail_return(&error));
    /* `pending_review`, `accepted`, `rejected` and `withdrawn` are the Proposal
    axis, which TERMINOLOGY.md keeps apart from Claim standing. `review list`
    already carries this value on each row as `status`, and `--status` filters
    it by that name; this view called the same value `standing` and was the last
    place in the CLI where a Proposal word travelled under the Claim word. */
    let status = decisions.get(proposal_id).map_or_else(
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
        crate::submission::rooted_path("records/claims/sha256", &proposal.subject.root)
            .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let claim_bytes = read_rooted_object(&repository_path, &claim_path, &proposal.subject.root)
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
        &repository_path,
        &submission_reference.path,
        &submission_reference.root,
    )
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let submission = vela_protocol::submission::SubmissionRecordV2::parse(&submission_bytes)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let verifications = repository
        .verifications
        .iter()
        .filter_map(|verification| {
            let bytes =
                read_rooted_object(&repository_path, &verification.path, &verification.root)
                    .unwrap_or_else(|error| crate::cli::fail_return(&error));
            let record = VerificationRecordEnvelopeV2::parse(&bytes)
                .unwrap_or_else(|error| crate::cli::fail_return(&error));
            verification_targets_proposal(&proposal, &claim, &record).then_some(json!({
                /* The handle belongs beside the root it comes from. The payload
                used to carry a `verification_record_id` and no longer does —
                it is derived from the retained envelope root, so the object
                that stores the root is the object that can state it. A reader
                that only had `record` had no way to name what it was reading. */
                "verification_record_id": record.id,
                "verification_record_root": verification.root,
                "record": record.record
            }))
        })
        .collect::<Vec<_>>();
    let decision = decisions.get(proposal_id);
    let withdrawal = withdrawals.get(proposal_id);
    let decision_inbox = crate::decision_inbox::review_context(&repository_path, proposal_id)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let payload = json!({
        "schema": "vela.review.v1",
        "ok": true,
        "command": "review.show",
        "repository_id": repository.repository_id,
        "repository_root": repository.canonical_root().unwrap_or_else(|error| crate::cli::fail_return(&error)),
        "proposal_id": proposal.id(),
        "proposal_root": reference.root,
        "status": status,
        "proposal": proposal,
        "claim": claim,
        "submission": submission.submission,
        "verification_records": verifications,
        "decision": decision,
        "withdrawal": withdrawal.map(|value| &value.withdrawal),
        "decision_inbox": decision_inbox,
        "authority_boundary": "Verification records report bounded checks. A producer may close its own pending Proposal; only a repository-authority Decision can change accepted scientific Standing.",
    });
    if json_out {
        crate::cli::print_json(&payload);
    } else {
        println!("review · {proposal_id} · {status}");
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
            println!("  decided: {status} by {actor} at {at}");
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
    record: &VerificationRecordEnvelopeV2,
) -> bool {
    if claim.claim_id != proposal.subject.id {
        return false;
    }
    record.record.subject.proposal_id == proposal.id()
        && record.record.subject.claim_id == proposal.subject.id
        && record.record.subject.submission_id == proposal.producer_package.id
        && record.record.subject.submission_root == proposal.producer_package.root
}

fn rooted_claim_for_proposal(root: &Path, proposal: &ProposalV1) -> Result<ClaimRecordV1, String> {
    let claim_path =
        crate::submission::rooted_path("records/claims/sha256", &proposal.subject.root)?;
    let claim_bytes = read_rooted_object(root, &claim_path, &proposal.subject.root)?;
    let claim = ClaimRecordV1::parse(&claim_bytes)?;
    if claim.canonical_bytes()? != claim_bytes || claim.claim_id != proposal.subject.id {
        return Err(format!(
            "current Proposal {} has the wrong canonical Claim bytes",
            proposal.id()
        ));
    }
    Ok(claim)
}

fn verification_targets_rooted_proposal(
    root: &Path,
    proposal: &ProposalV1,
    record: &VerificationRecordEnvelopeV2,
) -> Result<bool, String> {
    let claim = rooted_claim_for_proposal(root, proposal)?;
    Ok(verification_targets_proposal(proposal, &claim, record))
}

fn proposal_matches_signed_submission(
    proposal: &ProposalV1,
    claim: &ClaimRecordV1,
    submission: &SubmissionRecordV2,
) -> Result<(), String> {
    if proposal.actor != submission.submission.provenance.producer
        || proposal.caveats != submission.submission.caveats
    {
        return Err("Proposal actor or caveats disagree with its signed Submission".into());
    }

    let expected_action = match submission.submission.requested_change.kind.as_str() {
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
            .submission
            .requested_change
            .target
            .as_ref()
            .ok_or_else(|| "withdrawal Submission has no exact Claim target".to_string())?;
        if proposal.subject.id != target.claim_id || proposal.subject.root != target.claim_root {
            return Err("withdrawal Proposal does not bind its signed Submission target".into());
        }
        return Ok(());
    }

    if claim.assertion.text != submission.submission.claim.assertion
        || claim.assertion.kind != submission.submission.claim.claim_type
        || claim.created_at != submission.submission.provenance.emitted_at
        || !claim.extensions.is_empty()
    {
        return Err("Proposal Claim body disagrees with its signed Submission".into());
    }

    let mut expected_conditions = submission.submission.claim.conditions.clone();
    expected_conditions.extend(
        submission
            .submission
            .caveats
            .iter()
            .map(|caveat| format!("Caveat: {caveat}")),
    );
    if claim.conditions != expected_conditions {
        return Err("Proposal Claim conditions disagree with its signed Submission".into());
    }

    let expected_evidence = submission
        .submission
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

    let relation_matches = match submission.submission.requested_change.kind.as_str() {
        "add_claim" => claim.revision == 1 && claim.relations.is_empty(),
        "correct_claim" | "supersede_claim" => {
            let target = submission.submission.requested_change.target.as_ref();
            claim.revision > 1
                && claim.relations.len() == 1
                && target.is_some_and(|target| {
                    claim.relations[0].target_claim_id == target.claim_id
                        && claim.relations[0].kind
                            == if submission.submission.requested_change.kind == "correct_claim" {
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
        || claim.provenance[0].title != format!("Authenticated Submission {}", submission.id)
        || claim.provenance[0].authors != [submission.submission.provenance.producer.clone()]
    {
        return Err("Proposal Claim provenance disagrees with its signed Submission".into());
    }
    Ok(())
}

/// Load and validate the current repository identity and authority chain.
pub(crate) fn load_repository_at(
    root: &Path,
    require_authority_record: bool,
) -> Result<RepositoryV4, String> {
    let profile_source = fs::read_to_string(root.join("vela.toml"))
        .map_err(|error| format!("read vela.toml: {error}"))?;
    let profile = RepositoryProfileV1::from_toml_str(&profile_source)?;
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
    let repository = RepositoryV4::parse(&repository_bytes)?;
    if repository.repository_id != profile.repository_id
        || repository.repository_id != origin.repository_id
        || repository.profile_root != profile_root
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin.id()?
        || repository.origin_root != origin_root
    {
        return Err(
            "current Profile, repository manifest, and origin do not bind the same identity".into(),
        );
    }
    if require_authority_record {
        let loaded = crate::cli::load_repository_authority(root, &repository, &origin)?;
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

pub(crate) fn verify_repository_at(
    root: &Path,
    require_authority_record: bool,
) -> Result<RepositoryV4, String> {
    let repository = load_repository_at(root, false)?;
    let origin_bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    let object_bytes = read_object_set(root, &repository)?;
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
            || proposal.id() != reference.id
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
            SubmissionRecordV2::parse(object_bytes.get(&submission_reference.path).ok_or_else(
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
        if let Some(previous) =
            proposal_by_submission.insert(proposal.producer_package.id.clone(), proposal.id())
        {
            return Err(format!(
                "Submission {} is retained by multiple Proposals: {previous} and {}",
                proposal.producer_package.id,
                proposal.id()
            ));
        }
    }
    for reference in &repository.submissions {
        let bytes = object_bytes
            .get(&reference.path)
            .ok_or_else(|| format!("current object {} was not loaded", reference.path))?;
        let submission = SubmissionRecordV2::parse(bytes)?;
        if submission.bytes.clone().as_slice() != bytes.as_slice() || submission.id != reference.id
        {
            return Err(format!(
                "{} does not contain the declared canonical Submission",
                reference.path
            ));
        }
        if !proposal_by_submission.contains_key(&submission.id) {
            return Err(format!("{} has no exact retained Proposal", reference.path));
        }
    }
    for reference in &repository.verifications {
        let bytes = object_bytes
            .get(&reference.path)
            .ok_or_else(|| format!("current object {} was not loaded", reference.path))?;
        let verification = VerificationRecordEnvelopeV2::parse(bytes)?;
        if verification.bytes.as_slice() != bytes.as_slice() || verification.id != reference.id {
            return Err(format!(
                "{} does not contain the declared canonical Verification Record",
                reference.path
            ));
        }
        let proposal_reference = repository
            .proposals
            .iter()
            .find(|candidate| candidate.id == verification.record.subject.proposal_id)
            .ok_or_else(|| {
                format!(
                    "{} targets Proposal {} outside the current repository",
                    reference.path, verification.record.subject.proposal_id
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
            .find(|candidate| candidate.id == verification.record.subject.submission_id)
            .ok_or_else(|| {
                format!(
                    "{} targets Submission {} outside the current repository",
                    reference.path, verification.record.subject.submission_id
                )
            })?;
        if submission_reference.root != verification.record.subject.submission_root
            || submission_reference.path != proposal.producer_package.path
        {
            return Err(format!(
                "{} does not bind the current Submission reference",
                reference.path
            ));
        }
        for artifact_id in verification
            .record
            .subject
            .artifact_ids
            .iter()
            .chain(&verification.record.output_artifact_ids)
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
        if is_retired_path(&relative) {
            return Err(format!(
                "current repository retains retired protocol path {relative}"
            ));
        }
    }
    if require_authority_record {
        verify_repository_authority(root, &repository, &origin)?;
    }
    Ok(repository)
}

fn read_object_set(
    root: &Path,
    repository: &RepositoryV4,
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
pub(crate) fn verify_repository_allow_derived_drift_at(
    root: &Path,
) -> Result<RepositoryV4, String> {
    let repository = verify_repository_at(root, false)?;
    let origin_bytes = fs::read(root.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    let origin = RepositoryOriginV1::parse(&origin_bytes)?;
    verify_repository_authority(root, &repository, &origin)?;
    Ok(repository)
}

/// The repository manifest exactly as the repository's origin commit retains it.
///
/// This is the boundary `vela claims` reads a Claim's origin era from: a Claim
/// the origin manifest already bound came through the last compaction, and
/// everything else was admitted by the current authority chain since.
pub(crate) fn initial_repository(
    root: &Path,
    origin: &RepositoryOriginV1,
) -> Result<RepositoryV4, String> {
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
    let repository = RepositoryV4::parse(&repository_bytes)?;
    if repository.repository_id != origin.repository_id
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin.id()?
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

/// Read one Claim's retained bytes and hold them to the reference that named
/// them.
///
/// `read_rooted_object` proves the bytes match the declared root. This proves
/// the record inside them is the Claim the manifest said it was, which the root
/// alone does not: a manifest entry can bind a correct root under the wrong id.
pub(crate) fn read_claim(
    repository_path: &Path,
    reference: &ClaimStandingRefV1,
) -> Result<ClaimRecordV1, String> {
    let bytes = read_rooted_object(repository_path, &reference.path, &reference.claim_root)?;
    let claim = ClaimRecordV1::parse(&bytes)?;
    if claim.claim_id != reference.claim_id {
        return Err(format!(
            "retained bytes at {} carry Claim id {}, not the one the manifest binds",
            reference.path, claim.claim_id
        ));
    }
    Ok(claim)
}

pub(crate) fn read_rooted_object(
    root: &Path,
    path: &str,
    expected_root: &str,
) -> Result<Vec<u8>, String> {
    let bytes =
        fs::read(root.join(path)).map_err(|error| format!("read object {path}: {error}"))?;
    if root_bytes(&bytes) != expected_root {
        return Err(format!("object {path} does not match its declared root"));
    }
    let expected_name = expected_root.trim_start_matches("sha256:");
    if Path::new(path).file_stem().and_then(|value| value.to_str()) != Some(expected_name) {
        return Err(format!(
            "current object {path} filename does not match its declared root"
        ));
    }
    Ok(bytes)
}

fn verify_repository_authority(
    root: &Path,
    repository: &RepositoryV4,
    origin: &RepositoryOriginV1,
) -> Result<(), String> {
    /* Genesis is the only origin, so the initial roots are the empty ones.
    These read a predecessor's archived roots when one existed. */
    let initial_event_log_root = format!("sha256:{}", vela_protocol::events::event_log_hash(&[]));
    let initial_actor_registry_root = format!("sha256:{}", hex::encode(Sha256::digest([])));
    let loaded = crate::cli::load_repository_authority(root, repository, origin)?;
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
    if initialization.repository_id != repository.repository_id
        || initialization.initial_event_log_root != initial_event_log_root
        || initialization.initial_actor_registry_root != initial_actor_registry_root
        || initialization.new_authority_keyset_root != repository.authority_keyset_root
        || initialization.new_authorization_model_root != repository.authority_model_root
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
        || first.content.authorization.model_root != initialization.new_authorization_model_root
    {
        return Err("current authority record does not bind its exact event and origin".into());
    }
    let expected_after = vela_protocol::authority_history::authority_event_log_root(
        &initial_event_log_root,
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
        let path = crate::submission::rooted_path("records/claims/sha256", &proposal.subject.root)?;
        if let Some(previous) =
            current_record_paths.insert(path.clone(), proposal.subject.root.clone())
            && previous != proposal.subject.root
        {
            return Err(format!(
                "current Proposal Claim reference disagrees with retained bytes at {path}"
            ));
        }
    }
    verify_record_coverage(
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

fn repository_manifest_at_commit(root: &Path, commit: &str) -> Result<RepositoryV4, String> {
    let spec = format!("{commit}:.vela/repository.json");
    let output = vela_edge::git::output(root, &["show", &spec])?;
    if !output.status.success() {
        return Err(format!(
            "read repository manifest at {commit}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    RepositoryV4::parse(&output.stdout)
}

/// Replay unsigned evidence-manifest commits from the last exact signed
/// repository checkpoint. Between authority checkpoints the record store may
/// only add immutable evidence; a Decision creates the next signed checkpoint.
fn verify_routine_evidence_ancestry(
    root: &Path,
    signed_transitions: &[SignedRepositoryManifestTransition],
    current: &RepositoryV4,
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
    authority_checkpoint: &RepositoryV4,
    current: &RepositoryV4,
) -> Result<(), String> {
    authority_checkpoint.verify()?;
    current.verify()?;

    if authority_checkpoint.repository_id != current.repository_id
        || authority_checkpoint.profile_root != current.profile_root
        || authority_checkpoint.origin_id != current.origin_id
        || authority_checkpoint.origin_root != current.origin_root
    {
        return Err("routine evidence changes repository identity".into());
    }
    if authority_checkpoint.authority_keyset_root != current.authority_keyset_root
        || authority_checkpoint.authority_model_root != current.authority_model_root
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
pub(crate) fn verify_record_coverage(
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

/// The origin remote as a person refers to it: `vela-science/math`.
///
/// A locator, deliberately — `docs/CONTINUITY.md` §2 is explicit that a URL
/// says where to fetch bytes and never whose bytes they are. It leads the
/// status surface because it is how a reader navigates, not because it
/// identifies anything; a repository reached through a mirror prints that
/// mirror and is the same repository, which is the point being made.
///
/// Returns `None` rather than a placeholder when there is no origin: a
/// repository read out of a bundle or a bare path has no locator to show, and
/// an empty line is a truer answer than an invented one.
fn human_remote(repository: &Path) -> Option<String> {
    let remote = git_text(repository, &["remote", "get-url", "origin"]).ok()?;
    let remote = remote.trim();
    if remote.is_empty() {
        return None;
    }
    let trimmed = remote
        .strip_prefix("https://")
        .or_else(|| remote.strip_prefix("http://"))
        .or_else(|| remote.strip_prefix("ssh://git@"))
        .or_else(|| remote.strip_prefix("git@"))
        .unwrap_or(remote)
        .replacen(':', "/", 1);
    Some(trimmed.strip_suffix(".git").unwrap_or(&trimmed).to_string())
}

fn git_text(repository: &Path, args: &[&str]) -> Result<String, String> {
    vela_edge::git::text(repository, args)
}

/// The set of paths a current repository must no longer carry.
///
/// This is the verifier's own answer, and it is the only one. The authority
/// writer asks it before admitting an object draft, so a repository that
/// replay would refuse cannot be written in the first place.
pub(crate) fn is_retired_path(path: &str) -> bool {
    path == ".vela/actors.json"
        || path == "frontier.yaml"
        || path == "frontier.json"
        // `vela.lock` and `proof/` were a pre-v2 repository's dependency lock and
        // its loose proof directory. Both were documented as retired long
        // before they were refused: two repositories still carried dead
        // `.gitattributes` rules naming them, and refusing a path while a rule
        // for it was in the tree would have made a failure ambiguous. Those
        // rules are gone and no published repository names either path, so the
        // refusal is now the same one every other retired path gets.
        || path == "vela.lock"
        || path.starts_with("proof/")
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

/// Walk the repository's own content. Git's storage is not content.
///
/// `.git` is skipped, and the reason is a failure rather than tidiness. This
/// walk collects directories and reads them afterwards, so anything that
/// rewrites the tree underneath it produces a read of a path that no longer
/// exists. Git does exactly that on its own schedule: an auto-`gc` packs loose
/// objects and removes the two-hex directories that held them, and a repository
/// verified moments after a commit hits it. In CI that surfaced as
///
/// ```text
/// err · read ./.git/objects/c0: No such file or directory (os error 2)
/// ```
///
/// on one run in eight, from `vela authority trust pin`, with nothing wrong
/// with the repository at all.
///
/// The caller is checking for retired protocol paths — `proof/`, `vela.lock`,
/// `.vela/events/` — none of which can live inside `.git`. So the walk was
/// reading tens of thousands of object files to answer a question about none of
/// them, and racing Git to do it. A nested `.git` is skipped too: a vendored or
/// submodule checkout is another repository's storage, not this one's content.
///
/// Everything outside `.git` stays strict. A directory that disappears from
/// `records/` mid-walk is a concurrent write to retained state, and that is
/// worth failing on.
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
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
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
    use vela_protocol::proposal::{ProposalProducerPackage, ProposalSubject};
    use vela_protocol::signer_identity::{ActorClass, SignerIdentityV1};
    use vela_protocol::submission::{
        RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    };
    use vela_protocol::verification_record::{
        IndependenceDisclosure, VerificationMethod, VerificationRecordDraft, VerificationScope,
        VerificationSubject,
    };

    use super::*;

    /* The four spellings a forge actually hands out, plus the case that has no
    answer. `human_remote` leads the status surface, so a remote it mangles
    is the first line a reader sees. */
    #[test]
    fn origin_renders_as_a_person_refers_to_it() {
        let repository = tempfile::tempdir().expect("staging");
        let path = repository.path();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(path)
                .output()
                .expect("run git");
        };
        git(&["init", "--quiet", "."]);
        assert_eq!(human_remote(path), None, "no origin has no locator to show");

        for spelling in [
            "https://github.com/vela-science/math.git",
            "https://github.com/vela-science/math",
            "git@github.com:vela-science/math.git",
            "ssh://git@github.com/vela-science/math.git",
        ] {
            git(&["remote", "remove", "origin"]);
            git(&["remote", "add", "origin", spelling]);
            assert_eq!(
                human_remote(path).as_deref(),
                Some("github.com/vela-science/math"),
                "{spelling}"
            );
        }

        /* A mirror prints as itself. The locator is where the bytes were
        fetched from and never whose they are, which is the whole reason it
        is safe to lead with. */
        git(&["remote", "remove", "origin"]);
        git(&[
            "remote",
            "add",
            "origin",
            "https://codeberg.org/vela-science/math.git",
        ]);
        assert_eq!(
            human_remote(path).as_deref(),
            Some("codeberg.org/vela-science/math")
        );
    }

    #[test]
    fn current_repository_rejects_retired_profile_paths() {
        assert!(is_retired_path("frontier.yaml"));
        assert!(is_retired_path("frontier.json"));
        assert!(!is_retired_path("vela.toml"));
        assert!(is_retired_path("vela.lock"));
        assert!(is_retired_path("proof/erdos-203.lean"));
        // The prefix is the directory, not the word: a repository is free to
        // keep proofs anywhere it does not claim this exact retired layout.
        assert!(!is_retired_path("artifacts/proof-scripts/sidon.lean"));
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

    fn signed_submission_and_claim() -> (SubmissionRecordV2, ClaimRecordV1) {
        let key = SigningKey::from_bytes(&[71_u8; 32]);
        let producer = "agent:proposal-binding-fixture";
        let emitted_at = "2026-07-27T00:00:00Z";
        let identity =
            SignerIdentityV1::new(producer, ActorClass::Agent, &key, emitted_at).unwrap();
        let submission = SubmissionRecordV2::seal(
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
                text: submission.submission.claim.assertion.clone(),
                kind: submission.submission.claim.claim_type.clone(),
            },
            vec![
                submission.submission.claim.conditions[0].clone(),
                format!("Caveat: {}", submission.submission.caveats[0]),
            ],
            vec![vela_protocol::claim_record::ClaimEvidenceRef {
                relation: "supports".into(),
                artifact_id: None,
                artifact_root: submission.submission.artifacts[0].digest.clone(),
                artifact_path: Some(format!(
                    "records/artifacts/sha256/{}",
                    submission.submission.artifacts[0]
                        .digest
                        .strip_prefix("sha256:")
                        .unwrap()
                )),
            }],
            vec![vela_protocol::claim_record::ClaimSource {
                kind: "submission".into(),
                title: format!("Authenticated Submission {}", submission.id),
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
            submission.submission.provenance.producer.clone(),
            "2026-07-27T00:00:01Z".into(),
            "Review the exact signed Submission.".into(),
            ProposalProducerPackage {
                kind: "submission".into(),
                id: submission.id.clone(),
                root: submission.root.clone(),
                path: format!(
                    "records/submissions/sha256/{}.json",
                    submission.root.strip_prefix("sha256:").unwrap()
                ),
            },
            submission.submission.caveats.clone(),
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

    fn current_review_lineage() -> (ProposalV1, ClaimRecordV1, VerificationRecordEnvelopeV2) {
        let submission_root = root('2');
        let submission_id = vela_protocol::derive_handle("vsb_", &submission_root).unwrap();
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
                kind: "submission".into(),
                id: submission_id.clone(),
                root: submission_root.clone(),
                path: "records/submissions/sha256/fixture.json".into(),
            },
            vec!["The bounded result is not universal.".into()],
        )
        .unwrap();
        let key = SigningKey::from_bytes(&[73_u8; 32]);
        let verifier = "verifier:fixture";
        let identity =
            SignerIdentityV1::new(verifier, ActorClass::Org, &key, "2026-07-27T00:00:02Z").unwrap();
        let verification = VerificationRecordEnvelopeV2::seal(
            VerificationRecordDraft {
                subject: VerificationSubject {
                    claim_id: claim.claim_id.clone(),
                    artifact_ids: vec!["a".repeat(64)],
                    submission_id,
                    submission_root,
                    proposal_id: proposal.id(),
                    proposal_root: proposal.canonical_root().unwrap(),
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
            kind: EventKind::ClaimAsserted,
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

    fn repository_fixture() -> RepositoryV4 {
        RepositoryV4 {
            schema: vela_protocol::repository::REPOSITORY_SCHEMA_V4.into(),
            repository_id: "01234567-89ab-4def-8123-456789abcdef".into(),
            profile_root: root('1'),
            origin_id: vela_protocol::derive_handle("vro_", &root('2')).unwrap(),
            origin_root: root('2'),
            accepted_claims: Vec::new(),
            pending_claims: Vec::new(),
            proposals: Vec::new(),
            proposal_withdrawals: Vec::new(),
            submissions: Vec::new(),
            verifications: Vec::new(),
            artifacts: Vec::new(),
            authority_keyset_root: root('3'),
            authority_model_root: root('4'),
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
        verify_record_coverage(&authority, &evidence, &observed).unwrap();

        let mut unexplained = observed.clone();
        unexplained.insert("records/artifacts/sha256/c".into(), root('c'));
        assert_eq!(
            verify_record_coverage(&authority, &evidence, &unexplained).unwrap_err(),
            "current repository contains unexplained record records/artifacts/sha256/c"
        );

        let mut missing = observed.clone();
        missing.remove("records/claims/sha256/a.json");
        assert_eq!(
            verify_record_coverage(&authority, &evidence, &missing).unwrap_err(),
            "current repository is missing retained record records/claims/sha256/a.json"
        );

        let conflicting = BTreeMap::from([("records/claims/sha256/a.json".into(), root('d'))]);
        assert_eq!(
            verify_record_coverage(&authority, &conflicting, &observed).unwrap_err(),
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

        let mut wrong_submission_root = verification.clone();
        wrong_submission_root.record.subject.submission_root = root('9');
        assert!(!verification_targets_proposal(
            &proposal,
            &claim,
            &wrong_submission_root
        ));

        let mut wrong_proposal = verification.clone();
        wrong_proposal.record.subject.proposal_id = "vpr_0000000000000000".into();
        assert!(!verification_targets_proposal(
            &proposal,
            &claim,
            &wrong_proposal
        ));

        let mut wrong_claim = verification.clone();
        wrong_claim.record.subject.claim_id = "vf_0000000000000000".into();
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
            crate::submission::rooted_path("records/claims/sha256", &claim_root).unwrap();
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
