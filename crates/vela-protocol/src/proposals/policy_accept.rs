//! The policy lane: canonical acceptance whose AUTHORITY is a
//! human-signed standing policy (`vap_`) instead of a per-item key
//! ceremony.
//!
//! This is the flip the acceptance-policy module staged ("Today this
//! runs in SHADOW … so the autonomy can be proven before it is
//! granted"): a human signs a scoped, revocable [`AcceptancePolicy`]
//! ONCE; the deterministic evaluator then routes each landing, and a
//! `Permit` lands the SAME canonical accept event a human key would
//! have produced — with three differences that keep custody honest:
//!
//! 1. `reviewed_by` / the event actor is `policy:<vap_id>` (a machine
//!    actor, never counted as human review — see
//!    [`crate::events::actor_kind`]).
//! 2. The event carries no key signature. Its integrity chain is the
//!    `policy_lane` payload block: the full [`DecisionCertificate`]
//!    plus a receipt/proposal reference and the exact causal pre-state,
//!    content-addressed into the event id. `vela check --strict`
//!    reconstructs that pre-state, re-derives [`PolicyContext`] and the
//!    Engine verdict from retained evidence, then re-runs the evaluator
//!    against the persisted signed policy bytes.
//! 3. The policy file that authorized the accept is persisted
//!    content-addressed under `.vela/policies/<vap_id>.json` (+ sig),
//!    so verification survives policy rotation forever.
//!
//! What this deliberately does NOT change: the engine CI gate runs
//! exactly as it does for a human accept (strict, and `force` is
//! unreachable — there is no flag); `Defer` and `Deny` land nothing;
//! and no agent key ever signs anything here — the human's authority
//! arrived earlier, once, as the policy signature.
//!
//! `vela.policy-lane.v2` binds the signed-policy timestamp, decision instant,
//! parent event ids/root, complete pre-state attachment set, receipt root,
//! derived context, Engine verdict, and decision certificate. All other lane
//! shapes and policy-signature encodings fail strict replay.
//!
//! Finite wall-clock expiry is deliberately not an unsigned-event authority
//! input: an attacker could backdate the event and recompute every content
//! address. Until Vela has an authority-signed causal time lease, only the
//! explicit non-expiring sentinel can auto-Permit; finite policies still
//! produce Defer/Deny and remain available to the human review path.

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::bundle::FindingBundle;
use crate::events;
use crate::independence::independence_from_attachments;
use crate::policy::acceptance_policy::{
    ACCEPTANCE_POLICY_V0_2_SCHEMA, ActivePolicyMode, ActivePolicySnapshot, AuthorityMode, Decision,
    DecisionCertificate, Outcome, PolicyAuthority, PolicyContext, VerifiedPolicy, evaluate,
    resolve_policy_authority, verify_policy_signature_bytes,
};
use crate::project;
use crate::receipt_v1::ReceiptV1;
use crate::verifier_attachment::{
    AttachmentOutcome, GateStatus, MethodIntegrity, claim_digest, derive_gate_status,
};

use super::EngineVerdict;

#[cfg(test)]
use crate::policy::acceptance_policy::load_active_policy_snapshot;
#[cfg(test)]
use crate::repo;

/// The payload key on a policy-lane accept event.
pub const POLICY_LANE_PAYLOAD_KEY: &str = "policy_lane";
/// Evidence-derived policy lane. Unlike v1, every decision-critical fact is
/// rederived from retained public inputs and the exact causal pre-state.
pub const POLICY_LANE_SCHEMA_V2: &str = "vela.policy-lane.v2";
pub const POLICY_HEAD_PROPOSAL_KIND: &str = "governance.policy_head";
pub const POLICY_HEAD_SCHEMA: &str = "vela.policy-head.v1";
pub const LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND: &str = "governance.policy_legacy_retirement";
pub const LEGACY_POLICY_RETIREMENT_SCHEMA: &str = "vela.policy-legacy-retirement.v1";
const POLICY_TRANSITION_ROOT_SCHEMA: &str = "vela.policy-transition-root.v1";
const MAX_REVIEW_MATERIAL_BYTES: u64 = 1024 * 1024;
/// Finite wall-clock expiry cannot authorize an unsigned event: an event can
/// self-assert an earlier timestamp after the window closes. Until policies
/// gain an authority-signed causal lease/timestamp anchor, auto-Permit is
/// limited to an explicitly non-expiring policy. Finite policies still govern
/// Defer/Deny and human review normally.
pub const CAUSALLY_UNBOUNDED_POLICY_EXPIRY: &str = "9999-12-31T23:59:59Z";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyLaneStampV2 {
    schema: String,
    policy_id: String,
    policy_signed_at: String,
    decision_time: String,
    parent_event_log_root: String,
    parent_event_ids: Vec<String>,
    prestate_attachment_ids: Vec<String>,
    policy_head_event_id: String,
    policy_head_epoch: u32,
    rule_ids: Vec<String>,
    executor: String,
    context: PolicyContext,
    engine_gate: EngineVerdict,
    certificate: DecisionCertificate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyHeadAction {
    Activate,
    Rotate,
    Revoke,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyHeadPayload {
    pub schema: String,
    pub action: PolicyHeadAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_head_event_id: Option<String>,
    pub expected_parent_event_log_root: String,
    pub parent_event_ids: Vec<String>,
    pub epoch: u32,
}

/// Closed, content-bound intent for retiring one prelaunch policy pair that
/// cannot be interpreted as current authority. Paths are intentionally absent:
/// implementations derive the fixed active paths and same-id snapshot paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyPolicyRetirementPayload {
    pub schema: String,
    pub policy_id: String,
    pub policy_bytes_root: String,
    pub signature_bytes_root: String,
    pub retire_identical_snapshot_pair: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyHead {
    pub event_id: String,
    pub policy_id: Option<String>,
    pub epoch: u32,
    pub action: PolicyHeadAction,
    pub reviewed_at: String,
    pub parent_event_ids: Vec<String>,
}

/// Public byte-level state of the active-policy pair.
///
/// `Active` means only that the content-addressed policy bytes and detached
/// signature verify. Whether those bytes can authorize an unsigned Permit is
/// reported separately by [`PermitReadiness`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyState {
    Absent,
    StagedUnsigned,
    Active,
    Broken,
}

impl PolicyState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::StagedUnsigned => "staged_unsigned",
            Self::Active => "active",
            Self::Broken => "broken",
        }
    }
}

impl From<ActivePolicyMode> for PolicyState {
    fn from(mode: ActivePolicyMode) -> Self {
        match mode {
            ActivePolicyMode::Absent => Self::Absent,
            ActivePolicyMode::StagedUnsigned => Self::StagedUnsigned,
            ActivePolicyMode::Active => Self::Active,
        }
    }
}

/// Whether an evaluator Permit can use the standing policy without a fresh
/// human key ceremony. This is intentionally independent of evaluator outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermitReadiness {
    Ready,
    HumanOnly,
    Blocked,
}

impl PermitReadiness {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::HumanOnly => "human_only",
            Self::Blocked => "blocked",
        }
    }
}

/// One pure assessment of active-policy bytes plus frontier authority.
///
/// Public surfaces expose only `state`, `permit_readiness`, and stable reason
/// codes. Resolved authority/head objects and diagnostic detail remain typed
/// internal inputs for policy-lane application and repair guidance.
#[derive(Debug, Clone)]
pub struct PolicyAssessment {
    state: PolicyState,
    permit_readiness: PermitReadiness,
    reason_codes: Vec<String>,
    detail: Option<String>,
    authority: Option<PolicyAuthority>,
    head: Option<PolicyHead>,
}

impl PolicyAssessment {
    #[must_use]
    pub fn state(&self) -> PolicyState {
        self.state
    }

    #[must_use]
    pub fn permit_readiness(&self) -> PermitReadiness {
        self.permit_readiness
    }

    #[must_use]
    pub fn reason_codes(&self) -> &[String] {
        &self.reason_codes
    }

    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    fn ready_authority(&self) -> Option<&PolicyAuthority> {
        self.authority.as_ref()
    }

    fn ready_head(&self) -> Option<&PolicyHead> {
        self.head.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedReviewMaterial {
    schema: String,
    proposal_id: String,
    receipt_root: String,
    evaluated_at: String,
    route: RetainedReviewRoute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedReviewRoute {
    schema: String,
    policy_context: PolicyContext,
    policy_decision: Option<Decision>,
    policy_state: PolicyState,
    permit_readiness: PermitReadiness,
    reason_codes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    readiness_detail: Option<String>,
    engine_gate: EngineVerdict,
}

/// What a policy-lane acceptance produced.
#[derive(Debug, Clone)]
pub struct PolicyAcceptOutcome {
    pub event_id: String,
    pub certificate: DecisionCertificate,
    pub verdict: EngineVerdict,
    /// Exact relative files that retain the already-verified human-signed
    /// policy after active-policy rotation. Transactional callers stage these
    /// bytes with the event; this list contains no private key material.
    pub policy_snapshot_files: Vec<PolicySnapshotFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySnapshotFile {
    pub relative_path: PathBuf,
    pub bytes: Vec<u8>,
}

/// One immutable route computation over a single active-policy snapshot and
/// one staged frontier. Its fields are private so applying a policy route must
/// consume the exact evaluator decision and Engine verdict produced here.
#[derive(Debug, Clone)]
pub struct StagedPolicyRoute {
    proposal_id: String,
    decision_time: String,
    state_root_before: String,
    parent_event_ids: Vec<String>,
    prestate_attachment_ids: Vec<String>,
    context: PolicyContext,
    verified: Option<VerifiedPolicy>,
    decision: Option<Decision>,
    policy_assessment: PolicyAssessment,
    engine_gate: EngineVerdict,
    policy_snapshot_files: Vec<PolicySnapshotFile>,
}

impl StagedPolicyRoute {
    #[must_use]
    pub fn proposal_id(&self) -> &str {
        &self.proposal_id
    }

    #[must_use]
    pub fn decision_time(&self) -> &str {
        &self.decision_time
    }

    #[must_use]
    pub fn state_root_before(&self) -> &str {
        &self.state_root_before
    }

    #[must_use]
    pub fn context(&self) -> &PolicyContext {
        &self.context
    }

    #[must_use]
    pub fn decision(&self) -> Option<&Decision> {
        self.decision.as_ref()
    }

    #[must_use]
    pub fn engine_gate(&self) -> &EngineVerdict {
        &self.engine_gate
    }

    #[must_use]
    pub fn policy_state(&self) -> PolicyState {
        self.policy_assessment.state()
    }

    #[must_use]
    pub fn readiness_detail(&self) -> Option<&str> {
        self.policy_assessment.detail()
    }

    #[must_use]
    pub fn permit_readiness(&self) -> PermitReadiness {
        self.policy_assessment.permit_readiness()
    }

    #[must_use]
    pub fn policy_reason_codes(&self) -> &[String] {
        self.policy_assessment.reason_codes()
    }
}

/// Why the policy lane did not land anything. `Deferred` is the normal
/// exit for work that needs a human — the caller leaves the proposal
/// pending (it becomes a sign-queue item). `Denied` is reserved for an
/// intentional evaluator denial.
#[derive(Debug, Clone)]
pub enum PolicyLaneRefusal {
    /// The evaluator routed this to a named human.
    Deferred { reasons: Vec<String> },
    /// The evaluator prohibited this outright.
    Denied { reasons: Vec<String> },
    /// A structural error (missing proposal, IO, gate block…).
    Error(String),
}

/// Non-judgment inputs to the one policy-context derivation.
///
/// Callers may supply resolved frontier facts, but they cannot supply an
/// assurance, independence, or method-integrity verdict. Those are always
/// recomputed from durable verifier attachments here.
#[derive(Debug, Clone)]
pub struct PolicyContextInputs<'a> {
    pub proposal: &'a super::StateProposal,
    pub finding: &'a FindingBundle,
    pub attachments: &'a [crate::verifier_attachment::VerifierAttachment],
    pub replayability: Option<&'a str>,
    pub execution_binding: Option<&'a crate::receipt_v1::ExecutionBindingV1>,
    pub receipt_is_body_bound: bool,
    pub credential_valid: bool,
    pub target_contested: bool,
    pub downstream_dependents: u32,
}

/// Derive every field consumed by the policy language from typed frontier
/// facts. This is the only low-level builder used by landing, replay, review,
/// policy testing, policy suggestion, CLI, and MCP projections.
#[must_use]
pub fn derive_policy_context(input: PolicyContextInputs<'_>) -> PolicyContext {
    let digest = claim_digest(&input.finding.assertion.text);
    let relevant = input
        .attachments
        .iter()
        .filter(|attachment| attachment.target == input.finding.id)
        .cloned()
        .collect::<Vec<_>>();
    let gate = derive_gate_status(&digest, &relevant);
    let independence = independence_from_attachments(&digest, &relevant);
    let method_integrity_sound = gate.status == GateStatus::Verified
        && relevant
            .iter()
            .filter(|attachment| {
                attachment.claim_digest == digest
                    && attachment.match_to_claim.matches
                    && attachment.outcome == AttachmentOutcome::Passed
            })
            .all(|attachment| attachment.method_integrity == MethodIntegrity::Sound);

    let replayability = input.replayability.unwrap_or("unknown");
    let replayability_known = matches!(
        replayability,
        "exact" | "bounded" | "approximate" | "unavailable" | "unknown"
    );
    let governance_mutation = input.proposal.kind.starts_with("governance.")
        || input.proposal.target.r#type == "governance";

    PolicyContext {
        claim_class: format!("receipt_{}", input.finding.assertion.assertion_type),
        assurance_level: if gate.status == GateStatus::Verified {
            3
        } else {
            0
        },
        impact_tier: if governance_mutation { 4 } else { 1 },
        changed_findings: 1,
        downstream_dependents: input.downstream_dependents,
        assertion_text_mutated: input.proposal.kind == "finding.add",
        target_contested: input.target_contested || gate.status == GateStatus::Refuted,
        governance_mutation,
        independence_satisfied: gate.status == GateStatus::Verified && independence.satisfied,
        method_integrity_sound,
        credential_valid: input.credential_valid,
        has_unknown_fields: !input.receipt_is_body_bound || !replayability_known,
        replayability: if replayability_known {
            replayability.to_string()
        } else {
            "unknown".to_string()
        },
        execution_binding: input.execution_binding.cloned(),
    }
}

impl std::fmt::Display for PolicyLaneRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deferred { reasons } => write!(f, "policy deferred: {}", reasons.join(", ")),
            Self::Denied { reasons } => write!(f, "policy denied: {}", reasons.join(", ")),
            Self::Error(e) => write!(f, "{e}"),
        }
    }
}

/// Derive every policy-language fact for a receipt-backed submission from
/// retained, typed evidence. Callers cannot supply assurance, independence,
/// integrity, credential, or impact booleans. This is the single derivation
/// used both before routing and during strict replay.
pub fn derive_submission_policy_context(
    frontier: &project::Project,
    proposal_id: &str,
    receipt: &ReceiptV1,
    decision_time: &str,
) -> Result<PolicyContext, String> {
    let decision_at = chrono::DateTime::parse_from_rfc3339(decision_time)
        .map_err(|error| format!("policy decision time must be RFC3339: {error}"))?;
    let proposal = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    if proposal.kind.starts_with("governance.") || proposal.target.r#type != "finding" {
        return Err(format!(
            "policy auto-admission supports receipt-backed finding proposals, got {}",
            proposal.kind
        ));
    }
    let submission = proposal
        .payload
        .get("vela_submission")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "policy proposal has no typed vela_submission links".to_string())?;
    let declared_receipt_root = submission
        .get("receipt_root")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "policy proposal has no receipt_root".to_string())?;
    let actual_receipt_root = receipt
        .canonical_root()
        .map_err(|error| format!("derive receipt root: {error}"))?;
    if actual_receipt_root != declared_receipt_root {
        return Err(format!(
            "policy proposal receipt root mismatch: declared {declared_receipt_root}, got {actual_receipt_root}"
        ));
    }
    let finding: FindingBundle = proposal
        .payload
        .get("finding")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .or_else(|| {
            frontier
                .findings
                .iter()
                .find(|finding| finding.id == proposal.target.id)
                .cloned()
        })
        .ok_or_else(|| "receipt-backed proposal has no finding body".to_string())?;
    if receipt
        .as_value()
        .get("claim")
        .and_then(serde_json::Value::as_str)
        != Some(finding.assertion.text.as_str())
        || receipt
            .as_value()
            .get("type")
            .and_then(serde_json::Value::as_str)
            != Some(finding.assertion.assertion_type.as_str())
    {
        return Err("retained receipt claim/type does not match the proposal finding".to_string());
    }
    let emitted_at = receipt
        .as_value()
        .get("provenance")
        .and_then(|value| value.get("emitted_at"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "retained receipt has no provenance.emitted_at".to_string())?;
    let emitted_at = chrono::DateTime::parse_from_rfc3339(emitted_at)
        .map_err(|error| format!("receipt emitted_at must be RFC3339: {error}"))?;
    if emitted_at > decision_at {
        return Err("receipt was emitted after the policy decision time".to_string());
    }

    let relevant = frontier
        .verifier_attachments
        .iter()
        .filter(|attachment| attachment.target == finding.id)
        .cloned()
        .collect::<Vec<_>>();
    for attachment in &relevant {
        attachment
            .verify()
            .map_err(|error| format!("policy evidence attachment {}: {error}", attachment.id))?;
    }
    let replayability = receipt
        .as_value()
        .get("replayability")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let execution_binding = receipt
        .execution_binding()
        .map_err(|error| format!("derive receipt execution binding: {error}"))?;
    let target_contested = finding.flags.contested
        || submission
            .get("same_claim_findings")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .any(|related| {
                frontier
                    .findings
                    .iter()
                    .find(|candidate| candidate.id == related)
                    .is_some_and(|candidate| candidate.flags.contested)
            });
    let downstream_dependents = frontier
        .findings
        .iter()
        .filter(|candidate| {
            candidate
                .links
                .iter()
                .any(|link| crate::bundle::bare_finding_id(&link.target) == finding.id.as_str())
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX);
    Ok(derive_policy_context(PolicyContextInputs {
        proposal,
        finding: &finding,
        attachments: &relevant,
        replayability: Some(replayability),
        execution_binding: execution_binding.as_ref(),
        receipt_is_body_bound: true,
        credential_valid: receipt_producer_credential_valid(frontier, receipt, decision_time),
        target_contested,
        downstream_dependents,
    }))
}

const EXACT_FLOOR_ARTIFACT_KIND: &str = "vela-witness";
const MAX_EXACT_FLOOR_WITNESS_BYTES: u64 = 64 * 1024 * 1024;

fn exact_receipt_floor(
    frontier_dir: &Path,
    receipt: &ReceiptV1,
    claim: &str,
) -> Result<bool, String> {
    if receipt
        .as_value()
        .get("replayability")
        .and_then(serde_json::Value::as_str)
        != Some("exact")
    {
        return Ok(false);
    }
    let artifacts = receipt
        .as_value()
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "exact-floor receipt has no artifacts".to_string())?;
    let witnesses = artifacts
        .iter()
        .filter(|artifact| {
            artifact.get("kind").and_then(serde_json::Value::as_str)
                == Some(EXACT_FLOOR_ARTIFACT_KIND)
        })
        .collect::<Vec<_>>();
    if witnesses.is_empty() {
        return Ok(false);
    }
    if witnesses.len() != 1 {
        return Err(
            "exact-floor receipt must contain exactly one vela-witness artifact".to_string(),
        );
    }
    let descriptor = witnesses[0];
    let relative = descriptor
        .get("path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "exact-floor vela-witness has no path".to_string())?;
    let relative = Path::new(relative);
    if relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err("exact-floor vela-witness path is not frontier-relative".to_string());
    }
    let declared = descriptor
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "exact-floor vela-witness has no sha256".to_string())?;
    let bytes = read_frontier_regular_file(
        frontier_dir,
        relative,
        MAX_EXACT_FLOOR_WITNESS_BYTES,
        "exact-floor vela-witness",
    )?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != declared {
        return Err(format!(
            "exact-floor vela-witness digest mismatch: declared {declared}, got {actual}"
        ));
    }
    let witness: vela_verify::Witness = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse exact-floor vela-witness: {error}"))?;
    let (verified, faithfulness) = vela_verify::verify_witness_with_claim(claim, &witness);
    if !verified.ok {
        return Ok(false);
    }
    Ok(faithfulness.faithful)
}

/// Derive a submission context under the exact policy-language version.
///
/// Policy v0.1 retains its historical context bytes. Policy v0.2 may raise a
/// new `finding.add` from A0 to A2 only by re-reading one retained,
/// digest-bound Vela-native witness and passing both the frozen verifier and
/// the claim-fidelity check. Producer-reported verifier rows remain provenance.
pub fn derive_submission_policy_context_for_policy(
    frontier_dir: &Path,
    frontier: &project::Project,
    proposal_id: &str,
    receipt: &ReceiptV1,
    decision_time: &str,
    policy_schema: &str,
) -> Result<PolicyContext, String> {
    let mut context =
        derive_submission_policy_context(frontier, proposal_id, receipt, decision_time)?;
    if policy_schema != ACCEPTANCE_POLICY_V0_2_SCHEMA {
        return Ok(context);
    }
    if exact_receipt_floor(
        frontier_dir,
        receipt,
        &context_claim(frontier, proposal_id)?,
    )? {
        context.assurance_level = context.assurance_level.max(2);
        context.method_integrity_sound = true;
    }
    Ok(context)
}

