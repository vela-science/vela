//! Proposal-first frontier writes and proof freshness tracking.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::bundle::{Annotation, Artifact, ConfidenceMethod, FindingBundle};
use crate::canonical;
use crate::events::{self, NULL_HASH, StateActor, StateEvent, StateTarget};
use crate::project::{self, Project};
use crate::propagate::{self, PropagationAction};
use crate::repo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalWithdrawalAuthorization {
    pub proposal_root: String,
    pub producer_object_root: String,
    pub producer_object_path: String,
    pub payload_schema: String,
    pub identity_binding: crate::identity::IdentityBinding,
}

/// Resolve and verify the producer identity bound by a current Submission or
/// historical Receipt. This grants only the ability to withdraw that exact
/// still-pending Proposal; it is never reviewer or accepted-state authority.
pub fn proposal_withdrawal_authorization(
    frontier: &Path,
    proposal: &StateProposal,
) -> Result<ProposalWithdrawalAuthorization, String> {
    if proposal.status != "pending_review" {
        return Err(format!(
            "proposal {} is {}, not pending_review",
            proposal.id, proposal.status
        ));
    }
    let proposal_root = format!(
        "sha256:{}",
        canonical::sha256_canonical(proposal)
            .map_err(|error| format!("canonicalize proposal: {error}"))?
    );
    if let Some(submission) = proposal
        .payload
        .get("submission")
        .and_then(Value::as_object)
    {
        validate_current_submission_links(&Value::Object(submission.clone()))?;
        let submission_root = submission
            .get("submission_root")
            .and_then(Value::as_str)
            .ok_or("submission.submission_root must be a string")?
            .to_string();
        let submission_path = submission
            .get("submission_path")
            .and_then(Value::as_str)
            .ok_or("submission.submission_path must be a string")?
            .to_string();
        let relative = Path::new(&submission_path);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            return Err("Proposal Submission path must remain Frontier-relative".to_string());
        }
        let bytes = std::fs::read(frontier.join(relative))
            .map_err(|error| format!("read bound Submission {submission_path}: {error}"))?;
        let current = crate::submission_v1::SubmissionV1::parse(&bytes)
            .map_err(|error| format!("verify bound Submission {submission_path}: {error}"))?;
        current.verify()?;
        let observed_root = current.canonical_root()?;
        if observed_root != submission_root {
            return Err(format!(
                "bound Submission root mismatch: Proposal declares {submission_root}, observed {observed_root}"
            ));
        }
        let identity_binding = current.authentication.identity_binding.clone();
        if identity_binding.actor_id != proposal.actor.id
            || identity_binding.actor_class != crate::identity::ActorClass::Agent
        {
            return Err(
                "Proposal actor does not match its Submission producer identity".to_string(),
            );
        }
        return Ok(ProposalWithdrawalAuthorization {
            proposal_root,
            producer_object_root: submission_root,
            producer_object_path: submission_path,
            payload_schema: events::PROPOSAL_WITHDRAWAL_PAYLOAD_SCHEMA.to_string(),
            identity_binding,
        });
    }

    let submission = proposal
        .payload
        .get("vela_submission")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "Proposal {} has no producer-bound Submission and cannot be withdrawn",
                proposal.id
            )
        })?;
    let receipt_root = submission
        .get("receipt_root")
        .and_then(Value::as_str)
        .ok_or("vela_submission.receipt_root must be a string")?
        .to_string();
    let receipt_path = submission
        .get("receipt_path")
        .and_then(Value::as_str)
        .ok_or("vela_submission.receipt_path must be a string")?
        .to_string();
    let relative = Path::new(&receipt_path);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err("proposal receipt path must remain frontier-relative".to_string());
    }
    let bytes = std::fs::read(frontier.join(relative))
        .map_err(|error| format!("read bound Receipt {receipt_path}: {error}"))?;
    let receipt = crate::receipt_v1::ReceiptV1::parse(&bytes)
        .map_err(|error| format!("verify bound Receipt {receipt_path}: {error}"))?;
    let observed_root = receipt
        .canonical_root()
        .map_err(|error| format!("canonicalize bound Receipt: {error}"))?;
    if observed_root != receipt_root {
        return Err(format!(
            "bound Receipt root mismatch: proposal declares {receipt_root}, observed {observed_root}"
        ));
    }
    let binding_value = receipt
        .as_value()
        .pointer("/environment/vela:producer_context/identity_binding")
        .cloned()
        .ok_or("bound Receipt has no embedded producer identity binding")?;
    let identity_binding: crate::identity::IdentityBinding = serde_json::from_value(binding_value)
        .map_err(|error| format!("decode embedded producer identity binding: {error}"))?;
    identity_binding
        .verify()
        .map_err(|error| format!("verify embedded producer identity binding: {error}"))?;
    if identity_binding.actor_id != proposal.actor.id
        || identity_binding.actor_class != crate::identity::ActorClass::Agent
    {
        return Err("proposal actor does not match its Receipt producer identity".to_string());
    }
    Ok(ProposalWithdrawalAuthorization {
        proposal_root,
        producer_object_root: receipt_root,
        producer_object_path: receipt_path,
        payload_schema: events::PROPOSAL_WITHDRAWAL_PAYLOAD_SCHEMA_V1.to_string(),
        identity_binding,
    })
}

/// Verify, sign, append, and materialize one producer withdrawal in memory.
/// Filesystem installation remains the CLI transaction layer's responsibility.
pub fn apply_proposal_withdrawal(
    frontier: &Path,
    project: &mut Project,
    proposal_id: &str,
    actor_id: &str,
    reason: &str,
    timestamp: &str,
    key: &ed25519_dalek::SigningKey,
) -> Result<StateEvent, String> {
    let index = project
        .proposals
        .iter()
        .position(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("proposal {proposal_id} does not exist"))?;
    let authorization = proposal_withdrawal_authorization(frontier, &project.proposals[index])?;
    if actor_id != project.proposals[index].actor.id
        || actor_id != authorization.identity_binding.actor_id
    {
        return Err("withdrawal actor is not the Submission-bound Proposal producer".to_string());
    }
    let actual_key = crate::sign::pubkey_hex(key);
    if !actual_key.eq_ignore_ascii_case(&authorization.identity_binding.public_key_hex) {
        return Err("withdrawal key does not match the Submission-bound producer key".to_string());
    }
    let payload = events::ProposalWithdrawalPayload {
        schema: authorization.payload_schema.clone(),
        proposal_id: proposal_id.to_string(),
        proposal_root: authorization.proposal_root,
        submission_root: (authorization.payload_schema
            == events::PROPOSAL_WITHDRAWAL_PAYLOAD_SCHEMA)
            .then_some(authorization.producer_object_root.clone()),
        receipt_root: (authorization.payload_schema
            == events::PROPOSAL_WITHDRAWAL_PAYLOAD_SCHEMA_V1)
            .then_some(authorization.producer_object_root),
        identity_binding_id: authorization.identity_binding.binding_id,
    };
    let mut event =
        events::new_proposal_withdrawal_event(payload, actor_id, reason, Some(timestamp))?;
    event.signature = Some(crate::sign::sign_event(&event, key)?);
    project.events.push(event.clone());
    let proposal = &mut project.proposals[index];
    proposal.status = "withdrawn".to_string();
    proposal.decision_reason = Some(reason.to_string());
    proposal.reviewed_by = None;
    proposal.reviewed_at = None;
    proposal.applied_event_id = None;
    // Stats are part of the materialized snapshot. Keep the in-memory
    // candidate identical to a subsequent split-repository reload, which
    // always recomputes them after reading the canonical event files.
    project::recompute_stats(project);
    Ok(event)
}

/// Verify every stored withdrawal against its exact producer-object binding.
pub fn verify_proposal_withdrawals(frontier: &Path, project: &Project) -> Vec<String> {
    project
        .events
        .iter()
        .filter(|event| event.kind == events::EVENT_KIND_PROPOSAL_WITHDRAWN)
        .filter_map(|event| verify_proposal_withdrawal_event(frontier, project, event).err())
        .collect()
}

pub fn verify_proposal_withdrawal_event(
    frontier: &Path,
    project: &Project,
    event: &StateEvent,
) -> Result<(), String> {
    if event.kind != events::EVENT_KIND_PROPOSAL_WITHDRAWN {
        return Err(format!("event {} is not a proposal withdrawal", event.id));
    }
    let proposal = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == event.target.id)
        .ok_or_else(|| {
            format!(
                "withdrawal event {} targets missing proposal {}",
                event.id, event.target.id
            )
        })?;
    let authorization = proposal_withdrawal_authorization_for_terminal(frontier, proposal)
        .map_err(|error| format!("withdrawal event {}: {error}", event.id))?;
    let matching_withdrawals = project
        .events
        .iter()
        .filter(|candidate| {
            candidate.kind == events::EVENT_KIND_PROPOSAL_WITHDRAWN
                && candidate.target.r#type == "proposal"
                && candidate.target.id == proposal.id
        })
        .count();
    if matching_withdrawals != 1 {
        return Err(format!(
            "withdrawal event {}: proposal {} must have exactly one withdrawal event, found {matching_withdrawals}",
            event.id, proposal.id
        ));
    }
    if project.events.iter().any(|candidate| {
        candidate.target.r#type == "proposal"
            && candidate.target.id == proposal.id
            && matches!(
                candidate.kind.as_str(),
                events::EVENT_KIND_REVIEW_ACCEPTED
                    | events::EVENT_KIND_REVIEW_REJECTED
                    | events::EVENT_KIND_REVIEW_REVISION_REQUESTED
            )
    }) {
        return Err(format!(
            "withdrawal event {} conflicts with a human decision for proposal {}",
            event.id, proposal.id
        ));
    }
    let payload: events::ProposalWithdrawalPayload = serde_json::from_value(event.payload.clone())
        .map_err(|error| format!("withdrawal event {} has invalid payload: {error}", event.id))?;
    let producer_root_matches = match payload.schema.as_str() {
        events::PROPOSAL_WITHDRAWAL_PAYLOAD_SCHEMA => {
            payload.submission_root.as_deref() == Some(authorization.producer_object_root.as_str())
                && payload.receipt_root.is_none()
        }
        events::PROPOSAL_WITHDRAWAL_PAYLOAD_SCHEMA_V1 => {
            payload.receipt_root.as_deref() == Some(authorization.producer_object_root.as_str())
                && payload.submission_root.is_none()
        }
        _ => false,
    };
    if payload.schema != authorization.payload_schema
        || payload.proposal_id != proposal.id
        || event.target.r#type != "proposal"
        || event.target.id != proposal.id
        || payload.proposal_root != authorization.proposal_root
        || !producer_root_matches
        || payload.identity_binding_id != authorization.identity_binding.binding_id
        || event.actor.id != authorization.identity_binding.actor_id
        || event.actor.r#type != "agent"
        || event.before_hash != NULL_HASH
        || event.after_hash != NULL_HASH
        || event.id != events::compute_event_id(event)
        || !crate::sign::verify_event_signature(
            event,
            &authorization.identity_binding.public_key_hex,
        )
        .unwrap_or(false)
    {
        return Err(format!(
            "withdrawal event {} does not match its exact Proposal/Submission producer binding",
            event.id
        ));
    }
    Ok(())
}

/// Return the already verified withdrawal for an idempotent retry. A terminal
/// proposal without exactly one valid matching event fails closed.
pub fn existing_proposal_withdrawal(
    frontier: &Path,
    project: &Project,
    proposal_id: &str,
) -> Result<Option<StateEvent>, String> {
    let Some(proposal) = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
    else {
        return Err(format!("proposal {proposal_id} does not exist"));
    };
    if proposal.status != "withdrawn" {
        return Ok(None);
    }
    let matching = project
        .events
        .iter()
        .filter(|event| {
            event.kind == events::EVENT_KIND_PROPOSAL_WITHDRAWN
                && event.target.r#type == "proposal"
                && event.target.id == proposal_id
        })
        .cloned()
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(format!(
            "withdrawn proposal {proposal_id} must have exactly one withdrawal event, found {}",
            matching.len()
        ));
    }
    if let Some(error) = verify_proposal_withdrawals(frontier, project)
        .into_iter()
        .next()
    {
        return Err(error);
    }
    Ok(matching.into_iter().next())
}

fn proposal_withdrawal_authorization_for_terminal(
    frontier: &Path,
    proposal: &StateProposal,
) -> Result<ProposalWithdrawalAuthorization, String> {
    let mut pending = proposal.clone();
    pending.status = "pending_review".to_string();
    pending.decision_reason = None;
    pending.reviewed_by = None;
    pending.reviewed_at = None;
    pending.applied_event_id = None;
    proposal_withdrawal_authorization(frontier, &pending)
}

mod decision_inspection;
pub mod policy_accept;
mod types;
pub use decision_inspection::{DecisionInspection, inspect_named_decision};
pub use types::*;

pub fn new_proposal(
    kind: impl Into<String>,
    target: StateTarget,
    actor_id: impl Into<String>,
    actor_type: impl Into<String>,
    reason: impl Into<String>,
    payload: Value,
    source_refs: Vec<String>,
    caveats: Vec<String>,
) -> StateProposal {
    new_proposal_at(
        kind,
        target,
        actor_id,
        actor_type,
        reason,
        payload,
        source_refs,
        caveats,
        Utc::now().to_rfc3339(),
    )
}

/// Build a content-addressed proposal with an injected timestamp.
///
/// Transactional writers use one fixed instant for every staged object so a
/// retry or post-marker recovery never consults the wall clock again. The
/// timestamp remains non-canonical proposal metadata; the logical proposal ID
/// is derived by [`proposal_id`] exactly as before.
pub fn new_proposal_at(
    kind: impl Into<String>,
    target: StateTarget,
    actor_id: impl Into<String>,
    actor_type: impl Into<String>,
    reason: impl Into<String>,
    payload: Value,
    source_refs: Vec<String>,
    caveats: Vec<String>,
    created_at: impl Into<String>,
) -> StateProposal {
    let mut proposal = StateProposal {
        schema: PROPOSAL_SCHEMA.to_string(),
        id: String::new(),
        kind: kind.into(),
        target,
        actor: StateActor {
            id: actor_id.into(),
            r#type: actor_type.into(),
        },
        created_at: created_at.into(),
        drafted_at: None,
        reason: reason.into(),
        payload,
        source_refs,
        status: "pending_review".to_string(),
        reviewed_by: None,
        reviewed_at: None,
        decision_reason: None,
        applied_event_id: None,
        caveats,
        agent_run: None,
    };
    proposal.id = proposal_id(&proposal);
    proposal
}

/// Phase P (v0.5): `vpr_…` is content-addressed over the *logical* proposal
/// content only — `created_at` is excluded from the preimage. Identical
/// logical proposals (same actor, target, kind, reason, payload) deterministically
/// produce the same proposal_id regardless of when they were constructed.
///
/// This is the substrate property that makes agent retries idempotent.
/// `created_at` stays on the proposal as non-canonical metadata; replay-attack
/// detection layers on the signed envelope, not the content hash.
pub fn proposal_id(proposal: &StateProposal) -> String {
    let preimage = json!({
        "schema": proposal.schema,
        "kind": proposal.kind,
        "target": proposal.target,
        "actor": proposal.actor,
        "reason": proposal.reason,
        "payload": proposal.payload,
        "source_refs": proposal.source_refs,
        "caveats": proposal.caveats,
    });
    let bytes = canonical::to_canonical_bytes(&preimage).unwrap_or_default();
    format!("vpr_{}", &hex::encode(Sha256::digest(bytes))[..16])
}

pub fn is_placeholder_reviewer(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.is_empty()
        || normalized == "local-reviewer"
        || normalized == "local-user"
        || normalized == "reviewer"
        || normalized == "user"
        || normalized == "unknown"
        || normalized.starts_with("local-")
}

pub fn validate_reviewer_identity(value: &str) -> Result<(), String> {
    if is_placeholder_reviewer(value) {
        return Err(format!(
            "Reviewer identity '{}' is missing or placeholder. Use a stable named reviewer id.",
            value
        ));
    }
    Ok(())
}

/// v0.128: true iff an actor carries *reviewer* authority for the public
/// accept boundary. Doctrine: accept authority is the `reviewer:`
/// namespace and only that namespace — a non-placeholder
/// `reviewer:<name>` id. Historical actor tiers grant no authority. This is deliberately stricter
/// than `validate_reviewer_identity` (which only rejects placeholders):
/// here an `agent:` or bare id is refused outright.
#[must_use]
pub fn actor_has_reviewer_authority(actor: &crate::sign::ActorRecord) -> bool {
    let id = actor.id.trim();
    id.to_ascii_lowercase().starts_with("reviewer:") && !is_placeholder_reviewer(id)
}

pub fn summary(frontier: &Project) -> ProposalSummary {
    let mut out = ProposalSummary::default();
    let mut seen = BTreeSet::new();
    let finding_ids = frontier
        .findings
        .iter()
        .map(|finding| finding.id.as_str())
        .collect::<BTreeSet<_>>();
    let artifact_ids = frontier
        .artifacts
        .iter()
        .map(|artifact| artifact.id.as_str())
        .collect::<BTreeSet<_>>();
    for proposal in &frontier.proposals {
        out.total += 1;
        *out.by_kind.entry(proposal.kind.clone()).or_default() += 1;
        match proposal.status.as_str() {
            "pending_review" => out.pending_review += 1,
            "accepted" => out.accepted += 1,
            "rejected" => out.rejected += 1,
            "applied" => out.applied += 1,
            "withdrawn" => out.withdrawn += 1,
            _ => {}
        }
        if !seen.insert(proposal.id.clone()) {
            out.duplicate_ids.push(proposal.id.clone());
        }
        let target_known = match proposal.target.r#type.as_str() {
            "finding" => {
                proposal.kind == "finding.add" || finding_ids.contains(proposal.target.id.as_str())
            }
            "artifact" => {
                proposal.kind == "artifact.assert"
                    || artifact_ids.contains(proposal.target.id.as_str())
            }
            _ => true,
        };
        if !target_known {
            out.invalid_targets.push(proposal.target.id.clone());
        }
    }
    out.duplicate_ids.sort();
    out.duplicate_ids.dedup();
    out.invalid_targets.sort();
    out.invalid_targets.dedup();
    out
}

pub fn proposals_for_finding<'a>(
    frontier: &'a Project,
    finding_id: &str,
) -> Vec<&'a StateProposal> {
    frontier
        .proposals
        .iter()
        .filter(|proposal| proposal.target.r#type == "finding" && proposal.target.id == finding_id)
        .collect()
}

/// Phase P (v0.5): upsert by content address. If a proposal with the same
/// `vpr_…` already exists in the frontier, return the existing record instead
/// of inserting a duplicate. Combined with the `created_at`-free preimage,
/// this makes agent retries idempotent at the substrate level.
///
/// Decision creation is intentionally absent from this insertion primitive.
pub fn insert_pending_in_frontier(
    frontier: &mut Project,
    proposal: StateProposal,
) -> Result<CreateProposalResult, String> {
    let finding_id = proposal.target.id.clone();
    let proposal_id = proposal.id.clone();
    if let Some(existing) = frontier
        .proposals
        .iter()
        .find(|existing| existing.id == proposal_id)
    {
        return Ok(CreateProposalResult {
            proposal_id,
            finding_id,
            status: existing.status.clone(),
            applied_event_id: existing.applied_event_id.clone(),
        });
    }

    validate_new_proposal(frontier, &proposal)?;
    frontier.proposals.push(proposal);
    // Split repositories load proposal files in filename/id order. Keep every
    // in-memory insertion in that same canonical order so a transaction's
    // rendered postimage remains byte-identical after a subsequent official
    // materialization.
    frontier
        .proposals
        .sort_by(|left, right| left.id.cmp(&right.id));
    project::recompute_stats(frontier);
    Ok(CreateProposalResult {
        proposal_id,
        finding_id,
        status: "pending_review".to_string(),
        applied_event_id: None,
    })
}

