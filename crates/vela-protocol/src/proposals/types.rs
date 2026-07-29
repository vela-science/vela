//! Proposal data types: the StateProposal record, proof state, and
//! accept/preview/validation reports. Re-exported flat from the parent.

use super::*;

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
    /// v0.67: when an agent drafts a proposal long before the
    /// reviewer accepts it, `drafted_at` records the draft moment.
    /// `created_at` records the moment the proposal entered the
    /// canonical store. The throughput dashboard reads against
    /// `drafted_at` when present, falling back to `created_at`,
    /// so the "median proposal-to-event latency" surfaces real
    /// reviewer queue time rather than zero.
    /// Backward-compatible: pre-v0.67 proposals load with `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drafted_at: Option<String>,
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
    pub withdrawn: usize,
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
    /// Event-set commitment with only `attempt.claimed` coordination leases
    /// removed. Optional so historical proof-state records remain readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonlease_event_log_hash: Option<String>,
    pub packet_manifest_hash: Option<String>,
    pub status: String,
}

impl Default for ProofPacketState {
    fn default() -> Self {
        Self {
            generated_at: None,
            snapshot_hash: None,
            event_log_hash: None,
            nonlease_event_log_hash: None,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposalPreview {
    pub proposal_id: String,
    pub kind: String,
    pub target: StateTarget,
    pub reviewer: String,
    #[serde(default)]
    pub changed_findings: Vec<String>,
    /// Field-level before/after for each changed finding (assertion text, type,
    /// and confidence), so a reviewer reads what the change asserts, not only a
    /// count delta. Confidence is a formatted string to keep this struct `Eq`.
    #[serde(default)]
    pub changed_finding_details: Vec<ChangedFindingDetail>,
    #[serde(default)]
    pub changed_artifacts: Vec<String>,
    #[serde(default)]
    pub new_event_ids: Vec<String>,
    #[serde(default)]
    pub event_kinds: Vec<String>,
    pub findings_before: usize,
    pub findings_after: usize,
    pub findings_delta: isize,
    pub artifacts_before: usize,
    pub artifacts_after: usize,
    pub artifacts_delta: isize,
    pub events_before: usize,
    pub events_after: usize,
    pub events_delta: isize,
    pub proof_would_be_stale: bool,
    pub applied_event_id: String,
}

/// Field-level before/after for one changed finding. All fields are optional:
/// a `finding.add` has no `before`, a retract no `after`. Confidence is a
/// formatted string so the containing `ProposalPreview` can stay `Eq`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangedFindingDetail {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assertion_before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assertion_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assertion_type_before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assertion_type_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_after: Option<String>,
}

/// Build the field-level detail for each changed finding by looking it up in the
/// before and after states. Both lookups can miss (add/retract), so each side is
/// optional.

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