fn context_claim(frontier: &project::Project, proposal_id: &str) -> Result<String, String> {
    frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .and_then(|proposal| proposal.payload.get("finding"))
        .and_then(|finding| finding.get("assertion"))
        .and_then(|assertion| assertion.get("text"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| "receipt-backed proposal has no assertion text".to_string())
}

/// Derive policy facts for a proposal already retained in a frontier.
///
/// A parsed receipt is used only when the complete strict submission
/// derivation succeeds. Missing or incoherent material retains only the
/// structural claim class on top of [`PolicyContext::default`]; it never
/// reconstructs assurance, independence, integrity, credentials,
/// replayability, or graph impact. The caller supplies the observation instant
/// so credential validity cannot drift between policy testing, suggestion,
/// review, CLI, and MCP projections.
#[must_use]
pub fn derive_existing_proposal_policy_context(
    frontier: &project::Project,
    proposal_id: &str,
    receipt: Option<&ReceiptV1>,
    decision_time: &str,
) -> PolicyContext {
    let Some(proposal) = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
    else {
        return PolicyContext::default();
    };
    let claim_class = proposal_claim_class(proposal);
    if let Some(receipt) = receipt
        && let Ok(context) =
            derive_submission_policy_context(frontier, proposal_id, receipt, decision_time)
    {
        return context;
    }
    PolicyContext {
        claim_class,
        ..PolicyContext::default()
    }
}

/// Policy-version-aware retained-proposal projection used by policy previews.
#[must_use]
pub fn derive_existing_proposal_policy_context_for_policy(
    frontier_dir: &Path,
    frontier: &project::Project,
    proposal_id: &str,
    receipt: Option<&ReceiptV1>,
    decision_time: &str,
    policy_schema: &str,
) -> PolicyContext {
    let Some(proposal) = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
    else {
        return PolicyContext::default();
    };
    let claim_class = proposal_claim_class(proposal);
    if let Some(receipt) = receipt
        && let Ok(context) = derive_submission_policy_context_for_policy(
            frontier_dir,
            frontier,
            proposal_id,
            receipt,
            decision_time,
            policy_schema,
        )
    {
        return context;
    }
    PolicyContext {
        claim_class,
        ..PolicyContext::default()
    }
}

/// Structural class shared by existing-proposal policy projections.
#[must_use]
pub fn proposal_claim_class(proposal: &super::StateProposal) -> String {
    if proposal.kind == "finding.note" {
        return "finding_note".to_string();
    }
    if proposal.kind.starts_with("governance.") || proposal.target.r#type == "governance" {
        return "governance".to_string();
    }
    if proposal.kind == "finding.add"
        && let Some(
            claim_type @ ("computational" | "theoretical" | "empirical" | "negative"
            | "contradiction"),
        ) = proposal
            .payload
            .get("finding")
            .and_then(|finding| finding.get("assertion"))
            .and_then(|assertion| assertion.get("type"))
            .and_then(serde_json::Value::as_str)
    {
        return format!("receipt_{claim_type}");
    }
    let text = proposal
        .payload
        .get("assertion")
        .and_then(|assertion| assertion.get("text"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            proposal
                .payload
                .get("finding")
                .and_then(|finding| finding.get("assertion"))
                .and_then(|assertion| assertion.get("text"))
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            proposal
                .payload
                .get("text")
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or_default();
    classify_claim(text).to_string()
}

fn classify_claim(text: &str) -> &'static str {
    let text = text.to_lowercase();
    if text.contains("a309370") || text.contains("sidon") {
        "sidon_lower_bound"
    } else if text.contains("lean") || text.contains("formaliz") || text.contains("theorem") {
        "formal_theorem"
    } else if text.contains("oeis ") || text.contains("oeis:") {
        "oeis_sequence"
    } else if text.contains("erdős problem") || text.contains("erdos problem") {
        "erdos_problem"
    } else {
        "unknown"
    }
}

/// Resolve the receipt's producer proof-of-possession against frontier
/// authority at the fixed decision instant. Every malformed or ambiguous fact
/// fails closed.
#[must_use]
pub fn receipt_producer_credential_valid(
    frontier: &project::Project,
    receipt: &ReceiptV1,
    decision_time: &str,
) -> bool {
    let Some(binding_value) = receipt
        .as_value()
        .get("environment")
        .and_then(|value| value.get("vela:producer_context"))
        .and_then(|value| value.get("identity_binding"))
        .cloned()
    else {
        return false;
    };
    let Ok(binding) = serde_json::from_value::<crate::identity::IdentityBinding>(binding_value)
    else {
        return false;
    };
    if binding.verify().is_err() {
        return false;
    }
    let (Ok(decision_at), Ok(binding_at)) = (
        chrono::DateTime::parse_from_rfc3339(decision_time),
        chrono::DateTime::parse_from_rfc3339(&binding.created_at),
    ) else {
        return false;
    };
    if binding_at > decision_at {
        return false;
    }
    let mut matches = frontier.actors.iter().filter(|actor| {
        actor.id == binding.actor_id
            && actor.algorithm == "ed25519"
            && actor
                .public_key
                .eq_ignore_ascii_case(&binding.public_key_hex)
    });
    let Some(actor) = matches.next() else {
        return false;
    };
    if matches.next().is_some() {
        return false;
    }
    let Ok(actor_created_at) = chrono::DateTime::parse_from_rfc3339(&actor.created_at) else {
        return false;
    };
    if actor_created_at > binding_at {
        return false;
    }
    match actor.revoked_at.as_deref() {
        None => true,
        Some(revoked_at) => chrono::DateTime::parse_from_rfc3339(revoked_at)
            .is_ok_and(|revoked_at| revoked_at > decision_at),
    }
}

/// Accept a pending proposal under the frontier's active, human-signed
/// acceptance policy. The executor is the agent that drove the landing
/// (recorded in the certificate, carries zero authority). Returns
/// `Err(Deferred)` when a human is needed — the caller routes the
/// proposal to the sign queue, which is success-shaped for a landing.
#[cfg(test)]
fn accept_under_policy_at_path(
    path: &Path,
    proposal_id: &str,
    ctx: &PolicyContext,
    executor: &str,
) -> Result<PolicyAcceptOutcome, PolicyLaneRefusal> {
    let now = chrono::Utc::now().to_rfc3339();
    accept_under_policy_at_path_at(path, proposal_id, ctx, executor, &now)
}

#[cfg(test)]
fn accept_under_policy_at_path_at(
    path: &Path,
    proposal_id: &str,
    ctx: &PolicyContext,
    executor: &str,
    now: &str,
) -> Result<PolicyAcceptOutcome, PolicyLaneRefusal> {
    let mut frontier = repo::load_from_path(path).map_err(PolicyLaneRefusal::Error)?;
    let snapshot = load_active_policy_snapshot(path).map_err(PolicyLaneRefusal::Error)?;
    let staged = stage_policy_route_with_context_at(
        path,
        &frontier,
        proposal_id,
        ctx.clone(),
        now,
        &snapshot,
    )?;
    let review_context = staged.context.clone();
    let review_decision = staged.decision.clone();
    let review_policy_state = staged.policy_state();
    let review_permit_readiness = staged.permit_readiness();
    let review_reason_codes = staged.policy_reason_codes().to_vec();
    let review_readiness_detail = staged.readiness_detail().map(ToString::to_string);
    let review_engine_gate = staged.engine_gate.clone();
    let outcome = apply_staged_policy_route_in_frontier(&mut frontier, staged, executor)?;
    persist_policy_snapshot_files(path, &outcome.policy_snapshot_files)
        .map_err(PolicyLaneRefusal::Error)?;
    project::recompute_stats(&mut frontier);
    repo::save_to_path(path, &frontier).map_err(PolicyLaneRefusal::Error)?;
    persist_test_review_material(
        path,
        &frontier,
        proposal_id,
        now,
        &review_context,
        review_decision.as_ref(),
        review_policy_state,
        review_permit_readiness,
        &review_reason_codes,
        review_readiness_detail.as_deref(),
        &review_engine_gate,
    )
    .map_err(PolicyLaneRefusal::Error)?;
    Ok(outcome)
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn persist_test_review_material(
    path: &Path,
    frontier: &project::Project,
    proposal_id: &str,
    evaluated_at: &str,
    context: &PolicyContext,
    decision: Option<&Decision>,
    policy_state: PolicyState,
    permit_readiness: PermitReadiness,
    reason_codes: &[String],
    readiness_detail: Option<&str>,
    engine_gate: &EngineVerdict,
) -> Result<(), String> {
    let proposal = frontier
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("test review proposal {proposal_id} is missing"))?;
    let submission = proposal
        .payload
        .get("vela_submission")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "test review proposal has no vela_submission".to_string())?;
    let receipt_root = submission
        .get("receipt_root")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "test review proposal has no receipt_root".to_string())?;
    let review_path = submission
        .get("review_material_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "test review proposal has no review_material_path".to_string())?;
    let destination = path.join(review_path);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut material = json!({
        "schema": "vela.proposal-review-material.internal.v2",
        "proposal_id": proposal_id,
        "receipt_root": receipt_root,
        "evaluated_at": evaluated_at,
        "route": {
            "schema": "vela.staged-review-route.internal.v2",
            "policy_context": context,
            "policy_decision": decision,
            "policy_state": policy_state,
            "permit_readiness": permit_readiness,
            "reason_codes": reason_codes,
            "readiness_detail": readiness_detail,
            "engine_gate": engine_gate,
        }
    });
    if readiness_detail.is_none() {
        material["route"]
            .as_object_mut()
            .unwrap()
            .remove("readiness_detail");
    }
    std::fs::write(
        destination,
        serde_json::to_vec_pretty(&material).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

/// Pure, no-write policy route over an in-memory frontier.
///
/// The active policy and its detached signature are read and verified, but no
/// policy snapshot or scientific state is installed. On Permit, the candidate
/// event, exact context, engine verdict, and snapshot bytes are staged in the
/// supplied project. On Defer, Deny, human-only readiness, or a gate block, `frontier`
/// remains byte-for-byte unchanged.
#[cfg(test)]
fn accept_under_policy_in_frontier_at(
    path: &Path,
    frontier: &mut project::Project,
    proposal_id: &str,
    ctx: &PolicyContext,
    executor: &str,
    now: &str,
) -> Result<PolicyAcceptOutcome, PolicyLaneRefusal> {
    let snapshot = load_active_policy_snapshot(path).map_err(PolicyLaneRefusal::Error)?;
    let staged = stage_policy_route_with_context_at(
        path,
        frontier,
        proposal_id,
        ctx.clone(),
        now,
        &snapshot,
    )?;
    apply_staged_policy_route_in_frontier(frontier, staged, executor)
}

/// Compute the evaluator decision and strict Engine verdict exactly once over
/// one verified active-policy snapshot. Applying the returned opaque value
/// cannot reload policy paths or rerun a different gate.
pub fn stage_policy_route_in_frontier_at(
    path: &Path,
    frontier: &project::Project,
    proposal_id: &str,
    receipt: &ReceiptV1,
    now: &str,
    snapshot: &ActivePolicySnapshot,
) -> Result<StagedPolicyRoute, PolicyLaneRefusal> {
    let policy_assessment = assess_policy_readiness(frontier, Ok(snapshot), now);
    if policy_assessment.permit_readiness() == PermitReadiness::Blocked {
        return Err(PolicyLaneRefusal::Error(
            policy_assessment
                .detail()
                .unwrap_or("policy readiness assessment is blocked")
                .to_string(),
        ));
    }
    let policy_schema = snapshot
        .verified
        .as_ref()
        .map(|verified| verified.policy.schema.as_str());
    let context = match policy_schema {
        Some(schema) => derive_submission_policy_context_for_policy(
            path,
            frontier,
            proposal_id,
            receipt,
            now,
            schema,
        ),
        None => derive_submission_policy_context(frontier, proposal_id, receipt, now),
    }
    .map_err(PolicyLaneRefusal::Error)?;
    // Causal producer bindings are required before a signed policy can make an
    // autonomous decision. A closed or merely staged-unsigned lane has no
    // authority to exercise, so portable foreign receipts remain landable as
    // pending review without inventing Vela-private producer context.
    if policy_assessment.permit_readiness() == PermitReadiness::Ready {
        let parent_event_log_root = format!("sha256:{}", events::event_log_hash(&frontier.events));
        let receipt_parent_event_log_root =
            receipt_parent_event_log_root(receipt).ok_or_else(|| {
                PolicyLaneRefusal::Error(
                    "policy receipt has no typed producer-context event_log_root".to_string(),
                )
            })?;
        if receipt_parent_event_log_root != parent_event_log_root {
            return Err(PolicyLaneRefusal::Error(format!(
                "policy receipt is bound to {receipt_parent_event_log_root}, not the current causal pre-state {parent_event_log_root}"
            )));
        }
        let evented_attachment_ids =
            event_derived_attachment_ids(frontier).map_err(PolicyLaneRefusal::Error)?;
        let current_attachment_ids = sorted_unique_ids(
            frontier
                .verifier_attachments
                .iter()
                .map(|attachment| attachment.id.as_str()),
        );
        if evented_attachment_ids != current_attachment_ids {
            return Err(PolicyLaneRefusal::Error(format!(
                "policy pre-state attachments are not exactly event-derived (current {current_attachment_ids:?}, event-derived {evented_attachment_ids:?})"
            )));
        }
    }
    stage_policy_route_with_context_at(path, frontier, proposal_id, context, now, snapshot)
}

fn receipt_parent_event_log_root(receipt: &ReceiptV1) -> Option<&str> {
    receipt
        .as_value()
        .get("environment")?
        .get("vela:producer_context")?
        .get("event_log_root")?
        .as_str()
}

fn event_derived_attachment_ids(frontier: &project::Project) -> Result<Vec<String>, String> {
    let mut ids = BTreeSet::new();
    for event in frontier
        .events
        .iter()
        .filter(|event| event.kind == events::EVENT_KIND_VERIFIER_ATTACHMENT_ADDED)
    {
        if event.id != events::event_id(event) {
            return Err(format!(
                "verifier-attachment event {} does not rederive",
                event.id
            ));
        }
        let value = event.payload.get("attachment").ok_or_else(|| {
            format!(
                "verifier-attachment event {} has no payload.attachment",
                event.id
            )
        })?;
        let attachment: crate::verifier_attachment::VerifierAttachment =
            serde_json::from_value(value.clone()).map_err(|error| {
                format!(
                    "verifier-attachment event {} has malformed attachment: {error}",
                    event.id
                )
            })?;
        attachment.verify().map_err(|error| {
            format!(
                "verifier-attachment event {} has invalid attachment: {error}",
                event.id
            )
        })?;
        ids.insert(attachment.id);
    }
    Ok(ids.into_iter().collect())
}

fn stage_policy_route_with_context_at(
    path: &Path,
    frontier: &project::Project,
    proposal_id: &str,
    context: PolicyContext,
    now: &str,
    snapshot: &ActivePolicySnapshot,
) -> Result<StagedPolicyRoute, PolicyLaneRefusal> {
    let decision_at = chrono::DateTime::parse_from_rfc3339(now).map_err(|error| {
        PolicyLaneRefusal::Error(format!("policy evaluation time must be RFC3339: {error}"))
    })?;
    for parent in &frontier.events {
        let parent_at =
            chrono::DateTime::parse_from_rfc3339(&parent.timestamp).map_err(|error| {
                PolicyLaneRefusal::Error(format!(
                    "causal parent {} timestamp is invalid: {error}",
                    parent.id
                ))
            })?;
        if parent_at >= decision_at {
            return Err(PolicyLaneRefusal::Error(format!(
                "policy decision must occur after causal parent {}",
                parent.id
            )));
        }
    }
    if !frontier
        .proposals
        .iter()
        .any(|proposal| proposal.id == proposal_id)
    {
        return Err(PolicyLaneRefusal::Error(format!(
            "Proposal not found: {proposal_id}"
        )));
    }

    let engine_gate = super::preview_engine_verdict_in_frontier(frontier, path, proposal_id, true)
        .map_err(PolicyLaneRefusal::Error)?;
    let policy_assessment = assess_policy_readiness(frontier, Ok(snapshot), now);
    if policy_assessment.permit_readiness() == PermitReadiness::Blocked {
        return Err(PolicyLaneRefusal::Error(
            policy_assessment
                .detail()
                .unwrap_or("policy readiness assessment is blocked")
                .to_string(),
        ));
    }
    let verified = snapshot.verified.clone();
    let decision = verified
        .as_ref()
        .map(|verified| evaluate(&verified.policy, &context, now));
    let policy_snapshot_files = match &verified {
        Some(verified) => {
            prepare_policy_snapshot_files(snapshot, verified).map_err(PolicyLaneRefusal::Error)?
        }
        None => Vec::new(),
    };
    Ok(StagedPolicyRoute {
        proposal_id: proposal_id.to_string(),
        decision_time: now.to_string(),
        state_root_before: format!("sha256:{}", events::event_log_hash(&frontier.events)),
        parent_event_ids: sorted_unique_ids(frontier.events.iter().map(|event| event.id.as_str())),
        prestate_attachment_ids: sorted_unique_ids(
            frontier
                .verifier_attachments
                .iter()
                .map(|attachment| attachment.id.as_str()),
        ),
        context,
        verified,
        decision,
        policy_assessment,
        engine_gate,
        policy_snapshot_files,
    })
}

fn sorted_unique_ids<'a>(ids: impl Iterator<Item = &'a str>) -> Vec<String> {
    ids.map(ToString::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn parse_policy_head_payload(
    proposal: &super::StateProposal,
) -> Result<PolicyHeadPayload, String> {
    let payload: PolicyHeadPayload = serde_json::from_value(proposal.payload.clone())
        .map_err(|error| format!("policy-head proposal payload is malformed: {error}"))?;
    if serde_json::to_value(&payload).map_err(|error| error.to_string())? != proposal.payload {
        return Err("policy-head proposal payload is not the exact closed shape".to_string());
    }
    if payload.schema != POLICY_HEAD_SCHEMA {
        return Err(format!(
            "policy-head schema must be {POLICY_HEAD_SCHEMA}, got {}",
            payload.schema
        ));
    }
    if payload.parent_event_ids
        != sorted_unique_ids(payload.parent_event_ids.iter().map(String::as_str))
    {
        return Err("policy-head parent_event_ids must be sorted and unique".to_string());
    }
    if !payload
        .expected_parent_event_log_root
        .strip_prefix("sha256:")
        .is_some_and(|digest| {
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return Err("policy-head expected_parent_event_log_root must be sha256".to_string());
    }
    match payload.action {
        PolicyHeadAction::Activate | PolicyHeadAction::Rotate => {
            if !payload
                .policy_id
                .as_deref()
                .is_some_and(|id| id.starts_with("vap_"))
            {
                return Err("activate/rotate policy-head requires a vap_ policy_id".to_string());
            }
        }
        PolicyHeadAction::Revoke if payload.policy_id.is_some() => {
            return Err("revoke policy-head must not carry policy_id".to_string());
        }
        PolicyHeadAction::Revoke => {}
    }
    Ok(payload)
}

pub fn parse_legacy_policy_retirement_payload(
    proposal: &super::StateProposal,
) -> Result<LegacyPolicyRetirementPayload, String> {
    let payload: LegacyPolicyRetirementPayload =
        serde_json::from_value(proposal.payload.clone())
            .map_err(|error| format!("legacy-policy-retirement payload is malformed: {error}"))?;
    if serde_json::to_value(&payload).map_err(|error| error.to_string())? != proposal.payload {
        return Err("legacy-policy-retirement payload is not the exact closed shape".to_string());
    }
    if payload.schema != LEGACY_POLICY_RETIREMENT_SCHEMA {
        return Err(format!(
            "legacy-policy-retirement schema must be {LEGACY_POLICY_RETIREMENT_SCHEMA}, got {}",
            payload.schema
        ));
    }
    if !payload
        .policy_id
        .strip_prefix("vap_")
        .is_some_and(|digest| {
            digest.len() == 32
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    {
        return Err(
            "legacy-policy-retirement policy_id must be vap_ plus 32 lowercase hex characters"
                .to_string(),
        );
    }
    for (label, root) in [
        ("policy_bytes_root", payload.policy_bytes_root.as_str()),
        (
            "signature_bytes_root",
            payload.signature_bytes_root.as_str(),
        ),
    ] {
        if !root.strip_prefix("sha256:").is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }) {
            return Err(format!(
                "legacy-policy-retirement {label} must be sha256 plus 64 lowercase hex characters"
            ));
        }
    }
    Ok(payload)
}

/// Pure shape validation. Live filesystem roots, policy-head absence, replay,
/// and historical-use checks are intentionally performed by the CLI at prepare
/// and Decision Plan time: an applied retirement proposal must remain valid
/// after its target files are gone and after a later current policy is created.
pub fn validate_legacy_policy_retirement_proposal(
    frontier: &project::Project,
    proposal: &super::StateProposal,
) -> Result<(), String> {
    if proposal.kind != LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND
        || proposal.target.r#type != "governance"
        || proposal.target.id != frontier.frontier_id.as_deref().unwrap_or_default()
    {
        return Err(
            "legacy-policy-retirement proposal must target this frontier as governance".to_string(),
        );
    }
    parse_legacy_policy_retirement_payload(proposal).map(|_| ())
}

/// Refuse recovery when the named legacy object may have admitted anything.
/// A historical `policy.auto_admitted` event is conservatively unattributed,
/// so any such event blocks retirement even if no current policy-lane stamp
/// names this id.
pub fn ensure_legacy_policy_has_no_admissions(
    frontier: &project::Project,
    policy_id: &str,
) -> Result<(), String> {
    let policy_actor = format!("policy:{policy_id}");
    if frontier
        .events
        .iter()
        .any(|event| event.kind.as_str() == events::EVENT_KIND_POLICY_AUTO_ADMITTED)
    {
        return Err(
            "legacy policy retirement is unavailable because unattributed policy.auto_admitted history exists"
                .to_string(),
        );
    }
    if frontier.events.iter().any(|event| {
        event.actor.id == policy_actor
            || event
                .payload
                .get(POLICY_LANE_PAYLOAD_KEY)
                .and_then(|lane| lane.get("policy_id"))
                .and_then(serde_json::Value::as_str)
                == Some(policy_id)
    }) {
        return Err(format!(
            "legacy policy {policy_id} appears in policy-lane event history"
        ));
    }
    if frontier.proposals.iter().any(|proposal| {
        proposal.status == "applied" && proposal.reviewed_by.as_deref() == Some(&policy_actor)
    }) {
        return Err(format!(
            "legacy policy {policy_id} appears as an applied proposal reviewer"
        ));
    }
    Ok(())
}

fn verify_policy_head_review_authority(
    frontier: &project::Project,
    event: &events::StateEvent,
) -> Result<(), String> {
    if !(event.actor.id.starts_with("reviewer:") || event.actor.id.starts_with("steward:")) {
        return Err(format!(
            "policy-head review {} is not actored by a reviewer/steward",
            event.id
        ));
    }
    let mut actors = frontier
        .actors
        .iter()
        .filter(|actor| actor.id == event.actor.id && actor.algorithm == "ed25519");
    let actor = actors
        .next()
        .ok_or_else(|| format!("policy-head reviewer {} is not registered", event.actor.id))?;
    if actors.next().is_some() {
        return Err(format!(
            "policy-head reviewer {} is registered ambiguously",
            event.actor.id
        ));
    }
    let event_at = chrono::DateTime::parse_from_rfc3339(&event.timestamp)
        .map_err(|error| format!("policy-head review time is invalid: {error}"))?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&actor.created_at)
        .map_err(|error| format!("policy-head reviewer creation time is invalid: {error}"))?;
    if created_at > event_at {
        return Err("policy-head review predates reviewer registration".to_string());
    }
    if actor.revoked_at.as_deref().is_some_and(|revoked| {
        chrono::DateTime::parse_from_rfc3339(revoked).map_or(true, |time| time <= event_at)
    }) {
        return Err("policy-head review is at/after reviewer revocation".to_string());
    }
    if !crate::sign::verify_event_signature(event, &actor.public_key)? {
        return Err("policy-head review signature does not verify".to_string());
    }
    Ok(())
}

/// Derive the one linear, human-signed policy-head chain. Any fork, gap,
/// non-monotone epoch, stale parent set/root, or unsigned review fails closed.
pub fn derive_policy_head_chain(frontier: &project::Project) -> Result<Vec<PolicyHead>, String> {
    let proposals = frontier
        .proposals
        .iter()
        .filter(|proposal| proposal.kind == POLICY_HEAD_PROPOSAL_KIND)
        .map(|proposal| (proposal.id.as_str(), proposal))
        .collect::<HashMap<_, _>>();
    let events_by_id = frontier
        .events
        .iter()
        .map(|event| (event.id.as_str(), event))
        .collect::<HashMap<_, _>>();
    if events_by_id.len() != frontier.events.len() {
        return Err("policy-head derivation found duplicate event ids".to_string());
    }
    let canonical_events = crate::reducer::sorted_for_replay(&frontier.events);
    let event_positions = canonical_events
        .iter()
        .enumerate()
        .map(|(index, event)| (event.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut accepted = Vec::new();
    for event in frontier
        .events
        .iter()
        .filter(|event| event.kind.as_str() == events::EVENT_KIND_REVIEW_ACCEPTED)
    {
        let review: events::ReviewDecisionPayload =
            serde_json::from_value(event.payload.clone())
                .map_err(|error| format!("parse review event {}: {error}", event.id))?;
        if review.proposal_kind != POLICY_HEAD_PROPOSAL_KIND {
            continue;
        }
        if event.id != events::event_id(event) {
            return Err(format!("policy-head review {} does not rederive", event.id));
        }
        verify_policy_head_review_authority(frontier, event)?;
        let proposal = proposals.get(review.proposal_id.as_str()).ok_or_else(|| {
            format!(
                "policy-head review {} references missing proposal {}",
                event.id, review.proposal_id
            )
        })?;
        if proposal.id != super::proposal_id(proposal)
            || event.target.r#type != "proposal"
            || event.target.id != proposal.id
            || review.verdict != "accepted"
            || proposal.status != "applied"
            || proposal.applied_event_id.as_deref() != Some(event.id.as_str())
        {
            return Err(format!(
                "policy-head proposal/review linkage is inconsistent at {}",
                event.id
            ));
        }
        let payload = parse_policy_head_payload(proposal)?;
        if payload.parent_event_ids.iter().any(|id| id == &event.id) {
            return Err("policy-head review cannot be its own parent".to_string());
        }
        let review_position = *event_positions
            .get(event.id.as_str())
            .expect("review event came from the event vector");
        let exact_parent_ids = sorted_unique_ids(
            canonical_events[..review_position]
                .iter()
                .map(|parent| parent.id.as_str()),
        );
        if payload.parent_event_ids != exact_parent_ids {
            return Err(format!(
                "policy-head {} does not commit the exact preceding event-log prefix",
                event.id
            ));
        }
        let mut parents = Vec::with_capacity(payload.parent_event_ids.len());
        for id in &payload.parent_event_ids {
            let parent = events_by_id
                .get(id.as_str())
                .ok_or_else(|| format!("policy-head parent {id} is missing"))?;
            let parent_position = *event_positions
                .get(id.as_str())
                .expect("parent map and position map share the event vector");
            if parent_position >= review_position {
                return Err(format!(
                    "policy-head parent {id} does not occur before review {} in the event log",
                    event.id
                ));
            }
            if parent.id != events::event_id(parent) {
                return Err(format!("policy-head parent {id} does not rederive"));
            }
            parents.push((*parent).clone());
        }
        let root = format!("sha256:{}", events::event_log_hash(&parents));
        if root != payload.expected_parent_event_log_root {
            return Err(format!("policy-head {} parent root mismatch", event.id));
        }
        accepted.push((payload.epoch, event, payload));
    }
    accepted.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.id.cmp(&right.1.id)));
    let mut chain: Vec<PolicyHead> = Vec::with_capacity(accepted.len());
    let mut selected_policy_ids = BTreeSet::new();
    for (index, (epoch, event, payload)) in accepted.into_iter().enumerate() {
        let expected_epoch = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if epoch != expected_epoch {
            return Err(format!(
                "policy-head chain has fork/gap at epoch {epoch}; expected {expected_epoch}"
            ));
        }
        if index == 0 {
            if payload.action != PolicyHeadAction::Activate || payload.prior_head_event_id.is_some()
            {
                return Err("policy-head epoch 1 must be an unparented activate".to_string());
            }
        } else {
            let prior = chain.last().expect("non-empty chain");
            if payload.action == PolicyHeadAction::Activate
                || (prior.action == PolicyHeadAction::Revoke
                    && payload.action != PolicyHeadAction::Rotate)
                || payload.prior_head_event_id.as_deref() != Some(prior.event_id.as_str())
                || !payload
                    .parent_event_ids
                    .iter()
                    .any(|id| id == &prior.event_id)
            {
                return Err(format!(
                    "policy-head epoch {epoch} does not extend epoch {}",
                    prior.epoch
                ));
            }
        }
        if let Some(policy_id) = payload.policy_id.as_ref()
            && !selected_policy_ids.insert(policy_id.clone())
        {
            return Err(format!(
                "policy-head epoch {epoch} attempts to resurrect previously selected policy {policy_id}"
            ));
        }
        chain.push(PolicyHead {
            event_id: event.id.clone(),
            policy_id: payload.policy_id,
            epoch,
            action: payload.action,
            reviewed_at: event.timestamp.clone(),
            parent_event_ids: payload.parent_event_ids,
        });
    }
    Ok(chain)
}

#[must_use]
pub fn current_policy_head(frontier: &project::Project) -> Result<Option<PolicyHead>, String> {
    Ok(derive_policy_head_chain(frontier)?.pop())
}

/// Assess active-policy byte state and standing Permit authority once.
///
/// A malformed active pair is supplied as `Err` and is blocked. Valid signed
/// bytes remain `active` even when a missing/revoked/mismatched policy head,
/// finite wall-clock window, or unresolved signer keeps Permit human-only.
/// This separation prevents unavailable infrastructure from being
/// misrepresented as an evaluator Deny.
#[must_use]
pub fn assess_policy_readiness(
    frontier: &project::Project,
    snapshot: Result<&ActivePolicySnapshot, &str>,
    observed_at: &str,
) -> PolicyAssessment {
    let snapshot = match snapshot {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return PolicyAssessment {
                state: PolicyState::Broken,
                permit_readiness: PermitReadiness::Blocked,
                reason_codes: vec!["active_policy_broken".to_string()],
                detail: Some(error.to_string()),
                authority: None,
                head: None,
            };
        }
    };
    let state = PolicyState::from(snapshot.mode);
    if let Err(error) = chrono::DateTime::parse_from_rfc3339(observed_at) {
        return PolicyAssessment {
            state,
            permit_readiness: PermitReadiness::Blocked,
            reason_codes: vec!["policy_observation_time_invalid".to_string()],
            detail: Some(format!("policy observation time must be RFC3339: {error}")),
            authority: None,
            head: None,
        };
    }
    let head = match current_policy_head(frontier) {
        Ok(head) => head,
        Err(error) => {
            return PolicyAssessment {
                state,
                permit_readiness: PermitReadiness::Blocked,
                reason_codes: vec!["policy_head_invalid".to_string()],
                detail: Some(format!("policy-head chain is invalid: {error}")),
                authority: None,
                head: None,
            };
        }
    };

    match snapshot.mode {
        ActivePolicyMode::Absent => PolicyAssessment {
            state,
            permit_readiness: PermitReadiness::HumanOnly,
            reason_codes: vec!["policy_absent".to_string()],
            detail: None,
            authority: None,
            head,
        },
        ActivePolicyMode::StagedUnsigned => {
            let mut reason_codes = vec!["policy_unsigned".to_string()];
            if head
                .as_ref()
                .is_some_and(|head| head.action == PolicyHeadAction::Revoke)
            {
                reason_codes.push("policy_revoked".to_string());
            }
            PolicyAssessment {
                state,
                permit_readiness: PermitReadiness::HumanOnly,
                reason_codes,
                detail: None,
                authority: None,
                head,
            }
        }
        ActivePolicyMode::Active => {
            let Some(verified) = snapshot.verified.as_ref() else {
                return PolicyAssessment {
                    state: PolicyState::Broken,
                    permit_readiness: PermitReadiness::Blocked,
                    reason_codes: vec!["active_policy_broken".to_string()],
                    detail: Some("active policy snapshot has no verified policy".to_string()),
                    authority: None,
                    head: None,
                };
            };
            let mut reason_codes = Vec::new();
            let mut detail = None;
            if verified.policy.expires_at != CAUSALLY_UNBOUNDED_POLICY_EXPIRY {
                reason_codes.push("policy_wall_clock_expiry_unanchored".to_string());
                if verified.policy.is_expired(observed_at) {
                    reason_codes.push("policy_expired".to_string());
                }
            }
            if verified.policy.revocation_ref.is_some() {
                reason_codes.push("policy_revoked".to_string());
            }
            match head.as_ref() {
                None => reason_codes.push("policy_head_missing".to_string()),
                Some(head) if head.action == PolicyHeadAction::Revoke => {
                    reason_codes.push("policy_head_revoked".to_string());
                }
                Some(head) if head.policy_id.as_deref() != Some(verified.policy.id.as_str()) => {
                    reason_codes.push("policy_head_mismatch".to_string());
                }
                Some(_) => {}
            }

            let head_matches = head.as_ref().is_some_and(|head| {
                head.action != PolicyHeadAction::Revoke
                    && head.policy_id.as_deref() == Some(verified.policy.id.as_str())
            });
            let expired = reason_codes.iter().any(|code| code == "policy_expired");
            let authority = if head_matches && !expired && verified.policy.revocation_ref.is_none()
            {
                match resolve_policy_authority(frontier, verified, observed_at) {
                    Ok(authority) => Some(authority),
                    Err(error) => {
                        reason_codes.push("policy_authority_invalid".to_string());
                        detail = Some(error);
                        None
                    }
                }
            } else {
                None
            };
            if reason_codes.is_empty() {
                PolicyAssessment {
                    state,
                    permit_readiness: PermitReadiness::Ready,
                    reason_codes,
                    detail,
                    authority,
                    head,
                }
            } else {
                PolicyAssessment {
                    state,
                    permit_readiness: PermitReadiness::HumanOnly,
                    reason_codes,
                    detail,
                    authority: None,
                    head,
                }
            }
        }
    }
}

/// Validate a pending head proposal against the exact current causal state.
/// This is called again under the human accept lock, so a concurrent event
/// stales the proposal instead of silently changing its authority base.
pub fn validate_policy_head_proposal(
    frontier: &project::Project,
    proposal: &super::StateProposal,
) -> Result<(), String> {
    if proposal.kind != POLICY_HEAD_PROPOSAL_KIND
        || proposal.target.r#type != "governance"
        || proposal.target.id != frontier.frontier_id.as_deref().unwrap_or_default()
    {
        return Err("policy-head proposal must target this frontier as governance".to_string());
    }
    let payload = parse_policy_head_payload(proposal)?;
    let current_ids = sorted_unique_ids(frontier.events.iter().map(|event| event.id.as_str()));
    let current_root = format!("sha256:{}", events::event_log_hash(&frontier.events));
    if payload.parent_event_ids != current_ids
        || payload.expected_parent_event_log_root != current_root
    {
        return Err("policy-head proposal is stale against the current event log".to_string());
    }
    let chain = derive_policy_head_chain(frontier)?;
    let current = chain.last().cloned();
    match (current.as_ref(), payload.action) {
        (None, PolicyHeadAction::Activate)
            if payload.epoch == 1 && payload.prior_head_event_id.is_none() => {}
        (Some(head), PolicyHeadAction::Rotate)
            if payload.epoch == head.epoch.saturating_add(1)
                && payload.prior_head_event_id.as_deref() == Some(head.event_id.as_str()) => {}
        (Some(head), PolicyHeadAction::Revoke)
            if head.action != PolicyHeadAction::Revoke
                && payload.epoch == head.epoch.saturating_add(1)
                && payload.prior_head_event_id.as_deref() == Some(head.event_id.as_str()) => {}
        _ => {
            return Err(
                "policy-head action/epoch/prior does not extend the current head".to_string(),
            );
        }
    }
    if payload.action == PolicyHeadAction::Rotate
        && payload.policy_id == current.and_then(|head| head.policy_id)
    {
        return Err("policy-head rotate must name a different policy".to_string());
    }
    if payload.action == PolicyHeadAction::Rotate
        && payload.policy_id.as_ref().is_some_and(|policy_id| {
            chain
                .iter()
                .any(|head| head.policy_id.as_ref() == Some(policy_id))
        })
    {
        return Err("policy-head rotate cannot resurrect a previously selected policy".to_string());
    }
    Ok(())
}

/// Apply a previously staged route. This function consumes, rather than
/// recomputes, the exact policy decision and Engine verdict that review
/// material records.
pub fn apply_staged_policy_route_in_frontier(
    frontier: &mut project::Project,
    staged: StagedPolicyRoute,
    executor: &str,
) -> Result<PolicyAcceptOutcome, PolicyLaneRefusal> {
    let executor = executor.trim();
    if !(executor.starts_with("agent:") || executor.starts_with("ci:")) {
        return Err(PolicyLaneRefusal::Error(format!(
            "policy-lane executor must be an agent:/ci: actor, got `{executor}` — humans accept \
             with their key via `vela sign`"
        )));
    }
    if format!("sha256:{}", events::event_log_hash(&frontier.events)) != staged.state_root_before {
        return Err(PolicyLaneRefusal::Error(
            "staged policy route no longer matches the frontier event root".to_string(),
        ));
    }
    let Some(verified) = staged.verified.as_ref() else {
        return Err(PolicyLaneRefusal::Deferred {
            reasons: staged.policy_assessment.reason_codes().to_vec(),
        });
    };
    let decision = staged
        .decision
        .as_ref()
        .ok_or_else(|| PolicyLaneRefusal::Error("active route lost its decision".to_string()))?;
    match decision.outcome {
        Outcome::Permit => {}
        Outcome::Defer => {
            return Err(PolicyLaneRefusal::Deferred {
                reasons: decision.reasons.clone(),
            });
        }
        Outcome::Deny
            if staged.policy_assessment.permit_readiness() == PermitReadiness::HumanOnly
                && decision.reasons.iter().any(|reason| {
                    matches!(reason.as_str(), "policy_expired" | "policy_revoked")
                })
                && decision.reasons.iter().all(|reason| {
                    matches!(reason.as_str(), "policy_expired" | "policy_revoked")
                }) =>
        {
            return Err(PolicyLaneRefusal::Deferred {
                reasons: staged.policy_assessment.reason_codes().to_vec(),
            });
        }
        Outcome::Deny => {
            return Err(PolicyLaneRefusal::Denied {
                reasons: decision.reasons.clone(),
            });
        }
    }
    match staged.policy_assessment.permit_readiness() {
        PermitReadiness::HumanOnly => {
            return Err(PolicyLaneRefusal::Deferred {
                reasons: staged.policy_assessment.reason_codes().to_vec(),
            });
        }
        PermitReadiness::Blocked => {
            return Err(PolicyLaneRefusal::Error(
                staged
                    .policy_assessment
                    .detail()
                    .unwrap_or("policy readiness assessment is blocked")
                    .to_string(),
            ));
        }
        PermitReadiness::Ready => {}
    }
    let authority = staged
        .policy_assessment
        .ready_authority()
        .ok_or_else(|| PolicyLaneRefusal::Error("ready policy lost its authority".to_string()))?;
    let policy_head = staged
        .policy_assessment
        .ready_head()
        .ok_or_else(|| PolicyLaneRefusal::Error("ready policy lost its head".to_string()))?;
    if staged.engine_gate.status == "blocked" {
        return Err(PolicyLaneRefusal::Error(format!(
            "engine gate blocked policy-lane accept of {}: {} new blocking, {} new warning(s) \
             — nothing landed (the policy lane has no --force)",
            staged.proposal_id,
            staged.engine_gate.new_blocking.len(),
            staged.engine_gate.new_warnings.len()
        )));
    }
    let mut candidate: project::Project =
        serde_json::from_value(serde_json::to_value(&*frontier).map_err(|error| {
            PolicyLaneRefusal::Error(format!("clone staged frontier: {error}"))
        })?)
        .map_err(|error| PolicyLaneRefusal::Error(format!("clone staged frontier: {error}")))?;
    let (event_id, certificate) = accept_in_frontier_under_policy(
        &mut candidate,
        &staged.proposal_id,
        verified,
        authority,
        decision,
        &staged.context,
        &staged.engine_gate,
        &staged.state_root_before,
        &staged.parent_event_ids,
        &staged.prestate_attachment_ids,
        &policy_head.event_id,
        policy_head.epoch,
        executor,
        &staged.decision_time,
    )
    .map_err(PolicyLaneRefusal::Error)?;
    project::recompute_stats(&mut candidate);
    *frontier = candidate;
    Ok(PolicyAcceptOutcome {
        event_id,
        certificate,
        verdict: staged.engine_gate,
        policy_snapshot_files: staged.policy_snapshot_files,
    })
}

/// Derive the two certificate transition roots without a self-reference.
///
/// These are intentionally not whole-log roots. For a v1 policy-lane event,
/// `state_root_before` commits to the event content immediately before the
/// lane stamp (the complete `policy_lane` field removed), while
/// `state_root_after` commits to the final lane-stamped event with only the
/// certificate removed. Event `id`, signatures, and schema-artifact hints are
/// excluded by [`events::event_content_preimage_bytes`], matching normal event
/// content-addressing. Both roots therefore rederive from the final event,
/// while the final event can still content-address the certificate.
fn policy_transition_roots(event: &events::StateEvent) -> Result<(String, String), String> {
    let mut before = event.clone();
    let before_payload = before
        .payload
        .as_object_mut()
        .ok_or_else(|| "policy-lane event payload is not an object".to_string())?;
    before_payload.remove(POLICY_LANE_PAYLOAD_KEY);

    let mut after = event.clone();
    let lane = after
        .payload
        .get_mut(POLICY_LANE_PAYLOAD_KEY)
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| "policy-lane event has no object lane stamp".to_string())?;
    lane.remove("certificate");

    let root = |phase: &str, projected: &events::StateEvent| -> Result<String, String> {
        let event_content: serde_json::Value =
            serde_json::from_slice(&events::event_content_preimage_bytes(projected))
                .map_err(|error| format!("decode policy transition projection: {error}"))?;
        crate::canonical::sha256_canonical(&json!({
            "schema": POLICY_TRANSITION_ROOT_SCHEMA,
            "phase": phase,
            "event": event_content,
        }))
        .map(|digest| format!("sha256:{digest}"))
    };
    Ok((
        root("before_policy_lane_stamp", &before)?,
        root("after_without_certificate", &after)?,
    ))
}

/// The in-memory apply: mirrors `accept_proposal_in_frontier_with_custody`
/// with the authority swapped from reviewer-key custody to the verified
/// policy + certificate. No key is read; no signature lands on the event.
fn accept_in_frontier_under_policy(
    frontier: &mut project::Project,
    proposal_id: &str,
    verified: &VerifiedPolicy,
    authority: &PolicyAuthority,
    decision: &Decision,
    ctx: &PolicyContext,
    engine_gate: &EngineVerdict,
    parent_event_log_root: &str,
    parent_event_ids: &[String],
    prestate_attachment_ids: &[String],
    policy_head_event_id: &str,
    policy_head_epoch: u32,
    executor: &str,
    now: &str,
) -> Result<(String, DecisionCertificate), String> {
    let index = frontier
        .proposals
        .iter()
        .position(|p| p.id == proposal_id)
        .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?;
    let status = frontier.proposals[index].status.clone();
    if status == "rejected" {
        return Err(format!("Cannot accept rejected proposal {proposal_id}"));
    }
    if status == "applied" {
        return Err(format!(
            "Proposal {proposal_id} is already applied (idempotent no-op is the caller's exit 5)"
        ));
    }
    let proposal = frontier.proposals[index].clone();
    super::validate_proposal_shape(frontier, &proposal)?;

    let reviewer = format!("policy:{}", verified.policy.id);
    let reason = format!(
        "policy permit under {} (rules: {})",
        verified.policy.id,
        decision.matched_rule_ids.join(", ")
    );

    frontier.proposals[index].status = "accepted".to_string();
    frontier.proposals[index].reviewed_by = Some(reviewer.clone());
    frontier.proposals[index].reviewed_at = Some(now.to_string());
    frontier.proposals[index].decision_reason = Some(reason.clone());

    let initial_event_id = super::apply_proposal(frontier, &proposal, &reviewer, &reason, None)?;
    let event_index = frontier
        .events
        .iter()
        .position(|event| event.id == initial_event_id)
        .ok_or_else(|| format!("applied event {initial_event_id} not found"))?;
    {
        let event = &mut frontier.events[event_index];
        event.timestamp = now.to_string();
        event.id = events::event_id(event);
    }

    // First stamp every transition fact except the certificate. The two
    // certificate roots are then rederivable from this event projection,
    // avoiding a certificate -> event -> certificate hash cycle.
    {
        let ev = &mut frontier.events[event_index];
        if let serde_json::Value::Object(map) = &mut ev.payload {
            map.insert(
                POLICY_LANE_PAYLOAD_KEY.to_string(),
                json!({
                    "schema": POLICY_LANE_SCHEMA_V2,
                    "policy_id": verified.policy.id,
                    "policy_signed_at": verified.signed_at,
                    "decision_time": now,
                    "parent_event_log_root": parent_event_log_root,
                    "parent_event_ids": parent_event_ids,
                    "prestate_attachment_ids": prestate_attachment_ids,
                    "policy_head_event_id": policy_head_event_id,
                    "policy_head_epoch": policy_head_epoch,
                    "rule_ids": decision.matched_rule_ids,
                    "executor": executor,
                    "context": ctx,
                    "engine_gate": engine_gate,
                }),
            );
        } else {
            return Err("accept event payload is not an object".to_string());
        }
    }
    let (_, transition_root_after) = policy_transition_roots(&frontier.events[event_index])?;
    let assurance_profile = format!("assurance_level_a{}", ctx.assurance_level);
    let certificate = DecisionCertificate::build(
        decision,
        frontier.frontier_id.as_deref().unwrap_or_default(),
        proposal_id,
        parent_event_log_root,
        &transition_root_after,
        AuthorityMode::PolicyDelegation,
        authority.human_authorizers.clone(),
        executor,
        &assurance_profile,
        ctx.assurance_level,
        &proposal_claim_digest(&proposal),
        ctx.impact_tier,
        false,
    );

    // The full certificate enters the final event content address.
    let stamped_id = {
        let ev = &mut frontier.events[event_index];
        ev.payload
            .get_mut(POLICY_LANE_PAYLOAD_KEY)
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| "policy-lane stamp disappeared before certification".to_string())?
            .insert(
                "certificate".to_string(),
                serde_json::to_value(&certificate)
                    .map_err(|error| format!("encode decision certificate: {error}"))?,
            );
        ev.id = events::event_id(ev);
        ev.id.clone()
    };
    frontier.proposals[index].status = "applied".to_string();
    frontier.proposals[index].applied_event_id = Some(stamped_id.clone());

    Ok((stamped_id, certificate))
}