pub fn insert_pending_at_path(
    path: &Path,
    proposal: StateProposal,
) -> Result<CreateProposalResult, String> {
    let mut frontier = repo::load_from_path(path)?;
    let result = insert_pending_in_frontier(&mut frontier, proposal)?;
    repo::save_to_path(path, &frontier)?;
    Ok(result)
}

pub fn list(frontier: &Project, status: Option<&str>) -> Vec<StateProposal> {
    let mut proposals = frontier
        .proposals
        .iter()
        .filter(|proposal| status.is_none_or(|wanted| proposal.status == wanted))
        .cloned()
        .collect::<Vec<_>>();
    proposals.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    proposals
}

pub fn show<'a>(frontier: &'a Project, proposal_id: &str) -> Result<&'a StateProposal, String> {
    frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))
}

pub fn preview_at_path(
    path: &Path,
    proposal_id: &str,
    reviewer: &str,
) -> Result<ProposalPreview, String> {
    validate_reviewer_identity(reviewer)?;
    let frontier = repo::load_from_path(path)?;
    preview_in_frontier(&frontier, proposal_id, reviewer)
}

pub fn preview_in_frontier(
    frontier: &Project,
    proposal_id: &str,
    reviewer: &str,
) -> Result<ProposalPreview, String> {
    validate_reviewer_identity(reviewer)?;
    let proposal = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?
        .clone();
    if proposal.status == "applied" {
        let applied_event_id = proposal
            .applied_event_id
            .clone()
            .ok_or_else(|| format!("Proposal {} is applied but has no event id", proposal.id))?;
        return Ok(ProposalPreview {
            proposal_id: proposal.id,
            kind: proposal.kind,
            changed_findings: changed_targets_for_type(frontier, &proposal.target, "finding"),
            changed_finding_details: Vec::new(),
            changed_artifacts: changed_targets_for_type(frontier, &proposal.target, "artifact"),
            new_event_ids: vec![applied_event_id.clone()],
            event_kinds: frontier
                .events
                .iter()
                .find(|event| event.id == applied_event_id)
                .map(|event| vec![event.kind.to_string()])
                .unwrap_or_default(),
            target: proposal.target,
            reviewer: reviewer.to_string(),
            findings_before: frontier.findings.len(),
            findings_after: frontier.findings.len(),
            findings_delta: 0,
            artifacts_before: frontier.artifacts.len(),
            artifacts_after: frontier.artifacts.len(),
            artifacts_delta: 0,
            events_before: frontier.events.len(),
            events_after: frontier.events.len(),
            events_delta: 0,
            proof_would_be_stale: false,
            applied_event_id,
        });
    }
    if !matches!(proposal.status.as_str(), "pending_review" | "accepted") {
        return Err(format!(
            "Proposal {} cannot be previewed from status {}",
            proposal.id, proposal.status
        ));
    }
    let mut preview_state: Project = serde_json::from_value(
        serde_json::to_value(frontier).map_err(|e| format!("serialize frontier preview: {e}"))?,
    )
    .map_err(|e| format!("clone frontier preview: {e}"))?;
    let finding_ids_before = preview_state
        .findings
        .iter()
        .map(|finding| finding.id.clone())
        .collect::<BTreeSet<_>>();
    let artifact_ids_before = preview_state
        .artifacts
        .iter()
        .map(|artifact| artifact.id.clone())
        .collect::<BTreeSet<_>>();
    let findings_before = preview_state.findings.len();
    let artifacts_before = preview_state.artifacts.len();
    let events_before = preview_state.events.len();
    let event_id = apply_proposal(
        &mut preview_state,
        &proposal,
        reviewer,
        "Preview proposal application",
        None,
    )?;
    let findings_after = preview_state.findings.len();
    let artifacts_after = preview_state.artifacts.len();
    let events_after = preview_state.events.len();
    let new_events = preview_state
        .events
        .iter()
        .skip(events_before)
        .cloned()
        .collect::<Vec<_>>();
    let changed_findings = changed_finding_ids(&preview_state, &finding_ids_before, &new_events);
    let changed_finding_details =
        build_changed_finding_details(frontier, &preview_state, &changed_findings);
    Ok(ProposalPreview {
        proposal_id: proposal.id,
        kind: proposal.kind,
        target: proposal.target,
        reviewer: reviewer.to_string(),
        changed_findings,
        changed_finding_details,
        changed_artifacts: changed_artifact_ids(&preview_state, &artifact_ids_before, &new_events),
        new_event_ids: new_events.iter().map(|event| event.id.clone()).collect(),
        event_kinds: new_events
            .iter()
            .map(|event| event.kind.to_string())
            .collect(),
        findings_before,
        findings_after,
        findings_delta: findings_after as isize - findings_before as isize,
        artifacts_before,
        artifacts_after,
        artifacts_delta: artifacts_after as isize - artifacts_before as isize,
        events_before,
        events_after,
        events_delta: events_after as isize - events_before as isize,
        proof_would_be_stale: true,
        applied_event_id: event_id,
    })
}

fn changed_targets_for_type(
    frontier: &Project,
    target: &StateTarget,
    target_type: &str,
) -> Vec<String> {
    let known = match target_type {
        "finding" => frontier
            .findings
            .iter()
            .any(|finding| finding.id == target.id),
        "artifact" => frontier
            .artifacts
            .iter()
            .any(|artifact| artifact.id == target.id),
        _ => false,
    };
    if target.r#type == target_type && known {
        vec![target.id.clone()]
    } else {
        Vec::new()
    }
}

fn changed_finding_ids(
    preview_state: &Project,
    finding_ids_before: &BTreeSet<String>,
    new_events: &[StateEvent],
) -> Vec<String> {
    let mut ids = preview_state
        .findings
        .iter()
        .filter(|finding| !finding_ids_before.contains(&finding.id))
        .map(|finding| finding.id.clone())
        .collect::<BTreeSet<_>>();
    for event in new_events {
        if event.target.r#type == "finding" {
            ids.insert(event.target.id.clone());
        }
    }
    ids.into_iter().collect()
}

fn changed_artifact_ids(
    preview_state: &Project,
    artifact_ids_before: &BTreeSet<String>,
    new_events: &[StateEvent],
) -> Vec<String> {
    let mut ids = preview_state
        .artifacts
        .iter()
        .filter(|artifact| !artifact_ids_before.contains(&artifact.id))
        .map(|artifact| artifact.id.clone())
        .collect::<BTreeSet<_>>();
    for event in new_events {
        if event.target.r#type == "artifact" {
            ids.insert(event.target.id.clone());
        }
    }
    ids.into_iter().collect()
}

pub fn validate_source(source: &Path) -> Result<ProposalValidationReport, String> {
    let proposals = load_proposals(source)?;
    let mut report = ProposalValidationReport {
        checked: proposals.len(),
        ..ProposalValidationReport::default()
    };
    let scratch = project::assemble("proposal-validation", Vec::new(), 0, 0, "validate");
    let mut seen = BTreeSet::new();
    for proposal in proposals {
        if !seen.insert(proposal.id.clone()) {
            report.invalid += 1;
            report
                .errors
                .push(format!("Duplicate proposal id {}", proposal.id));
            continue;
        }
        report.proposal_ids.push(proposal.id.clone());
        match validate_standalone_proposal(&scratch, &proposal) {
            Ok(()) => report.valid += 1,
            Err(err) => {
                report.invalid += 1;
                report.errors.push(format!("{}: {}", proposal.id, err));
            }
        }
    }
    report.ok = report.invalid == 0;
    Ok(report)
}

pub fn export_to_path(
    frontier_path: &Path,
    output: &Path,
    status: Option<&str>,
) -> Result<usize, String> {
    let frontier = repo::load_from_path(frontier_path)?;
    let proposals = list(&frontier, status);
    let json = serde_json::to_string_pretty(&proposals)
        .map_err(|e| format!("Failed to serialize proposals for export: {e}"))?;
    std::fs::write(output, json).map_err(|e| {
        format!(
            "Failed to write proposal export '{}': {e}",
            output.display()
        )
    })?;
    Ok(proposals.len())
}

/// The Engine's read on an acceptance: what Evidence CI says about the
/// state the change would produce. Recomputable at any time from
/// `evidence_ci::run_project`; this captures the *delta* a single
/// acceptance introduces, which is what a reviewer (or the gate) acts on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineVerdict {
    /// `pass` (clean), `warn` (new review warnings), `blocked` (would be
    /// gated; only seen on the preview/error path), or `forced` (gated but
    /// overridden with --force and persisted).
    pub status: String,
    /// Release-blocking checks newly failing because of this change.
    pub new_blocking: Vec<String>,
    /// Review-readiness warnings this change introduces.
    pub new_warnings: Vec<String>,
    /// Whether a gate was overridden with --force.
    pub forced: bool,
    /// Whether warnings were treated as blocking (--strict).
    pub strict: bool,
    /// Post-accept Evidence CI counts, for context in the readout.
    pub release_blocking_failed: usize,
    pub warnings: usize,
}

/// A proposal kind is truth-bearing when accepting it changes what the
/// frontier asserts about the world. Process/provenance records, governance
/// cleanup, and mechanical repairs are not. This classification controls the
/// Engine gate only; it does not grant decision authority.
pub fn is_truth_bearing_kind(kind: &str) -> bool {
    !(NON_TRUTH_BEARING_KINDS.contains(&kind) || MECHANICAL_REPAIR_KINDS.contains(&kind))
}

/// Evaluate one already-prepared candidate transaction under the strict Engine
/// gate without applying proposals again.
///
/// Evidence CI is computed exactly once before and once after. New release
/// failures or review warnings block only when at least one accepted proposal
/// kind is truth-bearing; rejects and mechanical/process accepts remain visible
/// in the counts but cannot turn Evidence CI into a truth oracle. The function
/// mutates nothing and performs no path access beyond Evidence CI's bounded
/// verifier observations.
pub fn strict_engine_verdict_for_candidate(
    original: &Project,
    candidate: &Project,
    frontier_path: &Path,
    accepted_kinds: &[String],
) -> EngineVerdict {
    let before = crate::evidence_ci::run_project(original, frontier_path);
    let after = crate::evidence_ci::run_project(candidate, frontier_path);
    let before_blocking = crate::evidence_ci::release_blocking_failures(&before);
    let before_warnings = crate::evidence_ci::review_warnings(&before);
    let new_blocking = crate::evidence_ci::release_blocking_failures(&after)
        .difference(&before_blocking)
        .cloned()
        .collect::<Vec<_>>();
    let new_warnings = crate::evidence_ci::review_warnings(&after)
        .difference(&before_warnings)
        .cloned()
        .collect::<Vec<_>>();
    let gates = accepted_kinds
        .iter()
        .any(|kind| is_truth_bearing_kind(kind));
    let status = if gates && (!new_blocking.is_empty() || !new_warnings.is_empty()) {
        "blocked"
    } else if !new_warnings.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();
    EngineVerdict {
        status,
        new_blocking,
        new_warnings,
        forced: false,
        strict: true,
        release_blocking_failed: after.summary.release_blocking_failed,
        warnings: after.summary.warnings,
    }
}

/// Compute the Engine verdict a candidate acceptance *would* produce,
/// without persisting anything. Drives the review-time preview ("what
/// would CI say if I accept this?") on the CLI and the Workbench.
pub fn preview_engine_verdict(path: &Path, proposal_id: &str) -> Result<EngineVerdict, String> {
    let frontier = repo::load_from_path(path)?;
    preview_engine_verdict_in_frontier(&frontier, path, proposal_id, false)
}

/// Pure Engine preview over an already staged frontier projection.
///
/// Submission uses this before policy routing so the exact gate result can be
/// committed to its private transaction plan even when the proposal has not
/// yet been installed on disk. The clone is serialized rather than relying on
/// a partial hand-maintained projection of [`Project`].
pub fn preview_engine_verdict_in_frontier(
    frontier: &Project,
    path: &Path,
    proposal_id: &str,
    strict: bool,
) -> Result<EngineVerdict, String> {
    let kind = frontier
        .proposals
        .iter()
        .find(|p| p.id == proposal_id)
        .map(|p| p.kind.clone())
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;

    let before = crate::evidence_ci::run_project(frontier, path);
    let before_blocking = crate::evidence_ci::release_blocking_failures(&before);
    let before_warn = crate::evidence_ci::review_warnings(&before);

    // Apply on this in-memory copy under a synthetic reviewer; never saved.
    let encoded = serde_json::to_value(frontier).map_err(|error| error.to_string())?;
    let mut candidate: Project =
        serde_json::from_value(encoded).map_err(|error| error.to_string())?;
    accept_proposal_in_frontier_with_authority_at(
        &mut candidate,
        proposal_id,
        "reviewer:engine-preview",
        "engine ci preview",
        DecisionAuthority::Preview,
        None,
        None,
        false,
    )?;

    let after = crate::evidence_ci::run_project(&candidate, path);
    let new_blocking: Vec<String> = crate::evidence_ci::release_blocking_failures(&after)
        .difference(&before_blocking)
        .cloned()
        .collect();
    let new_warnings: Vec<String> = crate::evidence_ci::review_warnings(&after)
        .difference(&before_warn)
        .cloned()
        .collect();

    let status = if is_truth_bearing_kind(&kind)
        && (!new_blocking.is_empty() || (strict && !new_warnings.is_empty()))
    {
        "blocked"
    } else if !new_warnings.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();

    Ok(EngineVerdict {
        status,
        new_blocking,
        new_warnings,
        forced: false,
        strict,
        release_blocking_failed: after.summary.release_blocking_failed,
        warnings: after.summary.warnings,
    })
}

pub fn record_proof_export(frontier: &mut Project, record: ProofPacketRecord) {
    let nonlease_event_log_hash = (record.event_log_hash
        == events::event_log_hash(&frontier.events))
    .then(|| events::nonlease_event_log_hash(&frontier.events));
    frontier.proof_state.latest_packet = ProofPacketState {
        generated_at: Some(record.generated_at),
        snapshot_hash: Some(record.snapshot_hash),
        event_log_hash: Some(record.event_log_hash),
        // The derived exemption is granted only when the exporter supplied
        // the exact current full event root. A malformed or mismatched record
        // retains the historical shape and fails freshness normally.
        nonlease_event_log_hash,
        packet_manifest_hash: Some(record.packet_manifest_hash),
        status: "current".to_string(),
    };
    frontier.proof_state.last_event_at_export =
        frontier.events.last().map(|event| event.timestamp.clone());
    frontier.proof_state.stale_reason = None;
}

/// Upgrade a historical proof-state record to the explicit non-lease
/// commitment only when its old full-root commitment already proves that no
/// non-lease event has changed.
///
/// Returning `false` is deliberately non-mutating. A stale, incomplete, or
/// ambiguous record must be re-exported rather than repaired by inference.
pub fn backfill_nonlease_proof_root(frontier: &mut Project) -> bool {
    let state = &frontier.proof_state.latest_packet;
    if state.status == "never_exported"
        || state.status == "stale"
        || state.nonlease_event_log_hash.is_some()
    {
        return false;
    }
    let Some(recorded_event_root) = state.event_log_hash.as_deref() else {
        return false;
    };
    let full_root = events::event_log_hash(&frontier.events);
    let nonlease_root = events::nonlease_event_log_hash(&frontier.events);
    if recorded_event_root != full_root && recorded_event_root != nonlease_root {
        return false;
    }
    frontier.proof_state.latest_packet.nonlease_event_log_hash = Some(nonlease_root);
    true
}

pub fn mark_proof_stale(frontier: &mut Project, reason: String) {
    if frontier.proof_state.latest_packet.status != "never_exported" {
        frontier.proof_state.latest_packet.status = "stale".to_string();
        frontier.proof_state.stale_reason = Some(reason);
    }
}

pub fn proof_state_json(proof_state: &ProofState) -> Value {
    serde_json::to_value(proof_state).unwrap_or_else(|_| json!({"status": "never_exported"}))
}

