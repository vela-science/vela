//! Proposal-first frontier writes and proof freshness tracking.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::bundle::{Annotation, ConfidenceMethod, FindingBundle};
use crate::canonical;
use crate::events::{self, NULL_HASH, StateActor, StateEvent, StateTarget};
use crate::project::{self, Project};
use crate::propagate::{self, PropagationAction};
use crate::repo;

pub const PROPOSAL_SCHEMA: &str = "vela.proposal.v0.1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateProposal {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub id: String,
    pub kind: String,
    pub target: StateTarget,
    pub actor: StateActor,
    pub created_at: String,
    pub reason: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub source_refs: Vec<String>,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_event_id: Option<String>,
    #[serde(default)]
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposalSummary {
    pub total: usize,
    pub pending_review: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub applied: usize,
    #[serde(default)]
    pub by_kind: BTreeMap<String, usize>,
    #[serde(default)]
    pub duplicate_ids: Vec<String>,
    #[serde(default)]
    pub invalid_targets: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProofState {
    #[serde(default)]
    pub latest_packet: ProofPacketState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event_at_export: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProofPacketState {
    pub generated_at: Option<String>,
    pub snapshot_hash: Option<String>,
    pub event_log_hash: Option<String>,
    pub packet_manifest_hash: Option<String>,
    pub status: String,
}

impl Default for ProofPacketState {
    fn default() -> Self {
        Self {
            generated_at: None,
            snapshot_hash: None,
            event_log_hash: None,
            packet_manifest_hash: None,
            status: "never_exported".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateProposalResult {
    pub proposal_id: String,
    pub finding_id: String,
    pub status: String,
    pub applied_event_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ImportProposalReport {
    pub imported: usize,
    pub applied: usize,
    pub rejected: usize,
    pub duplicates: usize,
    pub wrote_to: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposalValidationReport {
    pub ok: bool,
    pub checked: usize,
    pub valid: usize,
    pub invalid: usize,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub proposal_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProofPacketRecord {
    pub generated_at: String,
    pub snapshot_hash: String,
    pub event_log_hash: String,
    pub packet_manifest_hash: String,
}

fn default_schema() -> String {
    PROPOSAL_SCHEMA.to_string()
}

#[allow(clippy::too_many_arguments)]
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
    let created_at = Utc::now().to_rfc3339();
    let mut proposal = StateProposal {
        schema: PROPOSAL_SCHEMA.to_string(),
        id: String::new(),
        kind: kind.into(),
        target,
        actor: StateActor {
            id: actor_id.into(),
            r#type: actor_type.into(),
        },
        created_at,
        reason: reason.into(),
        payload,
        source_refs,
        status: "pending_review".to_string(),
        reviewed_by: None,
        reviewed_at: None,
        decision_reason: None,
        applied_event_id: None,
        caveats,
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

pub fn summary(frontier: &Project) -> ProposalSummary {
    let mut out = ProposalSummary::default();
    let mut seen = BTreeSet::new();
    let finding_ids = frontier
        .findings
        .iter()
        .map(|finding| finding.id.as_str())
        .collect::<BTreeSet<_>>();
    for proposal in &frontier.proposals {
        out.total += 1;
        *out.by_kind.entry(proposal.kind.clone()).or_default() += 1;
        match proposal.status.as_str() {
            "pending_review" => out.pending_review += 1,
            "accepted" => out.accepted += 1,
            "rejected" => out.rejected += 1,
            "applied" => out.applied += 1,
            _ => {}
        }
        if !seen.insert(proposal.id.clone()) {
            out.duplicate_ids.push(proposal.id.clone());
        }
        if proposal.kind != "finding.add" && !finding_ids.contains(proposal.target.id.as_str()) {
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
/// `apply` semantics are also idempotent: if the same proposal+reviewer pair
/// has already been applied (proposal.applied_event_id is set), return the
/// existing event_id rather than emitting a duplicate canonical event.
pub fn create_or_apply(
    path: &Path,
    proposal: StateProposal,
    apply: bool,
) -> Result<CreateProposalResult, String> {
    let mut frontier = repo::load_from_path(path)?;
    let finding_id = proposal.target.id.clone();
    let proposal_id = proposal.id.clone();

    // Idempotent insert: if a proposal with this content-addressed id already
    // exists, skip insertion and treat the existing record as authoritative.
    let existing_idx = frontier
        .proposals
        .iter()
        .position(|existing| existing.id == proposal_id);
    if existing_idx.is_none() {
        validate_new_proposal(&frontier, &proposal)?;
        frontier.proposals.push(proposal);
    }

    let applied_event_id = if apply {
        // Idempotent apply: if the existing record was already applied, return
        // its event_id rather than emitting a duplicate event.
        if let Some(idx) = existing_idx
            && let Some(existing_event) = frontier.proposals[idx].applied_event_id.clone()
        {
            Some(existing_event)
        } else {
            let reviewer = frontier
                .proposals
                .iter()
                .find(|proposal| proposal.id == proposal_id)
                .map(|proposal| proposal.actor.id.clone())
                .ok_or_else(|| format!("Proposal not found after insertion: {proposal_id}"))?;
            Some(accept_proposal_in_frontier(
                &mut frontier,
                &proposal_id,
                &reviewer,
                "Applied locally from proposal creation",
            )?)
        }
    } else {
        existing_idx.and_then(|idx| frontier.proposals[idx].applied_event_id.clone())
    };

    project::recompute_stats(&mut frontier);
    repo::save_to_path(path, &frontier)?;
    Ok(CreateProposalResult {
        proposal_id,
        finding_id,
        status: applied_event_id
            .as_ref()
            .map_or_else(|| "pending_review".to_string(), |_| "applied".to_string()),
        applied_event_id,
    })
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

pub fn import_from_path(path: &Path, source: &Path) -> Result<ImportProposalReport, String> {
    let mut frontier = repo::load_from_path(path)?;
    let proposals = load_proposals(source)?;
    let wrote_to = path.display().to_string();
    let mut report = ImportProposalReport {
        wrote_to,
        ..ImportProposalReport::default()
    };
    for proposal in proposals {
        if frontier
            .proposals
            .iter()
            .any(|existing| existing.id == proposal.id)
        {
            report.duplicates += 1;
            continue;
        }
        validate_new_proposal(&frontier, &proposal)?;
        frontier.proposals.push(proposal.clone());
        report.imported += 1;
        match proposal.status.as_str() {
            "accepted" => {
                let reviewer = proposal
                    .reviewed_by
                    .as_deref()
                    .ok_or_else(|| {
                        format!("Accepted proposal {} missing reviewed_by", proposal.id)
                    })?
                    .to_string();
                let reason = proposal
                    .decision_reason
                    .clone()
                    .unwrap_or_else(|| "Imported accepted proposal".to_string());
                let _ =
                    accept_proposal_in_frontier(&mut frontier, &proposal.id, &reviewer, &reason)?;
                report.applied += 1;
            }
            "applied" => {
                let reviewer = proposal
                    .reviewed_by
                    .as_deref()
                    .ok_or_else(|| format!("Applied proposal {} missing reviewed_by", proposal.id))?
                    .to_string();
                let reason = proposal
                    .decision_reason
                    .clone()
                    .unwrap_or_else(|| "Imported applied proposal".to_string());
                let _ =
                    accept_proposal_in_frontier(&mut frontier, &proposal.id, &reviewer, &reason)?;
                report.applied += 1;
            }
            "rejected" => report.rejected += 1,
            _ => {}
        }
    }
    project::recompute_stats(&mut frontier);
    repo::save_to_path(path, &frontier)?;
    Ok(report)
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

pub fn accept_at_path(
    path: &Path,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
) -> Result<String, String> {
    let mut frontier = repo::load_from_path(path)?;
    let event_id = accept_proposal_in_frontier(&mut frontier, proposal_id, reviewer, reason)?;
    project::recompute_stats(&mut frontier);
    repo::save_to_path(path, &frontier)?;
    Ok(event_id)
}

pub fn reject_at_path(
    path: &Path,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
) -> Result<(), String> {
    let mut frontier = repo::load_from_path(path)?;
    reject_proposal_in_frontier(&mut frontier, proposal_id, reviewer, reason)?;
    project::recompute_stats(&mut frontier);
    repo::save_to_path(path, &frontier)?;
    Ok(())
}

pub fn record_proof_export(frontier: &mut Project, record: ProofPacketRecord) {
    frontier.proof_state.latest_packet = ProofPacketState {
        generated_at: Some(record.generated_at),
        snapshot_hash: Some(record.snapshot_hash),
        event_log_hash: Some(record.event_log_hash),
        packet_manifest_hash: Some(record.packet_manifest_hash),
        status: "current".to_string(),
    };
    frontier.proof_state.last_event_at_export =
        frontier.events.last().map(|event| event.timestamp.clone());
    frontier.proof_state.stale_reason = None;
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
    if proposal.target.r#type != "finding" {
        return Err("Only finding proposals are supported in this milestone".to_string());
    }
    if proposal.reason.trim().is_empty() {
        return Err("Proposal reason must be non-empty".to_string());
    }
    if !matches!(
        proposal.status.as_str(),
        "pending_review" | "accepted" | "rejected" | "applied"
    ) {
        return Err(format!("Unsupported proposal status '{}'", proposal.status));
    }
    match proposal.kind.as_str() {
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
        "finding.retract" => {
            let idx = require_existing_finding(frontier, &proposal.target.id)?;
            if frontier.findings[idx].flags.retracted {
                return Err(format!(
                    "Finding {} is already retracted",
                    proposal.target.id
                ));
            }
        }
        other => {
            return Err(format!("Unsupported proposal kind '{other}'"));
        }
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
    if proposal.target.r#type != "finding" {
        return Err("Only finding proposals are supported in v0".to_string());
    }
    if proposal.reason.trim().is_empty() {
        return Err("Proposal reason must be non-empty".to_string());
    }
    match proposal.kind.as_str() {
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
        other => return Err(format!("Unsupported proposal kind '{other}'")),
    }
    validate_decision_state(proposal)
}

fn require_existing_finding(frontier: &Project, finding_id: &str) -> Result<usize, String> {
    frontier
        .findings
        .iter()
        .position(|finding| finding.id == finding_id)
        .ok_or_else(|| format!("Finding not found: {finding_id}"))
}

fn accept_proposal_in_frontier(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
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
    let status = frontier.proposals[index].status.clone();
    if status == "rejected" {
        return Err(format!("Cannot accept rejected proposal {}", proposal_id));
    }
    if status == "applied" {
        return frontier.proposals[index]
            .applied_event_id
            .clone()
            .ok_or_else(|| format!("Proposal {} is applied but has no event id", proposal_id));
    }
    let proposal = frontier.proposals[index].clone();
    validate_proposal_shape(frontier, &proposal)?;
    frontier.proposals[index].status = "accepted".to_string();
    frontier.proposals[index].reviewed_by = Some(reviewer.to_string());
    frontier.proposals[index].reviewed_at = Some(Utc::now().to_rfc3339());
    frontier.proposals[index].decision_reason = Some(reason.to_string());
    let event_id = apply_proposal(frontier, &proposal, reviewer, reason)?;
    frontier.proposals[index].status = "applied".to_string();
    frontier.proposals[index].applied_event_id = Some(event_id.clone());
    Ok(event_id)
}

fn reject_proposal_in_frontier(
    frontier: &mut Project,
    proposal_id: &str,
    reviewer: &str,
    reason: &str,
) -> Result<(), String> {
    validate_reviewer_identity(reviewer)?;
    if reason.trim().is_empty() {
        return Err("Decision reason must be non-empty".to_string());
    }
    let index = frontier
        .proposals
        .iter()
        .position(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    match frontier.proposals[index].status.as_str() {
        "pending_review" | "accepted" => {}
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
    frontier.proposals[index].status = "rejected".to_string();
    frontier.proposals[index].reviewed_by = Some(reviewer.to_string());
    frontier.proposals[index].reviewed_at = Some(Utc::now().to_rfc3339());
    frontier.proposals[index].decision_reason = Some(reason.to_string());
    Ok(())
}

fn apply_proposal(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    decision_reason: &str,
) -> Result<String, String> {
    // Phase L: retraction emits a fan of events — one for the source
    // and one `finding.dependency_invalidated` per dependent in BFS
    // order. apply_retract is responsible for pushing all of them in
    // sequence; this branch only assigns the primary event ID.
    if proposal.kind.as_str() == "finding.retract" {
        let events = apply_retract(frontier, proposal, reviewer, decision_reason)?;
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
    let event = match proposal.kind.as_str() {
        "finding.add" => apply_add(frontier, proposal, reviewer, decision_reason)?,
        "finding.review" => apply_review(frontier, proposal, reviewer, decision_reason)?,
        "finding.caveat" => apply_caveat(frontier, proposal, reviewer, decision_reason)?,
        "finding.note" => apply_note(frontier, proposal, reviewer, decision_reason)?,
        "finding.confidence_revise" => {
            apply_confidence_revise(frontier, proposal, reviewer, decision_reason)?
        }
        "finding.reject" => apply_reject(frontier, proposal, reviewer, decision_reason)?,
        other => return Err(format!("Unsupported proposal kind '{other}'")),
    };
    let event_id = event.id.clone();
    frontier.events.push(event);
    mark_proof_stale(
        frontier,
        format!("Applied proposal {} after latest proof export", proposal.id),
    );
    Ok(event_id)
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
    if frontier
        .findings
        .iter()
        .any(|existing| existing.id == finding_id)
    {
        return Err(format!(
            "Refusing to add duplicate finding with existing finding ID {finding_id}"
        ));
    }
    frontier.findings.push(finding);
    let after_hash = events::finding_hash_by_id(frontier, &finding_id);
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.asserted",
        finding_id: &finding_id,
        actor_id: reviewer,
        actor_type: "human",
        reason: &proposal.reason,
        before_hash: NULL_HASH,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
        }),
        caveats: proposal.caveats.clone(),
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
        actor_type: "human",
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "status": status,
            "proposal_id": proposal.id,
        }),
        caveats: proposal.caveats.clone(),
    }))
}

fn apply_caveat(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let now = Utc::now().to_rfc3339();
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
        timestamp: now,
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
        actor_type: "human",
        reason: text,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload,
        caveats: proposal.caveats.clone(),
    }))
}

fn apply_note(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let now = Utc::now().to_rfc3339();
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
        timestamp: now,
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
        actor_type: "human",
        reason: text,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload,
        caveats: proposal.caveats.clone(),
    }))
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
) -> Result<StateEvent, String> {
    let finding_id = proposal.target.id.as_str();
    let idx = find_finding_index(frontier, finding_id)?;
    let before_hash = events::finding_hash(&frontier.findings[idx]);
    let now = Utc::now().to_rfc3339();
    let previous = frontier.findings[idx].confidence.score;
    let new_score = proposal
        .payload
        .get("confidence")
        .and_then(Value::as_f64)
        .ok_or("finding.confidence_revise proposal missing payload.confidence")?;
    frontier.findings[idx].confidence.score = new_score;
    frontier.findings[idx].confidence.basis = format!(
        "expert revision from {:.3} to {:.3}: {}",
        previous, new_score, proposal.reason
    );
    frontier.findings[idx].confidence.method = ConfidenceMethod::ExpertJudgment;
    frontier.findings[idx].updated = Some(now.clone());
    let after_hash = events::finding_hash(&frontier.findings[idx]);
    Ok(events::new_finding_event(events::FindingEventInput {
        kind: "finding.confidence_revised",
        finding_id,
        actor_id: reviewer,
        actor_type: "human",
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "previous_score": previous,
            "new_score": new_score,
            "proposal_id": proposal.id,
        }),
        caveats: proposal.caveats.clone(),
    }))
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
        actor_type: "human",
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
            "status": "rejected",
        }),
        caveats: proposal.caveats.clone(),
    }))
}