/// The content digest of what the proposal asserts — the same digest
/// family the exact-lane attachments bind to. Empty-payload proposals
/// digest the empty string (structurally valid; the evaluator's context
/// carries the real assurance story).
fn proposal_claim_digest(proposal: &super::StateProposal) -> String {
    let text = proposal
        .payload
        .get("assertion")
        .and_then(|a| a.get("text"))
        .and_then(|t| t.as_str())
        .or_else(|| {
            proposal
                .payload
                .get("finding")
                .and_then(|finding| finding.get("assertion"))
                .and_then(|assertion| assertion.get("text"))
                .and_then(|text| text.as_str())
        })
        .or_else(|| proposal.payload.get("text").and_then(|t| t.as_str()))
        .unwrap_or_default();
    crate::verifier_attachment::claim_digest(text)
}

/// Prepare the exact, already-verified active bytes under content-addressed
/// relative paths. No mutable policy path is read a second time.
pub fn prepare_policy_snapshot_files(
    snapshot: &ActivePolicySnapshot,
    verified: &VerifiedPolicy,
) -> Result<Vec<PolicySnapshotFile>, String> {
    let policy = snapshot
        .policy_bytes
        .clone()
        .ok_or_else(|| "verified active policy snapshot lost its policy bytes".to_string())?;
    let signature = snapshot
        .signature_bytes
        .clone()
        .ok_or_else(|| "verified active policy snapshot lost its signature bytes".to_string())?;
    let reverified = verify_policy_signature_bytes(
        &policy,
        &signature,
        Some(&verified.policy.id),
        "active policy snapshot",
    )?;
    if reverified.signer_pubkey_hex != verified.signer_pubkey_hex
        || reverified.signed_at != verified.signed_at
    {
        return Err("active policy snapshot does not match its verified policy".to_string());
    }
    Ok(vec![
        PolicySnapshotFile {
            relative_path: PathBuf::from(format!(".vela/policies/{}.json", verified.policy.id)),
            bytes: policy,
        },
        PolicySnapshotFile {
            relative_path: PathBuf::from(format!(".vela/policies/{}.sig.json", verified.policy.id)),
            bytes: signature,
        },
    ])
}