pub fn proposal_state_hash(proposals: &[StateProposal]) -> String {
    let bytes = canonical::to_canonical_bytes(proposals).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

fn load_proposals(source: &Path) -> Result<Vec<StateProposal>, String> {
    if source.is_file() {
        let data = std::fs::read_to_string(source)
            .map_err(|e| format!("Failed to read proposal file '{}': {e}", source.display()))?;
        if let Ok(proposals) = serde_json::from_str::<Vec<StateProposal>>(&data) {
            return Ok(proposals);
        }
        let proposal = serde_json::from_str::<StateProposal>(&data)
            .map_err(|e| format!("Failed to parse proposal JSON '{}': {e}", source.display()))?;
        return Ok(vec![proposal]);
    }
    if source.is_dir() {
        let mut entries = std::fs::read_dir(source)
            .map_err(|e| format!("Failed to read proposal dir '{}': {e}", source.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect::<Vec<_>>();
        entries.sort();
        let mut proposals = Vec::new();
        for path in entries {
            proposals.extend(load_proposals(&path)?);
        }
        return Ok(proposals);
    }
    Err(format!(
        "Proposal source does not exist: {}",
        source.display()
    ))
}

fn validate_new_proposal(frontier: &Project, proposal: &StateProposal) -> Result<(), String> {
    if proposal.schema != PROPOSAL_SCHEMA {
        return Err(format!("Unsupported proposal schema '{}'", proposal.schema));
    }
    if frontier
        .proposals
        .iter()
        .any(|existing| existing.id == proposal.id)
    {
        return Err(format!("Duplicate proposal id {}", proposal.id));
    }
    validate_proposal_shape(frontier, proposal)?;
    validate_decision_state(proposal)
}

fn validate_proposal_shape(frontier: &Project, proposal: &StateProposal) -> Result<(), String> {
    // v0.52: relax the finding-only constraint so the agent inbox
    // can deposit nulls and trajectories through the same review-
    // gated flow as findings. The proposal-kind dispatch below
    // enforces that target.type matches the kind family.
    if !matches!(
        proposal.target.r#type.as_str(),
        "finding" | "artifact" | "evidence_atom" | "frontier_observation" | "governance"
    ) {
        return Err(format!(
            "Unsupported proposal target type '{}'; valid: finding, artifact, evidence_atom, frontier_observation, governance",
            proposal.target.r#type
        ));
    }
    if proposal.reason.trim().is_empty() {
        return Err("Proposal reason must be non-empty".to_string());
    }
    if !matches!(
        proposal.status.as_str(),
        "pending_review" | "accepted" | "rejected" | "applied" | "withdrawn"
    ) {
        return Err(format!("Unsupported proposal status '{}'", proposal.status));
    }
    match proposal.kind.as_str() {
        policy_accept::POLICY_HEAD_PROPOSAL_KIND => {
            policy_accept::validate_policy_head_proposal(frontier, proposal)?;
        }
        policy_accept::LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND => {
            policy_accept::validate_legacy_policy_retirement_proposal(frontier, proposal)?;
        }
        "finding.add" => {
            let finding_value = proposal
                .payload
                .get("finding")
                .ok_or("finding.add proposal missing payload.finding")?
                .clone();
            let finding: FindingBundle = serde_json::from_value(finding_value)
                .map_err(|e| format!("Invalid finding.add payload: {e}"))?;
            if finding.id != proposal.target.id {
                return Err(format!(
                    "finding.add target {} does not match payload finding {}",
                    proposal.target.id, finding.id
                ));
            }
            if frontier
                .findings
                .iter()
                .any(|existing| existing.id == proposal.target.id)
            {
                return Err(format!(
                    "Refusing to add duplicate finding with existing finding ID {}",
                    proposal.target.id
                ));
            }
            if let Some(submission) = proposal.payload.get("vela_submission") {
                validate_submission_links(submission)?;
            }
            if let Some(submission) = proposal.payload.get("submission") {
                validate_current_submission_links(submission)?;
            }
        }
        "finding.review" => {
            require_existing_finding(frontier, &proposal.target.id)?;
            let status = proposal
                .payload
                .get("status")
                .and_then(Value::as_str)
                .ok_or("finding.review proposal missing payload.status")?;
            if !matches!(
                status,
                "accepted" | "approved" | "contested" | "needs_revision" | "rejected"
            ) {
                return Err(format!("Unsupported review proposal status '{status}'"));
            }
        }
        "finding.caveat" => {
            require_existing_finding(frontier, &proposal.target.id)?;
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.caveat proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.caveat payload.text must be non-empty".to_string());
            }
        }
        "finding.note" => {
            require_existing_finding(frontier, &proposal.target.id)?;
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.note proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.note payload.text must be non-empty".to_string());
            }
        }
        "finding.confidence_revise" => {
            require_existing_finding(frontier, &proposal.target.id)?;
            let score = proposal
                .payload
                .get("confidence")
                .and_then(Value::as_f64)
                .ok_or("finding.confidence_revise proposal missing payload.confidence")?;
            if !(0.0..=1.0).contains(&score) {
                return Err(
                    "finding.confidence_revise confidence must be between 0.0 and 1.0".to_string(),
                );
            }
        }
        "finding.reject" => {
            require_existing_finding(frontier, &proposal.target.id)?;
        }
        "finding.contribution.recorded" => {
            require_existing_finding(frontier, &proposal.target.id)?;
            let contribution: crate::bundle::Contribution =
                serde_json::from_value(proposal.payload.get("contribution").cloned().ok_or(
                    "finding.contribution.recorded proposal missing payload.contribution",
                )?)
                .map_err(|e| format!("malformed contribution: {e}"))?;
            contribution.validate()?;
        }
        "finding.retract" => {
            let idx = require_existing_finding(frontier, &proposal.target.id)?;
            if frontier.findings[idx].flags.retracted {
                return Err(format!(
                    "Finding {} is already retracted",
                    proposal.target.id
                ));
            }
        }
        "finding.supersede" => {
            let idx = require_existing_finding(frontier, &proposal.target.id)?;
            if frontier.findings[idx].flags.superseded {
                return Err(format!(
                    "Finding {} is already superseded",
                    proposal.target.id
                ));
            }
            let new_finding_value = proposal
                .payload
                .get("new_finding")
                .ok_or("finding.supersede proposal missing payload.new_finding")?
                .clone();
            let new_finding: FindingBundle = serde_json::from_value(new_finding_value)
                .map_err(|e| format!("Invalid finding.supersede payload.new_finding: {e}"))?;
            if new_finding.id == proposal.target.id {
                return Err(
                    "finding.supersede new_finding has same content address as the superseded target — change assertion text, type, or provenance to derive a distinct vf_…".to_string(),
                );
            }
            if frontier
                .findings
                .iter()
                .any(|existing| existing.id == new_finding.id)
            {
                return Err(format!(
                    "Refusing to add superseding finding with existing finding ID {}",
                    new_finding.id
                ));
            }
        }
        "artifact.assert" => {
            if proposal.target.r#type != "artifact" {
                return Err(format!(
                    "artifact.assert proposal target.type must be 'artifact', got '{}'",
                    proposal.target.r#type
                ));
            }
            let artifact_value = proposal
                .payload
                .get("artifact")
                .ok_or("artifact.assert proposal missing payload.artifact")?
                .clone();
            let artifact: Artifact = serde_json::from_value(artifact_value)
                .map_err(|e| format!("Invalid artifact.assert payload: {e}"))?;
            if artifact.id != proposal.target.id {
                return Err(format!(
                    "artifact.assert target {} does not match payload id {}",
                    proposal.target.id, artifact.id
                ));
            }
            if frontier.artifacts.iter().any(|a| a.id == artifact.id) {
                return Err(format!(
                    "Refusing to add duplicate artifact with existing id {}",
                    artifact.id
                ));
            }
        }
        "artifact.retract" => {
            if proposal.target.r#type != "artifact" {
                return Err(format!(
                    "artifact.retract proposal target.type must be 'artifact', got '{}'",
                    proposal.target.r#type
                ));
            }
            if proposal.reason.trim().is_empty() {
                return Err("artifact.retract proposal reason must be non-empty".to_string());
            }
            let artifact = frontier
                .artifacts
                .iter()
                .find(|artifact| artifact.id == proposal.target.id)
                .ok_or_else(|| format!("Artifact not found: {}", proposal.target.id))?;
            if artifact.retracted {
                return Err(format!("Artifact {} is already retracted", artifact.id));
            }
        }
        "verifier.attach" => {
            if proposal.target.r#type != "finding" {
                return Err(format!(
                    "verifier.attach proposal target.type must be 'finding', got '{}'",
                    proposal.target.r#type
                ));
            }
            let value = proposal
                .payload
                .get("attachment")
                .ok_or("verifier.attach proposal missing payload.attachment")?
                .clone();
            let att: crate::verifier_attachment::VerifierAttachment = serde_json::from_value(value)
                .map_err(|e| format!("Invalid verifier.attach payload: {e}"))?;
            att.verify()
                .map_err(|e| format!("verifier.attach attachment malformed: {e}"))?;
            if att.target != proposal.target.id {
                return Err(format!(
                    "verifier.attach attachment.target {} does not match proposal target {}",
                    att.target, proposal.target.id
                ));
            }
        }
        // v0.57: Mechanical finding-level span repair. Appends a
        // `{section, text}` span to the finding's evidence_spans.
        "finding.span_repair" => {
            if proposal.target.r#type != "finding" {
                return Err(format!(
                    "finding.span_repair target.type must be 'finding', got '{}'",
                    proposal.target.r#type
                ));
            }
            require_existing_finding(frontier, &proposal.target.id)?;
            let section = proposal
                .payload
                .get("section")
                .and_then(Value::as_str)
                .ok_or("finding.span_repair proposal missing payload.section")?;
            if section.trim().is_empty() {
                return Err("finding.span_repair payload.section must be non-empty".to_string());
            }
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.span_repair proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.span_repair payload.text must be non-empty".to_string());
            }
        }
        // v0.56: Mechanical evidence-atom locator repair. Targets one
        // evidence atom by id; payload carries the resolved locator
        // string and the parent source id it was derived from. The
        // proposal is mechanical: the locator is already present on
        // `frontier.sources[atom.source_id].locator`. A Decision Plan accepts
        // it and the canonical event lands the locator
        // on the atom while preserving the derivation in the payload.
        "evidence_atom.locator_repair" => {
            if proposal.target.r#type != "evidence_atom" {
                return Err(format!(
                    "evidence_atom.locator_repair target.type must be 'evidence_atom', got '{}'",
                    proposal.target.r#type
                ));
            }
            let atom_id = proposal.target.id.as_str();
            let atom = frontier
                .evidence_atoms
                .iter()
                .find(|atom| atom.id == atom_id)
                .ok_or_else(|| {
                    format!("evidence_atom.locator_repair targets unknown atom {atom_id}")
                })?;
            let locator = proposal
                .payload
                .get("locator")
                .and_then(Value::as_str)
                .ok_or("evidence_atom.locator_repair proposal missing payload.locator")?;
            if locator.trim().is_empty() {
                return Err(
                    "evidence_atom.locator_repair payload.locator must be non-empty".to_string(),
                );
            }
            let source_id = proposal
                .payload
                .get("source_id")
                .and_then(Value::as_str)
                .ok_or("evidence_atom.locator_repair proposal missing payload.source_id")?;
            if source_id.trim().is_empty() {
                return Err(
                    "evidence_atom.locator_repair payload.source_id must be non-empty".to_string(),
                );
            }
            if atom.source_id != source_id {
                return Err(format!(
                    "evidence_atom.locator_repair payload.source_id '{source_id}' does not match atom.source_id '{}'",
                    atom.source_id
                ));
            }
            // Refuse a no-op repair so the curation pipeline doesn't
            // emit empty events. An atom that already carries the same
            // locator should be filtered upstream.
            if let Some(existing) = &atom.locator
                && existing == locator
            {
                return Err(format!(
                    "evidence_atom {atom_id} already carries locator '{existing}'"
                ));
            }
            // Refuse a divergent overwrite. A different existing
            // locator is a chain-integrity issue, not a repair.
            if let Some(existing) = &atom.locator
                && existing != locator
            {
                return Err(format!(
                    "evidence_atom {atom_id} already carries locator '{existing}'; refusing to overwrite with '{locator}'"
                ));
            }
        }
        "research_trace.review" => {
            validate_research_trace_review_payload(proposal)?;
        }
        "correction_return.review" => {
            validate_correction_return_review_payload(proposal)?;
        }
        other => {
            return Err(format!("Unsupported proposal kind '{other}'"));
        }
    }
    Ok(())
}

fn validate_submission_links(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("finding.add payload.vela_submission must be an object")?;
    if object.get("schema").and_then(Value::as_str) != Some("vela.submission-links.internal.v1") {
        return Err("finding.add payload.vela_submission has an unsupported schema".to_string());
    }
    let valid_prefixed_digest = |field: &str, prefix: &str| -> Result<(), String> {
        let value = object
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("vela_submission.{field} must be a string"))?;
        let digest = value
            .strip_prefix(prefix)
            .ok_or_else(|| format!("vela_submission.{field} must start with {prefix}"))?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "vela_submission.{field} must contain 64 lowercase hex characters"
            ));
        }
        Ok(())
    };
    valid_prefixed_digest("receipt_root", "sha256:")?;
    valid_prefixed_digest("operation_id", "vop_")?;
    let record_id = object
        .get("record_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !record_id.strip_prefix("vrc_").is_some_and(|id| {
        id.len() == 16
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) {
        return Err("vela_submission.record_id must be vrc_<16 lowercase hex>".to_string());
    }
    let receipt_path = object
        .get("receipt_path")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let path = std::path::Path::new(receipt_path);
    if receipt_path.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err("vela_submission.receipt_path must be frontier-relative".to_string());
    }
    if let Some(review_path) = object.get("review_material_path").and_then(Value::as_str) {
        let receipt_digest = object
            .get("receipt_root")
            .and_then(Value::as_str)
            .and_then(|root| root.strip_prefix("sha256:"))
            .unwrap_or_default();
        let expected = format!("records/review/sha256/{receipt_digest}.json");
        if review_path != expected {
            return Err(format!(
                "vela_submission.review_material_path must be {expected}"
            ));
        }
    }
    Ok(())
}

fn validate_current_submission_links(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("finding.add payload.submission must be an object")?;
    if object.get("schema").and_then(Value::as_str)
        != Some("vela.submission-proposal-links.internal.v1")
    {
        return Err("finding.add payload.submission has an unsupported schema".to_string());
    }
    let submission_id = object
        .get("submission_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !submission_id.strip_prefix("vsb_").is_some_and(|id| {
        id.len() == 16
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) {
        return Err("submission.submission_id must be vsb_<16 lowercase hex>".to_string());
    }
    let submission_root = object
        .get("submission_root")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let digest = submission_root
        .strip_prefix("sha256:")
        .ok_or("submission.submission_root must be a full sha256: digest")?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(
            "submission.submission_root must contain 64 lowercase hex characters".to_string(),
        );
    }
    let operation_id = object
        .get("operation_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let operation_digest = operation_id
        .strip_prefix("vop_")
        .ok_or("submission.operation_id must start with vop_")?;
    if operation_digest.len() != 64
        || !operation_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("submission.operation_id must contain 64 lowercase hex characters".to_string());
    }
    let submission_path = object
        .get("submission_path")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected = format!("records/submissions/sha256/{digest}.json");
    if submission_path != expected {
        return Err(format!("submission.submission_path must be {expected}"));
    }
    Ok(())
}

fn validate_decision_state(proposal: &StateProposal) -> Result<(), String> {
    match proposal.status.as_str() {
        "pending_review" => Ok(()),
        "accepted" | "applied" | "rejected" => {
            let reviewer = proposal
                .reviewed_by
                .as_deref()
                .ok_or_else(|| format!("Proposal {} missing reviewed_by", proposal.id))?;
            validate_reviewer_identity(reviewer)?;
            if proposal
                .decision_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
            {
                return Err(format!("Proposal {} missing decision_reason", proposal.id));
            }
            if proposal.status == "applied" && proposal.applied_event_id.is_none() {
                return Err(format!(
                    "Applied proposal {} missing applied_event_id",
                    proposal.id
                ));
            }
            Ok(())
        }
        "withdrawn" => {
            if proposal.reviewed_by.is_some()
                || proposal.reviewed_at.is_some()
                || proposal.applied_event_id.is_some()
            {
                return Err(format!(
                    "Withdrawn proposal {} must not carry human review fields",
                    proposal.id
                ));
            }
            if proposal
                .decision_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
            {
                return Err(format!("Withdrawn proposal {} missing reason", proposal.id));
            }
            Ok(())
        }
        other => Err(format!("Unsupported proposal status '{}'", other)),
    }
}

fn validate_standalone_proposal(
    _frontier: &Project,
    proposal: &StateProposal,
) -> Result<(), String> {
    if proposal.schema != PROPOSAL_SCHEMA {
        return Err(format!("Unsupported proposal schema '{}'", proposal.schema));
    }
    if !matches!(
        proposal.target.r#type.as_str(),
        "finding" | "artifact" | "evidence_atom" | "frontier_observation" | "governance"
    ) {
        return Err(
            "Only finding, artifact, evidence_atom, frontier_observation, and governance proposals are supported in v0"
                .to_string(),
        );
    }
    if proposal.reason.trim().is_empty() {
        return Err("Proposal reason must be non-empty".to_string());
    }
    match proposal.kind.as_str() {
        policy_accept::POLICY_HEAD_PROPOSAL_KIND => {
            if proposal.target.r#type != "governance" {
                return Err("policy-head proposal target.type must be governance".to_string());
            }
            policy_accept::parse_policy_head_payload(proposal)?;
        }
        policy_accept::LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND => {
            if proposal.target.r#type != "governance" {
                return Err(
                    "legacy-policy-retirement proposal target.type must be governance".to_string(),
                );
            }
            policy_accept::parse_legacy_policy_retirement_payload(proposal)?;
        }
        "finding.add" => {
            let finding_value = proposal
                .payload
                .get("finding")
                .ok_or("finding.add proposal missing payload.finding")?
                .clone();
            let finding: FindingBundle = serde_json::from_value(finding_value)
                .map_err(|e| format!("Invalid finding.add payload: {e}"))?;
            if finding.id != proposal.target.id {
                return Err(format!(
                    "finding.add target {} does not match payload finding {}",
                    proposal.target.id, finding.id
                ));
            }
        }
        "finding.review" => {
            let status = proposal
                .payload
                .get("status")
                .and_then(Value::as_str)
                .ok_or("finding.review proposal missing payload.status")?;
            if !matches!(
                status,
                "accepted" | "approved" | "contested" | "needs_revision" | "rejected"
            ) {
                return Err(format!("Unsupported review proposal status '{status}'"));
            }
        }
        "finding.caveat" => {
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.caveat proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.caveat payload.text must be non-empty".to_string());
            }
        }
        "finding.note" => {
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.note proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.note payload.text must be non-empty".to_string());
            }
        }
        "finding.confidence_revise" => {
            let score = proposal
                .payload
                .get("confidence")
                .and_then(Value::as_f64)
                .ok_or("finding.confidence_revise proposal missing payload.confidence")?;
            if !(0.0..=1.0).contains(&score) {
                return Err(
                    "finding.confidence_revise confidence must be between 0.0 and 1.0".to_string(),
                );
            }
        }
        "finding.reject" | "finding.retract" => {}
        "artifact.retract" => {
            if proposal.target.r#type != "artifact" {
                return Err(format!(
                    "artifact.retract target.type must be 'artifact', got '{}'",
                    proposal.target.r#type
                ));
            }
        }
        "finding.supersede" => {
            let new_finding_value = proposal
                .payload
                .get("new_finding")
                .ok_or("finding.supersede proposal missing payload.new_finding")?
                .clone();
            let new_finding: FindingBundle = serde_json::from_value(new_finding_value)
                .map_err(|e| format!("Invalid finding.supersede payload.new_finding: {e}"))?;
            if new_finding.id == proposal.target.id {
                return Err(
                    "finding.supersede new_finding has same content address as the superseded target"
                        .to_string(),
                );
            }
        }
        // v0.57: standalone validation of finding span-repair.
        "finding.span_repair" => {
            if proposal.target.r#type != "finding" {
                return Err(format!(
                    "finding.span_repair target.type must be 'finding', got '{}'",
                    proposal.target.r#type
                ));
            }
            let section = proposal
                .payload
                .get("section")
                .and_then(Value::as_str)
                .ok_or("finding.span_repair proposal missing payload.section")?;
            if section.trim().is_empty() {
                return Err("finding.span_repair payload.section must be non-empty".to_string());
            }
            let text = proposal
                .payload
                .get("text")
                .and_then(Value::as_str)
                .ok_or("finding.span_repair proposal missing payload.text")?;
            if text.trim().is_empty() {
                return Err("finding.span_repair payload.text must be non-empty".to_string());
            }
        }
        // v0.56: standalone validation of an evidence-atom locator
        // repair. Mirrors the contextual validator in
        // `validate_proposal_shape`, except without frontier-side
        // existence checks (the standalone validator runs over an
        // exported proposal before it is loaded into a frontier).
        "evidence_atom.locator_repair" => {
            if proposal.target.r#type != "evidence_atom" {
                return Err(format!(
                    "evidence_atom.locator_repair target.type must be 'evidence_atom', got '{}'",
                    proposal.target.r#type
                ));
            }
            let locator = proposal
                .payload
                .get("locator")
                .and_then(Value::as_str)
                .ok_or("evidence_atom.locator_repair proposal missing payload.locator")?;
            if locator.trim().is_empty() {
                return Err(
                    "evidence_atom.locator_repair payload.locator must be non-empty".to_string(),
                );
            }
            let source_id = proposal
                .payload
                .get("source_id")
                .and_then(Value::as_str)
                .ok_or("evidence_atom.locator_repair proposal missing payload.source_id")?;
            if source_id.trim().is_empty() {
                return Err(
                    "evidence_atom.locator_repair payload.source_id must be non-empty".to_string(),
                );
            }
        }
        "finding.contribution.recorded" => {
            let contribution: crate::bundle::Contribution =
                serde_json::from_value(proposal.payload.get("contribution").cloned().ok_or(
                    "finding.contribution.recorded proposal missing payload.contribution",
                )?)
                .map_err(|e| format!("malformed contribution: {e}"))?;
            contribution.validate()?;
        }
        "research_trace.review" => {
            validate_research_trace_review_payload(proposal)?;
        }
        "correction_return.review" => {
            validate_correction_return_review_payload(proposal)?;
        }
        other => return Err(format!("Unsupported proposal kind '{other}'")),
    }
    validate_decision_state(proposal)
}

fn validate_research_trace_review_payload(proposal: &StateProposal) -> Result<(), String> {
    if proposal.target.r#type != "frontier_observation" {
        return Err(format!(
            "research_trace.review target.type must be 'frontier_observation', got '{}'",
            proposal.target.r#type
        ));
    }
    let trace_id = proposal
        .payload
        .get("trace_id")
        .and_then(Value::as_str)
        .ok_or("research_trace.review proposal missing payload.trace_id")?;
    if !trace_id.starts_with("vrt_") {
        return Err("research_trace.review payload.trace_id must start with `vrt_`".to_string());
    }
    let output_kind = proposal
        .payload
        .get("output_kind")
        .and_then(Value::as_str)
        .ok_or("research_trace.review proposal missing payload.output_kind")?;
    if !matches!(output_kind, "candidate_finding" | "open_need") {
        return Err(format!(
            "research_trace.review payload.output_kind must be candidate_finding or open_need, got '{output_kind}'"
        ));
    }
    if output_kind == "candidate_finding" && proposal.payload.get("candidate").is_none() {
        return Err(
            "research_trace.review candidate_finding missing payload.candidate".to_string(),
        );
    }
    if output_kind == "open_need" && proposal.payload.get("open_need").is_none() {
        return Err("research_trace.review open_need missing payload.open_need".to_string());
    }
    if proposal.payload.get("authority_boundary").is_none() {
        return Err("research_trace.review missing payload.authority_boundary".to_string());
    }
    if proposal.payload.get("formalization_fidelity").is_none() {
        return Err("research_trace.review missing payload.formalization_fidelity".to_string());
    }
    if !proposal
        .source_refs
        .iter()
        .any(|source_ref| source_ref == trace_id)
    {
        return Err(format!(
            "research_trace.review source_refs must include trace_id {trace_id}"
        ));
    }
    Ok(())
}