fn apply_retract(
    frontier: &mut Project,
    proposal: &StateProposal,
    reviewer: &str,
    _decision_reason: &str,
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
        actor_type: "human",
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
            "affected": cascade.affected,
            "cascade": cascade.cascade,
        }),
        caveats: vec!["Retraction impact is simulated over declared dependency links.".to_string()],
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
                actor_type: "human",
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

pub fn manifest_hash(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Failed to read manifest '{}': {e}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub fn repo_proposals_dir(root: &Path) -> PathBuf {
    root.join(".vela/proposals")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{
        Assertion, Conditions, Confidence, ConfidenceKind, ConfidenceMethod, Entity, Evidence,
        Extraction, Flags, Provenance,
    };
    use crate::project;
    use tempfile::TempDir;

    fn finding(id: &str) -> FindingBundle {
        FindingBundle {
            id: id.to_string(),
            version: 1,
            previous_version: None,
            assertion: Assertion {
                text: "Test finding".to_string(),
                assertion_type: "mechanism".to_string(),
                entities: vec![Entity {
                    name: "LRP1".to_string(),
                    entity_type: "protein".to_string(),
                    identifiers: serde_json::Map::new(),
                    canonical_id: None,
                    candidates: Vec::new(),
                    aliases: Vec::new(),
                    resolution_provenance: None,
                    resolution_confidence: 1.0,
                    resolution_method: None,
                    species_context: None,
                    needs_review: false,
                }],
                relation: None,
                direction: None,
            },
            evidence: Evidence {
                evidence_type: "experimental".to_string(),
                model_system: String::new(),
                species: None,
                method: "manual".to_string(),
                sample_size: None,
                effect_size: None,
                p_value: None,
                replicated: false,
                replication_count: None,
                evidence_spans: Vec::new(),
            },
            conditions: Conditions {
                text: "mouse".to_string(),
                species_verified: Vec::new(),
                species_unverified: Vec::new(),
                in_vitro: false,
                in_vivo: true,
                human_data: false,
                clinical_trial: false,
                concentration_range: None,
                duration: None,
                age_group: None,
                cell_type: None,
            },
            confidence: Confidence {
                kind: ConfidenceKind::FrontierEpistemic,
                score: 0.7,
                basis: "test".to_string(),
                method: ConfidenceMethod::ExpertJudgment,
                components: None,
                extraction_confidence: 1.0,
            },
            provenance: Provenance {
                source_type: "published_paper".to_string(),
                doi: None,
                pmid: None,
                pmc: None,
                openalex_id: None,
                url: None,
                title: "Test".to_string(),
                authors: Vec::new(),
                year: Some(2024),
                journal: None,
                license: None,
                publisher: None,
                funders: Vec::new(),
                extraction: Extraction::default(),
                review: None,
                citation_count: None,
            },
            flags: Flags {
                gap: false,
                negative_space: false,
                contested: false,
                retracted: false,
                declining: false,
                gravity_well: false,
                review_state: None,
            },
            links: Vec::new(),
            annotations: Vec::new(),
            attachments: Vec::new(),
            created: "2026-04-23T00:00:00Z".to_string(),
            updated: None,
        }
    }

    #[test]
    fn pending_review_proposal_does_not_mutate_frontier() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
        repo::save_to_path(&path, &frontier).unwrap();
        let proposal = new_proposal(
            "finding.review",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test".to_string(),
            },
            "reviewer:test",
            "human",
            "Mouse-only evidence",
            json!({"status": "contested"}),
            Vec::new(),
            Vec::new(),
        );
        create_or_apply(&path, proposal, false).unwrap();
        let loaded = repo::load_from_path(&path).unwrap();
        assert_eq!(loaded.events.len(), 1); // genesis only (proposal pending)
        assert_eq!(loaded.proposals.len(), 1);
        assert!(!loaded.findings[0].flags.contested);
    }

    #[test]
    fn applied_proposal_emits_event_and_stales_proof() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let mut frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
        record_proof_export(
            &mut frontier,
            ProofPacketRecord {
                generated_at: "2026-04-23T00:00:00Z".to_string(),
                snapshot_hash: "a".repeat(64),
                event_log_hash: "b".repeat(64),
                packet_manifest_hash: "c".repeat(64),
            },
        );
        repo::save_to_path(&path, &frontier).unwrap();
        let proposal = new_proposal(
            "finding.review",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test".to_string(),
            },
            "reviewer:test",
            "human",
            "Mouse-only evidence",
            json!({"status": "contested"}),
            Vec::new(),
            Vec::new(),
        );
        create_or_apply(&path, proposal, true).unwrap();
        let loaded = repo::load_from_path(&path).unwrap();
        assert_eq!(loaded.events.len(), 2); // genesis + applied
        assert!(loaded.findings[0].flags.contested);
        assert_eq!(loaded.proposals[0].status, "applied");
        assert_eq!(loaded.proof_state.latest_packet.status, "stale");
    }

    #[test]
    fn pending_note_proposal_does_not_mutate_annotations() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
        repo::save_to_path(&path, &frontier).unwrap();
        let proposal = new_proposal(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test".to_string(),
            },
            "reviewer:test",
            "human",
            "Track mouse-only evidence",
            json!({"text": "Track mouse-only evidence"}),
            Vec::new(),
            Vec::new(),
        );
        create_or_apply(&path, proposal, false).unwrap();
        let loaded = repo::load_from_path(&path).unwrap();
        assert_eq!(loaded.events.len(), 1); // genesis only
        assert_eq!(loaded.proposals.len(), 1);
        assert!(loaded.findings[0].annotations.is_empty());
        assert_eq!(loaded.proposals[0].kind, "finding.note");
    }

    #[test]
    fn applied_note_emits_noted_event_and_stales_proof() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let mut frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
        record_proof_export(
            &mut frontier,
            ProofPacketRecord {
                generated_at: "2026-04-23T00:00:00Z".to_string(),
                snapshot_hash: "a".repeat(64),
                event_log_hash: "b".repeat(64),
                packet_manifest_hash: "c".repeat(64),
            },
        );
        repo::save_to_path(&path, &frontier).unwrap();
        let proposal = new_proposal(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test".to_string(),
            },
            "reviewer:test",
            "human",
            "Track mouse-only evidence",
            json!({"text": "Track mouse-only evidence"}),
            Vec::new(),
            Vec::new(),
        );
        let result = create_or_apply(&path, proposal, true).unwrap();
        let loaded = repo::load_from_path(&path).unwrap();
        assert_eq!(loaded.events.len(), 2); // genesis + finding.noted
        assert_eq!(loaded.events[1].kind, "finding.noted");
        assert_eq!(loaded.findings[0].annotations.len(), 1);
        assert_eq!(loaded.proposals[0].status, "applied");
        assert_eq!(
            loaded.proposals[0].applied_event_id,
            result.applied_event_id
        );
        assert_eq!(loaded.proof_state.latest_packet.status, "stale");
    }

    #[test]
    fn retract_emits_per_dependent_cascade_events() {
        // Phase L: a retraction must emit one canonical
        // `finding.dependency_invalidated` event per affected dependent
        // in BFS depth order. Build a tiny dependency chain:
        //   src  <-supports- dep1  <-depends- dep2
        // and assert that retracting `src` produces three events:
        // [retracted(src), dep_invalidated(dep1, depth=1),
        //  dep_invalidated(dep2, depth=2)] all carrying the source's
        // canonical event ID as `upstream_event_id`.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let mut src = finding("vf_src");
        let mut dep1 = finding("vf_dep1");
        let mut dep2 = finding("vf_dep2");
        src.assertion.text = "src finding".into();
        dep1.assertion.text = "dep1 finding".into();
        dep2.assertion.text = "dep2 finding".into();
        // BFS edges flow from dependent → upstream via `target`.
        dep1.add_link("vf_src", "supports", "");
        dep2.add_link("vf_dep1", "depends", "");
        let frontier = project::assemble("test", vec![src, dep1, dep2], 0, 0, "test");
        repo::save_to_path(&path, &frontier).unwrap();

        let proposal = new_proposal(
            "finding.retract",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_src".to_string(),
            },
            "reviewer:test",
            "human",
            "Source paper retracted by publisher",
            json!({}),
            Vec::new(),
            Vec::new(),
        );
        create_or_apply(&path, proposal, true).unwrap();
        let loaded = repo::load_from_path(&path).unwrap();

        // genesis + 1 source retract + 2 cascade events = 4 total.
        assert_eq!(loaded.events.len(), 4, "{:?}", loaded.events);
        let kinds: Vec<&str> = loaded.events.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds[0], "frontier.created");
        assert_eq!(kinds[1], "finding.retracted");
        assert_eq!(kinds[2], "finding.dependency_invalidated");
        assert_eq!(kinds[3], "finding.dependency_invalidated");

        let source_event_id = loaded.events[1].id.clone();
        let dep1_event = &loaded.events[2];
        let dep2_event = &loaded.events[3];
        assert_eq!(dep1_event.target.id, "vf_dep1");
        assert_eq!(dep2_event.target.id, "vf_dep2");
        assert_eq!(
            dep1_event
                .payload
                .get("upstream_event_id")
                .and_then(|v| v.as_str()),
            Some(source_event_id.as_str())
        );
        assert_eq!(
            dep1_event.payload.get("depth").and_then(|v| v.as_u64()),
            Some(1)
        );
        assert_eq!(
            dep2_event.payload.get("depth").and_then(|v| v.as_u64()),
            Some(2)
        );
        // Both dependents must end up contested in materialized state.
        let dep1 = loaded.findings.iter().find(|f| f.id == "vf_dep1").unwrap();
        let dep2 = loaded.findings.iter().find(|f| f.id == "vf_dep2").unwrap();
        assert!(dep1.flags.contested);
        assert!(dep2.flags.contested);
        let src = loaded.findings.iter().find(|f| f.id == "vf_src").unwrap();
        assert!(src.flags.retracted);
    }

    #[test]
    fn proposal_id_is_content_addressed_independent_of_created_at() {
        // Phase P (v0.5): identical logical proposals constructed at different
        // times must produce the same `vpr_…`. This is the substrate property
        // that makes agent retries idempotent.
        let target = StateTarget {
            r#type: "finding".to_string(),
            id: "vf_test".to_string(),
        };
        let mut a = new_proposal(
            "finding.review",
            target.clone(),
            "reviewer:test",
            "human",
            "scope narrower than claim",
            json!({"status": "contested"}),
            Vec::new(),
            Vec::new(),
        );
        let mut b = new_proposal(
            "finding.review",
            target,
            "reviewer:test",
            "human",
            "scope narrower than claim",
            json!({"status": "contested"}),
            Vec::new(),
            Vec::new(),
        );
        // Force divergent timestamps; the IDs must still match.
        a.created_at = "2026-04-25T00:00:00Z".to_string();
        b.created_at = "2026-09-12T17:32:00Z".to_string();
        a.id = proposal_id(&a);
        b.id = proposal_id(&b);
        assert_eq!(a.id, b.id, "vpr_… must not depend on created_at");
    }

    #[test]
    fn create_or_apply_is_idempotent_under_repeated_calls() {
        // Phase P: invoking create_or_apply twice with identical content must
        // not duplicate the proposal nor emit two events. The second call
        // returns the same proposal_id and applied_event_id as the first.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
        repo::save_to_path(&path, &frontier).unwrap();

        let make = || {
            new_proposal(
                "finding.review",
                StateTarget {
                    r#type: "finding".to_string(),
                    id: "vf_test".to_string(),
                },
                "reviewer:test",
                "human",
                "agent retry test",
                json!({"status": "contested"}),
                Vec::new(),
                Vec::new(),
            )
        };

        let first = create_or_apply(&path, make(), true).unwrap();
        let second = create_or_apply(&path, make(), true).unwrap();

        assert_eq!(first.proposal_id, second.proposal_id);
        assert_eq!(first.applied_event_id, second.applied_event_id);

        let loaded = repo::load_from_path(&path).unwrap();
        assert_eq!(
            loaded.proposals.len(),
            1,
            "second create_or_apply must not insert a duplicate proposal"
        );
        // genesis + 1 applied review event = 2; not 3.
        assert_eq!(
            loaded.events.len(),
            2,
            "second create_or_apply must not emit a duplicate event"
        );
    }

    #[test]
    fn accepting_applied_proposal_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let frontier = project::assemble("test", vec![finding("vf_test")], 0, 0, "test");
        repo::save_to_path(&path, &frontier).unwrap();
        let proposal = new_proposal(
            "finding.review",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test".to_string(),
            },
            "reviewer:test",
            "human",
            "Mouse-only evidence",
            json!({"status": "contested"}),
            Vec::new(),
            Vec::new(),
        );
        let created = create_or_apply(&path, proposal, true).unwrap();
        let first_event = created.applied_event_id.clone().unwrap();
        let second_event =
            accept_at_path(&path, &created.proposal_id, "reviewer:test", "same").unwrap();
        assert_eq!(first_event, second_event);
    }
}