#[cfg(test)]
fn persist_policy_snapshot_files(
    frontier_dir: &Path,
    files: &[PolicySnapshotFile],
) -> Result<(), String> {
    for file in files {
        let path = frontier_dir.join(&file.relative_path);
        if path.exists() {
            let existing = std::fs::read(&path).map_err(|error| error.to_string())?;
            if existing != file.bytes {
                return Err(format!(
                    "policy snapshot {} already exists with different bytes",
                    path.display()
                ));
            }
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(&path, &file.bytes).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Verify every policy-lane event in a project against the persisted
/// signed policies and retained public evidence. Routing, context, Engine
/// verdict, causal pre-state, authority, and certificate must all rederive.
/// Returns one error string per failing event; empty = all lanes verify.
pub fn verify_policy_lane_events(project: &project::Project, frontier_dir: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    for ev in &project.events {
        let Some(lane_value) = ev.payload.get(POLICY_LANE_PAYLOAD_KEY) else {
            continue;
        };
        let result = verify_policy_lane_event_v2(project, frontier_dir, ev, lane_value);
        if let Err(error) = result {
            errors.push(format!("{}: {error}", ev.id));
        }
    }
    errors
}

fn verify_policy_lane_event_v2(
    project: &project::Project,
    frontier_dir: &Path,
    event: &events::StateEvent,
    lane_value: &serde_json::Value,
) -> Result<(), String> {
    let schema = lane_value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            "policy_lane schema is required; strict replay accepts only vela.policy-lane.v2"
                .to_string()
        })?;
    if schema != POLICY_LANE_SCHEMA_V2 {
        return Err(format!(
            "unsupported policy_lane schema {schema}; strict replay requires {POLICY_LANE_SCHEMA_V2}"
        ));
    }
    let lane: PolicyLaneStampV2 = serde_json::from_value(lane_value.clone())
        .map_err(|error| format!("policy_lane v2 is malformed or open-ended: {error}"))?;
    if serde_json::to_value(&lane).map_err(|error| format!("normalize policy_lane v2: {error}"))?
        != *lane_value
    {
        return Err(
            "policy_lane v2 is not the exact closed typed shape (unknown or defaulted field)"
                .to_string(),
        );
    }
    if event.id != events::event_id(event) {
        return Err("policy_lane event id does not rederive".to_string());
    }
    if event.signature.is_some() {
        return Err("policy_lane event must not carry an executor signature".to_string());
    }
    if event.actor.r#type != events::actor_kind(&event.actor.id) {
        return Err("policy_lane event actor type is inconsistent".to_string());
    }
    if event.actor.id != format!("policy:{}", lane.policy_id) {
        return Err(format!(
            "policy_lane actor mismatch ({} vs policy:{})",
            event.actor.id, lane.policy_id
        ));
    }
    if !(lane.executor.starts_with("agent:") || lane.executor.starts_with("ci:")) {
        return Err("policy_lane executor must be an agent:/ci: actor".to_string());
    }
    if lane.decision_time != event.timestamp {
        return Err("policy_lane decision_time does not match event timestamp".to_string());
    }
    let decision_at = chrono::DateTime::parse_from_rfc3339(&lane.decision_time)
        .map_err(|error| format!("policy_lane decision_time is not RFC3339: {error}"))?;
    verify_policy_lane_head_binding(project, event, &lane)?;
    let verified = load_policy_snapshot(frontier_dir, &lane.policy_id)?;
    if verified.policy.expires_at != CAUSALLY_UNBOUNDED_POLICY_EXPIRY {
        return Err(
            "policy_wall_clock_expiry_unanchored: strict replay cannot prove an unsigned Permit occurred inside a finite wall-clock window"
                .to_string(),
        );
    }
    if lane.policy_signed_at != verified.signed_at {
        return Err("policy_lane policy_signed_at does not match the signed snapshot".to_string());
    }
    let signed_at = chrono::DateTime::parse_from_rfc3339(&verified.signed_at)
        .map_err(|error| format!("policy signed_at is not RFC3339: {error}"))?;
    if signed_at > decision_at {
        return Err("policy decision predates its authority signature".to_string());
    }

    let (mut prestate, parent_root) = reconstruct_policy_prestate(project, event, &lane)?;
    if parent_root != lane.parent_event_log_root {
        return Err(format!(
            "policy_lane parent root mismatch: stamped {}, rederived {parent_root}",
            lane.parent_event_log_root
        ));
    }
    let authority =
        resolve_policy_authority(&prestate, &verified, &lane.decision_time).map_err(|error| {
            format!(
                "policy snapshot {} has no frontier authority: {error}",
                lane.policy_id
            )
        })?;

    let cert = &lane.certificate;
    let proposal = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == cert.proposal_id)
        .ok_or_else(|| format!("certificate proposal {} is missing", cert.proposal_id))?;
    let payload_proposal_id = event
        .payload
        .get("proposal_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "policy_lane event payload is missing proposal_id".to_string())?;
    if payload_proposal_id != proposal.id {
        return Err("event proposal_id does not match the certificate proposal".to_string());
    }
    if proposal.status != "applied"
        || proposal.applied_event_id.as_deref() != Some(event.id.as_str())
    {
        return Err("proposal applied_event_id does not link to the policy_lane event".to_string());
    }
    if proposal.target.r#type != event.target.r#type || proposal.target.id != event.target.id {
        return Err("policy_lane event target does not match its proposal".to_string());
    }
    if proposal.reviewed_by.as_deref() != Some(event.actor.id.as_str())
        || proposal.reviewed_at.as_deref() != Some(event.timestamp.as_str())
    {
        return Err("proposal review metadata does not match the policy_lane event".to_string());
    }
    let proposal_created_at = chrono::DateTime::parse_from_rfc3339(&proposal.created_at)
        .map_err(|error| format!("proposal created_at is not RFC3339: {error}"))?;
    if proposal_created_at > decision_at {
        return Err("policy decision predates the retained proposal".to_string());
    }

    let mut staged_proposal = proposal.clone();
    staged_proposal.status = "pending_review".to_string();
    staged_proposal.reviewed_by = None;
    staged_proposal.reviewed_at = None;
    staged_proposal.decision_reason = None;
    staged_proposal.applied_event_id = None;
    prestate.proposals.push(staged_proposal);
    project::recompute_stats(&mut prestate);

    let receipt = load_submission_receipt(frontier_dir, proposal)?;
    let receipt_parent_event_log_root = receipt_parent_event_log_root(&receipt)
        .ok_or_else(|| "policy receipt has no typed producer-context event_log_root".to_string())?;
    if receipt_parent_event_log_root != lane.parent_event_log_root {
        return Err(format!(
            "policy receipt causal root mismatch: retained {receipt_parent_event_log_root}, lane {}",
            lane.parent_event_log_root
        ));
    }
    let context = derive_submission_policy_context_for_policy(
        frontier_dir,
        &prestate,
        &proposal.id,
        &receipt,
        &lane.decision_time,
        &verified.policy.schema,
    )?;
    if lane.context != context {
        return Err(format!(
            "policy_lane context differs from retained evidence (stamped {}, rederived {}; stamped={:?}; rederived={:?})",
            lane.context.policy_language_digest()?,
            context.policy_language_digest()?,
            lane.context,
            context,
        ));
    }
    let decision = evaluate(&verified.policy, &context, &lane.decision_time);
    if decision.outcome != Outcome::Permit {
        return Err(format!(
            "re-evaluation under {} yields {:?}, not permit ({})",
            lane.policy_id,
            decision.outcome,
            decision.reasons.join(", ")
        ));
    }
    if lane.rule_ids != decision.matched_rule_ids {
        return Err("policy_lane rule_ids do not match the rederived decision".to_string());
    }
    let engine_gate =
        super::preview_engine_verdict_in_frontier(&prestate, frontier_dir, &proposal.id, true)?;
    if engine_gate != lane.engine_gate {
        return Err(
            "policy_lane Engine verdict does not rederive from the causal pre-state".to_string(),
        );
    }
    if engine_gate.status == "blocked" || engine_gate.forced || !engine_gate.strict {
        return Err("policy_lane Engine verdict is not an unforced strict pass/warn".to_string());
    }

    let review = load_review_material(frontier_dir, proposal)?;
    let receipt_root = receipt
        .canonical_root()
        .map_err(|error| format!("derive retained receipt root: {error}"))?;
    if review.schema != "vela.proposal-review-material.internal.v2"
        || review.route.schema != "vela.staged-review-route.internal.v2"
        || review.proposal_id != proposal.id
        || review.receipt_root != receipt_root
        || review.evaluated_at != lane.decision_time
        || review.route.policy_context != context
        || review.route.policy_decision.as_ref() != Some(&decision)
        || review.route.policy_state != PolicyState::Active
        || review.route.permit_readiness != PermitReadiness::Ready
        || !review.route.reason_codes.is_empty()
        || review.route.readiness_detail.is_some()
        || review.route.engine_gate != engine_gate
    {
        return Err(
            "retained review material does not match the rederived policy route".to_string(),
        );
    }

    let (_, state_root_after) = policy_transition_roots(event)?;
    let expected_profile = format!("assurance_level_a{}", context.assurance_level);
    let expected_claim_digest = proposal_claim_digest(proposal);
    let expected = DecisionCertificate::build(
        &decision,
        project.frontier_id.as_deref().unwrap_or_default(),
        &proposal.id,
        &parent_root,
        &state_root_after,
        AuthorityMode::PolicyDelegation,
        authority.human_authorizers.clone(),
        &lane.executor,
        &expected_profile,
        context.assurance_level,
        &expected_claim_digest,
        context.impact_tier,
        false,
    );
    let mut mismatches = Vec::new();
    if !cert.id_is_valid() {
        mismatches.push("id");
    }
    if cert.schema != expected.schema {
        mismatches.push("schema");
    }
    if cert.frontier_id != expected.frontier_id {
        mismatches.push("frontier_id");
    }
    if cert.proposal_id != expected.proposal_id {
        mismatches.push("proposal_id");
    }
    if cert.state_root_before != expected.state_root_before {
        mismatches.push("state_root_before");
    }
    if cert.state_root_after != expected.state_root_after {
        mismatches.push("state_root_after");
    }
    if cert.outcome != expected.outcome {
        mismatches.push("outcome");
    }
    if cert.policy_id != lane.policy_id || cert.policy_id != expected.policy_id {
        mismatches.push("policy_id");
    }
    if cert.rule_ids != expected.rule_ids {
        mismatches.push("rule_ids");
    }
    if cert.evaluator != expected.evaluator {
        mismatches.push("evaluator");
    }
    if cert.authority_mode != expected.authority_mode {
        mismatches.push("authority_mode");
    }
    if cert.human_authorizers != expected.human_authorizers {
        mismatches.push("human_authorizers");
    }
    if cert.executor != expected.executor {
        mismatches.push("executor");
    }
    if cert.assurance_profile != expected.assurance_profile {
        mismatches.push("assurance_profile");
    }
    if cert.assurance_level != expected.assurance_level {
        mismatches.push("assurance_level");
    }
    if cert.claim_digest != expected.claim_digest {
        mismatches.push("claim_digest");
    }
    if cert.impact_tier != expected.impact_tier {
        mismatches.push("impact_tier");
    }
    if cert.reasons != expected.reasons {
        mismatches.push("reasons");
    }
    if cert.audit_required != expected.audit_required {
        mismatches.push("audit_required");
    }
    if cert.id != expected.id {
        mismatches.push("expected_id");
    }
    if !mismatches.is_empty() {
        return Err(format!(
            "policy_lane certificate fields are inconsistent: {}",
            mismatches.join(", ")
        ));
    }
    Ok(())
}

fn verify_policy_lane_head_binding(
    project: &project::Project,
    event: &events::StateEvent,
    lane: &PolicyLaneStampV2,
) -> Result<(), String> {
    if !lane
        .parent_event_ids
        .iter()
        .any(|id| id == &lane.policy_head_event_id)
    {
        return Err("policy_lane does not parent its policy-head review".to_string());
    }
    let chain = derive_policy_head_chain(project)
        .map_err(|error| format!("policy-head chain is invalid: {error}"))?;
    let index = chain
        .iter()
        .position(|head| head.event_id == lane.policy_head_event_id)
        .ok_or_else(|| "policy_lane names a policy-head outside the signed chain".to_string())?;
    let head = &chain[index];
    if head.action == PolicyHeadAction::Revoke
        || head.policy_id.as_deref() != Some(lane.policy_id.as_str())
        || head.epoch != lane.policy_head_epoch
    {
        return Err("policy_lane policy does not match its signed policy-head".to_string());
    }
    let head_at = chrono::DateTime::parse_from_rfc3339(&head.reviewed_at)
        .map_err(|error| format!("policy-head review time is invalid: {error}"))?;
    let lane_at = chrono::DateTime::parse_from_rfc3339(&lane.decision_time)
        .map_err(|error| format!("policy lane decision time is invalid: {error}"))?;
    if lane_at < head_at {
        return Err("policy_lane predates policy-head activation".to_string());
    }
    if let Some(successor) = chain.get(index + 1)
        && !successor.parent_event_ids.iter().any(|id| id == &event.id)
    {
        return Err(
            "superseded policy_lane is absent from the successor head parent set".to_string(),
        );
    }
    Ok(())
}