fn validate_correction_return_review_payload(proposal: &StateProposal) -> Result<(), String> {
    if proposal.target.r#type != "frontier_observation" {
        return Err(format!(
            "correction_return.review target.type must be 'frontier_observation', got '{}'",
            proposal.target.r#type
        ));
    }
    let correction = proposal
        .payload
        .get("correction")
        .ok_or("correction_return.review proposal missing payload.correction")?;
    for field in [
        "target_id",
        "issue",
        "proposed_change",
        "source_locator",
        "evidence_span",
    ] {
        let value = correction
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!("correction_return.review payload.correction.{field} is required")
            })?;
        if value.trim().is_empty() {
            return Err(format!(
                "correction_return.review payload.correction.{field} must be non-empty"
            ));
        }
    }
    let verification_runs = correction
        .get("verification_run")
        .and_then(Value::as_array)
        .ok_or("correction_return.review payload.correction.verification_run must be an array")?;
    if verification_runs.is_empty() {
        return Err(
            "correction_return.review payload.correction.verification_run must be non-empty"
                .to_string(),
        );
    }
    let boundary = proposal
        .payload
        .get("claim_boundary")
        .and_then(Value::as_object)
        .ok_or("correction_return.review missing payload.claim_boundary")?;
    for field in [
        "claims_clinical_validity",
        "claims_external_adoption",
        "claims_external_validation",
        "claims_lab_validation",
        "claims_scientific_discovery",
        "claims_target_validation",
        "claims_treatment_advice",
    ] {
        match boundary.get(field).and_then(Value::as_bool) {
            Some(false) => {}
            Some(true) => {
                return Err(format!(
                    "correction_return.review payload.claim_boundary.{field} must be false"
                ));
            }
            None => {
                return Err(format!(
                    "correction_return.review payload.claim_boundary.{field} is required"
                ));
            }
        }
    }
    if !proposal.source_refs.iter().any(|source_ref| {
        source_ref == "correction-return.template.json"
            || (source_ref.starts_with("correction-return.") && source_ref.ends_with(".json"))
    }) {
        return Err(
            "correction_return.review source_refs must include correction-return.template.json or a correction-return.*.json file"
                .to_string(),
        );
    }
    Ok(())
}

fn require_existing_finding(frontier: &Project, finding_id: &str) -> Result<usize, String> {
    frontier
        .findings
        .iter()
        .position(|finding| finding.id == finding_id)
        .ok_or_else(|| format!("Finding not found: {finding_id}"))
}

// Proposal-kind classification is deliberately separate from authority.
// These lists tell policy and presentation code which records are mechanical
// or process-only; they do not mint a decision capability. A caller still
// needs a Decision Plan.

/// Mechanical, truth-preserving repair kinds. This is classification only;
/// it does not let an agent apply the proposal without decision authority.
const MECHANICAL_REPAIR_KINDS: &[&str] = &["finding.span_repair", "evidence_atom.locator_repair"];

/// Non-truth-bearing process and provenance kinds. Content-addressed artifact
/// registration and claim-granularity attribution assert no scientific claim
/// about the world. Legacy policy retirement is governance cleanup and remains
/// human-only through its dedicated acceptance branch; listing it here merely
/// prevents the scientific Evidence CI gate from treating byte retirement as a
/// scientific assertion. Classification never confers authority.
const NON_TRUTH_BEARING_KINDS: &[&str] = &[
    "artifact.assert",
    "artifact.add",
    "finding.contribution.recorded",
    policy_accept::LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND,
];

/// Proposal-level guards for exact-lane auto-admission (Phase 1A, the
/// de-human-gate). Returns `(admit, reasons)`; `reasons` is non-empty exactly
/// when refused. Pure and deterministic, so two implementations agree.
///
/// IMPORTANT — this is NOT the whole gate. The un-forgeable floor (a fresh
/// `vela reproduce` over the witness AND `vela_verify::claim_witness_faithful`
/// binding the parsed assertion to the witness structure) is applied by the
/// CLI command BEFORE this is called, because it needs the `vela-verify`
/// binary and the witness file, which the protocol crate does not see. This
/// function adds the protocol-level guards a human reviewer applies and then
/// delegates to the attachment corroboration predicate
/// [`crate::verifier_attachment::exact_lane_attachment_admit`]. See
/// `docs/VERIFICATION.md` for why the corroboration predicate alone is
/// insufficient (a `VerifierAttachment` is unsigned self-asserted data the
/// producing agent can author).
///
/// Fail-closed guard order:
///   1. kind allowlist: `finding.add` only.
///   2. target binding: target is this finding.
///   3. content-address drift-pin: the loaded finding body must content-address
///      to its own id (closes assertion-text edits after the id was minted).
///   4. lifecycle: the finding is neither retracted nor superseded.
///   5. synthetic: no `synthetic_source_requires_review` signal (caller-derived).
///   6. contradiction: no live open contradiction names this finding
///      (caller-derived, including freshly derived candidates).
///   7. producer != verifier: the proposing actor differs from every matched
///      attachment's `verifier_actor` (the producer cannot be its own
///      corroborator at the actor level).
///   8. delegate to the attachment predicate over the matched attachments.
pub fn exact_lane_eligible(
    proposal: &StateProposal,
    finding: &crate::bundle::FindingBundle,
    attachments: &[crate::verifier_attachment::VerifierAttachment],
    open_contradiction_finding_ids: &BTreeSet<String>,
    synthetic_unreviewed_finding_ids: &BTreeSet<String>,
    floor_sufficient: bool,
) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();

    // 1. kind allowlist.
    if proposal.kind != "finding.add" {
        reasons.push(format!(
            "exact-lane: proposal kind '{}' is not 'finding.add'",
            proposal.kind
        ));
        return (false, reasons);
    }

    // 2. target binding.
    if proposal.target.r#type != "finding" || proposal.target.id != finding.id {
        reasons.push("exact-lane: proposal target does not bind to this finding".to_string());
        return (false, reasons);
    }

    // 3. content-address drift-pin: the body must hash to its own id.
    let recomputed =
        crate::bundle::FindingBundle::content_address(&finding.assertion, &finding.provenance);
    if recomputed != finding.id {
        reasons.push(format!(
            "exact-lane: finding body does not content-address to its id (drift): {} != {}",
            recomputed, finding.id
        ));
        return (false, reasons);
    }

    // 4. lifecycle.
    if finding.flags.retracted || finding.flags.superseded {
        reasons.push("exact-lane: finding is retracted or superseded".to_string());
        return (false, reasons);
    }

    // 5. synthetic-source signal.
    if synthetic_unreviewed_finding_ids.contains(&finding.id) {
        reasons.push(
            "exact-lane: finding carries a synthetic_source_requires_review signal".to_string(),
        );
        return (false, reasons);
    }

    // 6. live open contradiction.
    if open_contradiction_finding_ids.contains(&finding.id) {
        reasons.push("exact-lane: a live open contradiction names this finding".to_string());
        return (false, reasons);
    }

    // The matched attachments (those bound to this finding).
    let matched: Vec<crate::verifier_attachment::VerifierAttachment> = attachments
        .iter()
        .filter(|a| a.target == finding.id)
        .cloned()
        .collect();

    // 7. producer != verifier: the proposing actor cannot also be a corroborator.
    let producer = proposal.actor.id.trim();
    if !producer.is_empty()
        && let Some(bad) = matched.iter().find(|a| a.verifier_actor.trim() == producer)
    {
        reasons.push(format!(
            "exact-lane: the proposing actor '{}' is also a verifier_actor on attachment '{}' \
             (producer cannot corroborate itself)",
            producer, bad.id
        ));
        return (false, reasons);
    }

    // 8. corroboration. When `floor_sufficient` (the caller established the
    // un-forgeable floor: a fresh frozen `vela reproduce` over the witness AND
    // `claim_witness_faithful` binding the parsed assertion to it), the FLOOR is
    // itself the proof of an exact lower-bound / size claim, so the
    // >=2-independent-attachment requirement (the GENERAL gate's bar, for claims
    // with no single frozen verifier) is waived — attachments become optional
    // corroboration. Otherwise the attachment predicate must derive Verified.
    if !floor_sufficient {
        let digest = crate::verifier_attachment::claim_digest(&finding.assertion.text);
        let (admit, att_reasons) =
            crate::verifier_attachment::exact_lane_attachment_admit(&digest, &matched);
        if !admit {
            reasons.extend(att_reasons);
            return (false, reasons);
        }
    }

    (true, reasons)
}

/// The verification trust tier of a finding (Phase 1A). An ordered ladder;
/// the machine advances the lower rungs, a human key-custody accept is the
/// only path to `Accepted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    Candidate,
    SchemaChecked,
    MachineVerified,
    Accepted,
}

impl TrustTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustTier::Candidate => "candidate",
            TrustTier::SchemaChecked => "schema_checked",
            TrustTier::MachineVerified => "machine_verified",
            TrustTier::Accepted => "accepted",
        }
    }
}

/// Project a finding's verification trust tier from canonical state + the
/// immutable log + live attachments (Phase 1A). A read-only projection, never
/// a stored field: recomputed fresh so a forged `policy.auto_admitted` event
/// cannot by itself raise the tier, and a later-weakened attachment set
/// silently lowers it.
///
/// - `Accepted`: the finding is landed in canonical state (`frontier.findings`)
///   and not retracted/superseded. Landing runs only through the key-custody
///   accept ceremony, so canonical membership IS the human-accept signal (no
///   reliance on which event kind the ceremony emitted). Strictly highest; the
///   machine never reaches it.
/// - `MachineVerified`: the finding is a PENDING `finding.add` proposal carrying
///   a `policy.auto_admitted` marker whose gate, recomputed LIVE from the
///   proposal's finding text + the current matched attachments, is `Verified`.
///   Machine-verified state is a separate queryable layer over pending
///   proposals; it is NEVER landed in `frontier.findings` (that is the human
///   tier), which preserves the charter boundary.
/// - `SchemaChecked`: at least one passing matched attachment, not yet Verified.
/// - `Candidate`: everything else, including retracted/superseded.
pub fn derive_trust_tier(frontier: &Project, finding_id: &str) -> TrustTier {
    use crate::verifier_attachment::{AttachmentOutcome, GateStatus, derive_gate_status};

    // Landed in canonical accepted state?
    if let Some(f) = frontier.findings.iter().find(|f| f.id == finding_id) {
        if f.flags.retracted || f.flags.superseded {
            return TrustTier::Candidate;
        }
        return TrustTier::Accepted;
    }

    let matched: Vec<crate::verifier_attachment::VerifierAttachment> = frontier
        .verifier_attachments
        .iter()
        .filter(|a| a.target == finding_id)
        .cloned()
        .collect();

    // A pending finding.add proposal for this finding, carrying an auto-admit.
    let pending = frontier.proposals.iter().find(|p| {
        p.kind == "finding.add"
            && p.applied_event_id.is_none()
            && (p.target.id == finding_id
                || p.payload
                    .get("finding")
                    .and_then(|f| f.get("id"))
                    .and_then(|i| i.as_str())
                    == Some(finding_id))
    });
    if let Some(p) = pending {
        let admitted = frontier.events.iter().any(|e| {
            e.kind.as_str() == "policy.auto_admitted"
                && e.payload.get("proposal_id").and_then(|v| v.as_str()) == Some(p.id.as_str())
        });
        if admitted
            && let Some(finding_val) = p.payload.get("finding")
            && let Ok(fb) =
                serde_json::from_value::<crate::bundle::FindingBundle>(finding_val.clone())
        {
            let digest = crate::verifier_attachment::claim_digest(&fb.assertion.text);
            if derive_gate_status(&digest, &matched).status == GateStatus::Verified {
                return TrustTier::MachineVerified;
            }
        }
    }

    if matched
        .iter()
        .any(|a| a.outcome == AttachmentOutcome::Passed)
    {
        return TrustTier::SchemaChecked;
    }
    TrustTier::Candidate
}

/// Result of a fixed-time, in-memory decision preparation.
///
/// This is transaction plumbing, not a serialized protocol object. The
/// prepared events are unsigned. A human-key caller may pass this exact set to
/// [`sign_prepared_decision_events`] only after rederiving and confirming its
/// Decision Plan under the frontier lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDecisionMutation {
    /// The domain event produced by an accept, or the review event for a
    /// reject/policy-head accept.
    primary_event_id: String,
    /// Every event appended by this one decision, in canonical order.
    appended_event_ids: Vec<String>,
    /// The `review.accepted` / `review.rejected` event that binds the decision
    /// provenance, including the decision-root input reference when supplied.
    decision_event_id: String,
    proposal_id: String,
    reviewer: String,
    first_event: usize,
    event_count_after: usize,
    binding: PreparedDecisionBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreparedDecisionBinding {
    Unbound,
    Bound { decision_root: String },
}

impl PreparedDecisionMutation {
    #[must_use]
    pub fn primary_event_id(&self) -> &str {
        &self.primary_event_id
    }

    #[must_use]
    pub fn appended_event_ids(&self) -> &[String] {
        &self.appended_event_ids
    }

    #[must_use]
    pub fn decision_event_id(&self) -> &str {
        &self.decision_event_id
    }

    #[must_use]
    pub fn proposal_id(&self) -> &str {
        &self.proposal_id
    }

    #[must_use]
    pub fn reviewer(&self) -> &str {
        &self.reviewer
    }

    #[must_use]
    pub fn decision_root(&self) -> Option<&str> {
        match &self.binding {
            PreparedDecisionBinding::Unbound => None,
            PreparedDecisionBinding::Bound { decision_root } => Some(decision_root),
        }
    }

    #[must_use]
    pub fn is_bound(&self) -> bool {
        self.decision_root().is_some()
    }
}

/// Check decision-time reviewer facts without reading or receiving a private
/// key. New Decision Plan reviews require a registered, active Ed25519 actor;
/// agent and CI identities can never enter this human seam. Legacy decision
/// creation APIs fail closed; there is no keyless bootstrap decision path.
pub fn validate_human_reviewer_authority_at(
    frontier: &Project,
    reviewer: &str,
    decided_at: &str,
) -> Result<crate::sign::ActorRecord, String> {
    validate_reviewer_identity(reviewer)?;
    let decision_at = chrono::DateTime::parse_from_rfc3339(decided_at)
        .map_err(|error| format!("decision time is invalid: {error}"))?;
    if reviewer.starts_with("agent:") || reviewer.starts_with("ci:") {
        return Err(format!(
            "reviewer '{reviewer}' may not enter a human Decision Plan"
        ));
    }
    let mut actors = frontier.actors.iter().filter(|actor| actor.id == reviewer);
    let actor = actors
        .next()
        .cloned()
        .ok_or_else(|| format!("reviewer '{reviewer}' is not registered on this frontier"))?;
    if actors.next().is_some() {
        return Err(format!(
            "reviewer '{reviewer}' is registered ambiguously (duplicate actor ids)"
        ));
    }
    if actor.algorithm != "ed25519"
        || hex::decode(&actor.public_key).map_or(true, |bytes| bytes.len() != 32)
    {
        return Err(format!(
            "reviewer '{}' must have a registered Ed25519 decision key",
            actor.id
        ));
    }
    if !actor_has_reviewer_authority(&actor)
        && !(actor.id.starts_with("steward:") && !is_placeholder_reviewer(&actor.id))
    {
        return Err(format!(
            "actor '{}' does not carry reviewer or steward decision authority",
            actor.id
        ));
    }
    let created_at = chrono::DateTime::parse_from_rfc3339(&actor.created_at)
        .map_err(|error| format!("reviewer '{}' creation time is invalid: {error}", actor.id))?;
    if created_at > decision_at {
        return Err(format!(
            "reviewer '{}' is not yet registered at {decided_at}",
            actor.id
        ));
    }
    if let Some(revoked_at) = actor.revoked_at.as_deref() {
        let revoked_at = chrono::DateTime::parse_from_rfc3339(revoked_at).map_err(|error| {
            format!(
                "reviewer '{}' revocation time is invalid: {error}",
                actor.id
            )
        })?;
        if decision_at >= revoked_at {
            return Err(format!(
                "reviewer key for actor '{}' is revoked as of {decided_at}",
                actor.id
            ));
        }
    }
    Ok(actor)
}

enum DecisionAuthority<'a> {
    LocalKey(&'a ed25519_dalek::SigningKey),
    PlanPreparation,
    /// Pure candidate preparation for an Era-1 repository-authority
    /// transaction. This grants no authority and performs no write: the CLI
    /// must authenticate the named principal, authorize the exact action, and
    /// install the resulting event/object set under one verified covering
    /// authority record.
    RepositoryPreparation,
    Preview,
}

impl<'a> DecisionAuthority<'a> {
    fn signing_key(&self) -> Option<&'a ed25519_dalek::SigningKey> {
        match self {
            Self::LocalKey(key) => Some(*key),
            _ => None,
        }
    }

    fn is_preview(&self) -> bool {
        matches!(self, Self::Preview)
    }
}

fn enforce_decision_authority(
    frontier: &Project,
    _proposal: &StateProposal,
    reviewer: &str,
    decided_at: &str,
    authority: &DecisionAuthority<'_>,
) -> Result<(), String> {
    match authority {
        DecisionAuthority::LocalKey(key) => {
            let actor = validate_human_reviewer_authority_at(frontier, reviewer, decided_at)?;
            let derived = hex::encode(key.verifying_key().to_bytes());
            if !derived.eq_ignore_ascii_case(&actor.public_key) {
                return Err(format!(
                    "the supplied key derives pubkey {}…, which does not match {reviewer}'s registered decision key {}…",
                    &derived[..12],
                    &actor.public_key[..actor.public_key.len().min(12)]
                ));
            }
            Ok(())
        }
        DecisionAuthority::PlanPreparation => {
            validate_human_reviewer_authority_at(frontier, reviewer, decided_at).map(|_| ())
        }
        DecisionAuthority::RepositoryPreparation => {
            chrono::DateTime::parse_from_rfc3339(decided_at)
                .map_err(|error| format!("repository decision time is invalid: {error}"))?;
            validate_reviewer_identity(reviewer)
        }
        DecisionAuthority::Preview => validate_reviewer_identity(reviewer),
    }
}

