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
    /// v0.22 (Agent Inbox): when a proposal originates from a scoped
    /// agent run (e.g. Literature Scout reading a PDF folder), this
    /// captures the model, the run id, and the wall-clock window.
    /// The substrate stays dumb — it does not know whether the
    /// proposer was a human, a Claude run, a GPT run, or a lab
    /// pipeline; this is informational provenance only, surfaced in
    /// the Workbench Inbox so reviewers can judge what they're
    /// looking at. Optional + skip-if-none so existing frontiers
    /// without proposals serialize byte-identically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_run: Option<AgentRun>,
}

/// Agent provenance attached to a `StateProposal`.
///
/// Doctrine: the substrate stays model-agnostic. Agents — Literature
/// Scout, Notes Compiler, Code Analyst, etc. — sit in the
/// `vela-scientist` crate (or external code) and write proposals into
/// a frontier through the existing protocol. This struct is the
/// reviewer-facing record of *who proposed what, with what model,
/// during which run* — never used as access control or trust
/// assignment.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRun {
    /// Stable agent name (e.g. "literature-scout"). Pairs with the
    /// proposal's `actor.id == "agent:literature-scout"`.
    pub agent: String,
    /// Model identifier (e.g. "claude-sonnet-4-6"). Free-form so the
    /// substrate never has to enumerate model names.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    /// Run identifier — typically a UUID or short hash. Lets the
    /// reviewer group multiple proposals that came out of the same
    /// agent invocation.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub run_id: String,
    /// ISO-8601 wall-clock start of the run.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub started_at: String,
    /// ISO-8601 wall-clock end. Optional because some agents stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    /// Free-form context the reviewer should see — e.g. the input
    /// folder path, the count of papers processed, the prompt
    /// version. Kept as a flat string map so it round-trips cleanly
    /// through canonical JSON without imposing a schema.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub context: BTreeMap<String, String>,
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

    // v0.13: materialize source/evidence/condition projections after every
    // applied proposal so the lint surface stops emitting `missing_source_record`
    // for findings whose provenance derives a SourceRecord that wasn't yet in
    // `frontier.sources`. Pre-v0.13, `vela normalize --write` was the only path
    // to populate these — but normalize refuses on event-ful frontiers, so any
    // frontier built via CLI proposals could never reach proof-ready state.
    // Materializing inline at apply time keeps source_records in lockstep with
    // findings; when no finding state changed (caveat/note/review on existing
    // findings) the projection is idempotent and bytes don't churn.
    if applied_event_id.is_some() {
        crate::sources::materialize_project(&mut frontier);
    } else {
        project::recompute_stats(&mut frontier);
    }
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
        "finding.supersede" => apply_supersede(frontier, proposal, reviewer, decision_reason)?,
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
            created_at: Utc::now().to_rfc3339(),
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
        actor_type: "human",
        reason: &proposal.reason,
        before_hash: &before_hash,
        after_hash: &after_hash,
        payload: json!({
            "proposal_id": proposal.id,
            "new_finding_id": new_finding_id,
        }),
        caveats: proposal.caveats.clone(),
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
                superseded: false,
            signature_threshold: None,
            jointly_accepted: false,
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

    #[test]
    fn v0_13_apply_materializes_source_records_inline() {
        // Pre-v0.13: vela check --strict on a CLI-built frontier flagged
        // `missing_source_record` because source_records weren't populated
        // until vela normalize --write — and normalize refuses on event-ful
        // frontiers. v0.13 materializes inline at apply time so source_records
        // grow in lockstep with findings.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let mut frontier = project::assemble("test", vec![], 0, 0, "test");
        repo::save_to_path(&path, &frontier).unwrap();
        // Add a finding via the standard finding.add proposal flow.
        let f = finding("vf_v013_inline_src");
        let proposal = new_proposal(
            "finding.add",
            StateTarget {
                r#type: "finding".to_string(),
                id: f.id.clone(),
            },
            "reviewer:test",
            "human",
            "Manual finding for v0.13 source-record materialization test",
            json!({"finding": f}),
            Vec::new(),
            Vec::new(),
        );
        create_or_apply(&path, proposal, true).unwrap();
        let loaded = repo::load_from_path(&path).unwrap();
        // Source records, evidence atoms, and condition records should all
        // be materialized — without any explicit normalize call.
        assert!(
            !loaded.sources.is_empty(),
            "v0.13: source_records should materialize inline at apply time"
        );
        assert!(
            !loaded.evidence_atoms.is_empty(),
            "v0.13: evidence_atoms should materialize inline at apply time"
        );
        assert!(
            !loaded.condition_records.is_empty(),
            "v0.13: condition_records should materialize inline at apply time"
        );
        // Sanity: stats reflect the new source registry.
        assert_eq!(loaded.stats.source_count, loaded.sources.len());
        // Suppress unused-mut warning when frontier isn't reused below.
        let _ = &mut frontier;
    }

    fn make_supersede_payload(old_id: &str, new_text: &str) -> (FindingBundle, Value) {
        let mut new_finding = finding("vf_supersede_new");
        new_finding.assertion.text = new_text.to_string();
        // Re-derive id from the new assertion text + provenance. For the
        // test we just hand-pick a distinct id; the real CLI uses
        // `build_finding_bundle` which content-addresses correctly.
        new_finding.id = format!(
            "vf_{:0>16}",
            old_id
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_add(b as u64))
        );
        let payload = json!({"new_finding": new_finding.clone()});
        (new_finding, payload)
    }

    #[test]
    fn v0_14_supersede_creates_new_finding_and_marks_old() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let mut frontier = project::assemble("test", vec![finding("vf_old")], 0, 0, "test");
        repo::save_to_path(&path, &frontier).unwrap();
        let (new_finding, payload) = make_supersede_payload("vf_old", "Newer claim");
        let proposal = new_proposal(
            "finding.supersede",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_old".to_string(),
            },
            "reviewer:test",
            "human",
            "Newer evidence updates the wording",
            payload,
            Vec::new(),
            Vec::new(),
        );
        let result = create_or_apply(&path, proposal, true).unwrap();
        assert!(result.applied_event_id.is_some());
        let loaded = repo::load_from_path(&path).unwrap();
        // Old finding now flagged superseded.
        let old = loaded.findings.iter().find(|f| f.id == "vf_old").unwrap();
        assert!(
            old.flags.superseded,
            "old finding should be flagged superseded"
        );
        // New finding present, with auto-injected supersedes link back to old.
        let new_f = loaded
            .findings
            .iter()
            .find(|f| f.id == new_finding.id)
            .expect("new finding should be in frontier");
        assert!(
            new_f
                .links
                .iter()
                .any(|l| l.target == "vf_old" && l.link_type == "supersedes"),
            "new finding should have an auto-injected supersedes link to old finding"
        );
        // Event with kind finding.superseded targeting old, payload carries new_finding_id.
        let supersede_event = loaded
            .events
            .iter()
            .find(|e| e.kind == "finding.superseded")
            .expect("a finding.superseded event should be emitted");
        assert_eq!(supersede_event.target.id, "vf_old");
        assert_eq!(
            supersede_event.payload["new_finding_id"].as_str(),
            Some(new_finding.id.as_str())
        );
        // suppress unused warning
        let _ = &mut frontier;
    }

    #[test]
    fn v0_14_supersede_refuses_already_superseded() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let mut old = finding("vf_already_done");
        old.flags.superseded = true;
        let frontier = project::assemble("test", vec![old], 0, 0, "test");
        repo::save_to_path(&path, &frontier).unwrap();
        let (_, payload) = make_supersede_payload("vf_already_done", "Newer wording");
        let proposal = new_proposal(
            "finding.supersede",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_already_done".to_string(),
            },
            "reviewer:test",
            "human",
            "Attempt to double-supersede",
            payload,
            Vec::new(),
            Vec::new(),
        );
        let result = create_or_apply(&path, proposal, true);
        assert!(
            result.is_err(),
            "double-supersede should be refused; got {result:?}"
        );
    }

    #[test]
    fn v0_14_supersede_refuses_same_content_address() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("frontier.json");
        let frontier = project::assemble("test", vec![finding("vf_same")], 0, 0, "test");
        repo::save_to_path(&path, &frontier).unwrap();
        // new_finding.id == target.id should be refused at validate-time.
        let mut new_finding = finding("vf_same");
        new_finding.assertion.text = "Different text but reused id".to_string();
        let proposal = new_proposal(
            "finding.supersede",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_same".to_string(),
            },
            "reviewer:test",
            "human",
            "Same id, should fail",
            json!({"new_finding": new_finding}),
            Vec::new(),
            Vec::new(),
        );
        let result = create_or_apply(&path, proposal, true);
        assert!(
            result.is_err(),
            "supersede with same content address should be refused; got {result:?}"
        );
    }

    /// v0.22 byte-stability: a proposal with `agent_run = None`
    /// must serialize without an `agent_run` field, so existing
    /// frontiers (none of which have agent_run today) round-trip
    /// byte-identically. The whole substrate guarantee depends on
    /// canonical-JSON not silently gaining new keys.
    #[test]
    fn agent_run_none_skips_serialization() {
        let p = new_proposal(
            "finding.add",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test0000000000".to_string(),
            },
            "reviewer:will-blair",
            "human",
            "test",
            json!({}),
            Vec::new(),
            Vec::new(),
        );
        let bytes = canonical::to_canonical_bytes(&p).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert!(
            !s.contains("agent_run"),
            "proposal without agent_run leaked the field into canonical JSON: {s}"
        );
    }

    /// And when `agent_run` *is* set, the same proposal id is
    /// produced regardless — `proposal_id`'s preimage explicitly
    /// excludes agent_run, so attaching provenance never changes
    /// the content address.
    #[test]
    fn agent_run_does_not_change_proposal_id() {
        let bare = new_proposal(
            "finding.add",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test0000000000".to_string(),
            },
            "agent:literature-scout",
            "agent",
            "scout extracted this from paper_014",
            json!({}),
            vec!["src_paper_014".to_string()],
            Vec::new(),
        );
        let id_bare = bare.id.clone();

        let mut with_run = bare.clone();
        with_run.agent_run = Some(AgentRun {
            agent: "literature-scout".to_string(),
            model: "claude-opus-4-7".to_string(),
            run_id: "vrun_abc1234567890def".to_string(),
            started_at: "2026-04-26T01:23:45Z".to_string(),
            finished_at: Some("2026-04-26T01:24:10Z".to_string()),
            context: BTreeMap::from([
                ("input_folder".to_string(), "./papers".to_string()),
                ("pdf_count".to_string(), "12".to_string()),
            ]),
        });
        let id_with_run = proposal_id(&with_run);
        assert_eq!(id_bare, id_with_run, "agent_run leaked into proposal_id preimage");
    }
}