fn reconstruct_policy_prestate(
    project: &project::Project,
    event: &events::StateEvent,
    lane: &PolicyLaneStampV2,
) -> Result<(project::Project, String), String> {
    if lane.parent_event_ids != sorted_unique_ids(lane.parent_event_ids.iter().map(String::as_str))
    {
        return Err("policy_lane parent_event_ids must be sorted and unique".to_string());
    }
    if lane.prestate_attachment_ids
        != sorted_unique_ids(lane.prestate_attachment_ids.iter().map(String::as_str))
    {
        return Err("policy_lane prestate_attachment_ids must be sorted and unique".to_string());
    }
    if lane.parent_event_ids.iter().any(|id| id == &event.id) {
        return Err("policy_lane event cannot be its own causal parent".to_string());
    }
    let by_id = project
        .events
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect::<HashMap<_, _>>();
    if by_id.len() != project.events.len() {
        return Err("frontier event ids are not unique".to_string());
    }
    let canonical_events = crate::reducer::sorted_for_replay(&project.events);
    let event_positions = canonical_events
        .iter()
        .enumerate()
        .map(|(index, candidate)| (candidate.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let lane_position = *event_positions.get(event.id.as_str()).ok_or_else(|| {
        format!(
            "policy_lane event {} is missing from the frontier",
            event.id
        )
    })?;
    let exact_parent_ids = sorted_unique_ids(
        canonical_events[..lane_position]
            .iter()
            .map(|parent| parent.id.as_str()),
    );
    if lane.parent_event_ids != exact_parent_ids {
        return Err(format!(
            "policy_lane event {} does not commit the exact preceding event-log prefix",
            event.id
        ));
    }
    let decision_at = chrono::DateTime::parse_from_rfc3339(&lane.decision_time)
        .map_err(|error| format!("policy decision time is not RFC3339: {error}"))?;
    let mut parent_events = Vec::with_capacity(lane.parent_event_ids.len());
    for id in &lane.parent_event_ids {
        let parent = by_id
            .get(id.as_str())
            .ok_or_else(|| format!("policy_lane causal parent {id} is missing"))?;
        let parent_position = *event_positions
            .get(id.as_str())
            .expect("parent map and position map share the event vector");
        if parent_position >= lane_position {
            return Err(format!(
                "policy_lane causal parent {id} does not occur before event {} in the event log",
                event.id
            ));
        }
        if parent.id != events::event_id(parent) {
            return Err(format!("policy_lane causal parent {id} does not rederive"));
        }
        let parent_at = chrono::DateTime::parse_from_rfc3339(&parent.timestamp)
            .map_err(|error| format!("causal parent {id} timestamp is invalid: {error}"))?;
        if parent_at > decision_at {
            return Err(format!(
                "causal parent {id} occurs after the policy decision time"
            ));
        }
        parent_events.push((*parent).clone());
    }
    let parent_root = format!("sha256:{}", events::event_log_hash(&parent_events));

    let sorted_parent_events = crate::reducer::sorted_for_replay(&parent_events);
    let (mut genesis, diagnostics) =
        crate::reducer::seed_genesis(&sorted_parent_events, &project.proposals);
    if !diagnostics.is_empty() {
        return Err(format!(
            "policy causal pre-state cannot hydrate: {}",
            diagnostics.join("; ")
        ));
    }
    // Import only truly immutable, non-evented genesis cache entries. A
    // finding established by any event in the retained log must come from the
    // causal parent prefix through `seed_genesis`; otherwise a future event's
    // already-materialized finding could leak backward into this decision.
    let proposals_by_id = project
        .proposals
        .iter()
        .map(|proposal| (proposal.id.as_str(), proposal))
        .collect::<HashMap<_, _>>();
    let mut evented_finding_ids = HashSet::new();
    for candidate in &project.events {
        match candidate.kind.as_str() {
            "finding.asserted" => {
                evented_finding_ids.insert(candidate.target.id.as_str());
                if let Some(id) = candidate
                    .payload
                    .get("finding")
                    .and_then(|finding| finding.get("id"))
                    .and_then(serde_json::Value::as_str)
                {
                    evented_finding_ids.insert(id);
                }
            }
            "finding.superseded" => {
                evented_finding_ids.insert(candidate.target.id.as_str());
                if let Some(id) = candidate
                    .payload
                    .get("new_finding")
                    .and_then(|finding| finding.get("id"))
                    .and_then(serde_json::Value::as_str)
                {
                    evented_finding_ids.insert(id);
                }
                if let Some(proposal) = candidate
                    .payload
                    .get("proposal_id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|id| proposals_by_id.get(id).copied())
                    && let Some(id) = proposal
                        .payload
                        .get("new_finding")
                        .and_then(|finding| finding.get("id"))
                        .and_then(serde_json::Value::as_str)
                {
                    evented_finding_ids.insert(id);
                }
            }
            _ => {}
        }
    }
    for finding in &project.findings {
        if !evented_finding_ids.contains(finding.id.as_str())
            && !genesis.iter().any(|candidate| candidate.id == finding.id)
        {
            genesis.push(finding.clone());
        }
    }
    let mut prestate = crate::reducer::replay_from_genesis(
        genesis,
        sorted_parent_events,
        &project.project.name,
        &project.project.description,
        &project.project.compiled_at,
        &project.project.compiler,
    )?;
    prestate.frontier_id = project.frontier_id.clone();
    prestate.actors = project
        .actors
        .iter()
        .filter(|actor| {
            chrono::DateTime::parse_from_rfc3339(&actor.created_at)
                .is_ok_and(|created_at| created_at <= decision_at)
        })
        .cloned()
        .collect();

    let replayed_attachment_ids = sorted_unique_ids(
        prestate
            .verifier_attachments
            .iter()
            .map(|attachment| attachment.id.as_str()),
    );
    if replayed_attachment_ids != lane.prestate_attachment_ids {
        return Err(format!(
            "policy pre-state attachment set is not causally derived (stamped {:?}, replayed {:?})",
            lane.prestate_attachment_ids, replayed_attachment_ids
        ));
    }
    project::recompute_stats(&mut prestate);
    Ok((prestate, parent_root))
}

fn load_submission_receipt(
    frontier_dir: &Path,
    proposal: &super::StateProposal,
) -> Result<ReceiptV1, String> {
    let submission = proposal
        .payload
        .get("vela_submission")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "policy proposal has no vela_submission links".to_string())?;
    let receipt_path = submission
        .get("receipt_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "policy proposal has no receipt_path".to_string())?;
    if !receipt_path.starts_with("records/receipts/sha256/") {
        return Err(
            "policy proposal receipt_path is outside the committed receipt store".to_string(),
        );
    }
    let bytes = read_frontier_regular_file(
        frontier_dir,
        Path::new(receipt_path),
        8 * 1024 * 1024,
        "policy receipt",
    )?;
    let receipt = ReceiptV1::parse(&bytes).map_err(|error| error.to_string())?;
    let declared = submission
        .get("receipt_root")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "policy proposal has no receipt_root".to_string())?;
    let actual = receipt
        .canonical_root()
        .map_err(|error| format!("derive policy receipt root: {error}"))?;
    if actual != declared {
        return Err(format!(
            "retained policy receipt root mismatch: declared {declared}, got {actual}"
        ));
    }
    Ok(receipt)
}

fn load_review_material(
    frontier_dir: &Path,
    proposal: &super::StateProposal,
) -> Result<RetainedReviewMaterial, String> {
    let path = proposal
        .payload
        .get("vela_submission")
        .and_then(|value| value.get("review_material_path"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "policy proposal has no review_material_path".to_string())?;
    if !path.starts_with("records/review/sha256/")
        || !proposal.source_refs.iter().any(|source| source == path)
    {
        return Err(
            "policy review material is not linked through the committed review store".to_string(),
        );
    }
    let bytes = read_frontier_regular_file(
        frontier_dir,
        Path::new(path),
        MAX_REVIEW_MATERIAL_BYTES,
        "policy review material",
    )?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse retained policy review material: {error}"))?;
    let material: RetainedReviewMaterial = serde_json::from_value(raw.clone())
        .map_err(|error| format!("parse retained policy review material: {error}"))?;
    if serde_json::to_value(&material)
        .map_err(|error| format!("normalize retained policy review material: {error}"))?
        != raw
    {
        return Err(
            "retained policy review material is not the exact closed typed shape".to_string(),
        );
    }
    Ok(material)
}

/// Load a persisted policy snapshot by id and verify its detached human
/// signature (same bar as `load_active_policy`, addressed by id).
fn load_policy_snapshot(frontier_dir: &Path, policy_id: &str) -> Result<VerifiedPolicy, String> {
    if !policy_id.strip_prefix("vap_").is_some_and(|suffix| {
        suffix.len() == 32
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) {
        return Err(format!("invalid policy snapshot id {policy_id}"));
    }
    let policy_relative = PathBuf::from(format!(".vela/policies/{policy_id}.json"));
    let signature_relative = PathBuf::from(format!(".vela/policies/{policy_id}.sig.json"));
    let policy = read_frontier_regular_file(
        frontier_dir,
        &policy_relative,
        1024 * 1024,
        "policy snapshot",
    )?;
    let signature = read_frontier_regular_file(
        frontier_dir,
        &signature_relative,
        1024 * 1024,
        "policy snapshot signature",
    )?;
    verify_policy_signature_bytes(
        &policy,
        &signature,
        Some(policy_id),
        &format!("policy snapshot {policy_id}"),
    )
}

fn read_frontier_regular_file(
    frontier_dir: &Path,
    relative: &Path,
    max_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, String> {
    if relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "{label} path must be normalized and frontier-relative: {}",
            relative.display()
        ));
    }
    let root = frontier_dir
        .canonicalize()
        .map_err(|error| format!("canonicalize frontier for {label}: {error}"))?;
    let components = relative.components().collect::<Vec<_>>();
    let mut current = root.clone();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            unreachable!("components were validated above")
        };
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|error| format!("inspect {label} {}: {error}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{label} path must not traverse a symlink: {}",
                current.display()
            ));
        }
        let is_leaf = index + 1 == components.len();
        if is_leaf {
            if !metadata.is_file() {
                return Err(format!(
                    "{label} must be a regular file: {}",
                    current.display()
                ));
            }
        } else if !metadata.is_dir() {
            return Err(format!(
                "{label} ancestor must be a directory: {}",
                current.display()
            ));
        }
    }
    // Read through an already-open descriptor, then prove that descriptor is
    // still the regular file named inside the frontier. This closes the usual
    // symlink-check/read race: any path swap before the checks changes the
    // canonical location or inode; a swap after them cannot change the open fd.
    let file = std::fs::File::open(&current)
        .map_err(|error| format!("open {label} {}: {error}", current.display()))?;
    let opened = file
        .metadata()
        .map_err(|error| format!("inspect open {label} {}: {error}", current.display()))?;
    if !opened.is_file() {
        return Err(format!(
            "{label} must be a regular file: {}",
            current.display()
        ));
    }
    let linked = std::fs::symlink_metadata(&current)
        .map_err(|error| format!("reinspect {label} {}: {error}", current.display()))?;
    if linked.file_type().is_symlink() || !linked.is_file() {
        return Err(format!(
            "{label} path must remain a non-symlink regular file: {}",
            current.display()
        ));
    }
    let canonical = current
        .canonicalize()
        .map_err(|error| format!("canonicalize {label} {}: {error}", current.display()))?;
    if !canonical.starts_with(&root) {
        return Err(format!(
            "{label} resolved outside the frontier: {}",
            canonical.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let named = std::fs::metadata(&current)
            .map_err(|error| format!("reinspect named {label} {}: {error}", current.display()))?;
        if opened.dev() != named.dev() || opened.ino() != named.ino() {
            return Err(format!(
                "{label} path changed while being opened: {}",
                current.display()
            ));
        }
    }
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {label} {}: {error}", current.display()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "{label} {} exceeds the {max_bytes}-byte limit",
            current.display()
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use crate::policy::acceptance_policy::{
        AcceptancePolicy, Constraints, PolicyRule, PolicySignatureRecord, Quorum,
        policy_signature_preimage,
    };
    use crate::proposals::new_proposal_at;
    use crate::receipt_v1::{ArtifactInput, ReceiptBuilder, ReceiptInput};
    use crate::verifier_attachment::{
        AdversarialProbe, AttachmentDraft, MatchToClaim, ProbeKind, ProbeResult, VerifierMethod,
    };
    use ed25519_dalek::Signer;
    use serde_json::json;
    use tempfile::TempDir;

    const SIGNED_AT: &str = "2026-07-03T00:00:00Z";
    const DECISION_AT: &str = "2026-07-13T00:00:00Z";

    fn test_signing_key() -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])
    }

    #[test]
    fn exact_receipt_floor_rederives_one_native_witness_and_claim() {
        let tmp = TempDir::new().unwrap();
        let relative = "artifacts/sidon.witness.json";
        std::fs::create_dir_all(tmp.path().join("artifacts")).unwrap();
        let witness = vela_verify::Witness::Sidon {
            n: 3,
            points: vec![vec![0, 0, 0], vec![1, 0, 0], vec![0, 1, 0], vec![0, 0, 1]],
            claimed_size: Some(4),
        };
        let bytes = serde_json::to_vec(&witness).unwrap();
        std::fs::write(tmp.path().join(relative), &bytes).unwrap();
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x37; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:floor-test".to_string(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-02T00:00:00Z".to_string(),
            },
            &key,
        )
        .unwrap();
        let claim = "There exists a Sidon subset of {0,1}^3 with at least 4 elements.";
        let build_receipt = |replayability: &str, artifacts: Vec<ArtifactInput>| {
            ReceiptBuilder::build(
                ReceiptInput::new(
                    claim.to_string(),
                    "computational".to_string(),
                    replayability.to_string(),
                    artifacts,
                    vec!["No optimality claim.".to_string()],
                    Vec::new(),
                    "agent:floor-test".to_string(),
                    "2026-07-02T00:00:00Z".to_string(),
                    format!("sha256:{}", "a".repeat(64)),
                    ".".to_string(),
                    format!("vop_{}", "b".repeat(64)),
                    "urn:vela:policy:none".to_string(),
                )
                .unwrap(),
                &identity,
            )
            .unwrap()
        };
        let descriptor = || {
            ArtifactInput::new(
                relative.to_string(),
                EXACT_FLOOR_ARTIFACT_KIND.to_string(),
                Some(digest.clone()),
                None,
            )
            .unwrap()
        };
        let receipt = build_receipt("exact", vec![descriptor()]);

        assert!(exact_receipt_floor(tmp.path(), &receipt, claim).unwrap());
        assert!(
            !exact_receipt_floor(
                tmp.path(),
                &receipt,
                "There exists a Sidon subset of {0,1}^3 with at least 5 elements."
            )
            .unwrap()
        );
        assert!(
            !exact_receipt_floor(
                tmp.path(),
                &build_receipt("bounded", vec![descriptor()]),
                claim
            )
            .unwrap()
        );
        assert!(
            !exact_receipt_floor(
                tmp.path(),
                &build_receipt(
                    "exact",
                    vec![
                        ArtifactInput::new(
                            relative.to_string(),
                            "search-log".to_string(),
                            Some(digest.clone()),
                            None,
                        )
                        .unwrap(),
                    ],
                ),
                claim
            )
            .unwrap()
        );
        assert!(
            exact_receipt_floor(
                tmp.path(),
                &build_receipt("exact", vec![descriptor(), descriptor()]),
                claim
            )
            .unwrap_err()
            .contains("exactly one")
        );

        std::fs::write(tmp.path().join(relative), b"{}\n").unwrap();
        assert!(
            exact_receipt_floor(tmp.path(), &receipt, claim)
                .unwrap_err()
                .contains("digest mismatch")
        );

        std::fs::remove_file(tmp.path().join(relative)).unwrap();
        assert!(
            exact_receipt_floor(tmp.path(), &receipt, claim)
                .unwrap_err()
                .contains("inspect exact-floor vela-witness")
        );

        let invalid = vela_verify::Witness::Sidon {
            n: 3,
            points: vec![vec![0, 0, 0], vec![1, 0, 0], vec![0, 1, 0], vec![1, 1, 0]],
            claimed_size: Some(4),
        };
        let invalid_bytes = serde_json::to_vec(&invalid).unwrap();
        std::fs::write(tmp.path().join(relative), &invalid_bytes).unwrap();
        let invalid_receipt = build_receipt(
            "exact",
            vec![
                ArtifactInput::new(
                    relative.to_string(),
                    EXACT_FLOOR_ARTIFACT_KIND.to_string(),
                    Some(format!("{:x}", Sha256::digest(&invalid_bytes))),
                    None,
                )
                .unwrap(),
            ],
        );
        assert!(!exact_receipt_floor(tmp.path(), &invalid_receipt, claim).unwrap());
    }

    #[test]
    fn policy_v0_2_raises_only_the_exact_native_witness_floor() {
        use crate::receipt_v1::ExecutionBindingV1;

        let tmp = TempDir::new().unwrap();
        crate::frontier_repo::initialize(
            tmp.path(),
            crate::frontier_repo::InitOptions {
                name: "exact-floor-test",
                initialize_git: false,
            },
        )
        .unwrap();
        let relative = "artifacts/sidon.witness.json";
        std::fs::create_dir_all(tmp.path().join("artifacts")).unwrap();
        let claim = "There exists a Sidon subset of {0,1}^3 with at least 4 elements.";
        let witness = vela_verify::Witness::Sidon {
            n: 3,
            points: vec![vec![0, 0, 0], vec![1, 0, 0], vec![0, 1, 0], vec![0, 0, 1]],
            claimed_size: Some(4),
        };
        let bytes = serde_json::to_vec(&witness).unwrap();
        std::fs::write(tmp.path().join(relative), &bytes).unwrap();
        let binding = ExecutionBindingV1 {
            schema: "vela.execution-binding.v1".to_string(),
            packet_root: format!("sha256:{}", "1".repeat(64)),
            profile_root: format!("sha256:{}", "2".repeat(64)),
            verifier_capsule_root: format!("sha256:{}", "3".repeat(64)),
            result_contract_root: format!("sha256:{}", "4".repeat(64)),
        };
        let producer_key = ed25519_dalek::SigningKey::from_bytes(&[0x37; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:floor-test".to_string(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-02T00:00:00Z".to_string(),
            },
            &producer_key,
        )
        .unwrap();
        let receipt = ReceiptBuilder::build(
            ReceiptInput::new(
                claim.to_string(),
                "computational".to_string(),
                "exact".to_string(),
                vec![
                    ArtifactInput::new(
                        relative.to_string(),
                        EXACT_FLOOR_ARTIFACT_KIND.to_string(),
                        Some(format!("{:x}", Sha256::digest(&bytes))),
                        None,
                    )
                    .unwrap(),
                ],
                vec!["No optimality claim.".to_string()],
                Vec::new(),
                "agent:floor-test".to_string(),
                "2026-07-02T00:00:00Z".to_string(),
                format!("sha256:{}", "a".repeat(64)),
                ".".to_string(),
                format!("vop_{}", "b".repeat(64)),
                "urn:vela:policy:none".to_string(),
            )
            .unwrap()
            .with_execution_binding(binding.clone())
            .unwrap(),
            &identity,
        )
        .unwrap();

        let mut project = repo::load_from_path(tmp.path()).unwrap();
        project.actors.push(crate::sign::ActorRecord {
            id: "agent:floor-test".to_string(),
            public_key: hex::encode(producer_key.verifying_key().to_bytes()),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-01T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        let mut finding = crate::proposals::tests::finding("vf_exact_floor");
        finding.assertion.text = claim.to_string();
        finding.assertion.assertion_type = "computational".to_string();
        let receipt_root = receipt.canonical_root().unwrap();
        let proposal = new_proposal_at(
            "finding.add",
            crate::events::StateTarget {
                r#type: "finding".to_string(),
                id: finding.id.clone(),
            },
            "agent:floor-test",
            "agent",
            "exact native witness",
            json!({
                "finding": finding,
                "vela_submission": {
                    "schema": "vela.submission-links.internal.v1",
                    "receipt_root": receipt_root,
                }
            }),
            Vec::new(),
            Vec::new(),
            "2026-07-02T00:00:01Z",
        );
        let proposal_id = proposal.id.clone();
        project.proposals.push(proposal);

        let historical =
            derive_submission_policy_context(&project, &proposal_id, &receipt, DECISION_AT)
                .unwrap();
        let v1 = derive_submission_policy_context_for_policy(
            tmp.path(),
            &project,
            &proposal_id,
            &receipt,
            DECISION_AT,
            crate::policy::acceptance_policy::ACCEPTANCE_POLICY_V0_1_SCHEMA,
        )
        .unwrap();
        assert_eq!(v1, historical, "v0.1 context bytes must not change");
        assert_eq!(v1.assurance_level, 0);
        assert!(!v1.method_integrity_sound);

        let v2 = derive_submission_policy_context_for_policy(
            tmp.path(),
            &project,
            &proposal_id,
            &receipt,
            DECISION_AT,
            ACCEPTANCE_POLICY_V0_2_SCHEMA,
        )
        .unwrap();
        assert_eq!(v2.assurance_level, 2);
        assert!(v2.method_integrity_sound);
        assert!(v2.assertion_text_mutated);
        assert_eq!(v2.execution_binding.as_ref(), Some(&binding));

        let mut policy = AcceptancePolicy {
            schema: ACCEPTANCE_POLICY_V0_2_SCHEMA.to_string(),
            id: String::new(),
            frontier_id: "frontier:test".to_string(),
            epoch: 1,
            issued_by: vec!["reviewer:test".to_string()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["reviewer".to_string()],
            },
            rules: vec![PolicyRule {
                id: "exact-native-witness".to_string(),
                effect: Outcome::Permit,
                claim_classes: vec!["receipt_computational".to_string()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    max_downstream_dependents: 0,
                    required_assurance_min: 2,
                    allow_semantic_text_change: true,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    require_independence: false,
                    require_method_integrity: true,
                    allowed_packet_roots: Some(vec![binding.packet_root.clone()]),
                    allowed_profile_roots: Some(vec![binding.profile_root.clone()]),
                    allowed_verifier_capsule_roots: Some(vec![
                        binding.verifier_capsule_root.clone(),
                    ]),
                    allowed_result_contract_roots: Some(vec![binding.result_contract_root.clone()]),
                    required_replayability: Some("exact".to_string()),
                },
            }],
            default: Outcome::Defer,
            expires_at: CAUSALLY_UNBOUNDED_POLICY_EXPIRY.to_string(),
            revocation_ref: None,
        };
        policy.id = policy.content_address();
        assert_eq!(evaluate(&policy, &v1, DECISION_AT).outcome, Outcome::Defer);
        assert_eq!(evaluate(&policy, &v2, DECISION_AT).outcome, Outcome::Permit);
    }

    fn clone_project(project: &project::Project) -> project::Project {
        serde_json::from_value(serde_json::to_value(project).unwrap()).unwrap()
    }

    fn permitting_policy(frontier_id: &str) -> AcceptancePolicy {
        let mut p = AcceptancePolicy {
            schema: "vela.acceptance_policy.v0.1".to_string(),
            id: String::new(),
            frontier_id: frontier_id.to_string(),
            epoch: 1,
            issued_by: vec!["reviewer:will".into()],
            quorum: Quorum {
                threshold: 1,
                eligible_roles: vec!["reviewer".into()],
            },
            rules: vec![PolicyRule {
                id: "review-exact-auto-v1".into(),
                effect: Outcome::Permit,
                claim_classes: vec!["receipt_computational".into()],
                constraints: Constraints {
                    max_changed_findings: 1,
                    max_downstream_dependents: 5,
                    required_assurance_min: 3,
                    allow_semantic_text_change: false,
                    allow_contested: false,
                    allow_governance_mutation: false,
                    require_independence: true,
                    require_method_integrity: true,
                    ..Constraints::default()
                },
            }],
            default: Outcome::Defer,
            expires_at: CAUSALLY_UNBOUNDED_POLICY_EXPIRY.into(),
            revocation_ref: None,
        };
        p.rules.push(PolicyRule {
            id: "reject-forbidden-v1".into(),
            effect: Outcome::Deny,
            claim_classes: vec!["forbidden".into()],
            constraints: p.rules[0].constraints.clone(),
        });
        p.id = p.content_address();
        p
    }

    fn write_active_policy(dir: &Path, mut policy: AcceptancePolicy) {
        policy.id = policy.content_address();
        let key = test_signing_key();
        let body = policy_signature_preimage(&policy, SIGNED_AT).unwrap();
        let sig = key.sign(&body);
        let pol_dir = dir.join(".vela").join("policies");
        std::fs::create_dir_all(&pol_dir).unwrap();
        std::fs::write(
            pol_dir.join("active.json"),
            serde_json::to_string_pretty(&policy).unwrap(),
        )
        .unwrap();
        std::fs::write(
            pol_dir.join("active.sig.json"),
            serde_json::to_string_pretty(&PolicySignatureRecord {
                policy_id: policy.id,
                signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
                signature: hex::encode(sig.to_bytes()),
                signed_at: SIGNED_AT.to_string(),
            })
            .unwrap(),
        )
        .unwrap();
    }

    fn mutate_active_policy(dir: &Path, mutate: impl FnOnce(&mut AcceptancePolicy)) {
        let raw = std::fs::read_to_string(dir.join(".vela/policies/active.json")).unwrap();
        let mut policy: AcceptancePolicy = serde_json::from_str(&raw).unwrap();
        mutate(&mut policy);
        write_active_policy(dir, policy);
    }

    fn expect_authority_defer(
        dir: &Path,
        proposal_id: &str,
        expected_reason_code: &str,
    ) -> PolicyLaneRefusal {
        let before =
            crate::canonical::to_canonical_bytes(&repo::load_from_path(dir).unwrap()).unwrap();
        let error = accept_under_policy_at_path_at(
            dir,
            proposal_id,
            &permitting_ctx(),
            "agent:prover",
            DECISION_AT,
        )
        .expect_err("invalid policy authority must route to a human");
        assert!(
            matches!(error, PolicyLaneRefusal::Deferred { .. }),
            "{error}"
        );
        assert!(error.to_string().contains(expected_reason_code), "{error}");
        assert_eq!(
            before,
            crate::canonical::to_canonical_bytes(&repo::load_from_path(dir).unwrap()).unwrap(),
            "authority deferral changed the canonical frontier"
        );
        error
    }

    fn expect_policy_head_blocked(
        dir: &Path,
        proposal_id: &str,
        expected_detail: &str,
    ) -> PolicyLaneRefusal {
        let before =
            crate::canonical::to_canonical_bytes(&repo::load_from_path(dir).unwrap()).unwrap();
        let error = accept_under_policy_at_path_at(
            dir,
            proposal_id,
            &permitting_ctx(),
            "agent:prover",
            DECISION_AT,
        )
        .expect_err("invalid policy-head integrity must block");
        assert!(matches!(error, PolicyLaneRefusal::Error(_)), "{error}");
        assert!(error.to_string().contains(expected_detail), "{error}");
        assert_eq!(
            before,
            crate::canonical::to_canonical_bytes(&repo::load_from_path(dir).unwrap()).unwrap(),
            "blocked policy-head check changed the canonical frontier"
        );
        error
    }

    fn permitting_ctx() -> PolicyContext {
        PolicyContext {
            claim_class: "receipt_computational".into(),
            assurance_level: 3,
            impact_tier: 1,
            changed_findings: 1,
            downstream_dependents: 0,
            assertion_text_mutated: false,
            target_contested: false,
            governance_mutation: false,
            independence_satisfied: true,
            method_integrity_sound: true,
            credential_valid: true,
            has_unknown_fields: false,
            replayability: "exact".to_string(),
            execution_binding: None,
        }
    }

    fn verified_attachment(
        target: &str,
        digest: &str,
        method: VerifierMethod,
        solver: &str,
        independent_of: Vec<String>,
        implementation: &str,
    ) -> crate::verifier_attachment::VerifierAttachment {
        crate::verifier_attachment::VerifierAttachment::build(AttachmentDraft {
            target: target.to_string(),
            claim_digest: digest.to_string(),
            verifier_method: method,
            solver_id: solver.to_string(),
            independent_of,
            match_to_claim: MatchToClaim {
                matches: true,
                checker_actor: "ci:test".to_string(),
            },
            adversarial_probes: vec![AdversarialProbe {
                kind: ProbeKind::FormalismFidelity,
                result: ProbeResult::Survived,
                note: String::new(),
            }],
            outcome: AttachmentOutcome::Passed,
            verifier_actor: "ci:test".to_string(),
            note: String::new(),
        })
        .unwrap()
        .with_method_integrity(MethodIntegrity::Sound)
        .unwrap()
        .with_implementation_id(implementation)
        .unwrap()
    }

    /// Initialize a REAL `.vela`-store frontier with one finding, one
    /// pending review proposal, and a HUMAN-SIGNED active policy.
    /// Returns the dir and the pending proposal id.
    fn seeded_frontier(tmp: &TempDir) -> (std::path::PathBuf, String) {
        let dir = tmp.path().to_path_buf();
        crate::frontier_repo::initialize(
            &dir,
            crate::frontier_repo::InitOptions {
                name: "policy-lane-test",
                initialize_git: false,
            },
        )
        .unwrap();
        let mut frontier = repo::load_from_path(&dir).unwrap();
        let key = test_signing_key();
        frontier.actors.push(crate::sign::ActorRecord {
            id: "reviewer:will".to_string(),
            public_key: hex::encode(key.verifying_key().to_bytes()),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-01T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        let mut finding = crate::proposals::tests::finding("vf_target");
        finding.assertion.assertion_type = "computational".to_string();
        let claim = finding.assertion.text.clone();
        let digest = claim_digest(&claim);
        let first = verified_attachment(
            &finding.id,
            &digest,
            VerifierMethod::ComputationalSearch,
            "solver-a",
            Vec::new(),
            "implementation-a",
        );
        let second = verified_attachment(
            &finding.id,
            &digest,
            VerifierMethod::ExactArithmeticRecompute,
            "solver-b",
            vec![first.id.clone()],
            "implementation-b",
        );
        frontier.verifier_attachments = vec![first.clone(), second.clone()];
        for attachment in [first, second] {
            frontier
                .events
                .push(events::new_finding_event(events::FindingEventInput {
                    kind: events::EVENT_KIND_VERIFIER_ATTACHMENT_ADDED,
                    finding_id: &finding.id,
                    actor_id: "ci:test",
                    actor_type: events::actor_kind("ci:test"),
                    reason: "test verified attachment",
                    before_hash: events::NULL_HASH,
                    after_hash: events::NULL_HASH,
                    payload: json!({"attachment": attachment}),
                    caveats: Vec::new(),
                    timestamp: Some("2026-07-10T00:00:00Z"),
                }));
        }
        let producer_key = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
        frontier.actors.push(crate::sign::ActorRecord {
            id: "agent:prover".to_string(),
            public_key: hex::encode(producer_key.verifying_key().to_bytes()),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-01T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        frontier.findings.push(finding.clone());
        let frontier_id = frontier.frontier_id.clone().unwrap();
        let policy = permitting_policy(&frontier_id);
        let parent_event_ids =
            sorted_unique_ids(frontier.events.iter().map(|event| event.id.as_str()));
        let parent_root = format!("sha256:{}", events::event_log_hash(&frontier.events));
        let mut head_proposal = new_proposal_at(
            POLICY_HEAD_PROPOSAL_KIND,
            crate::events::StateTarget {
                r#type: "governance".to_string(),
                id: frontier_id.clone(),
            },
            "reviewer:will",
            "human",
            "activate test policy head",
            serde_json::to_value(PolicyHeadPayload {
                schema: POLICY_HEAD_SCHEMA.to_string(),
                action: PolicyHeadAction::Activate,
                policy_id: Some(policy.id.clone()),
                prior_head_event_id: None,
                expected_parent_event_log_root: parent_root,
                parent_event_ids,
                epoch: 1,
            })
            .unwrap(),
            Vec::new(),
            Vec::new(),
            "2026-07-11T00:00:00Z",
        );
        let mut head_event = events::new_review_decision_event(
            &head_proposal.id,
            POLICY_HEAD_PROPOSAL_KIND,
            "accepted",
            None,
            "reviewer:will",
            "activate test policy head",
            Some("2026-07-11T00:00:01Z"),
        )
        .unwrap();
        head_event.signature = Some(crate::sign::sign_event(&head_event, &key).unwrap());
        head_proposal.status = "applied".to_string();
        head_proposal.reviewed_by = Some("reviewer:will".to_string());
        head_proposal.reviewed_at = Some(head_event.timestamp.clone());
        head_proposal.decision_reason = Some("activate test policy head".to_string());
        head_proposal.applied_event_id = Some(head_event.id.clone());
        frontier.proposals.push(head_proposal);
        frontier.events.push(head_event);
        project::recompute_stats(&mut frontier);
        repo::save_to_path(&dir, &frontier).unwrap();

        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:prover".to_string(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-02T00:00:00Z".to_string(),
            },
            &producer_key,
        )
        .unwrap();
        let receipt = ReceiptBuilder::build(
            ReceiptInput::new(
                claim,
                "computational".to_string(),
                "exact".to_string(),
                vec![
                    ArtifactInput::new(
                        "witness.json".to_string(),
                        "witness".to_string(),
                        Some("a".repeat(64)),
                        None,
                    )
                    .unwrap(),
                ],
                vec!["test fixture".to_string()],
                Vec::new(),
                "agent:prover".to_string(),
                "2026-07-02T00:00:00Z".to_string(),
                format!("sha256:{}", events::event_log_hash(&frontier.events)),
                ".".to_string(),
                format!("vop_{}", "b".repeat(64)),
                "urn:vela:policy:none".to_string(),
            )
            .unwrap(),
            &identity,
        )
        .unwrap();
        let receipt_root = receipt.canonical_root().unwrap();
        let receipt_hex = receipt_root.strip_prefix("sha256:").unwrap();
        let receipt_path = format!("records/receipts/sha256/{receipt_hex}.json");
        let review_path = format!("records/review/sha256/{receipt_hex}.json");
        std::fs::create_dir_all(dir.join("records/receipts/sha256")).unwrap();
        std::fs::write(dir.join(&receipt_path), receipt.canonical_bytes().unwrap()).unwrap();

        let proposal = new_proposal_at(
            "finding.review",
            crate::events::StateTarget {
                r#type: "finding".to_string(),
                id: "vf_target".to_string(),
            },
            "agent:prover",
            "agent",
            "exact witness re-derived",
            json!({
                "status": "accepted",
                "finding": finding,
                "vela_submission": {
                    "schema": "vela.submission-links.internal.v1",
                    "receipt_root": receipt_root,
                    "receipt_path": receipt_path,
                    "record_id": "vac_test",
                    "operation_id": format!("vop_{}", "b".repeat(64)),
                    "review_material_path": review_path,
                }
            }),
            vec![receipt_path, review_path],
            Vec::new(),
            "2026-07-12T00:00:00Z",
        );
        let pid = proposal.id.clone();
        super::super::insert_pending_at_path(&dir, proposal).unwrap();

        // Human-sign the policy with a throwaway key (the test's "Will").
        write_active_policy(&dir, policy);
        (dir, pid)
    }

    fn signed_policy_head_transition(
        frontier: &project::Project,
        action: PolicyHeadAction,
        policy_id: Option<String>,
        proposal_at: &str,
        review_at: &str,
        reason: &str,
    ) -> (super::super::StateProposal, events::StateEvent) {
        let current = current_policy_head(frontier).unwrap();
        let (epoch, prior_head_event_id) = match (action, current.as_ref()) {
            (PolicyHeadAction::Activate, None) => (1, None),
            (PolicyHeadAction::Rotate | PolicyHeadAction::Revoke, Some(head)) => {
                (head.epoch + 1, Some(head.event_id.clone()))
            }
            _ => panic!("invalid test transition"),
        };
        let parent_event_ids =
            sorted_unique_ids(frontier.events.iter().map(|event| event.id.as_str()));
        let mut proposal = new_proposal_at(
            POLICY_HEAD_PROPOSAL_KIND,
            crate::events::StateTarget {
                r#type: "governance".to_string(),
                id: frontier.frontier_id.as_deref().unwrap().to_string(),
            },
            "reviewer:will",
            "human",
            reason,
            serde_json::to_value(PolicyHeadPayload {
                schema: POLICY_HEAD_SCHEMA.to_string(),
                action,
                policy_id,
                prior_head_event_id,
                expected_parent_event_log_root: format!(
                    "sha256:{}",
                    events::event_log_hash(&frontier.events)
                ),
                parent_event_ids,
                epoch,
            })
            .unwrap(),
            Vec::new(),
            Vec::new(),
            proposal_at,
        );
        validate_policy_head_proposal(frontier, &proposal).unwrap();
        let mut event = events::new_review_decision_event(
            &proposal.id,
            POLICY_HEAD_PROPOSAL_KIND,
            "accepted",
            None,
            "reviewer:will",
            reason,
            Some(review_at),
        )
        .unwrap();
        event.signature = Some(crate::sign::sign_event(&event, &test_signing_key()).unwrap());
        proposal.status = "applied".to_string();
        proposal.reviewed_by = Some("reviewer:will".to_string());
        proposal.reviewed_at = Some(review_at.to_string());
        proposal.decision_reason = Some(reason.to_string());
        proposal.applied_event_id = Some(event.id.clone());
        (proposal, event)
    }

    fn append_signed_policy_head(
        frontier: &mut project::Project,
        action: PolicyHeadAction,
        policy_id: Option<String>,
        proposal_at: &str,
        review_at: &str,
        reason: &str,
    ) -> String {
        let (proposal, event) = signed_policy_head_transition(
            frontier,
            action,
            policy_id,
            proposal_at,
            review_at,
            reason,
        );
        let event_id = event.id.clone();
        frontier.proposals.push(proposal);
        frontier.events.push(event);
        project::recompute_stats(frontier);
        event_id
    }

    fn readdress_tampered_policy_event(
        project: &mut project::Project,
        proposal_id: &str,
        rebuild_certificate_id: bool,
    ) {
        let event = project
            .events
            .iter_mut()
            .find(|event| event.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some())
            .expect("policy event");
        if rebuild_certificate_id {
            let lane = event.payload[POLICY_LANE_PAYLOAD_KEY]
                .as_object_mut()
                .unwrap();
            let mut certificate: DecisionCertificate =
                serde_json::from_value(lane["certificate"].clone()).unwrap();
            certificate.id = certificate.content_address();
            lane.insert(
                "certificate".to_string(),
                serde_json::to_value(certificate).unwrap(),
            );
        }
        event.id = events::event_id(event);
        let event_id = event.id.clone();
        project
            .proposals
            .iter_mut()
            .find(|proposal| proposal.id == proposal_id)
            .unwrap()
            .applied_event_id = Some(event_id);
    }

    fn write_policy_snapshot(dir: &Path, mut policy: AcceptancePolicy) -> VerifiedPolicy {
        policy.id = policy.content_address();
        let key = test_signing_key();
        let body = policy_signature_preimage(&policy, SIGNED_AT).unwrap();
        let signature = PolicySignatureRecord {
            policy_id: policy.id.clone(),
            signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
            signature: hex::encode(key.sign(&body).to_bytes()),
            signed_at: SIGNED_AT.to_string(),
        };
        let policy_dir = dir.join(".vela/policies");
        std::fs::write(
            policy_dir.join(format!("{}.json", policy.id)),
            serde_json::to_vec_pretty(&policy).unwrap(),
        )
        .unwrap();
        std::fs::write(
            policy_dir.join(format!("{}.sig.json", policy.id)),
            serde_json::to_vec_pretty(&signature).unwrap(),
        )
        .unwrap();
        load_policy_snapshot(dir, &policy.id).unwrap()
    }

    /// Recompute every attacker-controlled policy-event binding after a test
    /// mutates the decision context or policy. This deliberately avoids tests
    /// that only demonstrate a stale content hash is rejected.
    fn fully_readdress_policy_event(
        frontier: &mut project::Project,
        proposal_id: &str,
        verified: &VerifiedPolicy,
        decision_time: &str,
        context: &PolicyContext,
    ) -> (Decision, EngineVerdict) {
        let proposal = frontier
            .proposals
            .iter()
            .find(|proposal| proposal.id == proposal_id)
            .unwrap()
            .clone();
        let authority = resolve_policy_authority(frontier, verified, decision_time).unwrap();
        let decision = evaluate(&verified.policy, context, decision_time);
        assert_eq!(decision.outcome, Outcome::Permit);
        let event_index = frontier
            .events
            .iter()
            .position(|event| event.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some())
            .unwrap();
        let (parent_root, executor, engine_gate) = {
            let lane = &frontier.events[event_index].payload[POLICY_LANE_PAYLOAD_KEY];
            (
                lane["parent_event_log_root"].as_str().unwrap().to_string(),
                lane["executor"].as_str().unwrap().to_string(),
                serde_json::from_value::<EngineVerdict>(lane["engine_gate"].clone()).unwrap(),
            )
        };
        {
            let event = &mut frontier.events[event_index];
            event.timestamp = decision_time.to_string();
            event.actor.id = format!("policy:{}", verified.policy.id);
            event.actor.r#type = events::actor_kind(&event.actor.id).to_string();
            event.signature = None;
            let lane = event.payload[POLICY_LANE_PAYLOAD_KEY]
                .as_object_mut()
                .unwrap();
            lane.insert("policy_id".to_string(), json!(verified.policy.id));
            lane.insert("policy_signed_at".to_string(), json!(verified.signed_at));
            lane.insert("decision_time".to_string(), json!(decision_time));
            lane.insert("rule_ids".to_string(), json!(decision.matched_rule_ids));
            lane.insert("context".to_string(), json!(context));
            lane.remove("certificate");
        }
        let (_, transition_root_after) =
            policy_transition_roots(&frontier.events[event_index]).unwrap();
        let certificate = DecisionCertificate::build(
            &decision,
            frontier.frontier_id.as_deref().unwrap(),
            proposal_id,
            &parent_root,
            &transition_root_after,
            AuthorityMode::PolicyDelegation,
            authority.human_authorizers,
            &executor,
            &format!("assurance_level_a{}", context.assurance_level),
            context.assurance_level,
            &proposal_claim_digest(&proposal),
            context.impact_tier,
            false,
        );
        let event_id = {
            let event = &mut frontier.events[event_index];
            event.payload[POLICY_LANE_PAYLOAD_KEY]["certificate"] = json!(certificate);
            event.id = events::event_id(event);
            event.id.clone()
        };
        let proposal = frontier
            .proposals
            .iter_mut()
            .find(|proposal| proposal.id == proposal_id)
            .unwrap();
        proposal.status = "applied".to_string();
        proposal.reviewed_by = Some(format!("policy:{}", verified.policy.id));
        proposal.reviewed_at = Some(decision_time.to_string());
        proposal.applied_event_id = Some(event_id);
        (decision, engine_gate)
    }

    #[test]
    fn permit_lands_canonical_event_with_verifiable_lane() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let store = dir.clone();

        let out = accept_under_policy_at_path(&dir, &pid, &permitting_ctx(), "agent:prover")
            .expect("permit lands");
        assert!(out.certificate.policy_id.starts_with("vap_"));
        assert_eq!(out.certificate.outcome, Outcome::Permit);
        assert_eq!(out.certificate.human_authorizers, ["reviewer:will"]);
        assert!(out.certificate.id_is_valid());
        assert_eq!(out.policy_snapshot_files.len(), 2);

        let loaded = repo::load_from_path(&store).unwrap();
        let ev = loaded
            .events
            .iter()
            .find(|e| e.id == out.event_id)
            .expect("event landed");
        assert!(ev.actor.id.starts_with("policy:vap_"));
        assert_eq!(events::actor_kind(&ev.actor.id), "agent");
        assert!(ev.signature.is_none());
        assert!(ev.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some());
        assert_eq!(
            ev.payload[POLICY_LANE_PAYLOAD_KEY]["schema"],
            POLICY_LANE_SCHEMA_V2
        );
        let head = current_policy_head(&loaded).unwrap().unwrap();
        assert_eq!(head.action, PolicyHeadAction::Activate);
        assert_eq!(
            ev.payload[POLICY_LANE_PAYLOAD_KEY]["policy_head_event_id"],
            head.event_id
        );
        assert_eq!(
            ev.payload[POLICY_LANE_PAYLOAD_KEY]["policy_head_epoch"],
            head.epoch
        );
        let stamped_certificate: DecisionCertificate =
            serde_json::from_value(ev.payload[POLICY_LANE_PAYLOAD_KEY]["certificate"].clone())
                .unwrap();
        assert_eq!(stamped_certificate, out.certificate);
        let (_, after) = policy_transition_roots(ev).unwrap();
        assert_eq!(
            stamped_certificate.state_root_before,
            ev.payload[POLICY_LANE_PAYLOAD_KEY]["parent_event_log_root"]
                .as_str()
                .unwrap()
        );
        assert_eq!(stamped_certificate.state_root_after, after);
        // Content address survived the stamp (id re-derives).
        assert_eq!(ev.id, events::event_id(ev));
        // Proposal applied and points at the stamped event.
        let p = loaded.proposals.iter().find(|p| p.id == pid).unwrap();
        assert_eq!(p.status, "applied");
        assert_eq!(p.applied_event_id.as_deref(), Some(out.event_id.as_str()));

        // The lane verifies.
        let errors = verify_policy_lane_events(&loaded, &dir);
        assert!(errors.is_empty(), "{errors:?}");

        // Tampering with the stamped context must fail verification.
        let mut tampered = repo::load_from_path(&store).unwrap();
        for ev in tampered.events.iter_mut() {
            if let Some(lane) = ev.payload.get_mut(POLICY_LANE_PAYLOAD_KEY) {
                lane["context"]["assurance_level"] = json!(0);
            }
        }
        readdress_tampered_policy_event(&mut tampered, &pid, false);
        let errors = verify_policy_lane_events(&tampered, &dir);
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("context differs from retained evidence"),
            "{errors:?}"
        );
    }

    #[test]
    fn policy_lane_without_v2_schema_is_a_strict_error() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        accept_under_policy_at_path_at(&dir, &pid, &permitting_ctx(), "agent:prover", DECISION_AT)
            .unwrap();
        let mut frontier = repo::load_from_path(&dir).unwrap();
        let lane = frontier
            .events
            .iter_mut()
            .find_map(|event| event.payload.get_mut(POLICY_LANE_PAYLOAD_KEY))
            .expect("permit event carries a policy lane")
            .as_object_mut()
            .expect("policy lane is an object");
        lane.remove("schema");

        let errors = verify_policy_lane_events(&frontier, &dir);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].contains("strict replay accepts only vela.policy-lane.v2"),
            "{errors:?}"
        );
    }

    #[test]
    fn rotate_or_revoke_requires_successor_to_parent_preexisting_policy_lanes() {
        for action in [PolicyHeadAction::Rotate, PolicyHeadAction::Revoke] {
            let tmp = TempDir::new().unwrap();
            let (dir, pid) = seeded_frontier(&tmp);
            accept_under_policy_at_path_at(
                &dir,
                &pid,
                &permitting_ctx(),
                "agent:prover",
                DECISION_AT,
            )
            .unwrap();
            let mut frontier = repo::load_from_path(&dir).unwrap();
            let next_policy_id = if action == PolicyHeadAction::Rotate {
                let mut next = permitting_policy(frontier.frontier_id.as_deref().unwrap());
                next.epoch = 2;
                next.rules[0].id = "rotated-rule-v2".to_string();
                next.id = next.content_address();
                Some(next.id)
            } else {
                None
            };
            append_signed_policy_head(
                &mut frontier,
                action,
                next_policy_id,
                "2026-07-14T00:00:00Z",
                "2026-07-14T00:00:01Z",
                "supersede the first policy",
            );
            assert!(verify_policy_lane_events(&frontier, &dir).is_empty());

            let mut forged = frontier
                .events
                .iter()
                .find(|event| event.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some())
                .unwrap()
                .clone();
            forged.reason = "fully readdressed append after supersession".to_string();
            forged.timestamp = "2026-07-15T00:00:00Z".to_string();
            let lane = forged.payload[POLICY_LANE_PAYLOAD_KEY]
                .as_object_mut()
                .unwrap();
            lane.insert("decision_time".to_string(), json!("2026-07-15T00:00:00Z"));
            let mut certificate: DecisionCertificate =
                serde_json::from_value(lane.remove("certificate").unwrap()).unwrap();
            let (_, state_root_after) = policy_transition_roots(&forged).unwrap();
            certificate.state_root_after = state_root_after;
            certificate.id = certificate.content_address();
            assert!(certificate.id_is_valid());
            forged.payload[POLICY_LANE_PAYLOAD_KEY]["certificate"] = json!(certificate);
            forged.id = events::event_id(&forged);
            assert_eq!(forged.id, events::event_id(&forged));
            frontier.events.push(forged);

            let errors = verify_policy_lane_events(&frontier, &dir);
            assert_eq!(errors.len(), 1, "{action:?}: {errors:?}");
            assert!(
                errors[0].contains("superseded policy_lane is absent"),
                "{action:?}: {errors:?}"
            );
        }
    }

    #[test]
    fn policy_head_rejects_forks_and_never_resurrects_a_selected_policy() {
        let tmp = TempDir::new().unwrap();
        let (dir, _) = seeded_frontier(&tmp);
        let base = repo::load_from_path(&dir).unwrap();
        let old_policy_id = current_policy_head(&base)
            .unwrap()
            .unwrap()
            .policy_id
            .unwrap();

        let mut left_policy = permitting_policy(base.frontier_id.as_deref().unwrap());
        left_policy.epoch = 2;
        left_policy.rules[0].id = "left-v2".to_string();
        left_policy.id = left_policy.content_address();
        let mut right_policy = left_policy.clone();
        right_policy.rules[0].id = "right-v2".to_string();
        right_policy.id = right_policy.content_address();
        let (left_proposal, left_event) = signed_policy_head_transition(
            &base,
            PolicyHeadAction::Rotate,
            Some(left_policy.id),
            "2026-07-14T00:00:00Z",
            "2026-07-14T00:00:01Z",
            "left competing child",
        );
        let (right_proposal, right_event) = signed_policy_head_transition(
            &base,
            PolicyHeadAction::Rotate,
            Some(right_policy.id),
            "2026-07-14T00:00:00Z",
            "2026-07-14T00:00:02Z",
            "right competing child",
        );
        let mut forked = clone_project(&base);
        forked.proposals.extend([left_proposal, right_proposal]);
        forked.events.extend([left_event, right_event]);
        let error = derive_policy_head_chain(&forked).unwrap_err();
        assert!(
            error.contains("exact preceding event-log prefix") || error.contains("fork/gap"),
            "{error}"
        );

        let mut reopened = base;
        append_signed_policy_head(
            &mut reopened,
            PolicyHeadAction::Revoke,
            None,
            "2026-07-14T00:00:00Z",
            "2026-07-14T00:00:01Z",
            "close the first policy",
        );
        let revoked = current_policy_head(&reopened).unwrap().unwrap();
        let parent_event_ids =
            sorted_unique_ids(reopened.events.iter().map(|event| event.id.as_str()));
        let resurrection = new_proposal_at(
            POLICY_HEAD_PROPOSAL_KIND,
            crate::events::StateTarget {
                r#type: "governance".to_string(),
                id: reopened.frontier_id.as_deref().unwrap().to_string(),
            },
            "reviewer:will",
            "human",
            "attempt to resurrect revoked bytes",
            serde_json::to_value(PolicyHeadPayload {
                schema: POLICY_HEAD_SCHEMA.to_string(),
                action: PolicyHeadAction::Rotate,
                policy_id: Some(old_policy_id),
                prior_head_event_id: Some(revoked.event_id),
                expected_parent_event_log_root: format!(
                    "sha256:{}",
                    events::event_log_hash(&reopened.events)
                ),
                parent_event_ids,
                epoch: revoked.epoch + 1,
            })
            .unwrap(),
            Vec::new(),
            Vec::new(),
            "2026-07-15T00:00:00Z",
        );
        let error = validate_policy_head_proposal(&reopened, &resurrection).unwrap_err();
        assert!(error.contains("cannot resurrect"), "{error}");

        let mut new_policy = permitting_policy(reopened.frontier_id.as_deref().unwrap());
        new_policy.epoch = 2;
        new_policy.rules[0].id = "new-after-revoke-v2".to_string();
        new_policy.id = new_policy.content_address();
        append_signed_policy_head(
            &mut reopened,
            PolicyHeadAction::Rotate,
            Some(new_policy.id.clone()),
            "2026-07-15T00:00:00Z",
            "2026-07-15T00:00:01Z",
            "rotate to new policy bytes",
        );
        let head = current_policy_head(&reopened).unwrap().unwrap();
        assert_eq!(head.action, PolicyHeadAction::Rotate);
        assert_eq!(head.policy_id, Some(new_policy.id));
        assert_eq!(head.epoch, 3);
    }

    #[test]
    fn head_and_lane_planning_reject_noncausal_future_parents() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let mut frontier = repo::load_from_path(&dir).unwrap();
        frontier
            .events
            .push(events::new_finding_event(events::FindingEventInput {
                kind: events::EVENT_KIND_ATTESTATION_RECORDED,
                finding_id: "vf_target",
                actor_id: "ci:test",
                actor_type: "ci",
                reason: "future-vector parent",
                before_hash: events::NULL_HASH,
                after_hash: events::NULL_HASH,
                payload: json!({"note": "future"}),
                caveats: Vec::new(),
                timestamp: Some("2026-07-20T00:00:00Z"),
            }));
        let snapshot = load_active_policy_snapshot(&dir).unwrap();
        let error = stage_policy_route_with_context_at(
            &dir,
            &frontier,
            &pid,
            permitting_ctx(),
            DECISION_AT,
            &snapshot,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("must occur after causal parent"),
            "{error}"
        );

        let mut attacked = repo::load_from_path(&dir).unwrap();
        attacked
            .events
            .push(events::new_finding_event(events::FindingEventInput {
                kind: events::EVENT_KIND_ATTESTATION_RECORDED,
                finding_id: "vf_target",
                actor_id: "ci:test",
                actor_type: "ci",
                reason: "future head parent",
                before_hash: events::NULL_HASH,
                after_hash: events::NULL_HASH,
                payload: json!({"note": "future"}),
                caveats: Vec::new(),
                timestamp: Some("2026-07-20T00:00:00Z"),
            }));
        let (proposal, event) = signed_policy_head_transition(
            &attacked,
            PolicyHeadAction::Revoke,
            None,
            "2026-07-14T00:00:00Z",
            "2026-07-14T00:00:01Z",
            "noncausal head",
        );
        attacked.proposals.push(proposal);
        attacked.events.push(event);
        let error = derive_policy_head_chain(&attacked).unwrap_err();
        assert!(
            error.contains("exact preceding event-log prefix")
                || error.contains("does not occur before review"),
            "{error}"
        );
    }

    #[test]
    fn causal_prestate_never_imports_a_future_evented_finding_from_current_cache() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        accept_under_policy_at_path_at(&dir, &pid, &permitting_ctx(), "agent:prover", DECISION_AT)
            .unwrap();
        let mut frontier = repo::load_from_path(&dir).unwrap();
        let future = crate::proposals::tests::finding("vf_future_evented");
        frontier.findings.push(future.clone());
        frontier
            .events
            .push(events::new_finding_event(events::FindingEventInput {
                kind: "finding.asserted",
                finding_id: &future.id,
                actor_id: "agent:future",
                actor_type: "agent",
                reason: "future event must not leak into prior policy context",
                before_hash: events::NULL_HASH,
                after_hash: &events::finding_hash(&future),
                payload: json!({"finding": future}),
                caveats: Vec::new(),
                timestamp: Some("2026-07-20T00:00:00Z"),
            }));
        let lane_event = frontier
            .events
            .iter()
            .find(|event| event.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some())
            .unwrap();
        let lane: PolicyLaneStampV2 =
            serde_json::from_value(lane_event.payload[POLICY_LANE_PAYLOAD_KEY].clone()).unwrap();
        let (prestate, _) = reconstruct_policy_prestate(&frontier, lane_event, &lane).unwrap();
        assert!(
            prestate
                .findings
                .iter()
                .all(|finding| finding.id != "vf_future_evented")
        );
    }

    #[test]
    fn public_stage_derives_context_and_refuses_a_stale_receipt_base() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let frontier = repo::load_from_path(&dir).unwrap();
        let proposal = frontier
            .proposals
            .iter()
            .find(|proposal| proposal.id == pid)
            .unwrap();
        let receipt = load_submission_receipt(&dir, proposal).unwrap();
        let snapshot = load_active_policy_snapshot(&dir).unwrap();
        let staged = stage_policy_route_in_frontier_at(
            &dir,
            &frontier,
            &pid,
            &receipt,
            DECISION_AT,
            &snapshot,
        )
        .expect("evidence-derived public stage");
        assert_eq!(staged.context(), &permitting_ctx());
        assert_eq!(staged.decision().unwrap().outcome, Outcome::Permit);

        let mut stale: project::Project =
            serde_json::from_value(serde_json::to_value(&frontier).unwrap()).unwrap();
        stale
            .events
            .push(events::new_finding_event(events::FindingEventInput {
                kind: events::EVENT_KIND_ATTESTATION_RECORDED,
                finding_id: "vf_target",
                actor_id: "ci:test",
                actor_type: "ci",
                reason: "advance the causal base",
                before_hash: events::NULL_HASH,
                after_hash: events::NULL_HASH,
                payload: json!({"target_event_id": "vev_fixture"}),
                caveats: Vec::new(),
                timestamp: Some("2026-07-12T12:00:00Z"),
            }));
        let error =
            stage_policy_route_in_frontier_at(&dir, &stale, &pid, &receipt, DECISION_AT, &snapshot)
                .expect_err("receipt base must stale after any event-log advance");
        assert!(
            error
                .to_string()
                .contains("not the current causal pre-state"),
            "{error}"
        );

        // The same stale/foreign causal binding must not block a route that
        // cannot autonomously Permit. Removing the signed head makes the
        // active bytes human-only; the receipt is still reviewed, but its
        // producer-context root is not treated as autonomous authority.
        let mut human_only = clone_project(&frontier);
        let head_proposal_ids = human_only
            .proposals
            .iter()
            .filter(|proposal| proposal.kind == POLICY_HEAD_PROPOSAL_KIND)
            .map(|proposal| proposal.id.clone())
            .collect::<BTreeSet<_>>();
        human_only
            .proposals
            .retain(|proposal| !head_proposal_ids.contains(&proposal.id));
        human_only.events.retain(|event| {
            event
                .payload
                .get("proposal_id")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|id| !head_proposal_ids.contains(id))
        });
        project::recompute_stats(&mut human_only);
        let staged = stage_policy_route_in_frontier_at(
            &dir,
            &human_only,
            &pid,
            &receipt,
            DECISION_AT,
            &snapshot,
        )
        .expect("human-only route must not require autonomous producer bindings");
        assert_eq!(staged.permit_readiness(), PermitReadiness::HumanOnly);
        assert!(
            staged
                .policy_reason_codes()
                .iter()
                .any(|code| code == "policy_head_missing")
        );
        let error = apply_staged_policy_route_in_frontier(&mut human_only, staged, "agent:prover")
            .expect_err("human-only Permit must defer");
        assert!(
            matches!(error, PolicyLaneRefusal::Deferred { .. }),
            "{error}"
        );
    }

    #[test]
    fn fully_readdressed_forged_green_context_fails_evidence_rederivation() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        accept_under_policy_at_path_at(&dir, &pid, &permitting_ctx(), "agent:prover", DECISION_AT)
            .expect("baseline permit lands");
        let mut forged = repo::load_from_path(&dir).unwrap();
        let verified = crate::policy::acceptance_policy::load_active_policy(&dir)
            .unwrap()
            .unwrap();
        let mut forged_context = permitting_ctx();
        forged_context.assurance_level = 4;
        let (forged_decision, engine_gate) = fully_readdress_policy_event(
            &mut forged,
            &pid,
            &verified,
            DECISION_AT,
            &forged_context,
        );
        persist_test_review_material(
            &dir,
            &forged,
            &pid,
            DECISION_AT,
            &forged_context,
            Some(&forged_decision),
            PolicyState::Active,
            PermitReadiness::Ready,
            &[],
            None,
            &engine_gate,
        )
        .unwrap();

        let event = forged
            .events
            .iter()
            .find(|event| event.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some())
            .unwrap();
        assert_eq!(event.id, events::event_id(event));
        let certificate: DecisionCertificate =
            serde_json::from_value(event.payload[POLICY_LANE_PAYLOAD_KEY]["certificate"].clone())
                .unwrap();
        assert!(certificate.id_is_valid());
        assert_eq!(certificate.assurance_level, 4);

        let errors = verify_policy_lane_events(&forged, &dir);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].contains("context differs from retained evidence"),
            "{errors:?}"
        );
    }

    #[test]
    fn fully_readdressed_backdated_finite_expiry_permit_fails_closed() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        accept_under_policy_at_path_at(&dir, &pid, &permitting_ctx(), "agent:prover", DECISION_AT)
            .expect("baseline permit lands");
        let mut backdated = repo::load_from_path(&dir).unwrap();
        let mut finite = permitting_policy(backdated.frontier_id.as_deref().unwrap());
        finite.expires_at = "2026-07-13T00:00:00Z".to_string();
        let finite = write_policy_snapshot(&dir, finite);
        let backdated_time = "2026-07-12T12:00:00Z";
        let context = permitting_ctx();
        let (decision, engine_gate) =
            fully_readdress_policy_event(&mut backdated, &pid, &finite, backdated_time, &context);
        persist_test_review_material(
            &dir,
            &backdated,
            &pid,
            backdated_time,
            &context,
            Some(&decision),
            PolicyState::Active,
            PermitReadiness::Ready,
            &[],
            None,
            &engine_gate,
        )
        .unwrap();

        let event = backdated
            .events
            .iter()
            .find(|event| event.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some())
            .unwrap();
        assert_eq!(event.id, events::event_id(event));
        let certificate: DecisionCertificate =
            serde_json::from_value(event.payload[POLICY_LANE_PAYLOAD_KEY]["certificate"].clone())
                .unwrap();
        assert!(certificate.id_is_valid());
        assert_eq!(decision.outcome, Outcome::Permit);

        let errors = verify_policy_lane_events(&backdated, &dir);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].contains("does not match its signed policy-head"),
            "{errors:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn policy_snapshot_symlink_is_rejected_without_following() {
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let outcome = accept_under_policy_at_path_at(
            &dir,
            &pid,
            &permitting_ctx(),
            "agent:prover",
            DECISION_AT,
        )
        .expect("baseline permit lands");
        let loaded = repo::load_from_path(&dir).unwrap();
        let snapshot = dir.join(format!(
            ".vela/policies/{}.json",
            outcome.certificate.policy_id
        ));
        std::fs::remove_file(&snapshot).unwrap();
        symlink("active.json", &snapshot).unwrap();

        let errors = verify_policy_lane_events(&loaded, &dir);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].contains("must not traverse a symlink"),
            "{errors:?}"
        );
    }

    #[test]
    fn strengthened_certificate_replay_rejects_every_inconsistent_binding() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        accept_under_policy_at_path_at(&dir, &pid, &permitting_ctx(), "agent:prover", DECISION_AT)
            .expect("permit lands");
        let baseline = repo::load_from_path(&dir).unwrap();

        let cases = [
            (
                "schema",
                json!("vela.decision_certificate.invalid"),
                "schema",
            ),
            ("frontier_id", json!("vfr_other"), "frontier_id"),
            ("proposal_id", json!("vpr_missing"), "certificate proposal"),
            (
                "state_root_before",
                json!(format!("sha256:{}", "a".repeat(64))),
                "state_root_before",
            ),
            (
                "state_root_after",
                json!(format!("sha256:{}", "b".repeat(64))),
                "state_root_after",
            ),
            ("outcome", json!("deny"), "outcome"),
            ("policy_id", json!("vap_other"), "policy_id"),
            ("rule_ids", json!(["wrong-rule"]), "rule_ids"),
            ("evaluator", json!("other-evaluator"), "evaluator"),
            ("authority_mode", json!("direct_human"), "authority_mode"),
            (
                "human_authorizers",
                json!(["reviewer:mallory"]),
                "human_authorizers",
            ),
            ("executor", json!("agent:other"), "executor"),
            (
                "assurance_profile",
                json!("assurance_level_a0"),
                "assurance_profile",
            ),
            ("assurance_level", json!(0), "assurance_level"),
            (
                "claim_digest",
                json!(format!("sha256:{}", "c".repeat(64))),
                "claim_digest",
            ),
            ("impact_tier", json!(4), "impact_tier"),
            ("reasons", json!(["forged_reason"]), "reasons"),
            ("audit_required", json!(true), "audit_required"),
        ];
        for (field, value, expected_error) in cases {
            let mut tampered: project::Project =
                serde_json::from_value(serde_json::to_value(&baseline).expect("encode baseline"))
                    .expect("clone baseline");
            let event = tampered
                .events
                .iter_mut()
                .find(|event| event.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some())
                .unwrap();
            event.payload[POLICY_LANE_PAYLOAD_KEY]["certificate"][field] = value;
            readdress_tampered_policy_event(&mut tampered, &pid, true);
            let errors = verify_policy_lane_events(&tampered, &dir);
            assert_eq!(errors.len(), 1, "{field}: {errors:?}");
            assert!(
                errors[0].contains(expected_error),
                "{field}: expected {expected_error}, got {errors:?}"
            );
        }
    }

    #[test]
    fn strengthened_lane_rejects_certificate_id_rules_and_applied_link_tampering() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        accept_under_policy_at_path_at(&dir, &pid, &permitting_ctx(), "agent:prover", DECISION_AT)
            .expect("permit lands");
        let baseline = repo::load_from_path(&dir).unwrap();

        let mut bad_id: project::Project =
            serde_json::from_value(serde_json::to_value(&baseline).unwrap()).unwrap();
        let event = bad_id
            .events
            .iter_mut()
            .find(|event| event.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some())
            .unwrap();
        event.payload[POLICY_LANE_PAYLOAD_KEY]["certificate"]["id"] = json!("vdc_forged");
        readdress_tampered_policy_event(&mut bad_id, &pid, false);
        let errors = verify_policy_lane_events(&bad_id, &dir);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("id"), "{errors:?}");

        let mut bad_rules: project::Project =
            serde_json::from_value(serde_json::to_value(&baseline).unwrap()).unwrap();
        let event = bad_rules
            .events
            .iter_mut()
            .find(|event| event.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some())
            .unwrap();
        event.payload[POLICY_LANE_PAYLOAD_KEY]["rule_ids"] = json!(["wrong-rule"]);
        readdress_tampered_policy_event(&mut bad_rules, &pid, false);
        let errors = verify_policy_lane_events(&bad_rules, &dir);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("rule_ids"), "{errors:?}");

        let mut downgraded: project::Project =
            serde_json::from_value(serde_json::to_value(&baseline).unwrap()).unwrap();
        let event = downgraded
            .events
            .iter_mut()
            .find(|event| event.payload.get(POLICY_LANE_PAYLOAD_KEY).is_some())
            .unwrap();
        event.payload[POLICY_LANE_PAYLOAD_KEY]
            .as_object_mut()
            .unwrap()
            .remove("schema");
        readdress_tampered_policy_event(&mut downgraded, &pid, false);
        let errors = verify_policy_lane_events(&downgraded, &dir);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].contains("strict replay accepts only vela.policy-lane.v2"),
            "{errors:?}"
        );

        let mut bad_link: project::Project =
            serde_json::from_value(serde_json::to_value(&baseline).unwrap()).unwrap();
        bad_link
            .proposals
            .iter_mut()
            .find(|proposal| proposal.id == pid)
            .unwrap()
            .applied_event_id = Some("vev_wrong".to_string());
        let errors = verify_policy_lane_events(&bad_link, &dir);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("applied_event_id"), "{errors:?}");
    }

    #[test]
    fn evaluator_defer_and_expired_policy_route_land_nothing() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let mut ctx = permitting_ctx();
        ctx.assurance_level = 1; // below the rule's floor -> Defer (default)
        let err =
            accept_under_policy_at_path(&dir, &pid, &ctx, "agent:prover").expect_err("must defer");
        assert!(matches!(err, PolicyLaneRefusal::Deferred { .. }), "{err}");
        let loaded = repo::load_from_path(&dir).unwrap();
        assert_eq!(
            loaded
                .proposals
                .iter()
                .find(|p| p.id == pid)
                .unwrap()
                .status,
            "pending_review"
        );

        let mut in_memory: project::Project =
            serde_json::from_value(serde_json::to_value(&loaded).unwrap()).unwrap();
        let before = crate::canonical::to_canonical_bytes(&in_memory).unwrap();
        let err = accept_under_policy_in_frontier_at(
            &dir,
            &mut in_memory,
            &pid,
            &ctx,
            "agent:prover",
            "2026-07-13T00:00:00Z",
        )
        .expect_err("pure route must defer");
        assert!(matches!(err, PolicyLaneRefusal::Deferred { .. }));
        assert_eq!(
            before,
            crate::canonical::to_canonical_bytes(&in_memory).unwrap()
        );

        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("HOME", &dir)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        };
        mutate_active_policy(&dir, |policy| {
            policy.expires_at = "2099-12-31T23:59:59Z".to_string();
        });
        git(&["init", "-q"]);
        git(&["config", "user.email", "test@vela.invalid"]);
        git(&["config", "user.name", "Vela Test"]);
        git(&["add", "-A"]);
        git(&[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "expiry baseline",
        ]);
        let canonical_before = crate::canonical::to_canonical_bytes(
            &repo::load_from_path(&dir).expect("load expiry baseline"),
        )
        .unwrap();
        let head_before = git(&["rev-parse", "HEAD"]);
        let index_before = git(&["write-tree"]);

        let err = accept_under_policy_at_path_at(
            &dir,
            &pid,
            &permitting_ctx(),
            "agent:prover",
            "2100-01-01T00:00:00Z",
        )
        .expect_err("expired signed policy requires a human");
        assert!(
            matches!(
                err,
                PolicyLaneRefusal::Deferred { ref reasons }
                    if reasons.iter().any(|reason| reason == "policy_expired")
            ),
            "{err}"
        );
        assert_eq!(
            crate::canonical::to_canonical_bytes(&repo::load_from_path(&dir).unwrap()).unwrap(),
            canonical_before,
            "expiry deferral changed the canonical frontier"
        );
        assert_eq!(git(&["rev-parse", "HEAD"]), head_before);
        assert_eq!(git(&["write-tree"]), index_before);
        assert!(
            git(&["status", "--porcelain=v1", "--untracked-files=all"]).is_empty(),
            "expiry deferral changed the Git worktree"
        );
    }

    #[test]
    fn fully_ready_policy_preserves_intentional_evaluator_deny() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let snapshot = load_active_policy_snapshot(&dir).unwrap();
        let frontier = repo::load_from_path(&dir).unwrap();
        let assessment = assess_policy_readiness(&frontier, Ok(&snapshot), DECISION_AT);
        assert_eq!(assessment.state(), PolicyState::Active);
        assert_eq!(assessment.permit_readiness(), PermitReadiness::Ready);

        let mut context = permitting_ctx();
        context.claim_class = "forbidden".to_string();
        let error =
            accept_under_policy_at_path_at(&dir, &pid, &context, "agent:prover", DECISION_AT)
                .expect_err("an explicit deny rule must remain a denial");
        assert!(
            matches!(
                error,
                PolicyLaneRefusal::Denied { ref reasons }
                    if reasons.iter().any(|reason| reason == "explicit_deny_rule")
            ),
            "{error}"
        );
    }

    #[test]
    fn active_policy_without_head_is_human_only_and_permit_defers() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let mut frontier = repo::load_from_path(&dir).unwrap();
        let head_proposal_ids = frontier
            .proposals
            .iter()
            .filter(|proposal| proposal.kind == POLICY_HEAD_PROPOSAL_KIND)
            .map(|proposal| proposal.id.clone())
            .collect::<BTreeSet<_>>();
        frontier
            .proposals
            .retain(|proposal| !head_proposal_ids.contains(&proposal.id));
        frontier.events.retain(|event| {
            event
                .payload
                .get("proposal_id")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|id| !head_proposal_ids.contains(id))
        });
        project::recompute_stats(&mut frontier);
        let snapshot = load_active_policy_snapshot(&dir).unwrap();
        let assessment = assess_policy_readiness(&frontier, Ok(&snapshot), DECISION_AT);
        assert_eq!(assessment.state(), PolicyState::Active);
        assert_eq!(assessment.permit_readiness(), PermitReadiness::HumanOnly);
        assert_eq!(assessment.reason_codes(), ["policy_head_missing"]);

        let staged = stage_policy_route_with_context_at(
            &dir,
            &frontier,
            &pid,
            permitting_ctx(),
            DECISION_AT,
            &snapshot,
        )
        .unwrap();
        let error = apply_staged_policy_route_in_frontier(&mut frontier, staged, "agent:prover")
            .expect_err("missing policy head must defer a Permit");
        assert!(
            matches!(
                error,
                PolicyLaneRefusal::Deferred { ref reasons }
                    if reasons == &["policy_head_missing"]
            ),
            "{error}"
        );
    }

    #[test]
    fn finite_unexpired_policy_is_human_only_and_permit_defers() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        mutate_active_policy(&dir, |policy| {
            policy.expires_at = "2099-12-31T23:59:59Z".to_string();
        });
        let snapshot = load_active_policy_snapshot(&dir).unwrap();
        let mut frontier = repo::load_from_path(&dir).unwrap();
        let assessment = assess_policy_readiness(&frontier, Ok(&snapshot), DECISION_AT);
        assert_eq!(assessment.state(), PolicyState::Active);
        assert_eq!(assessment.permit_readiness(), PermitReadiness::HumanOnly);
        assert!(
            assessment
                .reason_codes()
                .iter()
                .any(|code| code == "policy_wall_clock_expiry_unanchored")
        );

        let staged = stage_policy_route_with_context_at(
            &dir,
            &frontier,
            &pid,
            permitting_ctx(),
            DECISION_AT,
            &snapshot,
        )
        .unwrap();
        let error = apply_staged_policy_route_in_frontier(&mut frontier, staged, "agent:prover")
            .expect_err("finite Permit authority must route to a human");
        assert!(
            matches!(error, PolicyLaneRefusal::Deferred { .. }),
            "{error}"
        );
    }

    #[test]
    fn malformed_policy_head_chain_is_blocked_and_staging_errors() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let snapshot = load_active_policy_snapshot(&dir).unwrap();
        let mut frontier = repo::load_from_path(&dir).unwrap();
        let head_proposal_id = frontier
            .proposals
            .iter()
            .find(|proposal| proposal.kind == POLICY_HEAD_PROPOSAL_KIND)
            .unwrap()
            .id
            .clone();
        let head_event = frontier
            .events
            .iter_mut()
            .find(|event| {
                event
                    .payload
                    .get("proposal_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(head_proposal_id.as_str())
            })
            .unwrap();
        head_event.reason.push_str(" tampered");

        let assessment = assess_policy_readiness(&frontier, Ok(&snapshot), DECISION_AT);
        assert_eq!(assessment.state(), PolicyState::Active);
        assert_eq!(assessment.permit_readiness(), PermitReadiness::Blocked);
        assert_eq!(assessment.reason_codes(), ["policy_head_invalid"]);
        assert!(
            assessment
                .detail()
                .is_some_and(|detail| detail.contains("policy-head chain is invalid"))
        );
        let error = stage_policy_route_with_context_at(
            &dir,
            &frontier,
            &pid,
            permitting_ctx(),
            DECISION_AT,
            &snapshot,
        )
        .expect_err("malformed policy head must block staging");
        assert!(matches!(error, PolicyLaneRefusal::Error(_)), "{error}");
    }

    #[test]
    fn human_executor_is_refused() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let err = accept_under_policy_at_path(&dir, &pid, &permitting_ctx(), "reviewer:will")
            .expect_err("humans use their key");
        assert!(err.to_string().contains("agent:/ci:"), "{err}");
    }

    #[test]
    fn unsigned_policy_defers_with_canonical_reason() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        std::fs::remove_file(dir.join(".vela/policies/active.sig.json")).unwrap();
        let err = accept_under_policy_at_path(&dir, &pid, &permitting_ctx(), "agent:prover")
            .expect_err("unsigned policy requires human authority");
        assert!(
            matches!(
                err,
                PolicyLaneRefusal::Deferred { ref reasons }
                    if reasons == &["policy_unsigned"]
            ),
            "{err}"
        );
    }

    #[test]
    fn unregistered_or_ambiguous_policy_signer_is_human_only() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let mut frontier = repo::load_from_path(&dir).unwrap();
        frontier.actors.clear();
        repo::save_to_path(&dir, &frontier).unwrap();
        expect_policy_head_blocked(&dir, &pid, "not registered");

        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let mut frontier = repo::load_from_path(&dir).unwrap();
        let mut duplicate = frontier.actors[0].clone();
        duplicate.id = "reviewer:second".to_string();
        frontier.actors.push(duplicate);
        repo::save_to_path(&dir, &frontier).unwrap();
        expect_authority_defer(&dir, &pid, "policy_authority_invalid");
    }

    #[test]
    fn wrong_frontier_policy_is_human_only() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        mutate_active_policy(&dir, |policy| {
            policy.frontier_id = "vfr_other".to_string();
        });
        expect_authority_defer(&dir, &pid, "policy_head_mismatch");
    }

    #[test]
    fn revoked_or_not_yet_registered_policy_signer_is_human_only() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let mut frontier = repo::load_from_path(&dir).unwrap();
        frontier.actors[0].revoked_at = Some("2026-07-10T00:00:00Z".to_string());
        frontier.actors[0].revoked_reason = Some("test rotation".to_string());
        repo::save_to_path(&dir, &frontier).unwrap();
        expect_policy_head_blocked(&dir, &pid, "at/after reviewer revocation");

        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let mut frontier = repo::load_from_path(&dir).unwrap();
        frontier.actors[0].created_at = "2026-07-04T00:00:00Z".to_string();
        repo::save_to_path(&dir, &frontier).unwrap();
        expect_authority_defer(&dir, &pid, "policy_authority_invalid");
    }

    #[test]
    fn signer_must_be_named_in_issued_by() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        mutate_active_policy(&dir, |policy| {
            policy.issued_by = vec!["reviewer:someone-else".to_string()];
        });
        expect_authority_defer(&dir, &pid, "policy_head_mismatch");
    }

    #[test]
    fn singular_signature_requires_one_reviewer_or_steward_quorum() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        mutate_active_policy(&dir, |policy| {
            policy.quorum.threshold = 2;
        });
        expect_authority_defer(&dir, &pid, "policy_head_mismatch");

        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        mutate_active_policy(&dir, |policy| {
            policy.quorum.eligible_roles = vec!["agent".to_string()];
        });
        expect_authority_defer(&dir, &pid, "policy_head_mismatch");

        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        mutate_active_policy(&dir, |policy| {
            policy.quorum.eligible_roles = vec!["steward".to_string()];
        });
        expect_authority_defer(&dir, &pid, "policy_head_mismatch");
    }

    #[test]
    fn bound_policy_signature_rejects_signed_at_rewrite() {
        let tmp = TempDir::new().unwrap();
        let (dir, _) = seeded_frontier(&tmp);
        let signature_path = dir.join(".vela/policies/active.sig.json");
        let mut signature: PolicySignatureRecord =
            serde_json::from_slice(&std::fs::read(&signature_path).unwrap()).unwrap();
        signature.signed_at = "2026-07-04T00:00:00Z".to_string();
        std::fs::write(
            &signature_path,
            serde_json::to_vec_pretty(&signature).unwrap(),
        )
        .unwrap();

        let error = crate::policy::acceptance_policy::load_active_policy(&dir)
            .expect_err("rewriting bound signed_at must invalidate the signature");
        assert!(error.contains("signature does not verify"), "{error}");
    }

    #[test]
    fn staged_route_keeps_one_verified_policy_snapshot_across_active_swap() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let snapshot = crate::policy::acceptance_policy::load_active_policy_snapshot(&dir).unwrap();
        let original_policy_id = snapshot.verified.as_ref().unwrap().policy.id.clone();
        let original_policy_bytes = snapshot.policy_bytes.clone().unwrap();
        let original_signature_bytes = snapshot.signature_bytes.clone().unwrap();
        let mut frontier = repo::load_from_path(&dir).unwrap();
        let staged = stage_policy_route_with_context_at(
            &dir,
            &frontier,
            &pid,
            permitting_ctx(),
            DECISION_AT,
            &snapshot,
        )
        .expect("stage under policy A");

        mutate_active_policy(&dir, |policy| {
            policy.rules.clear();
            policy.default = Outcome::Deny;
            policy.epoch += 1;
        });
        let replacement = crate::policy::acceptance_policy::load_active_policy(&dir)
            .unwrap()
            .unwrap();
        assert_ne!(replacement.policy.id, original_policy_id);

        let outcome = apply_staged_policy_route_in_frontier(&mut frontier, staged, "agent:prover")
            .expect("apply consumes staged policy A without reloading active policy B");
        assert_eq!(outcome.certificate.policy_id, original_policy_id);
        assert_eq!(
            outcome.policy_snapshot_files[0].bytes,
            original_policy_bytes
        );
        assert_eq!(
            outcome.policy_snapshot_files[1].bytes,
            original_signature_bytes
        );
    }

    #[test]
    fn strict_replay_rechecks_frontier_authority() {
        let tmp = TempDir::new().unwrap();
        let (dir, pid) = seeded_frontier(&tmp);
        let out = accept_under_policy_at_path_at(
            &dir,
            &pid,
            &permitting_ctx(),
            "agent:prover",
            DECISION_AT,
        )
        .expect("valid authority permits");
        let mut loaded = repo::load_from_path(&dir).unwrap();
        assert!(verify_policy_lane_events(&loaded, &dir).is_empty());
        loaded.actors[0].revoked_at = Some(DECISION_AT.to_string());
        let errors = verify_policy_lane_events(&loaded, &dir);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("no frontier authority"), "{errors:?}");
        assert!(errors[0].contains("revoked"), "{errors:?}");

        let event = loaded
            .events
            .iter()
            .find(|event| event.id == out.event_id)
            .unwrap();
        assert_eq!(event.timestamp, DECISION_AT);
    }

    #[test]
    fn legacy_policy_retirement_payload_is_closed_and_frontier_bound() {
        let frontier = project::assemble("legacy-retirement", vec![], 0, 0, "test");
        let payload = LegacyPolicyRetirementPayload {
            schema: LEGACY_POLICY_RETIREMENT_SCHEMA.to_string(),
            policy_id: "vap_e0abc750544408e637bd90e0661bac15".to_string(),
            policy_bytes_root: format!("sha256:{}", "a".repeat(64)),
            signature_bytes_root: format!("sha256:{}", "b".repeat(64)),
            retire_identical_snapshot_pair: true,
        };
        let mut proposal = crate::proposals::new_proposal_at(
            LEGACY_POLICY_RETIREMENT_PROPOSAL_KIND,
            events::StateTarget {
                r#type: "governance".to_string(),
                id: frontier.frontier_id().to_string(),
            },
            "agent:test",
            "agent",
            "retire unsupported prelaunch policy bytes",
            serde_json::to_value(payload).unwrap(),
            vec![],
            vec![],
            "2026-07-15T00:00:00Z",
        );
        validate_legacy_policy_retirement_proposal(&frontier, &proposal).unwrap();

        proposal.payload.as_object_mut().unwrap().insert(
            "caller_selected_path".to_string(),
            serde_json::json!("/tmp/key"),
        );
        let error = parse_legacy_policy_retirement_payload(&proposal).unwrap_err();
        assert!(
            error.contains("malformed") || error.contains("closed shape"),
            "{error}"
        );
    }

    #[test]
    fn legacy_policy_retirement_refuses_known_admission_history_shapes() {
        let mut frontier = project::assemble("legacy-retirement", vec![], 0, 0, "test");
        let id = "vap_e0abc750544408e637bd90e0661bac15";
        let mut proposal = crate::proposals::new_proposal_at(
            "finding.note",
            events::StateTarget {
                r#type: "finding".to_string(),
                id: "vf_fixture".to_string(),
            },
            "agent:test",
            "agent",
            "fixture",
            serde_json::json!({"text":"fixture"}),
            vec![],
            vec![],
            "2026-07-15T00:00:00Z",
        );
        proposal.status = "applied".to_string();
        proposal.reviewed_by = Some(format!("policy:{id}"));
        proposal.reviewed_at = Some("2026-07-15T01:00:00Z".to_string());
        proposal.decision_reason = Some("fixture".to_string());
        proposal.applied_event_id = Some("vev_fixture".to_string());
        frontier.proposals.push(proposal);
        let error = ensure_legacy_policy_has_no_admissions(&frontier, id).unwrap_err();
        assert!(error.contains("applied proposal reviewer"), "{error}");

        let event = |kind: &str, actor: &str, payload: serde_json::Value| events::StateEvent {
            schema: events::EVENT_SCHEMA.to_string(),
            id: "vev_legacy_history_fixture".to_string(),
            kind: kind.into(),
            target: events::StateTarget {
                r#type: "proposal".to_string(),
                id: "vpr_fixture".to_string(),
            },
            actor: events::StateActor {
                id: actor.to_string(),
                r#type: events::actor_kind(actor).to_string(),
            },
            timestamp: "2026-07-15T00:00:00Z".to_string(),
            reason: "fixture".to_string(),
            before_hash: events::NULL_HASH.to_string(),
            after_hash: events::NULL_HASH.to_string(),
            payload,
            caveats: vec![],
            signature: None,
        };

        let mut unattributed = project::assemble("legacy-retirement", vec![], 0, 0, "test");
        unattributed.events.push(event(
            events::EVENT_KIND_POLICY_AUTO_ADMITTED,
            "policy:historical",
            json!({"proposal_id":"vpr_fixture"}),
        ));
        let error = ensure_legacy_policy_has_no_admissions(&unattributed, id).unwrap_err();
        assert!(error.contains("unattributed"), "{error}");

        let mut attributed = project::assemble("legacy-retirement", vec![], 0, 0, "test");
        attributed.events.push(event(
            events::EVENT_KIND_REVIEW_ACCEPTED,
            "reviewer:test",
            json!({(POLICY_LANE_PAYLOAD_KEY):{"policy_id":id}}),
        ));
        let error = ensure_legacy_policy_has_no_admissions(&attributed, id).unwrap_err();
        assert!(error.contains("policy-lane event history"), "{error}");
    }
}