/// Accept exactly one policy-head proposal at a caller-bound instant.
///
/// This is the transaction planner seam used by the human policy ceremony:
/// the CLI reads the key once, fixes one timestamp, signs both the policy and
/// the existing `review.accepted` authority event, then journals their public
/// postimages together. It cannot be used for ordinary proposal kinds.
pub fn accept_policy_head_proposal_in_frontier_at(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    signing_key: &ed25519_dalek::SigningKey,
    decided_at: &str,
) -> Result<String, String> {
    chrono::DateTime::parse_from_rfc3339(decided_at)
        .map_err(|error| format!("policy-head decision time is invalid: {error}"))?;
    let proposal = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    if proposal.kind != policy_accept::POLICY_HEAD_PROPOSAL_KIND {
        return Err("fixed-time policy-head acceptance cannot accept another proposal kind".into());
    }
    accept_proposal_in_frontier_with_authority_at(
        frontier,
        proposal_id,
        reviewer,
        reason,
        DecisionAuthority::LocalKey(signing_key),
        None,
        Some(decided_at),
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn accept_proposal_in_frontier_with_authority_at(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    authority: DecisionAuthority<'_>,
    provenance: Option<&crate::provenance::Provenance>,
    fixed_decided_at: Option<&str>,
    record_review_event: bool,
) -> Result<String, String> {
    validate_reviewer_identity(reviewer)?;
    if reason.trim().is_empty() {
        return Err("Decision reason must be non-empty".to_string());
    }
    let index = frontier
        .proposals
        .iter()
        .position(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    let is_legacy_retirement =
        frontier.proposals[index].kind == policy_accept::LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND;
    if is_legacy_retirement && !authority.is_preview() {
        return Err(
            "legacy-policy retirement must use the recoverable human Decision Plan".to_string(),
        );
    }
    let status = frontier.proposals[index].status.clone();
    if status == "rejected" {
        return Err(format!("Cannot accept rejected proposal {}", proposal_id));
    }
    if status == "applied" {
        let proposal = frontier.proposals[index].clone();
        let decided_at = fixed_decided_at
            .map(ToString::to_string)
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        enforce_decision_authority(frontier, &proposal, reviewer, &decided_at, &authority)?;
        return frontier.proposals[index]
            .applied_event_id
            .clone()
            .ok_or_else(|| format!("Proposal {} is applied but has no event id", proposal_id));
    }
    let proposal = frontier.proposals[index].clone();
    validate_proposal_shape(frontier, &proposal)?;
    let decided_at = fixed_decided_at
        .map(ToString::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    enforce_decision_authority(frontier, &proposal, reviewer, &decided_at, &authority)?;
    let signing_key = authority.signing_key();
    if proposal.kind == policy_accept::POLICY_HEAD_PROPOSAL_KIND {
        if signing_key.is_none() && !record_review_event {
            return Err(
                "policy-head acceptance requires a real human event signature; custody-only or keyless review cannot activate authority"
                    .to_string(),
            );
        }
        if !(reviewer.starts_with("reviewer:") || reviewer.starts_with("steward:")) {
            return Err("policy-head acceptance requires a reviewer:/steward: actor".to_string());
        }
        let decided_at = decided_at.clone();
        let actor = validate_human_reviewer_authority_at(frontier, reviewer, &decided_at)?;
        if signing_key.is_some_and(|key| {
            !actor
                .public_key
                .eq_ignore_ascii_case(&hex::encode(key.verifying_key().to_bytes()))
        }) {
            return Err(
                "policy-head signer key does not match the resolved frontier actor".to_string(),
            );
        }
        let review_time = chrono::DateTime::parse_from_rfc3339(&decided_at)
            .map_err(|error| format!("policy-head decision time is invalid: {error}"))?;
        for parent in &frontier.events {
            let parent_time =
                chrono::DateTime::parse_from_rfc3339(&parent.timestamp).map_err(|error| {
                    format!("policy-head parent {} time is invalid: {error}", parent.id)
                })?;
            if parent_time >= review_time {
                return Err(format!(
                    "policy-head review must occur after causal parent {}",
                    parent.id
                ));
            }
        }
        let mut event = events::new_review_decision_event(
            &proposal.id,
            &proposal.kind,
            "accepted",
            None,
            reviewer,
            reason,
            Some(&decided_at),
        )?;
        if let Some(provenance) = provenance
            && !provenance.is_empty()
        {
            crate::provenance::attach_to_payload(&mut event.payload, provenance)?;
            event.id = events::compute_event_id(&event);
        }
        if let Some(key) = signing_key {
            event.signature = Some(crate::sign::sign_event(&event, key)?);
        }
        let event_id = event.id.clone();
        frontier.events.push(event);
        frontier.proposals[index].status = "applied".to_string();
        frontier.proposals[index].reviewed_by = Some(reviewer.to_string());
        frontier.proposals[index].reviewed_at = Some(decided_at);
        frontier.proposals[index].decision_reason = Some(reason.to_string());
        frontier.proposals[index].applied_event_id = Some(event_id.clone());
        mark_proof_stale(
            frontier,
            format!("Accepted policy-head proposal {}", proposal.id),
        );
        return Ok(event_id);
    }
    if is_legacy_retirement {
        // Retirement changes fixed governance files outside Project. The pure
        // Engine preview may simulate its review event, but every real
        // protocol acceptance path must fail closed: only vela-cli's private,
        // recoverable Decision Plan can bind this event to the atomic
        // Authority-file deletions.
        let mut event = events::new_review_decision_event(
            &proposal.id,
            &proposal.kind,
            "accepted",
            None,
            reviewer,
            reason,
            Some(&decided_at),
        )?;
        if let Some(provenance) = provenance
            && !provenance.is_empty()
        {
            crate::provenance::attach_to_payload(&mut event.payload, provenance)?;
            event.id = events::compute_event_id(&event);
        }
        let event_id = event.id.clone();
        frontier.events.push(event);
        frontier.proposals[index].status = "applied".to_string();
        frontier.proposals[index].reviewed_by = Some(reviewer.to_string());
        frontier.proposals[index].reviewed_at = Some(decided_at);
        frontier.proposals[index].decision_reason = Some(reason.to_string());
        frontier.proposals[index].applied_event_id = Some(event_id.clone());
        mark_proof_stale(
            frontier,
            format!("Accepted legacy-policy retirement proposal {}", proposal.id),
        );
        return Ok(event_id);
    }
    frontier.proposals[index].status = "accepted".to_string();
    frontier.proposals[index].reviewed_by = Some(reviewer.to_string());
    frontier.proposals[index].reviewed_at = Some(decided_at.clone());
    frontier.proposals[index].decision_reason = Some(reason.to_string());
    let domain_provenance = if record_review_event {
        None
    } else {
        provenance
    };
    let event_id = apply_proposal_at(
        frontier,
        &proposal,
        reviewer,
        reason,
        domain_provenance,
        fixed_decided_at,
    )?;
    frontier.proposals[index].status = "applied".to_string();
    frontier.proposals[index].applied_event_id = Some(event_id.clone());
    // Sign the accept event under the reviewer's key: the signature is
    // over the canonical event bytes (signature field excluded), so the
    // content-addressed id is unchanged and the accept is attributable
    // by cryptography, not by string.
    if let Some(key) = signing_key
        && let Some(ev) = frontier.events.iter_mut().find(|e| e.id == event_id)
    {
        ev.signature = Some(crate::sign::sign_event(ev, key)?);
    }
    if record_review_event {
        let decided_at = fixed_decided_at.ok_or_else(|| {
            "prepared acceptance requires a caller-bound decision time".to_string()
        })?;
        push_signed_review_event(
            frontier,
            &proposal.id,
            &proposal.kind,
            "accepted",
            Some(event_id.clone()),
            reviewer,
            reason,
            decided_at,
            signing_key,
            provenance,
        )?;
    }
    Ok(event_id)
}

/// Build, sign, and append a `review.*` decision event to the log. The
/// event is the tamper-evident, replayable record of the decision — the
/// thing a reject previously lacked entirely. Signed under the reviewer
/// key when present (custody is enforced by the caller before this runs),
/// so the decision is non-repudiable; the content-addressed id is over the
/// unsigned shape, so signing never changes it. `decided_at` is reused for
/// both the event timestamp and the proposal's `reviewed_at`, so the two
/// never diverge by a second clock read.
#[allow(clippy::too_many_arguments)]
fn push_signed_review_event(
    frontier: &mut Project,
    proposal_id: &str,
    proposal_kind: &str,
    verdict: &str,
    applied_event_id: Option<String>,
    reviewer: &str,
    reason: &str,
    decided_at: &str,
    signing_key: Option<&ed25519_dalek::SigningKey>,
    provenance: Option<&crate::provenance::Provenance>,
) -> Result<String, String> {
    let mut event = events::new_review_decision_event(
        proposal_id,
        proposal_kind,
        verdict,
        applied_event_id,
        reviewer,
        reason,
        Some(decided_at),
    )?;
    if let Some(provenance) = provenance
        && !provenance.is_empty()
    {
        crate::provenance::attach_to_payload(&mut event.payload, provenance)?;
        event.id = events::compute_event_id(&event);
    }
    if let Some(key) = signing_key {
        event.signature = Some(crate::sign::sign_event(&event, key)?);
    }
    let event_id = event.id.clone();
    frontier.events.push(event);
    mark_proof_stale(
        frontier,
        format!("Recorded review decision on proposal {proposal_id} after latest proof export"),
    );
    Ok(event_id)
}

#[allow(clippy::too_many_arguments)]
fn reject_proposal_in_frontier_signed_at(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    authority: DecisionAuthority<'_>,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<(), String> {
    validate_reviewer_identity(reviewer)?;
    // A reject is a truth-bearing review verdict with no process exception:
    // burying a proposal is as much a decision as applying one. It is
    // reserved for a registered human reviewer executing a Decision Plan.
    if reviewer.starts_with("agent:") || reviewer.starts_with("ci:") {
        return Err(format!(
            "reviewer '{reviewer}' may not reject proposals: review decisions are \
             reserved for named human reviewers (key custody). Agents may propose, \
             attach mechanical evidence, or draft — never decide."
        ));
    }
    if reason.trim().is_empty() {
        return Err("Decision reason must be non-empty".to_string());
    }
    chrono::DateTime::parse_from_rfc3339(decided_at)
        .map_err(|error| format!("decision time is invalid: {error}"))?;
    let index = frontier
        .proposals
        .iter()
        .position(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    match frontier.proposals[index].status.as_str() {
        "pending_review" | "accepted" | "needs_revision" => {}
        "rejected" => {
            return Err(format!("Proposal {} is already rejected", proposal_id));
        }
        "applied" => {
            return Err(format!("Proposal {} is already applied", proposal_id));
        }
        other => {
            return Err(format!("Unsupported proposal status '{}'", other));
        }
    }
    let proposal_kind = frontier.proposals[index].kind.clone();
    let proposal = frontier.proposals[index].clone();
    enforce_decision_authority(frontier, &proposal, reviewer, decided_at, &authority)?;
    let signing_key = authority.signing_key();
    frontier.proposals[index].status = "rejected".to_string();
    frontier.proposals[index].reviewed_by = Some(reviewer.to_string());
    frontier.proposals[index].reviewed_at = Some(decided_at.to_string());
    frontier.proposals[index].decision_reason = Some(reason.to_string());
    push_signed_review_event(
        frontier,
        proposal_id,
        &proposal_kind,
        "rejected",
        None,
        reviewer,
        reason,
        decided_at,
        signing_key,
        provenance,
    )?;
    Ok(())
}

fn prepared_decision_mutation(
    frontier: &Project,
    first_event: usize,
    primary_event_id: String,
    proposal_id: &str,
    reviewer: &str,
) -> Result<PreparedDecisionMutation, String> {
    let appended = frontier
        .events
        .get(first_event..)
        .ok_or_else(|| "prepared decision event range is invalid".to_string())?;
    let appended_event_ids = appended
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let decision_event_id = appended
        .iter()
        .rev()
        .find(|event| {
            event.target.r#type == "proposal"
                && event.target.id == proposal_id
                && matches!(
                    event.kind.as_str(),
                    events::EVENT_KIND_REVIEW_ACCEPTED | events::EVENT_KIND_REVIEW_REJECTED
                )
        })
        .map(|event| event.id.clone())
        .ok_or_else(|| "prepared decision did not append a review decision event".to_string())?;
    let prepared = PreparedDecisionMutation {
        primary_event_id,
        appended_event_ids,
        decision_event_id,
        proposal_id: proposal_id.to_string(),
        reviewer: reviewer.to_string(),
        first_event,
        event_count_after: frontier.events.len(),
        binding: PreparedDecisionBinding::Unbound,
    };
    validate_prepared_decision_invariants(frontier, &prepared, true)?;
    Ok(prepared)
}

fn clone_project(project: &Project) -> Result<Project, String> {
    serde_json::from_value(serde_json::to_value(project).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn decision_root_refs(event: &StateEvent) -> Result<Vec<String>, String> {
    let Some(provenance) = event.payload.get("provenance") else {
        return Ok(Vec::new());
    };
    let object = provenance
        .as_object()
        .ok_or_else(|| format!("event {} provenance must be an object", event.id))?;
    let Some(input_refs) = object.get("input_refs") else {
        return Ok(Vec::new());
    };
    let input_refs = input_refs
        .as_array()
        .ok_or_else(|| format!("event {} provenance.input_refs must be an array", event.id))?;
    let mut refs = Vec::new();
    for value in input_refs {
        let value = value.as_str().ok_or_else(|| {
            format!(
                "event {} provenance.input_refs must contain only strings",
                event.id
            )
        })?;
        if value.starts_with(crate::provenance::DECISION_ROOT_INPUT_REF_PREFIX) {
            refs.push(value.to_string());
        }
    }
    Ok(refs)
}

fn validate_prepared_decision_invariants(
    frontier: &Project,
    prepared: &PreparedDecisionMutation,
    require_unsigned: bool,
) -> Result<(), String> {
    if prepared.appended_event_ids.is_empty() {
        return Err("prepared decision has no appended events".to_string());
    }
    if prepared.first_event + prepared.appended_event_ids.len() != prepared.event_count_after
        || prepared.event_count_after > frontier.events.len()
    {
        return Err("prepared decision event range is not contiguous".to_string());
    }
    let events = &frontier.events[prepared.first_event..prepared.event_count_after];
    let exact_ids = events
        .iter()
        .map(|event| event.id.as_str())
        .collect::<Vec<_>>();
    if exact_ids
        != prepared
            .appended_event_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    {
        return Err("prepared decision ids do not match their contiguous event range".to_string());
    }
    let unique_ids = prepared.appended_event_ids.iter().collect::<BTreeSet<_>>();
    if unique_ids.len() != prepared.appended_event_ids.len() {
        return Err("prepared decision contains duplicate event ids".to_string());
    }
    if !unique_ids.contains(&prepared.primary_event_id)
        || !unique_ids.contains(&prepared.decision_event_id)
    {
        return Err(
            "prepared primary and decision events must belong to the exact event set".to_string(),
        );
    }
    for event_id in &prepared.appended_event_ids {
        if frontier
            .events
            .iter()
            .filter(|event| event.id == *event_id)
            .count()
            != 1
        {
            return Err(format!(
                "prepared decision event id is not globally unique: {event_id}"
            ));
        }
    }

    let proposals = frontier
        .proposals
        .iter()
        .filter(|proposal| proposal.id == prepared.proposal_id)
        .collect::<Vec<_>>();
    if proposals.len() != 1 {
        return Err(format!(
            "prepared proposal {} is missing or duplicated",
            prepared.proposal_id
        ));
    }
    let proposal = proposals[0];
    let mut decision_events = 0_usize;
    for event in events {
        if event.id != events::compute_event_id(event) {
            return Err(format!("prepared event {} id does not rederive", event.id));
        }
        if event.actor.id != prepared.reviewer
            || event.actor.r#type != events::actor_kind(&prepared.reviewer)
        {
            return Err(format!(
                "prepared event {} actor does not match reviewer {}",
                event.id, prepared.reviewer
            ));
        }
        if event.payload.get("proposal_id").and_then(Value::as_str)
            != Some(prepared.proposal_id.as_str())
        {
            return Err(format!(
                "prepared event {} does not bind proposal {}",
                event.id, prepared.proposal_id
            ));
        }
        if require_unsigned && event.signature.is_some() {
            return Err(format!("prepared event {} is already signed", event.id));
        }
        let refs = decision_root_refs(event)?;
        let is_review_decision = event.target.r#type == "proposal"
            && event.target.id == prepared.proposal_id
            && matches!(
                event.kind.as_str(),
                events::EVENT_KIND_REVIEW_ACCEPTED | events::EVENT_KIND_REVIEW_REJECTED
            );
        if is_review_decision {
            decision_events += 1;
            if event.id != prepared.decision_event_id {
                return Err("prepared set contains an untracked review decision event".to_string());
            }
        }
        if event.id == prepared.decision_event_id {
            if !is_review_decision {
                return Err("prepared decision event has inconsistent target or kind".to_string());
            }
            match &prepared.binding {
                PreparedDecisionBinding::Unbound if !refs.is_empty() => {
                    return Err(
                        "unbound prepared decision already carries a decision-root reference"
                            .to_string(),
                    );
                }
                PreparedDecisionBinding::Unbound => {}
                PreparedDecisionBinding::Bound { decision_root } => {
                    let expected = crate::provenance::decision_root_input_ref(decision_root)?;
                    if refs != [expected] {
                        return Err(
                            "bound prepared decision must carry exactly one canonical decision-root reference"
                                .to_string(),
                        );
                    }
                }
            }
        } else if !refs.is_empty() {
            return Err(format!(
                "non-decision event {} carries a decision-root reference",
                event.id
            ));
        }
    }
    if decision_events != 1 {
        return Err("prepared decision must contain exactly one review decision event".to_string());
    }
    let decision_event = events
        .iter()
        .find(|event| event.id == prepared.decision_event_id)
        .expect("membership checked above");
    if decision_event
        .payload
        .get("proposal_kind")
        .and_then(Value::as_str)
        != Some(proposal.kind.as_str())
    {
        return Err("prepared decision event proposal kind is inconsistent".to_string());
    }
    if proposal.reviewed_by.as_deref() != Some(prepared.reviewer.as_str())
        || proposal.reviewed_at.as_deref() != Some(decision_event.timestamp.as_str())
    {
        return Err(
            "prepared proposal review metadata does not match its decision event".to_string(),
        );
    }
    match proposal.status.as_str() {
        "applied"
            if proposal.applied_event_id.as_deref() == Some(prepared.primary_event_id.as_str())
                && decision_event.kind == events::EVENT_KIND_REVIEW_ACCEPTED => {}
        "rejected"
            if proposal.applied_event_id.is_none()
                && decision_event.kind == events::EVENT_KIND_REVIEW_REJECTED => {}
        _ => {
            return Err(
                "prepared proposal status/applied_event_id is inconsistent with its primary event"
                    .to_string(),
            );
        }
    }
    Ok(())
}

/// Apply one exact accept to an in-memory project without receiving or reading
/// a private key. The caller supplies the sole timestamp used by the proposal
/// status, every semantic state timestamp, and every appended event.
///
/// This mutates only the provided `Project`. It always appends an unsigned
/// `review.accepted` event in addition to any domain events. After the final
/// plan root is known, bind it with [`bind_decision_root_to_prepared`]; then,
/// after a locked rederivation, sign exactly the returned event set with
/// [`sign_prepared_decision_events`]. Legacy policy retirement is deliberately
/// excluded because its accepted event is valid only when atomically journaled
/// with fixed Authority-file deletions by vela-cli's private Decision Plan.
pub fn prepare_proposal_accept_in_memory_at(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<PreparedDecisionMutation, String> {
    let mut candidate = clone_project(frontier)?;
    let prepared = prepare_proposal_accept_on_candidate(
        &mut candidate,
        proposal_id,
        reviewer,
        reason,
        provenance,
        decided_at,
    )?;
    *frontier = candidate;
    Ok(prepared)
}

fn prepare_proposal_accept_on_candidate(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<PreparedDecisionMutation, String> {
    if frontier.proposals.iter().any(|proposal| {
        proposal.id == proposal_id
            && proposal.kind == policy_accept::LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND
    }) {
        return Err(
            "legacy-policy retirement cannot use a public protocol preparation API; use the recoverable human Decision Plan"
                .to_string(),
        );
    }
    validate_human_reviewer_authority_at(frontier, reviewer, decided_at)?;
    let first_event = frontier.events.len();
    let primary_event_id = accept_proposal_in_frontier_with_authority_at(
        frontier,
        proposal_id,
        reviewer,
        reason,
        DecisionAuthority::PlanPreparation,
        provenance,
        Some(decided_at),
        true,
    )?;
    crate::sources::materialize_project(frontier);
    prepared_decision_mutation(
        frontier,
        first_event,
        primary_event_id,
        proposal_id,
        reviewer,
    )
}

/// Clone-only convenience wrapper for the first item in a Decision Plan.
/// Callers can chain later items by applying the mutating in-memory seam to the
/// returned candidate; the input project is never changed. The same legacy
/// policy retirement exclusion as [`prepare_proposal_accept_in_memory_at`]
/// applies.
pub fn prepare_proposal_accept_candidate_at(
    frontier: &Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<(Project, PreparedDecisionMutation), String> {
    let mut candidate = clone_project(frontier)?;
    let prepared = prepare_proposal_accept_on_candidate(
        &mut candidate,
        proposal_id,
        reviewer,
        reason,
        provenance,
        decided_at,
    )?;
    Ok((candidate, prepared))
}

/// Build an ordinary scientific acceptance candidate for an Era-1
/// repository-authority transaction.
///
/// The returned project and events are unsigned semantic postimages only.
/// This function deliberately cannot activate policy heads or legacy-policy
/// retirement. It confers no authority: callers must bind the exact candidate
/// to authenticated principal, Cedar authorization, semantic approval,
/// repository-authority event coverage, and one recoverable transaction.
pub fn prepare_repository_authority_accept_candidate_at(
    frontier: &Project,
    proposal_id: &str,
    principal_id: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<(Project, PreparedDecisionMutation), String> {
    let proposal = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    if matches!(
        proposal.kind.as_str(),
        policy_accept::POLICY_HEAD_PROPOSAL_KIND
            | policy_accept::LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND
    ) {
        return Err(
            "repository-authority scientific acceptance cannot administer legacy policy".into(),
        );
    }
    let mut candidate = clone_project(frontier)?;
    let first_event = candidate.events.len();
    let primary_event_id = accept_proposal_in_frontier_with_authority_at(
        &mut candidate,
        proposal_id,
        principal_id,
        reason,
        DecisionAuthority::RepositoryPreparation,
        provenance,
        Some(decided_at),
        true,
    )?;
    crate::sources::materialize_project(&mut candidate);
    let prepared = prepared_decision_mutation(
        &candidate,
        first_event,
        primary_event_id,
        proposal_id,
        principal_id,
    )?;
    Ok((candidate, prepared))
}

/// Apply one exact reject to an in-memory project without receiving or reading
/// a private key. The result is an unsigned, fixed-time `review.rejected`
/// event, optionally carrying a decision-root provenance input reference.
pub fn prepare_proposal_reject_in_memory_at(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<PreparedDecisionMutation, String> {
    let mut candidate = clone_project(frontier)?;
    let prepared = prepare_proposal_reject_on_candidate(
        &mut candidate,
        proposal_id,
        reviewer,
        reason,
        provenance,
        decided_at,
    )?;
    *frontier = candidate;
    Ok(prepared)
}

fn prepare_proposal_reject_on_candidate(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<PreparedDecisionMutation, String> {
    validate_human_reviewer_authority_at(frontier, reviewer, decided_at)?;
    let first_event = frontier.events.len();
    reject_proposal_in_frontier_signed_at(
        frontier,
        proposal_id,
        reviewer,
        reason,
        DecisionAuthority::PlanPreparation,
        provenance,
        decided_at,
    )?;
    let primary_event_id = frontier
        .events
        .get(first_event)
        .map(|event| event.id.clone())
        .ok_or_else(|| "prepared reject appended no event".to_string())?;
    prepared_decision_mutation(
        frontier,
        first_event,
        primary_event_id,
        proposal_id,
        reviewer,
    )
}

/// Clone-only reject counterpart to
/// [`prepare_proposal_accept_candidate_at`].
pub fn prepare_proposal_reject_candidate_at(
    frontier: &Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    decided_at: &str,
) -> Result<(Project, PreparedDecisionMutation), String> {
    let mut candidate = clone_project(frontier)?;
    let prepared = prepare_proposal_reject_on_candidate(
        &mut candidate,
        proposal_id,
        reviewer,
        reason,
        provenance,
        decided_at,
    )?;
    Ok((candidate, prepared))
}

/// Insert the final decision root into the prepared review event and rederive
/// its content address before any signature is made.
///
/// Only the dedicated `review.*` event carries this input reference. Domain and
/// fanout event IDs therefore remain stable, including the
/// `review.accepted.payload.applied_event_id` pointer, and the Decision Plan has
/// no hash cycle. Policy-head acceptance uses its review event as the primary
/// event, so this helper also updates the proposal's cached applied-event id.
pub fn bind_decision_root_to_prepared(
    frontier: &mut Project,
    prepared: &mut PreparedDecisionMutation,
    decision_root: &str,
) -> Result<(), String> {
    crate::provenance::decision_root_input_ref(decision_root)?;
    match prepared.decision_root() {
        Some(existing) if existing == decision_root => {
            return validate_prepared_decision_invariants(frontier, prepared, true);
        }
        Some(_) => {
            return Err(
                "prepared decision is already bound; rebuild it before changing the root"
                    .to_string(),
            );
        }
        None => {}
    }
    let mut candidate = clone_project(frontier)?;
    let mut candidate_prepared = prepared.clone();
    bind_decision_root_to_prepared_inner(&mut candidate, &mut candidate_prepared, decision_root)?;
    validate_prepared_decision_invariants(&candidate, &candidate_prepared, true)?;
    *frontier = candidate;
    *prepared = candidate_prepared;
    Ok(())
}

fn bind_decision_root_to_prepared_inner(
    frontier: &mut Project,
    prepared: &mut PreparedDecisionMutation,
    decision_root: &str,
) -> Result<(), String> {
    validate_prepared_decision_invariants(frontier, prepared, true)?;
    let old_event_id = prepared.decision_event_id.clone();
    let index = frontier
        .events
        .iter()
        .position(|event| event.id == old_event_id)
        .ok_or_else(|| format!("prepared decision event not found: {old_event_id}"))?;
    let event = &frontier.events[index];
    if event.signature.is_some() {
        return Err("cannot bind a decision root after the decision event is signed".to_string());
    }
    if !matches!(
        event.kind.as_str(),
        events::EVENT_KIND_REVIEW_ACCEPTED | events::EVENT_KIND_REVIEW_REJECTED
    ) {
        return Err(format!(
            "prepared decision event {} has non-review kind '{}'",
            event.id, event.kind
        ));
    }
    let mut candidate_event: StateEvent =
        serde_json::from_value(serde_json::to_value(event).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let mut provenance = candidate_event
        .payload
        .get("provenance")
        .cloned()
        .map(serde_json::from_value::<crate::provenance::Provenance>)
        .transpose()
        .map_err(|error| format!("prepared decision provenance is invalid: {error}"))?
        .unwrap_or_default();
    provenance.bind_decision_root(decision_root)?;
    crate::provenance::attach_to_payload(&mut candidate_event.payload, &provenance)?;
    candidate_event.id = events::compute_event_id(&candidate_event);
    let new_event_id = candidate_event.id.clone();

    if frontier
        .events
        .iter()
        .enumerate()
        .any(|(other_index, other)| other_index != index && other.id == new_event_id)
    {
        return Err(format!(
            "decision-root binding collides with existing event {new_event_id}"
        ));
    }
    frontier.events[index] = candidate_event;
    for proposal in &mut frontier.proposals {
        if proposal.applied_event_id.as_deref() == Some(old_event_id.as_str()) {
            proposal.applied_event_id = Some(new_event_id.clone());
        }
    }
    for event_id in &mut prepared.appended_event_ids {
        if *event_id == old_event_id {
            *event_id = new_event_id.clone();
        }
    }
    if prepared.primary_event_id == old_event_id {
        prepared.primary_event_id = new_event_id.clone();
    }
    prepared.decision_event_id = new_event_id;
    prepared.binding = PreparedDecisionBinding::Bound {
        decision_root: decision_root.to_string(),
    };
    Ok(())
}

/// Sign the exact unsigned events returned by a prepared decision mutation.
///
/// Every event is validated before any signature is written, so a malformed or
/// stale set produces zero signature delta. This function performs no I/O and
/// no clock read; key resolution and human confirmation remain CLI concerns.
pub fn sign_prepared_decision_events(
    frontier: &mut Project,
    prepared: &PreparedDecisionMutation,
    reviewer: &str,
    signing_key: &ed25519_dalek::SigningKey,
) -> Result<(), String> {
    if reviewer != prepared.reviewer {
        return Err(format!(
            "prepared reviewer '{}' does not match signing reviewer '{reviewer}'",
            prepared.reviewer
        ));
    }
    if !prepared.is_bound() {
        return Err("prepared decision must be decision-root bound before signing".to_string());
    }
    validate_prepared_decision_invariants(frontier, prepared, true)?;
    let decision_event = &frontier.events[prepared.first_event
        + prepared
            .appended_event_ids
            .iter()
            .position(|event_id| event_id == &prepared.decision_event_id)
            .expect("prepared invariant checks decision-event membership")];
    let actor =
        validate_human_reviewer_authority_at(frontier, reviewer, &decision_event.timestamp)?;
    let derived = hex::encode(signing_key.verifying_key().to_bytes());
    if !derived.eq_ignore_ascii_case(&actor.public_key) {
        return Err(format!(
            "the supplied key does not match {reviewer}'s registered decision key"
        ));
    }
    let indexes = (prepared.first_event..prepared.event_count_after).collect::<Vec<_>>();
    let signatures = indexes
        .iter()
        .map(|index| crate::sign::sign_event(&frontier.events[*index], signing_key))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, signature) in indexes.into_iter().zip(signatures) {
        frontier.events[index].signature = Some(signature);
    }
    Ok(())
}

/// Validate the exact event set before or after an external signer operates.
///
/// This is the seed-free counterpart to [`sign_prepared_decision_events`]. It
/// exposes no authority surface: the caller still has to obtain Ed25519
/// signatures and every returned signature is verified by the decision
/// executor before a frontier write. The helper exists so a separate custody
/// process can sign without returning private key material to `vela`.
pub fn validate_prepared_decision_for_external_signing(
    frontier: &Project,
    prepared: &PreparedDecisionMutation,
    reviewer: &str,
    require_unsigned: bool,
) -> Result<Vec<StateEvent>, String> {
    if reviewer != prepared.reviewer {
        return Err(format!(
            "prepared reviewer '{}' does not match signing reviewer '{reviewer}'",
            prepared.reviewer
        ));
    }
    if !prepared.is_bound() {
        return Err("prepared decision must be decision-root bound before signing".to_string());
    }
    validate_prepared_decision_invariants(frontier, prepared, require_unsigned)?;
    let decision_event = &frontier.events[prepared.first_event
        + prepared
            .appended_event_ids
            .iter()
            .position(|event_id| event_id == &prepared.decision_event_id)
            .expect("prepared invariant checks decision-event membership")];
    validate_human_reviewer_authority_at(frontier, reviewer, &decision_event.timestamp)?;
    Ok((prepared.first_event..prepared.event_count_after)
        .map(|index| frontier.events[index].clone())
        .collect())
}

pub(crate) fn apply_proposal(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    decision_reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
) -> Result<String, String> {
    apply_proposal_at(
        frontier,
        proposal,
        reviewer,
        decision_reason,
        provenance,
        None,
    )
}

fn apply_proposal_at(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    decision_reason: &str,
    provenance: Option<&crate::provenance::Provenance>,
    fixed_decided_at: Option<&str>,
) -> Result<String, String> {
    // Phase L: retraction emits a fan of events — one for the source
    // and one `finding.dependency_invalidated` per dependent in BFS
    // order. apply_retract is responsible for pushing all of them in
    // sequence; this branch only assigns the primary event ID.
    if proposal.kind.as_str() == "finding.retract" {
        let events = apply_retract(
            frontier,
            proposal,
            reviewer,
            decision_reason,
            fixed_decided_at,
        )?;
        let primary_id = events
            .first()
            .map(|event| event.id.clone())
            .ok_or_else(|| "apply_retract returned no events".to_string())?;
        for event in events {
            frontier.events.push(event);
        }
        mark_proof_stale(
            frontier,
            format!("Applied proposal {} after latest proof export", proposal.id),
        );
        return Ok(primary_id);
    }
    // v0.55: confidence_revise can also fan out a cascade when the new
    // score crosses below the 0.5 propagation threshold. Same fan-out
    // pattern as retract.
    if proposal.kind.as_str() == "finding.confidence_revise" {
        let events = apply_confidence_revise(
            frontier,
            proposal,
            reviewer,
            decision_reason,
            fixed_decided_at,
        )?;
        let primary_id = events
            .first()
            .map(|event| event.id.clone())
            .ok_or_else(|| "apply_confidence_revise returned no events".to_string())?;
        for event in events {
            frontier.events.push(event);
        }
        mark_proof_stale(
            frontier,
            format!("Applied proposal {} after latest proof export", proposal.id),
        );
        return Ok(primary_id);
    }
    let mut event = match proposal.kind.as_str() {
        "finding.add" => apply_add(frontier, proposal, reviewer, decision_reason)?,
        "finding.review" => apply_review(frontier, proposal, reviewer, decision_reason)?,
        "research_trace.review" | "correction_return.review" => {
            apply_frontier_observation_review(proposal, reviewer, decision_reason)?
        }
        "finding.caveat" => apply_caveat(
            frontier,
            proposal,
            reviewer,
            decision_reason,
            fixed_decided_at,
        )?,
        "finding.note" => apply_note(
            frontier,
            proposal,
            reviewer,
            decision_reason,
            fixed_decided_at,
        )?,
        "finding.reject" => apply_reject(frontier, proposal, reviewer, decision_reason)?,
        "finding.contribution.recorded" => {
            apply_contribution(frontier, proposal, reviewer, decision_reason)?
        }
        "finding.supersede" => apply_supersede(
            frontier,
            proposal,
            reviewer,
            decision_reason,
            fixed_decided_at,
        )?,
        "artifact.assert" => apply_artifact_assert(frontier, proposal, reviewer, decision_reason)?,
        "artifact.retract" => {
            apply_artifact_retract(frontier, proposal, reviewer, decision_reason)?
        }
        "verifier.attach" => apply_verifier_attach(frontier, proposal, reviewer, decision_reason)?,
        // v0.56: mechanical evidence-atom locator repair.
        "evidence_atom.locator_repair" => {
            apply_evidence_atom_locator_repair(frontier, proposal, reviewer, decision_reason)?
        }
        // v0.57: mechanical finding-level span repair.
        "finding.span_repair" => {
            apply_finding_span_repair(frontier, proposal, reviewer, decision_reason)?
        }
        other => return Err(format!("Unsupported proposal kind '{other}'")),
    };
    if let Some(decided_at) = fixed_decided_at {
        event.timestamp = decided_at.to_string();
        event.id = events::compute_event_id(&event);
    }
    // Co-authorship: when a non-human (an AI that drafted, CI that attested)
    // contributed, record it as signed-over attribution on this single decision
    // event. The reviewer stays the accountable signer; the provenance carries
    // zero authority (validated non-human in `attach_to_payload`). Because the
    // block enters the signed payload, the content-addressed id is re-derived.
    // None leaves the event byte-identical, so existing frontiers are untouched.
    if let Some(prov) = provenance
        && !prov.is_empty()
    {
        crate::provenance::attach_to_payload(&mut event.payload, prov)?;
        event.id = events::event_id(&event);
    }
    let event_id = event.id.clone();
    frontier.events.push(event);
    mark_proof_stale(
        frontier,
        format!("Applied proposal {} after latest proof export", proposal.id),
    );
    Ok(event_id)
}

fn apply_frontier_observation_review(
    proposal: &StateProposal,
    reviewer: &str,
    decision_reason: &str,
) -> Result<StateEvent, String> {
    match proposal.kind.as_str() {
        "research_trace.review" => validate_research_trace_review_payload(proposal)?,
        "correction_return.review" => validate_correction_return_review_payload(proposal)?,
        other => {
            return Err(format!(
                "Unsupported frontier observation proposal kind '{other}'"
            ));
        }
    }
    let mut event = StateEvent {
        schema: events::EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: events::EVENT_KIND_FRONTIER_OBSERVATION_REVIEWED.into(),
        target: proposal.target.clone(),
        actor: StateActor {
            id: reviewer.to_string(),
            r#type: "human".to_string(),
        },
        timestamp: Utc::now().to_rfc3339(),
        reason: proposal.reason.clone(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({
            "proposal_id": proposal.id,
            "proposal_kind": proposal.kind,
            "status": "accepted",
            "decision_reason": decision_reason,
            "reviewed_payload": proposal.payload,
            "source_refs": proposal.source_refs,
        }),
        caveats: proposal.caveats.clone(),
        signature: None,
    };
    events::validate_event_payload(event.kind.as_str(), &event.payload)?;
    event.id = events::compute_event_id(&event);
    Ok(event)
}

/// v0.14: `finding.supersede` — first-class flow for *changing a claim's text*.
///
/// Until v0.14 the only way to update a finding was to stack caveats/notes
/// on top, because the assertion text is part of the content address. The
/// substrate-correct path for a real correction is a *new* content-addressed
/// finding that explicitly supersedes the old one. This proposal kind:
///
/// 1. Validates the old finding exists and is not already superseded.
/// 2. Adds the new finding bundle (a fresh `vf_…` content address) to
///    `frontier.findings`.
/// 3. Auto-injects a `supersedes` link from the new finding's `links` to the
///    old finding's id (if not already present in the payload).
/// 4. Sets `flags.superseded = true` on the old finding.
/// 5. Emits a `finding.superseded` canonical event targeting the *old*
///    finding (since that's the state change). The new finding's existence
///    is recorded in the event payload as `new_finding_id`.
///
/// Both findings remain queryable; readers walk the supersedes chain via
/// the link or via the `flags.superseded` marker.
fn apply_supersede(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
    fixed_decided_at: Option<&str>,
) -> Result<StateEvent, String> {
    use crate::bundle::Link;

    let old_id = proposal.target.id.clone();
    let new_finding_value = proposal
        .payload
        .get("new_finding")
        .ok_or("finding.supersede proposal missing payload.new_finding")?
        .clone();
    let mut new_finding: FindingBundle = serde_json::from_value(new_finding_value)
        .map_err(|e| format!("Invalid finding.supersede payload.new_finding: {e}"))?;

    // Locate the old finding before mutating; capture before_hash for the event.
    let old_idx = find_finding_index(frontier, &old_id)?;
    if frontier.findings[old_idx].flags.superseded {
        return Err(format!(
            "Refusing to supersede already-superseded finding {old_id}"
        ));
    }
    if new_finding.id == old_id {
        return Err(
            "Refusing to supersede with a finding that has the same content address as the old finding (assertion / type / provenance_id are unchanged)".to_string(),
        );
    }
    if frontier
        .findings
        .iter()
        .any(|existing| existing.id == new_finding.id)
    {
        return Err(format!(
            "Refusing to add superseding finding with existing finding ID {}",
            new_finding.id
        ));
    }
    let before_hash = events::finding_hash(&frontier.findings[old_idx]);

    // Auto-inject the supersedes link if the caller didn't already include it.
    let already_links_old = new_finding
        .links
        .iter()
        .any(|l| l.target == old_id && l.link_type == "supersedes");
    if !already_links_old {
        new_finding.links.push(Link {
            target: old_id.clone(),
            link_type: "supersedes".to_string(),
            note: format!(
                "Supersedes {old_id} via finding.supersede proposal {}.",
                proposal.id
            ),
            inferred_by: "reviewer".to_string(),
            created_at: fixed_decided_at
                .map(ToString::to_string)
                .unwrap_or_else(|| Utc::now().to_rfc3339()),
            mechanism: None,
        });
    }

    let new_finding_id = new_finding.id.clone();
    frontier.findings.push(new_finding);
    frontier.findings[old_idx].flags.superseded = true;
    let after_hash = events::finding_hash(&frontier.findings[old_idx]);

    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.superseded",
        finding_id: &old_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
            "new_finding_id": new_finding_id,
        }),
        caveats: proposal.caveats.clone(),
        timestamp: fixed_decided_at,
    }))
}

fn apply_add(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_value = proposal
        .payload
        .get("finding")
        .ok_or("finding.add proposal missing payload.finding")?
        .clone();
    let finding: FindingBundle = serde_json::from_value(finding_value)
        .map_err(|e| format!("Invalid finding.add payload: {e}"))?;
    let finding_id = finding.id.clone();
    // Activity is not state: an accepted finding may not depend on an
    // activity-plane id (`vac_`/`vrr_`). A search/trace/retrieval is recorded in
    // the activity plane and referenced by content address, never admitted as
    // accepted lineage (the `activity::assert_not_in_lineage` law, at the write).
    if let Some(l) = finding
        .links
        .iter()
        .find(|l| crate::activity::is_activity_id(&l.target))
    {
        return Err(format!(
            "finding.add refused: link target `{}` is an activity-plane id; activity is non-authoritative and cannot enter lineage",
            l.target
        ));
    }
    if frontier
        .findings
        .iter()
        .any(|existing| existing.id == finding_id)
    {
        return Err(format!(
            "Refusing to add duplicate finding with existing finding ID {finding_id}"
        ));
    }
    // Prior-art collision: an EXACT duplicate of an accepted finding's
    // statement is refused unless the proposal names what it supersedes
    // (the Sakana rediscovery failure mode, made mechanical).
    {
        let new_hash = crate::canonical::normalized_statement_hash(&finding.assertion.text);
        let declares_supersession = proposal.payload.get("supersedes").is_some()
            || proposal.payload.get("improves_on").is_some();
        if !declares_supersession
            && let Some(dup) = frontier.findings.iter().find(|f| {
                crate::canonical::normalized_statement_hash(&f.assertion.text) == new_hash
            })
        {
            return Err(format!(
                "prior-art collision: statement duplicates accepted finding {} — name it via payload.supersedes/improves_on or change the claim",
                dup.id
            ));
        }
    }
    frontier.findings.push(finding);
    let after_hash = events::finding_hash_by_id(frontier, &finding_id);
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.asserted",
        finding_id: &finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: NULL_HASH,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
        }),
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

fn apply_artifact_assert(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let artifact_value = proposal
        .payload
        .get("artifact")
        .ok_or("artifact.assert proposal missing payload.artifact")?
        .clone();
    let artifact: Artifact = serde_json::from_value(artifact_value)
        .map_err(|e| format!("Invalid artifact.assert payload: {e}"))?;
    let artifact_id = artifact.id.clone();
    if frontier
        .artifacts
        .iter()
        .any(|existing| existing.id == artifact_id)
    {
        return Err(format!(
            "Refusing to add duplicate artifact with existing id {artifact_id}"
        ));
    }
    frontier.artifacts.push(artifact.clone());
    let mut event = StateEvent {
        schema: events::EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: events::EVENT_KIND_ARTIFACT_ASSERTED.into(),
        target: StateTarget {
            r#type: "artifact".to_string(),
            id: artifact_id,
        },
        actor: StateActor {
            id: reviewer.to_string(),
            r#type: if reviewer.starts_with("agent:") {
                "agent"
            } else {
                "human"
            }
            .to_string(),
        },
        timestamp: Utc::now().to_rfc3339(),
        reason: proposal.reason.clone(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({
            "proposal_id": proposal.id,
            "artifact": artifact,
        }),
        caveats: proposal.caveats.clone(),
        signature: None,
    };
    events::validate_event_payload(event.kind.as_str(), &event.payload)?;
    event.id = events::compute_event_id(&event);
    Ok(event)
}

fn apply_artifact_retract(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let artifact_id = proposal.target.id.as_str();
    let artifact = frontier
        .artifacts
        .iter_mut()
        .find(|artifact| artifact.id == artifact_id)
        .ok_or_else(|| format!("Artifact not found: {artifact_id}"))?;
    if artifact.retracted {
        return Err(format!("Artifact {artifact_id} is already retracted"));
    }
    artifact.retracted = true;
    let mut event = StateEvent {
        schema: events::EVENT_SCHEMA.to_string(),
        id: String::new(),
        kind: events::EVENT_KIND_ARTIFACT_RETRACTED.into(),
        target: StateTarget {
            r#type: "artifact".to_string(),
            id: artifact_id.to_string(),
        },
        actor: StateActor {
            id: reviewer.to_string(),
            r#type: events::actor_kind(reviewer).to_string(),
        },
        timestamp: Utc::now().to_rfc3339(),
        reason: proposal.reason.clone(),
        before_hash: NULL_HASH.to_string(),
        after_hash: NULL_HASH.to_string(),
        payload: json!({
            "proposal_id": proposal.id,
        }),
        caveats: proposal.caveats.clone(),
        signature: None,
    };
    events::validate_event_payload(event.kind.as_str(), &event.payload)?;
    event.id = events::compute_event_id(&event);
    Ok(event)
}

/// Bind a verifier attachment to a finding (`target.type == "finding"`). Appends
/// to the sidecar `verifier_attachments` collection and emits
/// `verifier_attachment.added`. Per-finding trust-gate status is derived on read.
pub fn append_proposal_verifier_attachment(
    frontier: &mut Project,
    proposal_id: &str,
    attachment: crate::verifier_attachment::VerifierAttachment,
    verifier_actor: &str,
    timestamp: &str,
    key: &ed25519_dalek::SigningKey,
) -> Result<StateEvent, String> {
    let proposal = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("proposal {proposal_id} does not exist"))?;
    if proposal.status != "pending_review" {
        return Err(format!(
            "proposal {proposal_id} is {}, not pending_review",
            proposal.status
        ));
    }
    if proposal.kind != "finding.add" || proposal.target.r#type != "finding" {
        return Err(format!(
            "proposal-scoped verifier evidence requires a pending finding.add proposal, got {} targeting {}",
            proposal.kind, proposal.target.r#type
        ));
    }
    let finding: FindingBundle = serde_json::from_value(
        proposal
            .payload
            .get("finding")
            .ok_or("finding.add proposal missing payload.finding")?
            .clone(),
    )
    .map_err(|error| format!("parse proposal finding: {error}"))?;
    attachment.verify()?;
    if attachment.target != finding.id || proposal.target.id != finding.id {
        return Err(format!(
            "attachment target {} does not bind proposal finding {}",
            attachment.target, finding.id
        ));
    }
    let expected_digest = crate::verifier_attachment::claim_digest(&finding.assertion.text);
    if attachment.claim_digest != expected_digest {
        return Err(format!(
            "attachment claim_digest is stale: expected {expected_digest}, got {}",
            attachment.claim_digest
        ));
    }
    let expected_root = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(finding.assertion.text.trim().as_bytes()))
    );
    if attachment.claim_root != expected_root {
        return Err(format!(
            "attachment claim_root is stale: expected {expected_root}, got {}",
            attachment.claim_root
        ));
    }
    if attachment.verifier_actor != verifier_actor {
        return Err(format!(
            "attachment verifier_actor {} does not match --as {verifier_actor}",
            attachment.verifier_actor
        ));
    }
    if crate::events::actor_kind(verifier_actor) != "agent" {
        return Err(
            "verifier attachment actor must use agent:, ci:, or verifier: identity".to_string(),
        );
    }
    if attachment.implementation_id.trim().is_empty() {
        return Err("durable verifier attachment requires implementation_id".to_string());
    }
    if attachment.execution_evidence_roots.is_empty() {
        return Err("durable verifier attachment requires execution_evidence_roots".to_string());
    }
    if attachment.adversarial_probes.is_empty()
        || attachment
            .adversarial_probes
            .iter()
            .any(|probe| probe.evidence_root.is_empty())
    {
        return Err(
            "durable verifier attachment requires at least one adversarial probe and an evidence_root for every probe"
                .to_string(),
        );
    }
    for prior_id in &attachment.independent_of {
        let prior = frontier
            .verifier_attachments
            .iter()
            .find(|candidate| candidate.id == *prior_id)
            .ok_or_else(|| format!("independent_of references unknown attachment {prior_id}"))?;
        let shared = attachment
            .lineage_couplings
            .iter()
            .filter(|tag| prior.lineage_couplings.contains(tag))
            .cloned()
            .collect::<Vec<_>>();
        if !shared.is_empty() {
            return Err(format!(
                "attachment falsely declares independence from {prior_id}; shared failure domain [{}]",
                shared.join(", ")
            ));
        }
    }
    if frontier
        .verifier_attachments
        .iter()
        .any(|existing| existing.id == attachment.id)
    {
        return Err(format!(
            "verifier attachment {} is already retained",
            attachment.id
        ));
    }
    let reason = format!("retain proposal-scoped verifier evidence for {proposal_id}");
    let mut event = events::new_finding_event(events::FindingEventInput {
        kind: events::EVENT_KIND_VERIFIER_ATTACHMENT_ADDED,
        finding_id: &finding.id,
        actor_id: verifier_actor,
        actor_type: events::actor_kind(verifier_actor),
        reason: &reason,
        before_hash: NULL_HASH,
        after_hash: NULL_HASH,
        payload: json!({
            "proposal_id": proposal_id,
            "attachment": attachment,
        }),
        caveats: vec![
            "Verifier evidence does not accept, approve, or finalize the proposal.".to_string(),
        ],
        timestamp: Some(timestamp),
    });
    event.signature = Some(crate::sign::sign_event(&event, key)?);
    crate::reducer::apply_event(frontier, &event)?;
    frontier.events.push(event.clone());
    project::recompute_stats(frontier);
    Ok(event)
}

fn apply_verifier_attach(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    if proposal.target.r#type != "finding" {
        return Err(format!(
            "verifier.attach target.type must be 'finding', got '{}'",
            proposal.target.r#type
        ));
    }
    let value = proposal
        .payload
        .get("attachment")
        .ok_or("verifier.attach proposal missing payload.attachment")?
        .clone();
    let att: crate::verifier_attachment::VerifierAttachment = serde_json::from_value(value)
        .map_err(|e| format!("Invalid verifier.attach payload: {e}"))?;
    att.verify()
        .map_err(|e| format!("verifier.attach attachment malformed: {e}"))?;
    if att.target != proposal.target.id {
        return Err(format!(
            "verifier.attach attachment.target {} does not match proposal target {}",
            att.target, proposal.target.id
        ));
    }
    // Activity is not state: a verifier gate may not attach to, or claim
    // independence from, an activity-plane id (`vac_`/`vrr_`).
    if crate::activity::is_activity_id(&att.target) {
        return Err(format!(
            "verifier.attach refused: target `{}` is an activity-plane id (activity is not lineage)",
            att.target
        ));
    }
    if let Some(indep) = att
        .independent_of
        .iter()
        .find(|i| crate::activity::is_activity_id(i))
    {
        return Err(format!(
            "verifier.attach refused: independent_of `{indep}` is an activity-plane id"
        ));
    }
    if !frontier.verifier_attachments.iter().any(|a| a.id == att.id) {
        frontier.verifier_attachments.push(att.clone());
    }
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: events::EVENT_KIND_VERIFIER_ATTACHMENT_ADDED,
        finding_id: &proposal.target.id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: NULL_HASH,
        after_hash: NULL_HASH,
        payload: json!({ "proposal_id": proposal.id, "attachment": att }),
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

fn apply_review(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let status = proposal
        .payload
        .get("status")
        .and_then(Value::as_str)
        .ok_or("finding.review proposal missing payload.status")?;
    use crate::bundle::ReviewState;
    let new_state = match status {
        "accepted" | "approved" => ReviewState::Accepted,
        "contested" => ReviewState::Contested,
        "needs_revision" => ReviewState::NeedsRevision,
        "rejected" => ReviewState::Rejected,
        other => return Err(format!("Unknown review proposal status '{other}'")),
    };
    frontier.findings[idx].flags.contested = new_state.implies_contested();
    frontier.findings[idx].flags.review_state = Some(new_state);
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.reviewed",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "status": status,
            "proposal_id": proposal.id,
        }),
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

/// Append a claim-granularity attribution to a finding's provenance. Descriptive
/// only: no flag, no confidence, no review state. Idempotent — a contribution
/// already present (same unit + agent + role) is not re-added.
fn apply_contribution(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let contribution: crate::bundle::Contribution = serde_json::from_value(
        proposal
            .payload
            .get("contribution")
            .cloned()
            .ok_or("finding.contribution.recorded proposal missing payload.contribution")?,
    )
    .map_err(|e| format!("malformed contribution: {e}"))?;
    contribution.validate()?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let existing = &mut frontier.findings[idx].provenance.contributions;
    if !existing.iter().any(|c| {
        c.unit == contribution.unit
            && c.agent_id == contribution.agent_id
            && c.role == contribution.role
    }) {
        existing.push(contribution.clone());
    }
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    let payload = json!({
        "contribution": serde_json::to_value(&contribution)
            .map_err(|e| format!("serialize contribution: {e}"))?,
        "proposal_id": proposal.id,
    });
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.contribution.recorded",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload,
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

fn apply_caveat(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
    fixed_decided_at: Option<&str>,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let now = fixed_decided_at
        .map(ToString::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let text = proposal
        .payload
        .get("text")
        .and_then(Value::as_str)
        .ok_or("finding.caveat proposal missing payload.text")?;
    let provenance = extract_annotation_provenance(&proposal.payload);
    let annotation_id = annotation_id(finding_id, text, reviewer, &now);
    frontier.findings[idx].annotations.push(Annotation {
        id: annotation_id.clone(),
        text: text.to_string(),
        author: reviewer.to_string(),
        timestamp: now.clone(),
        provenance: provenance.clone(),
    });
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    let mut payload = json!({
        "annotation_id": annotation_id,
        "text": text,
        "proposal_id": proposal.id,
    });
    if let Some(prov) = &provenance {
        payload["provenance"] = serde_json::to_value(prov).unwrap_or(Value::Null);
    }
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.caveated",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: text,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload,
        caveats: proposal.caveats.clone(),
        timestamp: Some(&now),
    }))
}

fn apply_note(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
    fixed_decided_at: Option<&str>,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let now = fixed_decided_at
        .map(ToString::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let text = proposal
        .payload
        .get("text")
        .and_then(Value::as_str)
        .ok_or("finding.note proposal missing payload.text")?;
    let provenance = extract_annotation_provenance(&proposal.payload);
    let annotation_id = annotation_id(finding_id, text, reviewer, &now);
    frontier.findings[idx].annotations.push(Annotation {
        id: annotation_id.clone(),
        text: text.to_string(),
        author: reviewer.to_string(),
        timestamp: now.clone(),
        provenance: provenance.clone(),
    });
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    let mut payload = json!({
        "annotation_id": annotation_id,
        "text": text,
        "proposal_id": proposal.id,
    });
    if let Some(prov) = &provenance {
        payload["provenance"] = serde_json::to_value(prov).unwrap_or(Value::Null);
    }
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.noted",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: text,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload,
        caveats: proposal.caveats.clone(),
        timestamp: Some(&now),
    }))
}

/// v0.57: Apply a `finding.span_repair` proposal. Appends a
/// `{section, text}` span to `state.findings[i].evidence.evidence_spans`
/// and emits one signed `finding.span_repaired` event.
fn apply_finding_span_repair(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let section = proposal
        .payload
        .get("section")
        .and_then(Value::as_str)
        .ok_or("finding.span_repair proposal missing payload.section")?
        .to_string();
    let text = proposal
        .payload
        .get("text")
        .and_then(Value::as_str)
        .ok_or("finding.span_repair proposal missing payload.text")?
        .to_string();
    let idx = find_finding_index(frontier, finding_id)?;
    let already_present = frontier.findings[idx]
        .evidence
        .evidence_spans
        .iter()
        .any(|existing| {
            existing.get("section").and_then(Value::as_str) == Some(section.as_str())
                && existing.get("text").and_then(Value::as_str) == Some(text.as_str())
        });
    if already_present {
        return Err(format!(
            "finding {finding_id} already carries an identical (section, text) span"
        ));
    }
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let span_value = json!({"section": section, "text": text});
    frontier.findings[idx]
        .evidence
        .evidence_spans
        .push(span_value);
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    let payload = json!({
        "proposal_id": proposal.id,
        "section": section,
        "text": text,
    });
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.span_repaired",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload,
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

/// v0.56: Apply an `evidence_atom.locator_repair` proposal. Sets
/// `locator` on the named evidence atom, removes the
/// "missing evidence locator" caveat, and emits one signed
/// `evidence_atom.locator_repaired` canonical event. The before/after
/// hashes are over the canonical bytes of the named atom only, so a
/// chain validator can confirm the exact atom changed and exactly the
/// named repair was applied.
fn apply_evidence_atom_locator_repair(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let atom_id = proposal.target.id.as_str();
    let locator = proposal
        .payload
        .get("locator")
        .and_then(Value::as_str)
        .ok_or("evidence_atom.locator_repair proposal missing payload.locator")?
        .to_string();
    let source_id = proposal
        .payload
        .get("source_id")
        .and_then(Value::as_str)
        .ok_or("evidence_atom.locator_repair proposal missing payload.source_id")?
        .to_string();

    let idx = frontier
        .evidence_atoms
        .iter()
        .position(|atom| atom.id == atom_id)
        .ok_or_else(|| format!("evidence_atom.locator_repair targets unknown atom {atom_id}"))?;
    if frontier.evidence_atoms[idx].source_id != source_id {
        return Err(format!(
            "evidence_atom.locator_repair payload.source_id '{source_id}' does not match atom.source_id '{}'",
            frontier.evidence_atoms[idx].source_id
        ));
    }
    if let Some(existing) = &frontier.evidence_atoms[idx].locator {
        if existing == &locator {
            return Err(format!(
                "evidence_atom {atom_id} already carries locator '{existing}'"
            ));
        }
        return Err(format!(
            "evidence_atom {atom_id} already carries locator '{existing}'; refusing to overwrite with '{locator}'"
        ));
    }

    let before_hash = events::evidence_atom_hash(&frontier.evidence_atoms[idx]);
    frontier.evidence_atoms[idx].locator = Some(locator.clone());
    frontier.evidence_atoms[idx]
        .caveats
        .retain(|c| c != "missing evidence locator");
    let after_hash = events::evidence_atom_hash(&frontier.evidence_atoms[idx]);

    let payload = json!({
        "proposal_id": proposal.id,
        "locator": locator,
        "source_id": source_id,
    });

    Ok(events::new_evidence_atom_locator_repair_event(
        atom_id,
        reviewer,
        "human",
        &proposal.reason,
        &before_hash,
        &after_hash,
        payload,
        proposal.caveats.clone(),
    ))
}

/// Phase β (v0.6): pull optional structured provenance off a note/caveat
/// proposal payload. The propose-* tools accept it; the validator gates
/// it; this helper threads it through to the materialized annotation
/// and the canonical event payload.
fn extract_annotation_provenance(payload: &Value) -> Option<crate::bundle::ProvenanceRef> {
    let prov = payload.get("provenance")?;
    let parsed: crate::bundle::ProvenanceRef = serde_json::from_value(prov.clone()).ok()?;
    if parsed.has_identifier() {
        Some(parsed)
    } else {
        None
    }
}

fn apply_confidence_revise(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
    fixed_decided_at: Option<&str>,
) -> Result<Vec<StateEvent>, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let now = fixed_decided_at
        .map(ToString::to_string)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let previous = frontier.findings[idx].confidence.score;
    let new_score = proposal
        .payload
        .get("confidence")
        .and_then(Value::as_f64)
        .ok_or("finding.confidence_revise proposal missing payload.confidence")?;

    // v0.55: when the revised confidence crosses the propagation threshold
    // (previous >= 0.5, new < 0.5), invoke the same cascade pattern that
    // `apply_retract` uses — emit `finding.dependency_invalidated` events for
    // each downstream supports/depends finding at depth ≤ MAX_DEPTH. Pre-v0.55
    // this path silently mutated confidence without firing the cascade, which
    // forced callers to chase a separate `vela propagate --reduce-confidence`
    // command for the substrate's signature feature.
    let cascade_threshold_crossed = previous >= 0.5 && new_score < 0.5;

    let pre_cascade_hashes: std::collections::HashMap<String, String> = if cascade_threshold_crossed
    {
        frontier
            .findings
            .iter()
            .map(|finding| (finding.id.clone(), events::finding_hash(finding)))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    let before_hash = events::finding_hash(&frontier.findings[idx]);

    // Apply the local mutation first so propagate_correction sees the new
    // confidence on the source finding.
    frontier.findings[idx].confidence.score = new_score;
    frontier.findings[idx].confidence.basis = format!(
        "expert revision from {:.3} to {:.3}: {}",
        previous, new_score, proposal.reason
    );
    frontier.findings[idx].confidence.method = ConfidenceMethod::ExpertJudgment;
    frontier.findings[idx].updated = Some(now.clone());

    let cascade = if cascade_threshold_crossed {
        Some(propagate::propagate_correction(
            frontier,
            finding_id,
            PropagationAction::ConfidenceReduced { new_score },
        ))
    } else {
        None
    };

    let after_hash = events::finding_hash(&frontier.findings[idx]);

    let source_event = events::new_finding_event(events::FindingEventInput {
        kind: "finding.confidence_revised",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "previous_score": previous,
            "new_score": new_score,
            "updated_at": now,
            "proposal_id": proposal.id,
            "cascade_fired": cascade_threshold_crossed,
            "affected": cascade.as_ref().map(|c| c.affected).unwrap_or(0),
        }),
        caveats: proposal.caveats.clone(),
        timestamp: Some(&now),
    });

    let source_event_id = source_event.id.clone();
    let mut emitted = vec![source_event];

    if let Some(cascade) = cascade {
        // Mirror apply_retract's per-dependent dependency_invalidated emission:
        // each affected dep at each depth gets a canonical event with the
        // before/after hash boundary so chain validation works downstream.
        for (depth_idx, level) in cascade.cascade.iter().enumerate() {
            let depth = (depth_idx as u32) + 1;
            for dep_id in level {
                let before = pre_cascade_hashes
                    .get(dep_id)
                    .cloned()
                    .unwrap_or_else(|| events::NULL_HASH.to_string());
                let after = events::finding_hash_by_id(frontier, dep_id);
                emitted.push(events::new_finding_event(events::FindingEventInput {
                    kind: "finding.dependency_invalidated",
                    finding_id: dep_id,
                    actor_id: reviewer,
                    actor_type: events::actor_kind(reviewer),
                    reason: &format!(
                        "Upstream finding {finding_id} confidence reduced to {new_score:.2}; cascade depth {depth}"
                    ),
                    before_hash: &before,
                    after_hash: &after,
                    payload: json!({
                        "upstream_finding_id": finding_id,
                        "upstream_event_id": source_event_id,
                        "depth": depth,
                        "new_score": new_score,
                        "previous_score": previous,
                        "proposal_id": proposal.id,
                    }),
                    caveats: vec![],
                    timestamp: fixed_decided_at,
                }));
            }
        }
    }

    Ok(emitted)
}

fn apply_reject(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    frontier.findings[idx].flags.contested = true;
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.rejected",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
            "status": "rejected",
        }),
        caveats: proposal.caveats.clone(),
        timestamp: None,
    }))
}

fn apply_retract(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
    fixed_decided_at: Option<&str>,
) -> Result<Vec<StateEvent>, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    if frontier.findings[idx].flags.retracted {
        return Err(format!("Finding {finding_id} is already retracted"));
    }
    // Phase L: capture every finding's pre-cascade hash so each emitted
    // `finding.dependency_invalidated` event can name a real before_hash
    // that matches whatever event last touched that dep.
    let pre_cascade_hashes: std::collections::HashMap<String, String> = frontier
        .findings
        .iter()
        .map(|finding| (finding.id.clone(), events::finding_hash(finding)))
        .collect();

    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let cascade =
        propagate::propagate_correction(frontier, finding_id, PropagationAction::Retracted);
    let after_hash = events::finding_hash_by_id(frontier, finding_id);

    let source_event = events::new_finding_event(events::FindingEventInput {
        kind: "finding.retracted",
        finding_id,
        actor_id: reviewer,
        actor_type: events::actor_kind(reviewer),
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
            "affected": cascade.affected,
            "cascade": cascade.cascade,
        }),
        caveats: vec!["Retraction impact is simulated over declared dependency links.".to_string()],
        timestamp: fixed_decided_at,
    });
    let source_event_id = source_event.id.clone();

    let mut emitted = vec![source_event];

    // Phase L: emit one canonical `finding.dependency_invalidated`
    // event per affected dependent, in BFS depth order. Each event
    // carries the before/after hash boundary for that specific dep so
    // chain validation works downstream.
    for (depth_idx, level) in cascade.cascade.iter().enumerate() {
        let depth = (depth_idx as u32) + 1;
        for dep_id in level {
            let before = pre_cascade_hashes
                .get(dep_id)
                .cloned()
                .unwrap_or_else(|| events::NULL_HASH.to_string());
            let after = events::finding_hash_by_id(frontier, dep_id);
            emitted.push(events::new_finding_event(events::FindingEventInput {
                kind: "finding.dependency_invalidated",
                finding_id: dep_id,
                actor_id: reviewer,
                actor_type: events::actor_kind(reviewer),
                reason: &format!("Upstream finding {finding_id} retracted; cascade depth {depth}"),
                before_hash: &before,
                after_hash: &after,
                payload: json!({
                    "upstream_finding_id": finding_id,
                    "upstream_event_id": source_event_id,
                    "depth": depth,
                    "proposal_id": proposal.id,
                }),
                caveats: vec![],
                timestamp: fixed_decided_at,
            }));
        }
    }

    Ok(emitted)
}

fn find_finding_index(frontier: &Project, finding_id: &str) -> Result<usize, String> {
    frontier
        .findings
        .iter()
        .position(|finding| finding.id == finding_id)
        .ok_or_else(|| format!("Finding not found: {finding_id}"))
}

fn annotation_id(finding_id: &str, text: &str, author: &str, timestamp: &str) -> String {
    let hash = Sha256::digest(format!("{finding_id}|{text}|{author}|{timestamp}").as_bytes());
    format!("ann_{}", &hex::encode(hash)[..16])
}

// ── Review-decision projection + parity (status derived from the log) ──
//
// A proposal's decision state is no longer a free-floating mutable field:
// it is a PROJECTION of the signed `review.*` events (and, for accepts,
// the domain event the accept produced). The stored `status` is a cache
// of that projection. `verify_proposal_decision_parity` is the gate that
// pins the cache to the log — if someone hand-edits a `status` field, or a
// decision exists with no signed event behind it, parity fails. That is
// the tamper-evidence the mutable field never had.

/// A decision reconstructed from the event log for one proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedDecision {
    /// `applied` | `rejected` | `needs_revision`.
    pub status: String,
    /// The reviewer that made the latest decision.
    pub reviewer: String,
    /// The latest decision event's timestamp.
    pub decided_at: String,
    /// The exact reason carried by the latest decision event.
    pub reason: String,
    /// The domain event applied by an acceptance, when one exists. Historical
    /// accepts that use their review event as the applied record carry that
    /// review event id here.
    pub applied_event_id: Option<String>,
    /// The `review.*` event id that carried the decision, when one exists.
    /// `None` for an accept whose only trace is its domain event (the
    /// pre-`review.accepted` accept path; see module note).
    pub review_event_id: Option<String>,
}

/// Reduce the event log to the current decision for a single proposal.
///
/// Folds, in timestamp order:
///   - `review.rejected` → rejected
///   - `review.revision_requested` → needs_revision
///   - `review.accepted` → applied
///   - `proposal.withdrawn` → withdrawn
///   - any domain event produced by an accept of this proposal
///     (matched via the proposal's `applied_event_id`) → applied
///
/// The latest decision wins. Returns `None` when no decision event exists
/// (the proposal is pending).
pub fn proposal_status_from_log(
    frontier: &Project,
    proposal_id: &str,
    applied_event_id: Option<&str>,
) -> Option<DerivedDecision> {
    proposal_status_from_logs(frontier, &[], proposal_id, applied_event_id)
}

/// Derive proposal standing across the immutable legacy event log and the
/// verified repository-authority event log.
///
/// Callers must pass only Authority events whose covering DSSE history has
/// already verified. Keeping that verification outside this pure reducer
/// avoids turning a proposal projection into a filesystem or key loader.
pub fn proposal_status_from_logs(
    frontier: &Project,
    authority_events: &[crate::authority::AuthorityEventV1],
    proposal_id: &str,
    applied_event_id: Option<&str>,
) -> Option<DerivedDecision> {
    let mut decisions: Vec<DerivedDecision> = Vec::new();
    for event in &frontier.events {
        let is_review_for_this = event.target.r#type == "proposal"
            && event.target.id == proposal_id
            && matches!(
                event.kind.as_str(),
                events::EVENT_KIND_REVIEW_ACCEPTED
                    | events::EVENT_KIND_REVIEW_REJECTED
                    | events::EVENT_KIND_REVIEW_REVISION_REQUESTED
                    | events::EVENT_KIND_PROPOSAL_WITHDRAWN
            );
        if is_review_for_this {
            let status = match event.kind.as_str() {
                events::EVENT_KIND_REVIEW_ACCEPTED => "applied",
                events::EVENT_KIND_REVIEW_REJECTED => "rejected",
                events::EVENT_KIND_PROPOSAL_WITHDRAWN => "withdrawn",
                _ => "needs_revision",
            };
            decisions.push(DerivedDecision {
                status: status.to_string(),
                reviewer: event.actor.id.clone(),
                decided_at: event.timestamp.clone(),
                reason: event.reason.clone(),
                applied_event_id: if status == "applied" {
                    event
                        .payload
                        .get("applied_event_id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| {
                            (applied_event_id == Some(event.id.as_str())).then(|| event.id.clone())
                        })
                } else {
                    None
                },
                review_event_id: Some(event.id.clone()),
            });
            continue;
        }
        // An accept's domain event is its decision trace when no explicit
        // review.accepted exists (the historical accept path).
        if let Some(applied) = applied_event_id
            && event.id == applied
        {
            decisions.push(DerivedDecision {
                status: "applied".to_string(),
                reviewer: event.actor.id.clone(),
                decided_at: event.timestamp.clone(),
                reason: event.reason.clone(),
                applied_event_id: Some(event.id.clone()),
                review_event_id: None,
            });
        }
    }
    for authority_event in authority_events {
        let event = &authority_event.content;
        let is_review_for_this = event.target.r#type == "proposal"
            && event.target.id == proposal_id
            && matches!(
                event.kind.as_str(),
                events::EVENT_KIND_REVIEW_ACCEPTED
                    | events::EVENT_KIND_REVIEW_REJECTED
                    | events::EVENT_KIND_REVIEW_REVISION_REQUESTED
            );
        if !is_review_for_this {
            continue;
        }
        let status = match event.kind.as_str() {
            events::EVENT_KIND_REVIEW_ACCEPTED => "applied",
            events::EVENT_KIND_REVIEW_REJECTED => "rejected",
            _ => "needs_revision",
        };
        decisions.push(DerivedDecision {
            status: status.to_string(),
            reviewer: event.actor.id.clone(),
            decided_at: event.timestamp.clone(),
            reason: event.reason.clone(),
            applied_event_id: if status == "applied" {
                event
                    .payload
                    .get("applied_event_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            } else {
                None
            },
            review_event_id: Some(authority_event.id.clone()),
        });
        continue;
    }
    for authority_event in authority_events {
        let Some(applied) = applied_event_id else {
            break;
        };
        let Ok(semantic_event_id) = authority_event.semantic_event_id() else {
            continue;
        };
        if semantic_event_id == applied {
            decisions.push(DerivedDecision {
                status: "applied".to_string(),
                reviewer: authority_event.content.actor.id.clone(),
                decided_at: authority_event.content.timestamp.clone(),
                reason: authority_event.content.reason.clone(),
                applied_event_id: Some(semantic_event_id),
                review_event_id: None,
            });
        }
    }
    decisions.sort_by(|a, b| {
        a.decided_at
            .cmp(&b.decided_at)
            // At one decision instant the explicit review event is the
            // authoritative proposal projection; its linked domain event is
            // the scientific transition. Keep the review record last.
            .then_with(|| {
                a.review_event_id
                    .is_some()
                    .cmp(&b.review_event_id.is_some())
            })
    });
    decisions.pop()
}

/// Verify that every proposal's stored decision state is backed by the
/// event log, and vice versa. Returns a list of human-readable conflicts
/// (empty == parity holds). This is the invariant the conformance gate
/// runs: it makes the mutable `status` field a verifiable projection
/// rather than an unconstrained side-table.
///
/// Checks, per proposal:
///   - a decided status (`applied` / `rejected` / `needs_revision`) MUST
///     have a backing event in the log (a `review.*` event, or for
///     `applied` the referenced domain event);
///   - the stored status MUST equal the status derived from the log;
///   - `pending_review` MUST NOT have a decision event.
/// And globally:
///   - every `review.*` event MUST reference a proposal that exists.
pub fn verify_proposal_decision_parity(frontier: &Project) -> Vec<String> {
    verify_proposal_decision_parity_with_authority(frontier, &[])
}

/// Verify proposal standing against both legacy events and an already
/// verified repository-authority event history.
pub fn verify_proposal_decision_parity_with_authority(
    frontier: &Project,
    authority_events: &[crate::authority::AuthorityEventV1],
) -> Vec<String> {
    let mut conflicts = Vec::new();
    let mut proposal_ids = BTreeSet::new();

    for proposal in &frontier.proposals {
        let expected_id = proposal_id(proposal);
        if proposal.id != expected_id {
            conflicts.push(format!(
                "proposal {} logical content derives id {expected_id}",
                proposal.id
            ));
        }
        if !proposal_ids.insert(proposal.id.as_str()) {
            conflicts.push(format!("proposal {} occurs more than once", proposal.id));
        }

        let derived = proposal_status_from_logs(
            frontier,
            authority_events,
            &proposal.id,
            proposal.applied_event_id.as_deref(),
        );
        match proposal.status.as_str() {
            "pending_review" => {
                if let Some(d) = derived {
                    conflicts.push(format!(
                        "proposal {} is stored pending_review but the log carries a {} decision ({})",
                        proposal.id,
                        d.status,
                        d.review_event_id.as_deref().unwrap_or("domain event")
                    ));
                }
                if proposal.reviewed_by.is_some()
                    || proposal.reviewed_at.is_some()
                    || proposal.decision_reason.is_some()
                    || proposal.applied_event_id.is_some()
                {
                    conflicts.push(format!(
                        "proposal {} is pending_review but carries terminal decision fields",
                        proposal.id
                    ));
                }
            }
            "accepted" => {
                // Transient in-memory state only; never persisted.
                conflicts.push(format!(
                    "proposal {} is stored in transient 'accepted' state (should be 'applied')",
                    proposal.id
                ));
            }
            stored @ ("applied" | "rejected" | "needs_revision" | "withdrawn") => {
                match derived {
                    None => conflicts.push(format!(
                        "proposal {} is stored '{}' but NO decision event backs it in the log \
                         — a decision with no signed, replayable record (the silent-drop vector)",
                        proposal.id, stored
                    )),
                    Some(d) if d.status != stored => conflicts.push(format!(
                        "proposal {} is stored '{}' but the log's latest decision is '{}'",
                        proposal.id, stored, d.status
                    )),
                    // Historical accepts predate explicit `review.accepted`
                    // events. Their domain event proves status and the stored
                    // decision metadata remains legacy display context.
                    // Explicit review events, including every current
                    // decision path, bind the complete decision projection.
                    Some(d) if d.review_event_id.is_some() => {
                        let expected_reviewer =
                            (stored != "withdrawn").then_some(d.reviewer.as_str());
                        let expected_reviewed_at =
                            (stored != "withdrawn").then_some(d.decided_at.as_str());
                        if proposal.reviewed_by.as_deref() != expected_reviewer
                            || proposal.reviewed_at.as_deref() != expected_reviewed_at
                            || proposal.decision_reason.as_deref() != Some(d.reason.as_str())
                            || proposal.applied_event_id != d.applied_event_id
                        {
                            conflicts.push(format!(
                                "proposal {} stored decision fields do not match the latest {} event projection",
                                proposal.id, stored
                            ));
                        }
                    }
                    Some(_) => {}
                }
            }
            other => conflicts.push(format!(
                "proposal {} has unknown status '{}'",
                proposal.id, other
            )),
        }
    }

    for event in &frontier.events {
        if matches!(
            event.kind.as_str(),
            events::EVENT_KIND_REVIEW_ACCEPTED
                | events::EVENT_KIND_REVIEW_REJECTED
                | events::EVENT_KIND_REVIEW_REVISION_REQUESTED
                | events::EVENT_KIND_PROPOSAL_WITHDRAWN
        ) && !proposal_ids.contains(event.target.id.as_str())
        {
            conflicts.push(format!(
                "review event {} targets proposal {} which does not exist in the frontier",
                event.id, event.target.id
            ));
        }
    }
    for event in authority_events {
        if matches!(
            event.content.kind.as_str(),
            events::EVENT_KIND_REVIEW_ACCEPTED
                | events::EVENT_KIND_REVIEW_REJECTED
                | events::EVENT_KIND_REVIEW_REVISION_REQUESTED
        ) && !proposal_ids.contains(event.content.target.id.as_str())
        {
            conflicts.push(format!(
                "repository-authority review event {} targets proposal {} which does not exist in the frontier",
                event.id, event.content.target.id
            ));
        }
        if event.content.kind == events::EVENT_KIND_REVIEW_ACCEPTED {
            let applied = event
                .content
                .payload
                .get("applied_event_id")
                .and_then(Value::as_str);
            let Some(applied) = applied else {
                conflicts.push(format!(
                    "repository-authority review event {} lacks payload.applied_event_id",
                    event.id
                ));
                continue;
            };
            let legacy_match = frontier
                .events
                .iter()
                .any(|candidate| candidate.id == applied);
            let authority_matches = authority_events
                .iter()
                .filter_map(|candidate| candidate.semantic_event_id().ok())
                .filter(|semantic_id| semantic_id == applied)
                .count();
            if !legacy_match && authority_matches == 0 {
                conflicts.push(format!(
                    "repository-authority review event {} references missing applied semantic event {}",
                    event.id, applied
                ));
            } else if usize::from(legacy_match) + authority_matches != 1 {
                conflicts.push(format!(
                    "repository-authority review event {} resolves applied semantic event {} ambiguously",
                    event.id, applied
                ));
            }
        }
    }

    conflicts
}

fn build_changed_finding_details(
    before: &Project,
    after: &Project,
    ids: &[String],
) -> Vec<ChangedFindingDetail> {
    ids.iter()
        .map(|id| {
            let fa = before.findings.iter().find(|f| &f.id == id);
            let fb = after.findings.iter().find(|f| &f.id == id);
            ChangedFindingDetail {
                id: id.clone(),
                assertion_before: fa.map(|f| f.assertion.text.clone()),
                assertion_after: fb.map(|f| f.assertion.text.clone()),
                assertion_type_before: fa.map(|f| f.assertion.assertion_type.clone()),
                assertion_type_after: fb.map(|f| f.assertion.assertion_type.clone()),
                confidence_before: fa.map(|f| format!("{:.2}", f.confidence.score)),
                confidence_after: fb.map(|f| format!("{:.2}", f.confidence.score)),
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests;
