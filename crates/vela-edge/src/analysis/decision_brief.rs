//! One read-only review projection over existing Vela state.
//!
//! `DecisionBrief` is presentation input, not a verdict, signing preimage, or
//! second state model. The builder borrows a canonical `Project`, the durable
//! Receipt v1 bytes, and the already-derived policy context/result. It performs
//! no I/O, reads no clock or key, and mutates nothing.
//!
//! The private `DecisionFacts` keeps known typed leaves separate from
//! decision-critical unknowns. Its digest exists only to catch projection
//! drift while this read contract is in testing; it is not authority.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_protocol::acceptance_policy::{Decision, Outcome, PolicyContext};
use vela_protocol::project::Project;
use vela_protocol::proposals::policy_accept::StagedPolicyRoute;
use vela_protocol::proposals::{self, EngineVerdict, StateProposal};
use vela_protocol::receipt_v1::{
    AttestationBinding, ReceiptV1, acceptance_scope_from_receipt, lineage_from_receipt,
};
use vela_protocol::verifier_attachment::{GateStatus, claim_digest, derive_gate_status};

pub const DECISION_BRIEF_SCHEMA: &str = "vela.decision-brief.testing.v1";
pub const DECISION_BRIEF_STABILITY: &str = "testing";
const DECISION_FACTS_DOMAIN: &str = "vela.decision-facts.testing.v1";
const MAX_MISSING_FACTS: usize = 16;
const MAX_CORE_TEXT_BYTES: usize = 4 * 1024;
const MAX_CAVEAT_BYTES: usize = 2 * 1024;
const MAX_REFERENCE_BYTES: usize = 512;
const MAX_RAW_REFERENCES: usize = 64;
const MAX_PRODUCER_CHECKS: usize = 32;
const MAX_FACET_DEPTH: usize = 8;
const MAX_FACET_NODES: usize = 256;
const MAX_FACET_ARRAY_ITEMS: usize = 32;
const MAX_FACET_OBJECT_FIELDS: usize = 64;
const MAX_FACET_STRING_BYTES: usize = 1024;

/// Receipt bytes available to the read projection.
///
/// Missing, invalid, and legacy material are review facts, not projection
/// failures. They produce a visible, non-signable brief when the proposal
/// depends on the material. Only a corrupt in-memory frontier is an error.
#[derive(Debug, Clone, Copy)]
pub struct ReceiptMaterial<'a> {
    source: ReceiptMaterialSource<'a>,
}

#[derive(Debug, Clone, Copy)]
enum ReceiptMaterialSource<'a> {
    Present(&'a ReceiptV1),
    Legacy(&'a ReceiptV1),
    Missing { reason: &'a str },
    Invalid { reason: &'a str },
}

impl<'a> ReceiptMaterial<'a> {
    #[must_use]
    pub fn from_receipt(receipt: &'a ReceiptV1) -> Self {
        let source = match receipt.attestation_binding() {
            AttestationBinding::Bound => ReceiptMaterialSource::Present(receipt),
            AttestationBinding::LegacyUnbound => ReceiptMaterialSource::Legacy(receipt),
        };
        Self { source }
    }

    #[must_use]
    pub fn missing(reason: &'a str) -> Self {
        Self {
            source: ReceiptMaterialSource::Missing { reason },
        }
    }

    #[must_use]
    pub fn invalid(reason: &'a str) -> Self {
        Self {
            source: ReceiptMaterialSource::Invalid { reason },
        }
    }

    fn receipt(self) -> Option<&'a ReceiptV1> {
        match self.source {
            ReceiptMaterialSource::Present(receipt) | ReceiptMaterialSource::Legacy(receipt) => {
                Some(receipt)
            }
            ReceiptMaterialSource::Missing { .. } | ReceiptMaterialSource::Invalid { .. } => None,
        }
    }

    fn blocks_accept(self, receipt_required: bool) -> bool {
        match self.source {
            ReceiptMaterialSource::Present(_) | ReceiptMaterialSource::Legacy(_) => false,
            ReceiptMaterialSource::Invalid { .. } => true,
            ReceiptMaterialSource::Missing { .. } => receipt_required,
        }
    }

    fn is_legacy(self) -> bool {
        matches!(self.source, ReceiptMaterialSource::Legacy(_))
    }

    #[cfg(test)]
    fn test_legacy(receipt: &'a ReceiptV1) -> Self {
        Self {
            source: ReceiptMaterialSource::Legacy(receipt),
        }
    }
}

/// One sealed policy-route input for review.
///
/// A live route can only be constructed from the protocol's opaque
/// [`StagedPolicyRoute`]. Callers therefore cannot combine a context from one
/// route with a decision or Engine verdict from another. The unavailable form
/// keeps broken or inapplicable routes visible without manufacturing facts.
#[derive(Debug, Clone, Copy)]
pub struct ReviewRoute<'a> {
    source: ReviewRouteSource<'a>,
}

#[derive(Debug, Clone, Copy)]
enum ReviewRouteSource<'a> {
    Staged(&'a StagedPolicyRoute),
    HumanOnly {
        policy_state: &'a str,
        reason: &'a str,
    },
    Unavailable {
        policy_state: &'a str,
        reason: &'a str,
    },
    #[cfg(test)]
    Test {
        context: &'a PolicyContext,
        decision: Option<&'a Decision>,
        engine_gate: Option<&'a EngineVerdict>,
        policy_state: &'a str,
        authority_error: Option<&'a str>,
    },
}

impl<'a> ReviewRoute<'a> {
    /// Seal the exact context, evaluator decision, Engine verdict, proposal,
    /// causal head, and evaluation instant produced by protocol staging.
    #[must_use]
    pub fn from_staged(route: &'a StagedPolicyRoute) -> Self {
        Self {
            source: ReviewRouteSource::Staged(route),
        }
    }

    /// Mark a proposal kind as intentionally outside the policy lane. This is
    /// a human-review route, not a broken route, and supplies no fabricated
    /// policy context, decision, or Engine preview.
    #[must_use]
    pub fn human_only(policy_state: &'a str, reason: &'a str) -> Self {
        Self {
            source: ReviewRouteSource::HumanOnly {
                policy_state,
                reason,
            },
        }
    }

    /// Preserve a visible degraded brief when no coherent staged route exists.
    /// No context, decision, or Engine fact is inferred in this state.
    #[must_use]
    pub fn unavailable(policy_state: &'a str, reason: &'a str) -> Self {
        Self {
            source: ReviewRouteSource::Unavailable {
                policy_state,
                reason,
            },
        }
    }

    fn context(self) -> Option<&'a PolicyContext> {
        match self.source {
            ReviewRouteSource::Staged(route) => Some(route.context()),
            ReviewRouteSource::HumanOnly { .. } => None,
            ReviewRouteSource::Unavailable { .. } => None,
            #[cfg(test)]
            ReviewRouteSource::Test { context, .. } => Some(context),
        }
    }

    fn decision(self) -> Option<&'a Decision> {
        match self.source {
            ReviewRouteSource::Staged(route) => route.decision(),
            ReviewRouteSource::HumanOnly { .. } => None,
            ReviewRouteSource::Unavailable { .. } => None,
            #[cfg(test)]
            ReviewRouteSource::Test { decision, .. } => decision,
        }
    }

    fn engine_gate(self) -> Option<&'a EngineVerdict> {
        match self.source {
            ReviewRouteSource::Staged(route) => Some(route.engine_gate()),
            ReviewRouteSource::HumanOnly { .. } => None,
            ReviewRouteSource::Unavailable { .. } => None,
            #[cfg(test)]
            ReviewRouteSource::Test { engine_gate, .. } => engine_gate,
        }
    }

    fn policy_state(self) -> &'a str {
        match self.source {
            ReviewRouteSource::Staged(route) => route.policy_state(),
            ReviewRouteSource::HumanOnly { policy_state, .. } => policy_state,
            ReviewRouteSource::Unavailable { policy_state, .. } => policy_state,
            #[cfg(test)]
            ReviewRouteSource::Test { policy_state, .. } => policy_state,
        }
    }

    fn authority_error(self) -> Option<&'a str> {
        match self.source {
            ReviewRouteSource::Staged(route) => route.authority_error(),
            ReviewRouteSource::HumanOnly { .. } => None,
            ReviewRouteSource::Unavailable { reason, .. } => Some(reason),
            #[cfg(test)]
            ReviewRouteSource::Test {
                authority_error, ..
            } => authority_error,
        }
    }

    fn requires_engine_preview(self) -> bool {
        !matches!(self.source, ReviewRouteSource::HumanOnly { .. })
    }

    fn validate_binding(
        self,
        proposal_id: &str,
        event_log_root: &str,
        observed_at: &str,
    ) -> Result<(), String> {
        let ReviewRouteSource::Staged(route) = self.source else {
            return Ok(());
        };
        if route.proposal_id() != proposal_id {
            return Err(format!(
                "staged policy route is for proposal {}, not {proposal_id}",
                route.proposal_id()
            ));
        }
        if route.state_root_before() != event_log_root {
            return Err(format!(
                "staged policy route is bound to {}, not current head {event_log_root}",
                route.state_root_before()
            ));
        }
        if route.decision_time() != observed_at {
            return Err(format!(
                "staged policy route was evaluated at {}, not projection instant {observed_at}",
                route.decision_time()
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn test(
        context: &'a PolicyContext,
        decision: Option<&'a Decision>,
        engine_gate: Option<&'a EngineVerdict>,
        policy_state: &'a str,
        authority_error: Option<&'a str>,
    ) -> Self {
        Self {
            source: ReviewRouteSource::Test {
                context,
                decision,
                engine_gate,
                policy_state,
                authority_error,
            },
        }
    }
}

/// Operational publication is a separate axis owned by the caller. The edge
/// layer carries its already-derived root/state without depending on the CLI's
/// Git transaction implementation.
#[derive(Debug, Clone, Copy)]
pub struct PublicationProjection<'a> {
    pub root: &'a str,
    pub state: &'a str,
}

/// Borrowed inputs for the pure projection.
///
/// `route` seals the existing shared policy derivation, evaluator result, and
/// Engine preview. This module deliberately does not maintain a second
/// policy-context builder.
#[derive(Debug, Clone, Copy)]
pub struct DecisionBriefInput<'a> {
    pub proposal_id: &'a str,
    pub receipt: ReceiptMaterial<'a>,
    pub route: ReviewRoute<'a>,
    pub observed_at: &'a str,
    pub replay_ok: bool,
    pub publication: Option<PublicationProjection<'a>>,
}

/// One selected queue item. The sort key and signability are derived from the
/// same facts as the brief so queue, next, status, diff, MCP, and sign cannot
/// silently disagree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewSnapshot {
    pub observed_at: String,
    pub event_log_root: String,
    pub sort_key: ReviewSortKey,
    pub brief: DecisionBrief,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ReviewSortKey {
    pub created_at: String,
    pub proposal_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionBrief {
    pub schema: String,
    pub stability: String,
    pub change: DecisionChange,
    pub basis: DecisionBasis,
    pub impact: DecisionImpact,
    pub authority: DecisionAuthority,
    pub audit: DecisionAudit,
    pub missing: Vec<MissingDecisionFact>,
    pub facets: DecisionFacets,
}

impl DecisionBrief {
    /// Find a named decision action without relying on vector position.
    #[must_use]
    pub fn action(&self, action: &str) -> Option<&DecisionAction> {
        self.authority.action(action)
    }

    /// Whether the current coherent facts permit a human accept action.
    #[must_use]
    pub fn accept_ready(&self) -> bool {
        self.action("accept")
            .is_some_and(DecisionAction::is_available)
    }

    /// Whether the current coherent facts permit a human reject action.
    #[must_use]
    pub fn reject_ready(&self) -> bool {
        self.action("reject")
            .is_some_and(DecisionAction::is_available)
    }

    /// Find an extensible typed facet by its stable map key.
    #[must_use]
    pub fn facet(&self, name: &str) -> Option<&TypedDecisionFacet> {
        self.facets.get(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionChange {
    pub subject: DecisionSubject,
    pub fixed_base: FixedBase,
    pub claim: String,
    pub before: Option<ClaimState>,
    pub after: Option<ClaimState>,
    pub requested_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionSubject {
    #[serde(rename = "type")]
    pub subject_type: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixedBase {
    /// The canonical head at which the brief was derived.
    pub event_log_root: String,
    /// The head declared by the producer receipt. A mismatch is surfaced as a
    /// critical warning; it is never silently normalized to the current head.
    pub receipt_event_log_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaimState {
    pub id: String,
    #[serde(rename = "type")]
    pub claim_type: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionBasis {
    pub primary_evidence_roots: Vec<EvidenceRoot>,
    pub check_state: DecisionCheckState,
    pub main_caveat: Option<String>,
    pub attributed_interpretation: Option<AttributedInterpretation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceRoot {
    pub kind: String,
    pub root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionCheckState {
    /// Derived only from durable verifier attachments in the Project.
    pub gate_status: String,
    pub gate_reasons: Vec<String>,
    pub durable_verifier_count: usize,
    pub durable_verifier_snapshot_root: String,
    pub engine_status: Option<String>,
    pub engine_new_blocking: Vec<String>,
    pub engine_new_warnings: Vec<String>,
    /// Producer reports remain attributed provenance and cannot raise the gate.
    pub producer_reported: Vec<ProducerReportedCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProducerReportedCheck {
    pub method: String,
    pub outcome: String,
    pub authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttributedInterpretation {
    pub actor: String,
    pub authority: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionImpact {
    pub downstream_effect: DownstreamEffect,
    pub correction_path: CorrectionPath,
    pub critical_warnings: Vec<CriticalWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DownstreamEffect {
    pub changed_findings: u32,
    pub downstream_dependents: u32,
    pub impact_tier: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CorrectionPath {
    pub while_pending: Vec<String>,
    pub after_acceptance: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CriticalWarning {
    pub code: String,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionAuthority {
    pub frontier: FrontierReference,
    pub route: String,
    pub scope: String,
    pub why_human: Vec<String>,
    pub actions: Vec<DecisionAction>,
}

impl DecisionAuthority {
    #[must_use]
    pub fn action(&self, action: &str) -> Option<&DecisionAction> {
        self.actions
            .iter()
            .find(|candidate| candidate.action == action)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionAction {
    pub action: String,
    pub eligibility: String,
    pub reasons: Vec<String>,
}

impl DecisionAction {
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.eligibility == "available"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrontierReference {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionAudit {
    pub observed_at: String,
    pub proposal_id: String,
    pub proposal_root: String,
    pub decision_facts_root: String,
    pub receipt_root: Option<String>,
    pub declared_receipt_root: Option<String>,
    pub artifact_root: Option<String>,
    pub policy_input_root: String,
    pub policy_result_root: String,
    pub publication_root: Option<String>,
    pub raw_references_root: String,
    pub raw_references: Vec<String>,
    pub raw_references_truncated: usize,
    pub missing_root: String,
    pub missing_truncated: usize,
    pub truncations: Vec<TruncationFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct TruncationFact {
    pub field: String,
    pub full_root: String,
    pub omitted_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct MissingDecisionFact {
    pub field: String,
    pub reason: String,
}

/// Sorted, extensible typed facets. New facets do not require changing the
/// Decision Brief struct or freezing another fixed field inventory.
pub type DecisionFacets = BTreeMap<String, TypedDecisionFacet>;

/// Bounded typed facet. `full_root` binds the complete selected source value;
/// `data` is the safe bounded projection and never fetches or executes a
/// locator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypedDecisionFacet {
    pub schema: String,
    pub critical: bool,
    pub full_root: String,
    pub truncated: bool,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LeaseFact {
    pub obligation_id: String,
    pub claimant_actor: String,
    pub claimed_at: String,
    pub lease_ttl_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContradictionFact {
    pub id: String,
    pub other_subject: String,
    pub adjudicated: bool,
}

/// Build the one testing-stage Decision Brief for a pending proposal.
///
/// The function is intentionally read-only and deterministic. A caller may
/// invoke it before any human answer or key-custody ceremony.
pub fn build_decision_brief(
    project: &Project,
    input: DecisionBriefInput<'_>,
) -> Result<DecisionBrief, String> {
    Ok(build_review_snapshot(project, input)?.brief)
}

pub fn build_review_snapshot(
    project: &Project,
    input: DecisionBriefInput<'_>,
) -> Result<ReviewSnapshot, String> {
    let proposal = proposal_from_project(project, input.proposal_id)?;
    let sort_key = ReviewSortKey {
        created_at: chrono::DateTime::parse_from_rfc3339(&proposal.created_at)
            .map_err(|error| format!("proposal {} created_at: {error}", proposal.id))?
            .to_utc()
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        proposal_id: proposal.id.clone(),
    };
    let facts = DecisionFacts::build(project, input)?;
    Ok(ReviewSnapshot {
        observed_at: input.observed_at.to_string(),
        event_log_root: facts.change.fixed_base.event_log_root.clone(),
        sort_key,
        brief: facts.into_brief()?,
    })
}

#[derive(Debug)]
struct DecisionFacts {
    known: Vec<KnownDecisionFact>,
    unknowns: Vec<MissingDecisionFact>,
    change: DecisionChange,
    basis: DecisionBasis,
    impact: DecisionImpact,
    authority: DecisionAuthority,
    audit: AuditFacts,
    facets: DecisionFacets,
}

#[derive(Debug)]
struct AuditFacts {
    observed_at: String,
    proposal_id: String,
    proposal_root: String,
    receipt_root: Option<String>,
    declared_receipt_root: Option<String>,
    artifact_root: Option<String>,
    policy_input_root: String,
    policy_result_root: String,
    engine_gate_root: String,
    semantic_effect_root: String,
    publication_root: Option<String>,
    raw_references_root: String,
    raw_references: Vec<String>,
    raw_references_truncated: usize,
    missing_root: String,
    missing_truncated: usize,
    truncations: Vec<TruncationFact>,
}

#[derive(Debug, Serialize)]
struct KnownDecisionFact {
    name: String,
    value: DecisionFactValue,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum DecisionFactValue {
    Root(String),
}

#[derive(Serialize)]
struct DecisionFactsDigest<'a> {
    facts: &'a [KnownDecisionFact],
    unknowns: &'a [MissingDecisionFact],
}

impl DecisionFacts {
    fn build(project: &Project, input: DecisionBriefInput<'_>) -> Result<Self, String> {
        let proposal = proposal_from_project(project, input.proposal_id)?;
        if proposal.status != "pending_review" {
            return Err(format!(
                "decision brief requires a pending_review proposal, got {}",
                proposal.status
            ));
        }
        let expected_id = proposals::proposal_id(proposal);
        if proposal.id != expected_id {
            return Err(format!(
                "proposal id does not match logical content: stored {}, derived {expected_id}",
                proposal.id
            ));
        }

        let mut unknowns = Vec::new();
        let mut truncations = Vec::new();
        let submission = proposal.payload.get("vela_submission");
        let receipt_required = submission.is_some();
        if matches!(input.route.source, ReviewRouteSource::HumanOnly { .. })
            && proposal.kind == "finding.add"
            && proposal.target.r#type == "finding"
            && input
                .receipt
                .receipt()
                .is_some_and(|receipt| receipt.attestation_binding() == AttestationBinding::Bound)
        {
            return Err(
                "body-bound receipt finding.add proposals require a staged policy route"
                    .to_string(),
            );
        }
        let declared_receipt_root = submission
            .and_then(|value| value.get("receipt_root"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let receipt_path = submission
            .and_then(|value| value.get("receipt_path"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty());

        match input.receipt.source {
            ReceiptMaterialSource::Missing { reason } if receipt_required => {
                push_missing(&mut unknowns, "basis.receipt", reason);
            }
            ReceiptMaterialSource::Invalid { reason } => {
                push_missing(&mut unknowns, "basis.receipt", reason);
            }
            ReceiptMaterialSource::Legacy(_) => {
                // Exact legacy bytes remain reviewable by a human, but never
                // become policy or verifier authority.
            }
            ReceiptMaterialSource::Present(_) | ReceiptMaterialSource::Missing { .. } => {}
        }
        if receipt_required && declared_receipt_root.is_none() {
            push_missing(
                &mut unknowns,
                "audit.declared_receipt_root",
                "proposal_receipt_binding_absent",
            );
        }
        if receipt_required && receipt_path.is_none() {
            push_missing(
                &mut unknowns,
                "audit.raw_references.receipt_path",
                "durable_receipt_locator_absent",
            );
        }

        let receipt = input.receipt.receipt();
        let receipt_value = receipt.map(ReceiptV1::as_value);
        let receipt_root = receipt
            .map(|receipt| receipt.canonical_root().map_err(|error| error.to_string()))
            .transpose()?;
        let artifact_root = receipt_value
            .and_then(|value| value.get("artifacts"))
            .map(typed_root)
            .transpose()?;
        if receipt_required && receipt_root.is_none() {
            push_missing(
                &mut unknowns,
                "audit.receipt_root",
                "canonical_receipt_unavailable",
            );
        }
        if let (Some(declared), Some(actual)) = (&declared_receipt_root, &receipt_root)
            && declared != actual
        {
            push_missing(
                &mut unknowns,
                "audit.receipt_root",
                "proposal_receipt_root_mismatch",
            );
        }

        let proposal_root = typed_root(proposal)?;
        let event_log_root = format!(
            "sha256:{}",
            vela_protocol::events::event_log_hash(&project.events)
        );
        input
            .route
            .validate_binding(&proposal.id, &event_log_root, input.observed_at)?;
        let human_routed = vela_protocol::events::actor_kind(&proposal.actor.id) == "human";
        let policy_context = input.route.context();
        let policy_input_root = if let Some(context) = policy_context {
            context.policy_language_digest()?
        } else if let ReviewRouteSource::HumanOnly {
            policy_state,
            reason,
        } = input.route.source
        {
            typed_root(&json!({
                "state": "human_only",
                "policy_state": policy_state,
                "reason_root": text_root(reason),
            }))?
        } else {
            push_missing(
                &mut unknowns,
                "authority.policy_context",
                "coherent_policy_route_unavailable",
            );
            typed_root(&json!({"state": "unavailable"}))?
        };
        let policy_result_root = typed_root(&review_route_value(input.route, human_routed))?;
        let receipt_event_log_root = receipt_value
            .and_then(|value| value.pointer("/environment/vela:producer_context/event_log_root"))
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let before_raw = project
            .findings
            .iter()
            .find(|finding| finding.id == proposal.target.id)
            .map(|finding| ClaimState {
                id: finding.id.clone(),
                claim_type: finding.assertion.assertion_type.clone(),
                text: finding.assertion.text.clone(),
            });
        let after_raw = claim_state_from_proposal(proposal);
        if after_raw.is_none()
            && matches!(
                proposal.kind.as_str(),
                "finding.add" | "finding.update" | "finding.supersede"
            )
        {
            push_missing(
                &mut unknowns,
                "change.after",
                "proposal_semantic_after_absent",
            );
        }
        let receipt_claim = receipt_value
            .and_then(|value| value.get("claim"))
            .and_then(Value::as_str);
        let claim_full = after_raw
            .as_ref()
            .map(|state| state.text.as_str())
            .or(receipt_claim)
            .unwrap_or(&proposal.reason)
            .to_string();
        let semantic_effect_root = typed_root(&json!({
            "subject": {
                "type": proposal.target.r#type,
                "id": proposal.target.id,
            },
            "claim": &claim_full,
            "before": &before_raw,
            "after": &after_raw,
            "requested_action": proposal.kind,
        }))?;
        let claim_raw = claim_full.as_str();
        let claim = bounded_text(
            "change.claim",
            claim_raw,
            MAX_CORE_TEXT_BYTES,
            &mut truncations,
        );
        let before =
            before_raw.map(|state| bound_claim_state("change.before", state, &mut truncations));
        let after =
            after_raw.map(|state| bound_claim_state("change.after", state, &mut truncations));
        let change = DecisionChange {
            subject: DecisionSubject {
                subject_type: proposal.target.r#type.clone(),
                id: proposal.target.id.clone(),
            },
            fixed_base: FixedBase {
                event_log_root: event_log_root.clone(),
                receipt_event_log_root: receipt_event_log_root.clone(),
            },
            claim: claim.clone(),
            before,
            after,
            requested_action: proposal.kind.clone(),
        };

        let mut attachments = project
            .verifier_attachments
            .iter()
            .filter(|attachment| attachment.target == proposal.target.id)
            .cloned()
            .collect::<Vec<_>>();
        attachments.sort_by(|left, right| left.id.cmp(&right.id));
        let durable_verifier_snapshot_root = typed_root(&attachments)?;
        let gate = derive_gate_status(&claim_digest(claim_raw), &attachments);
        let producer_reported = receipt_value
            .map(|receipt| producer_reported_checks(receipt, &mut truncations))
            .unwrap_or_default();
        let engine_gate = input.route.engine_gate();
        let engine_gate_root = typed_root(&engine_gate)?;
        let check_state = DecisionCheckState {
            gate_status: gate_status_name(gate.status).to_string(),
            gate_reasons: gate.reasons,
            durable_verifier_count: attachments.len(),
            durable_verifier_snapshot_root: durable_verifier_snapshot_root.clone(),
            engine_status: engine_gate.map(|gate| gate.status.clone()),
            engine_new_blocking: engine_gate
                .map(|gate| gate.new_blocking.clone())
                .unwrap_or_default(),
            engine_new_warnings: engine_gate
                .map(|gate| gate.new_warnings.clone())
                .unwrap_or_default(),
            producer_reported,
        };
        let mut primary_evidence_roots = Vec::new();
        if let Some(root) = &receipt_root {
            primary_evidence_roots.push(EvidenceRoot {
                kind: "receipt".to_string(),
                root: root.clone(),
            });
        } else if let Some(root) = &declared_receipt_root {
            primary_evidence_roots.push(EvidenceRoot {
                kind: "declared_receipt_unavailable".to_string(),
                root: root.clone(),
            });
        }
        if let Some(root) = &artifact_root {
            primary_evidence_roots.push(EvidenceRoot {
                kind: "artifact_set".to_string(),
                root: root.clone(),
            });
        }
        primary_evidence_roots.push(EvidenceRoot {
            kind: "durable_verifier_snapshot".to_string(),
            root: durable_verifier_snapshot_root,
        });

        let caveat_raw = receipt_value
            .and_then(|value| value.get("caveats"))
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .or_else(|| proposal.caveats.first().map(String::as_str));
        let main_caveat = caveat_raw.map(|value| {
            bounded_text(
                "basis.main_caveat",
                value,
                MAX_CAVEAT_BYTES,
                &mut truncations,
            )
        });
        if main_caveat.is_none() {
            push_missing(&mut unknowns, "basis.main_caveat", "decision_caveat_absent");
        }
        let producer_actor = receipt_value
            .and_then(|value| value.pointer("/provenance/submitter/actor"))
            .and_then(Value::as_str)
            .unwrap_or(&proposal.actor.id);
        let receipt_status_authority = receipt_value
            .and_then(|value| value.pointer("/status/authority"))
            .and_then(Value::as_str);
        let attributed_interpretation = receipt_claim.map(|text| AttributedInterpretation {
            actor: bounded_text(
                "basis.attributed_interpretation.actor",
                producer_actor,
                MAX_REFERENCE_BYTES,
                &mut truncations,
            ),
            authority: receipt_status_authority.unwrap_or("producer").to_string(),
            text: bounded_text(
                "basis.attributed_interpretation.text",
                text,
                MAX_CORE_TEXT_BYTES,
                &mut truncations,
            ),
        });
        if receipt_required && attributed_interpretation.is_none() {
            push_missing(
                &mut unknowns,
                "basis.attributed_interpretation",
                "producer_interpretation_unavailable",
            );
        }
        let basis = DecisionBasis {
            primary_evidence_roots,
            check_state,
            main_caveat,
            attributed_interpretation,
        };

        if project.frontier_id.is_none() {
            push_missing(&mut unknowns, "authority.frontier.id", "frontier_id_absent");
        }
        if !input.replay_ok {
            push_missing(
                &mut unknowns,
                "authority.replay_integrity",
                "frontier_replay_diverged",
            );
        }

        let (route, why_human, mut accept_blockers, policy_decision) =
            review_route_authority(input.route, human_routed);
        let why_human = why_human
            .into_iter()
            .enumerate()
            .map(|(index, reason)| {
                bounded_text(
                    &format!("authority.why_human[{index}]"),
                    &reason,
                    MAX_CAVEAT_BYTES,
                    &mut truncations,
                )
            })
            .collect();
        if input.receipt.blocks_accept(receipt_required) {
            accept_blockers.push("receipt_material_unavailable_or_invalid".to_string());
        }
        if declared_receipt_root
            .as_deref()
            .zip(receipt_root.as_deref())
            .is_some_and(|(declared, actual)| declared != actual)
        {
            accept_blockers.push("proposal_receipt_root_mismatch".to_string());
        }
        if !input.replay_ok {
            accept_blockers.push("frontier_replay_diverged".to_string());
        }
        if engine_gate.is_some_and(|gate| gate.status == "blocked" || !gate.new_blocking.is_empty())
        {
            accept_blockers.push("engine_gate_blocked".to_string());
        }
        if engine_gate.is_none() && input.route.requires_engine_preview() {
            push_missing(
                &mut unknowns,
                "basis.check_state.engine_status",
                "engine_preview_unavailable",
            );
            accept_blockers.push("engine_preview_unavailable".to_string());
        }
        if input.receipt.is_legacy() {
            // Human review remains possible because the proposal binds the
            // complete canonical receipt root. Only autonomous policy use is
            // prohibited.
        }
        accept_blockers.sort();
        accept_blockers.dedup();
        let actions = vec![
            DecisionAction {
                action: "accept".to_string(),
                eligibility: if accept_blockers.is_empty() {
                    "available"
                } else {
                    "blocked"
                }
                .to_string(),
                reasons: accept_blockers.clone(),
            },
            DecisionAction {
                action: "reject".to_string(),
                eligibility: "available".to_string(),
                reasons: Vec::new(),
            },
        ];

        let receipt_scope = receipt_value
            .and_then(acceptance_scope_from_receipt)
            .map(|scope| scope.as_str().to_string());
        if receipt_required && receipt_scope.is_none() {
            push_missing(
                &mut unknowns,
                "authority.scope",
                "acceptance_scope_unavailable",
            );
        }
        let scope = receipt_scope.unwrap_or_else(|| "frontier_review".to_string());

        let work_lease = work_lease_facet(project, &proposal.target.id)?;
        let challenge = challenge_facet(project, receipt_value, &proposal.target.id)?;
        let publication = input.publication.map(publication_facet).transpose()?;
        let acceptance_authority = acceptance_authority_facet(
            receipt,
            receipt_value,
            input.route,
            &policy_input_root,
            &policy_result_root,
        )?;
        let mut facets = rich_facets(
            project,
            proposal,
            receipt_value,
            &basis.check_state,
            policy_context.is_some_and(|context| context.independence_satisfied),
        )?;
        insert_optional_facet(&mut facets, "work_lease", work_lease);
        insert_optional_facet(&mut facets, "challenge", challenge);
        insert_optional_facet(&mut facets, "acceptance_authority", acceptance_authority);
        insert_optional_facet(&mut facets, "publication", publication);

        let mut critical_warnings = critical_warnings(
            project,
            proposal,
            input.receipt,
            &event_log_root,
            receipt_event_log_root.as_deref(),
            receipt_root.as_deref(),
            declared_receipt_root.as_deref(),
            receipt_claim,
            claim_raw,
            gate.status,
            policy_decision.map(|decision| decision.outcome),
        );
        if facets.get("challenge").is_some_and(|facet| facet.critical) {
            critical_warnings.push(CriticalWarning {
                code: "active_challenge".to_string(),
                reference: Some(proposal.target.id.clone()),
            });
        }
        if input.receipt.is_legacy() {
            critical_warnings.push(CriticalWarning {
                code: "legacy_unbound_receipt".to_string(),
                reference: receipt_root.clone(),
            });
        }
        for blocker in &accept_blockers {
            critical_warnings.push(CriticalWarning {
                code: blocker.clone(),
                reference: Some(proposal.id.clone()),
            });
        }
        critical_warnings.sort_by(|left, right| {
            (&left.code, &left.reference).cmp(&(&right.code, &right.reference))
        });
        critical_warnings.dedup();

        let impact = DecisionImpact {
            downstream_effect: policy_context.map_or_else(
                || conservative_downstream_effect(project, proposal),
                |context| DownstreamEffect {
                    changed_findings: context.changed_findings,
                    downstream_dependents: context.downstream_dependents,
                    impact_tier: context.impact_tier,
                },
            ),
            correction_path: correction_path(proposal),
            critical_warnings,
        };
        let authority = DecisionAuthority {
            frontier: FrontierReference {
                id: project.frontier_id.clone(),
                name: project.project.name.clone(),
            },
            route,
            scope,
            why_human,
            actions,
        };

        let (raw_references, raw_references_root, raw_references_truncated) =
            raw_references(proposal, receipt_value, receipt_path, &mut truncations)?;
        let publication_root = input
            .publication
            .map(|publication| publication.root.to_string());
        unknowns.sort();
        unknowns.dedup();
        let missing_root = typed_root(&unknowns)?;
        let missing_truncated = unknowns.len().saturating_sub(MAX_MISSING_FACTS);
        let audit = AuditFacts {
            observed_at: input.observed_at.to_string(),
            proposal_id: proposal.id.clone(),
            proposal_root,
            receipt_root,
            declared_receipt_root,
            artifact_root,
            policy_input_root,
            policy_result_root,
            engine_gate_root,
            semantic_effect_root,
            publication_root,
            raw_references_root,
            raw_references,
            raw_references_truncated,
            missing_root,
            missing_truncated,
            truncations,
        };

        let known = known_facts(&change, &basis, &impact, &authority, &audit, &facets)?;
        Ok(Self {
            known,
            unknowns,
            change,
            basis,
            impact,
            authority,
            audit,
            facets,
        })
    }

    fn decision_facts_root(&self) -> Result<String, String> {
        let body = DecisionFactsDigest {
            facts: &self.known,
            unknowns: &self.unknowns,
        };
        let bytes = vela_protocol::canonical::to_canonical_bytes(&body)?;
        let mut digest = Sha256::new();
        digest.update(DECISION_FACTS_DOMAIN.as_bytes());
        digest.update([0]);
        digest.update(bytes);
        Ok(format!("sha256:{}", hex::encode(digest.finalize())))
    }

    fn into_brief(self) -> Result<DecisionBrief, String> {
        let decision_facts_root = self.decision_facts_root()?;
        let missing = self
            .unknowns
            .iter()
            .take(MAX_MISSING_FACTS)
            .cloned()
            .collect();
        Ok(DecisionBrief {
            schema: DECISION_BRIEF_SCHEMA.to_string(),
            stability: DECISION_BRIEF_STABILITY.to_string(),
            change: self.change,
            basis: self.basis,
            impact: self.impact,
            authority: self.authority,
            audit: DecisionAudit {
                observed_at: self.audit.observed_at,
                proposal_id: self.audit.proposal_id,
                proposal_root: self.audit.proposal_root,
                decision_facts_root,
                receipt_root: self.audit.receipt_root,
                declared_receipt_root: self.audit.declared_receipt_root,
                artifact_root: self.audit.artifact_root,
                policy_input_root: self.audit.policy_input_root,
                policy_result_root: self.audit.policy_result_root,
                publication_root: self.audit.publication_root,
                raw_references_root: self.audit.raw_references_root,
                raw_references: self.audit.raw_references,
                raw_references_truncated: self.audit.raw_references_truncated,
                missing_root: self.audit.missing_root,
                missing_truncated: self.audit.missing_truncated,
                truncations: self.audit.truncations,
            },
            missing,
            facets: self.facets,
        })
    }
}

fn review_route_value(route: ReviewRoute<'_>, human_routed: bool) -> Value {
    if let ReviewRouteSource::HumanOnly {
        policy_state,
        reason,
    } = route.source
    {
        return json!({
            "state": "human_only",
            "policy_state": policy_state,
            "reason_root": text_root(reason),
            "decision": null,
            "engine_gate": null,
        });
    }
    if let ReviewRouteSource::Unavailable {
        policy_state,
        reason,
    } = route.source
    {
        return json!({
            "state": "unavailable",
            "policy_state": policy_state,
            "reason_root": text_root(reason),
            "decision": null,
            "engine_gate": null,
        });
    }
    json!({
        "state": if route.authority_error().is_some() { "broken" } else { "staged" },
        "policy_state": route.policy_state(),
        "authority_error_root": route.authority_error().map(text_root),
        "decision": route.decision(),
        "engine_gate": route.engine_gate(),
        "human_routed": human_routed,
    })
}

fn review_route_authority(
    route: ReviewRoute<'_>,
    human_routed: bool,
) -> (String, Vec<String>, Vec<String>, Option<&Decision>) {
    if let ReviewRouteSource::HumanOnly { reason, .. } = route.source {
        return (
            "defer".to_string(),
            vec![reason.to_string()],
            Vec::new(),
            None,
        );
    }
    if let Some(error) = route.authority_error() {
        return (
            "broken".to_string(),
            vec![format!("{}: {error}", route.policy_state())],
            vec!["policy_route_unavailable".to_string()],
            route.decision(),
        );
    }
    match route.decision() {
        None => (
            "defer".to_string(),
            vec![format!("policy_lane_{}", route.policy_state())],
            Vec::new(),
            None,
        ),
        Some(decision) => match decision.outcome {
            Outcome::Defer => (
                "defer".to_string(),
                decision.reasons.clone(),
                Vec::new(),
                Some(decision),
            ),
            Outcome::Deny => (
                "deny".to_string(),
                decision.reasons.clone(),
                vec!["policy_denied".to_string()],
                Some(decision),
            ),
            Outcome::Permit if human_routed => (
                "defer".to_string(),
                vec!["human-origin landing requires an explicit human decision".to_string()],
                Vec::new(),
                Some(decision),
            ),
            Outcome::Permit => (
                "permit_pending".to_string(),
                vec!["a policy Permit remains pending and requires repair".to_string()],
                vec!["pending_permit_invariant".to_string()],
                Some(decision),
            ),
        },
    }
}

fn text_root(value: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"vela.review-text.v1");
    digest.update([0]);
    digest.update(value.as_bytes());
    format!("sha256:{}", hex::encode(digest.finalize()))
}

fn bounded_text(
    field: &str,
    value: &str,
    limit: usize,
    truncations: &mut Vec<TruncationFact>,
) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    let mut end = limit.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    truncations.push(TruncationFact {
        field: field.to_string(),
        full_root: text_root(value),
        omitted_bytes: value.len().saturating_sub(end),
    });
    format!("{}…", &value[..end])
}

fn bound_claim_state(
    field: &str,
    mut state: ClaimState,
    truncations: &mut Vec<TruncationFact>,
) -> ClaimState {
    state.text = bounded_text(
        &format!("{field}.text"),
        &state.text,
        MAX_CORE_TEXT_BYTES,
        truncations,
    );
    state
}

fn acceptance_authority_facet(
    receipt: Option<&ReceiptV1>,
    receipt_value: Option<&Value>,
    route: ReviewRoute<'_>,
    policy_input_root: &str,
    policy_result_root: &str,
) -> Result<Option<TypedDecisionFacet>, String> {
    let (receipt, value, decision) = match (receipt, receipt_value, route.decision()) {
        (Some(receipt), Some(value), Some(decision)) => (receipt, value, decision),
        _ => return Ok(None),
    };
    typed_facet(
        "vela.decision-brief.facet.acceptance-authority.testing.v1",
        decision.outcome != Outcome::Permit,
        json!({
            "receipt_policy_ref": value
            .pointer("/acceptance/policyRef")
            .and_then(Value::as_str)
            .unwrap_or("urn:vela:policy:none"),
            "receipt_status_authority": value
            .pointer("/status/authority")
            .and_then(Value::as_str)
            .unwrap_or("producer"),
            "receipt_attestation_binding": match receipt.attestation_binding() {
                AttestationBinding::Bound => "bound",
                AttestationBinding::LegacyUnbound => "legacy_unbound",
            },
            "policy_id": decision.policy_id,
            "evaluator": decision.evaluator,
            "matched_rule_ids": decision.matched_rule_ids,
            "policy_input_root": policy_input_root,
            "policy_result_root": policy_result_root,
        }),
    )
    .map(Some)
}

fn rich_facets(
    project: &Project,
    proposal: &StateProposal,
    receipt: Option<&Value>,
    check_state: &DecisionCheckState,
    independence_satisfied: bool,
) -> Result<DecisionFacets, String> {
    let mut facets = DecisionFacets::default();
    let claim_type = proposal
        .payload
        .pointer("/finding/assertion/type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut attestations = project
        .statement_attestations
        .iter()
        .filter(|attestation| attestation.target == proposal.target.id)
        .map(|attestation| {
            json!({
                "id": attestation.id,
                "verdict": format!("{:?}", attestation.verdict).to_lowercase(),
                "informal_ref": attestation.informal_ref,
                "formal_ref": attestation.formal_ref,
                "attested_by": attestation.attested_by,
            })
        })
        .collect::<Vec<_>>();
    attestations.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    if claim_type == "theoretical" || !attestations.is_empty() {
        facets.insert(
            "formal_fidelity".to_string(),
            typed_facet(
                "vela.decision-brief.facet.formal-fidelity.testing.v1",
                claim_type == "theoretical" && attestations.is_empty(),
                json!({
                    "claim_type": claim_type,
                    "statement_attestations": attestations,
                }),
            )?,
        );
    }

    facets.insert(
        "gate_matrix".to_string(),
        typed_facet(
            "vela.decision-brief.facet.gate-matrix.testing.v1",
            check_state.gate_status == "refuted"
                || check_state.engine_status.as_deref() == Some("blocked"),
            json!({
                "durable_gate": {
                    "status": check_state.gate_status,
                    "reasons": check_state.gate_reasons,
                    "attachment_count": check_state.durable_verifier_count,
                    "snapshot_root": check_state.durable_verifier_snapshot_root,
                },
                "engine": {
                    "status": check_state.engine_status,
                    "new_blocking": check_state.engine_new_blocking,
                    "new_warnings": check_state.engine_new_warnings,
                }
            }),
        )?,
    );

    if let Some(receipt) = receipt {
        if let Some(lineage) = receipt.get("lineage") {
            facets.insert("evidence_lineage".to_string(), typed_facet(
                "vela.decision-brief.facet.evidence-lineage.testing.v1",
                false,
                json!({
                    "parents": lineage.get("parents").cloned().unwrap_or_else(|| json!([])),
                    "derived_from": lineage.get("derived_from").cloned().unwrap_or_else(|| json!([])),
                    "source_refs": lineage.get("source_refs").cloned().unwrap_or_else(|| json!([])),
                    "supersedes": lineage.get("supersedes").cloned().unwrap_or_else(|| json!([])),
                }),
            )?);
        }
        if let Some(status) = receipt.get("status") {
            facets.insert("hypothesis_evolution".to_string(), typed_facet(
                "vela.decision-brief.facet.hypothesis-evolution.testing.v1",
                false,
                json!({
                    "kind": status.get("kind").cloned().unwrap_or(Value::Null),
                    "evidence_status": status.get("evidence_status").cloned().unwrap_or(Value::Null),
                    "authority": status.get("authority").cloned().unwrap_or(Value::Null),
                }),
            )?);
        }
        if let Some(distillation) = receipt.get("distillation") {
            facets.insert(
                "distillation".to_string(),
                typed_facet(
                    "vela.decision-brief.facet.distillation.testing.v1",
                    distillation.get("status").and_then(Value::as_str) == Some("missing"),
                    select_object_fields(
                        distillation,
                        &[
                            "status",
                            "uri",
                            "digest",
                            "audience",
                            "level",
                            "rubric",
                            "comprehension_budget",
                            "inheritance_note",
                            "known_gaps",
                        ],
                    ),
                )?,
            );
        }
        if let Some(contributors) = receipt.get("contributors") {
            facets.insert(
                "contributor_roles".to_string(),
                typed_facet(
                    "vela.decision-brief.facet.contributor-roles.testing.v1",
                    false,
                    contributors.clone(),
                )?,
            );
        }

        let identities = receipt
            .get("signature_identities")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|values| values.values())
            .filter_map(|identity| {
                Some(json!({
                    "role": identity.get("role")?.as_str()?,
                    "mechanism": identity.get("mechanism")?.as_str()?,
                    "reference": identity.get("signatureRef").cloned().unwrap_or(Value::Null),
                }))
            })
            .collect::<Vec<_>>();
        let external_profiles = receipt
            .pointer("/machine/external_profiles")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|profile| -> Result<Value, String> {
                let certificate_root = profile.get("certificate").map(typed_root).transpose()?;
                Ok(json!({
                    "profile": profile.get("profile").cloned().unwrap_or(Value::Null),
                    "certificate_root": certificate_root,
                }))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !identities.is_empty() || !external_profiles.is_empty() {
            facets.insert(
                "external_certificates".to_string(),
                typed_facet(
                    "vela.decision-brief.facet.external-certificates.testing.v1",
                    false,
                    json!({
                        "identity_mechanisms": identities,
                        "external_profiles": external_profiles,
                    }),
                )?,
            );
        }
    }

    let same_claim = proposal
        .payload
        .pointer("/vela_submission/same_claim_findings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !same_claim.is_empty() || independence_satisfied {
        facets.insert(
            "replication_diversity".to_string(),
            typed_facet(
                "vela.decision-brief.facet.replication-diversity.testing.v1",
                false,
                json!({
                    "same_claim_findings": same_claim,
                    "independence_satisfied": independence_satisfied,
                }),
            )?,
        );
    }
    Ok(facets)
}

fn select_object_fields(value: &Value, fields: &[&str]) -> Value {
    let mut selected = serde_json::Map::new();
    for field in fields {
        if let Some(value) = value.get(*field) {
            selected.insert((*field).to_string(), value.clone());
        }
    }
    Value::Object(selected)
}

fn typed_facet(
    schema: &str,
    critical: bool,
    full_data: Value,
) -> Result<TypedDecisionFacet, String> {
    let full_root = typed_root(&full_data)?;
    let mut budget = MAX_FACET_NODES;
    let (data, truncated) = bound_facet_value(&full_data, 1, &mut budget);
    Ok(TypedDecisionFacet {
        schema: schema.to_string(),
        critical,
        full_root,
        truncated,
        data,
    })
}

fn bound_facet_value(value: &Value, depth: usize, budget: &mut usize) -> (Value, bool) {
    if *budget == 0 || depth > MAX_FACET_DEPTH {
        return (json!({"truncated": true}), true);
    }
    *budget -= 1;
    match value {
        Value::String(text) if text.len() > MAX_FACET_STRING_BYTES => {
            let mut end = MAX_FACET_STRING_BYTES;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            (Value::String(format!("{}…", &text[..end])), true)
        }
        Value::Array(values) => {
            let mut truncated = values.len() > MAX_FACET_ARRAY_ITEMS;
            let mut out = Vec::new();
            for value in values.iter().take(MAX_FACET_ARRAY_ITEMS) {
                let (value, child_truncated) = bound_facet_value(value, depth + 1, budget);
                truncated |= child_truncated;
                out.push(value);
            }
            (Value::Array(out), truncated)
        }
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            let mut truncated = keys.len() > MAX_FACET_OBJECT_FIELDS;
            let mut out = serde_json::Map::new();
            for key in keys.into_iter().take(MAX_FACET_OBJECT_FIELDS) {
                let (value, child_truncated) = bound_facet_value(&values[key], depth + 1, budget);
                truncated |= child_truncated;
                out.insert(key.clone(), value);
            }
            (Value::Object(out), truncated)
        }
        _ => (value.clone(), false),
    }
}

fn proposal_from_project<'a>(
    project: &'a Project,
    proposal_id: &str,
) -> Result<&'a StateProposal, String> {
    let mut matches = project
        .proposals
        .iter()
        .filter(|proposal| proposal.id == proposal_id);
    let proposal = matches
        .next()
        .ok_or_else(|| format!("proposal {proposal_id} not found in frontier"))?;
    if matches.next().is_some() {
        return Err(format!(
            "proposal {proposal_id} is duplicated in frontier projection"
        ));
    }
    Ok(proposal)
}

fn claim_state_from_proposal(proposal: &StateProposal) -> Option<ClaimState> {
    let candidate = proposal.payload.get("finding").unwrap_or(&proposal.payload);
    let text = candidate
        .pointer("/assertion/text")
        .and_then(|value| value.as_str())
        .or_else(|| candidate.get("text").and_then(|value| value.as_str()))?;
    let claim_type = candidate
        .pointer("/assertion/type")
        .and_then(|value| value.as_str())
        .or_else(|| {
            candidate
                .pointer("/assertion/assertion_type")
                .and_then(|value| value.as_str())
        })
        .or_else(|| candidate.get("type").and_then(|value| value.as_str()))
        .unwrap_or("unknown");
    let id = candidate
        .get("id")
        .and_then(|value| value.as_str())
        .unwrap_or(&proposal.target.id);
    Some(ClaimState {
        id: id.to_string(),
        claim_type: claim_type.to_string(),
        text: text.to_string(),
    })
}

fn producer_reported_checks(
    receipt: &serde_json::Value,
    truncations: &mut Vec<TruncationFact>,
) -> Vec<ProducerReportedCheck> {
    receipt
        .get("verifier_runs")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .take(MAX_PRODUCER_CHECKS)
        .enumerate()
        .filter_map(|(index, run)| {
            Some(ProducerReportedCheck {
                method: bounded_text(
                    &format!("basis.check_state.producer_reported[{index}].method"),
                    run.get("method")?.as_str()?,
                    MAX_REFERENCE_BYTES,
                    truncations,
                ),
                outcome: bounded_text(
                    &format!("basis.check_state.producer_reported[{index}].outcome"),
                    run.get("outcome")?.as_str()?,
                    MAX_REFERENCE_BYTES,
                    truncations,
                ),
                authority: "producer".to_string(),
            })
        })
        .collect()
}

fn gate_status_name(status: GateStatus) -> &'static str {
    match status {
        GateStatus::NeedsVerification => "needs_verification",
        GateStatus::Verified => "verified",
        GateStatus::Refuted => "refuted",
    }
}

fn correction_path(proposal: &StateProposal) -> CorrectionPath {
    let after_acceptance = if proposal.target.r#type == "finding" {
        vec![
            "finding.retract".to_string(),
            "finding.supersede".to_string(),
        ]
    } else {
        Vec::new()
    };
    CorrectionPath {
        while_pending: vec!["reject".to_string()],
        after_acceptance,
    }
}

fn conservative_downstream_effect(project: &Project, proposal: &StateProposal) -> DownstreamEffect {
    let downstream_dependents = project
        .findings
        .iter()
        .filter(|candidate| {
            candidate.links.iter().any(|link| {
                vela_protocol::bundle::bare_finding_id(&link.target) == proposal.target.id
            })
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX);
    DownstreamEffect {
        changed_findings: u32::from(proposal.target.r#type == "finding"),
        downstream_dependents,
        // Missing a coherent route can never make the apparent risk smaller.
        impact_tier: if proposal.kind.starts_with("governance.") {
            4
        } else {
            3
        },
    }
}

fn insert_optional_facet(
    facets: &mut DecisionFacets,
    name: &str,
    facet: Option<TypedDecisionFacet>,
) {
    if let Some(facet) = facet {
        facets.insert(name.to_string(), facet);
    }
}

fn work_lease_facet(
    project: &Project,
    subject_id: &str,
) -> Result<Option<TypedDecisionFacet>, String> {
    let mut leases = project
        .attempt_claims
        .iter()
        .filter(|claim| claim.obligation_id == subject_id)
        .map(|claim| LeaseFact {
            obligation_id: claim.obligation_id.clone(),
            claimant_actor: claim.claimant_actor.clone(),
            claimed_at: claim.claimed_at.clone(),
            lease_ttl_seconds: claim.lease_ttl_seconds,
        })
        .collect::<Vec<_>>();
    leases.sort_by(|left, right| {
        (&left.obligation_id, &left.claimed_at, &left.claimant_actor).cmp(&(
            &right.obligation_id,
            &right.claimed_at,
            &right.claimant_actor,
        ))
    });
    if leases.is_empty() {
        return Ok(None);
    }
    // This facet records coordination, not scientific authority. Expiry is
    // intentionally not guessed without an explicit read-time instant.
    typed_facet(
        "vela.decision-brief.facet.work-lease.testing.v1",
        false,
        json!({"leases": leases}),
    )
    .map(Some)
}

fn challenge_facet(
    project: &Project,
    receipt: Option<&serde_json::Value>,
    subject_id: &str,
) -> Result<Option<TypedDecisionFacet>, String> {
    let existing = project
        .findings
        .iter()
        .find(|finding| finding.id == subject_id);
    let target_contested = existing.is_some_and(|finding| finding.flags.contested);
    let target_superseded = existing.is_some_and(|finding| finding.flags.superseded);
    let mut open_contradictions = project
        .contradictions
        .iter()
        .filter(|contradiction| {
            contradiction.is_open()
                && (contradiction.finding_a == subject_id || contradiction.finding_b == subject_id)
        })
        .map(|contradiction| ContradictionFact {
            id: contradiction.contradiction_id.clone(),
            other_subject: if contradiction.finding_a == subject_id {
                contradiction.finding_b.clone()
            } else {
                contradiction.finding_a.clone()
            },
            adjudicated: contradiction.is_adjudicated(),
        })
        .collect::<Vec<_>>();
    open_contradictions.sort_by(|left, right| left.id.cmp(&right.id));
    let mut supersedes = receipt
        .and_then(lineage_from_receipt)
        .map(|lineage| lineage.supersedes)
        .unwrap_or_default();
    supersedes.sort();
    supersedes.dedup();
    let critical = target_contested || !open_contradictions.is_empty();
    if !critical && !target_superseded && supersedes.is_empty() {
        return Ok(None);
    }
    typed_facet(
        "vela.decision-brief.facet.challenge.testing.v1",
        critical,
        json!({
            "target_contested": target_contested,
            "target_superseded": target_superseded,
            "open_contradictions": open_contradictions,
            "supersedes": supersedes,
        }),
    )
    .map(Some)
}

fn publication_facet(input: PublicationProjection<'_>) -> Result<TypedDecisionFacet, String> {
    if input.root.trim().is_empty() {
        return Err("publication root must be non-empty when supplied".to_string());
    }
    if input.state.trim().is_empty() {
        return Err("publication state must be non-empty when supplied".to_string());
    }
    typed_facet(
        "vela.decision-brief.facet.publication.testing.v1",
        false,
        json!({
            "root": input.root,
            "state": input.state,
        }),
    )
}

#[allow(clippy::too_many_arguments)]
fn critical_warnings(
    project: &Project,
    proposal: &StateProposal,
    receipt: ReceiptMaterial<'_>,
    event_log_root: &str,
    receipt_event_log_root: Option<&str>,
    receipt_root: Option<&str>,
    declared_receipt_root: Option<&str>,
    receipt_claim: Option<&str>,
    proposed_claim: &str,
    gate_status: GateStatus,
    policy_outcome: Option<Outcome>,
) -> Vec<CriticalWarning> {
    let mut warnings = Vec::new();
    if receipt_event_log_root.is_some_and(|root| root != event_log_root) {
        warnings.push(CriticalWarning {
            code: "receipt_base_stale".to_string(),
            reference: receipt_event_log_root.map(ToString::to_string),
        });
    }
    if let (Some(declared), Some(actual)) = (declared_receipt_root, receipt_root)
        && declared != actual
    {
        warnings.push(CriticalWarning {
            code: "proposal_receipt_root_mismatch".to_string(),
            reference: declared_receipt_root.map(ToString::to_string),
        });
    }
    if receipt_claim.is_some_and(|claim| claim != proposed_claim) {
        warnings.push(CriticalWarning {
            code: "proposal_receipt_claim_mismatch".to_string(),
            reference: Some(proposal.id.clone()),
        });
    }
    if receipt.is_legacy() {
        warnings.push(CriticalWarning {
            code: "legacy_unbound_receipt".to_string(),
            reference: receipt_root.map(ToString::to_string),
        });
    }
    if gate_status == GateStatus::Refuted {
        warnings.push(CriticalWarning {
            code: "durable_gate_refuted".to_string(),
            reference: Some(proposal.target.id.clone()),
        });
    }
    if policy_outcome == Some(Outcome::Deny) {
        warnings.push(CriticalWarning {
            code: "policy_denied".to_string(),
            reference: Some(proposal.id.clone()),
        });
    }
    if project
        .findings
        .iter()
        .find(|finding| finding.id == proposal.target.id)
        .is_some_and(|finding| finding.flags.contested)
    {
        warnings.push(CriticalWarning {
            code: "target_contested".to_string(),
            reference: Some(proposal.target.id.clone()),
        });
    }
    warnings
}

fn raw_references(
    proposal: &StateProposal,
    receipt: Option<&serde_json::Value>,
    receipt_path: Option<&str>,
    truncations: &mut Vec<TruncationFact>,
) -> Result<(Vec<String>, String, usize), String> {
    let mut references = proposal.source_refs.clone();
    if let Some(path) = receipt_path {
        references.push(path.to_string());
    }
    if let Some(lineage) = receipt.and_then(lineage_from_receipt) {
        references.extend(lineage.parents);
        references.extend(lineage.derived_from);
        references.extend(lineage.source_refs);
        references.extend(lineage.supersedes);
    }
    if let Some(artifacts) = receipt
        .and_then(|value| value.get("artifacts"))
        .and_then(Value::as_array)
    {
        for artifact in artifacts {
            if let Some(path) = artifact.get("path").and_then(|value| value.as_str()) {
                references.push(path.to_string());
            }
            if let Some(uri) = artifact.get("uri").and_then(|value| value.as_str()) {
                references.push(uri.to_string());
            }
        }
    }
    references.sort();
    references.dedup();
    let full_root = typed_root(&references)?;
    let truncated = references.len().saturating_sub(MAX_RAW_REFERENCES);
    let bounded = references
        .iter()
        .take(MAX_RAW_REFERENCES)
        .enumerate()
        .map(|(index, reference)| {
            bounded_text(
                &format!("audit.raw_references[{index}]"),
                reference,
                MAX_REFERENCE_BYTES,
                truncations,
            )
        })
        .collect();
    Ok((bounded, full_root, truncated))
}

fn push_missing(unknowns: &mut Vec<MissingDecisionFact>, field: &str, reason: &str) {
    unknowns.push(MissingDecisionFact {
        field: field.to_string(),
        reason: reason.to_string(),
    });
}

fn known_facts(
    change: &DecisionChange,
    basis: &DecisionBasis,
    impact: &DecisionImpact,
    authority: &DecisionAuthority,
    audit: &AuditFacts,
    facets: &DecisionFacets,
) -> Result<Vec<KnownDecisionFact>, String> {
    let mut facts = vec![
        root_value_fact("event_log_root", &change.fixed_base.event_log_root),
        root_value_fact("proposal_root", &audit.proposal_root),
        root_value_fact("semantic_effect", &audit.semantic_effect_root),
        root_value_fact(
            "verifier_snapshot_root",
            &basis.check_state.durable_verifier_snapshot_root,
        ),
        root_value_fact("engine_verdict", &audit.engine_gate_root),
        root_fact("acceptance_scope", &authority.scope)?,
        root_fact("impact.downstream_effect", &impact.downstream_effect)?,
        root_value_fact("policy_input_root", &audit.policy_input_root),
        root_value_fact("policy_result_root", &audit.policy_result_root),
        root_value_fact("raw_references_root", &audit.raw_references_root),
        root_value_fact("missing_facts_root", &audit.missing_root),
    ];
    if let Some(root) = &audit.receipt_root {
        facts.push(root_value_fact("receipt_root", root));
    }
    if let Some(root) = &audit.artifact_root {
        facts.push(root_value_fact("artifact_set_root", root));
    }
    if let Some(root) = &audit.declared_receipt_root {
        facts.push(root_value_fact("declared_receipt_root", root));
    }
    if let Some(root) = &audit.publication_root {
        facts.push(root_value_fact("audit.publication_root", root));
    }
    for (name, facet) in facets {
        facts.push(root_value_fact(&format!("facet.{name}"), &facet.full_root));
    }
    facts.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(facts)
}

fn root_value_fact(name: &str, value: &str) -> KnownDecisionFact {
    KnownDecisionFact {
        name: name.to_string(),
        value: DecisionFactValue::Root(value.to_string()),
    }
}

fn root_fact<T: Serialize + ?Sized>(name: &str, value: &T) -> Result<KnownDecisionFact, String> {
    Ok(root_value_fact(name, &typed_root(value)?))
}

fn typed_root<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    vela_protocol::canonical::sha256_canonical(value).map(|root| format!("sha256:{root}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vela_protocol::acceptance_policy::{Decision, Outcome, PolicyContext};
    use vela_protocol::events::StateTarget;
    use vela_protocol::identity::{ActorClass, IdentityBinding};
    use vela_protocol::receipt_v1::{
        ArtifactInput, ProducerReportedRun, ReceiptBuilder, ReceiptInput,
    };
    use vela_protocol::test_support::{make_finding, make_project};

    use super::*;

    struct Fixture {
        project: Project,
        receipt: ReceiptV1,
        proposal_id: String,
        policy_context: PolicyContext,
        policy_decision: Decision,
        engine_gate: EngineVerdict,
        receipt_root: String,
        event_log_root: String,
    }

    fn identity(actor: &str) -> IdentityBinding {
        // Frozen, public origin-binding fixture. DecisionBrief itself has no
        // key input and never reads or creates key material.
        assert_eq!(actor, "agent:decision-brief-test");
        IdentityBinding {
            schema: "vela.identity_binding.v0.1".to_string(),
            binding_id: "vib_7067542ae284b71a".to_string(),
            actor_id: actor.to_string(),
            actor_class: ActorClass::Agent,
            public_key_hex: "fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618"
                .to_string(),
            created_at: "2026-07-13T12:00:00Z".to_string(),
            signature: "cb5dda1a80e38de6b023f1ddc9346d77dc112d1fa38c61512b10057822432908a076bd08509e965b927dd6a0d04f83e9f952a78cf5a5b762bacc574b06bf2b05"
                .to_string(),
        }
    }

    fn fixture() -> Fixture {
        let actor = "agent:decision-brief-test";
        let claim = "The bounded witness has the declared checksum.";
        let mut project = make_project("Decision brief frontier", vec![]);
        let event_log_root = format!(
            "sha256:{}",
            vela_protocol::events::event_log_hash(&project.events)
        );
        let receipt = ReceiptBuilder::build(
            ReceiptInput::new(
                claim.to_string(),
                "computational".to_string(),
                "exact".to_string(),
                vec![
                    ArtifactInput::new(
                        "witnesses/result.json".to_string(),
                        "witness".to_string(),
                        Some("a".repeat(64)),
                        Some("https://example.test/result.json".to_string()),
                    )
                    .unwrap(),
                ],
                vec!["This establishes only the bounded case.".to_string()],
                vec![
                    ProducerReportedRun::producer_reported(
                        "producer.check".to_string(),
                        "pass".to_string(),
                    )
                    .unwrap(),
                ],
                actor.to_string(),
                "2026-07-13T12:34:56Z".to_string(),
                event_log_root.clone(),
                ".".to_string(),
                format!("vop_{}", "b".repeat(64)),
                "urn:vela:policy:none".to_string(),
            )
            .unwrap(),
            &identity(actor),
        )
        .unwrap();
        let receipt_root = receipt.canonical_root().unwrap();
        let finding = make_finding("vf_decision_brief", 0.3, "computational");
        let mut finding_value = serde_json::to_value(&finding).unwrap();
        finding_value["assertion"]["text"] = json!(claim);
        let proposal = proposals::new_proposal_at(
            "finding.add",
            StateTarget {
                r#type: "finding".to_string(),
                id: finding.id,
            },
            actor,
            "agent",
            "producer interpretation pending review",
            json!({
                "finding": finding_value,
                "vela_submission": {
                    "schema": "vela.submission-links.internal.v1",
                    "receipt_root": receipt_root,
                    "receipt_path": "records/receipts/sha256/receipt.json",
                    "record_id": "vrc_decisionbrief",
                    "operation_id": format!("vop_{}", "b".repeat(64)),
                }
            }),
            vec!["urn:source:z".to_string(), "urn:source:a".to_string()],
            vec!["This establishes only the bounded case.".to_string()],
            "2026-07-13T12:35:00Z",
        );
        let proposal_id = proposal.id.clone();
        project.proposals.push(proposal);
        Fixture {
            project,
            receipt,
            proposal_id,
            policy_context: PolicyContext {
                claim_class: "receipt_computational".to_string(),
                assurance_level: 0,
                impact_tier: 1,
                changed_findings: 1,
                downstream_dependents: 0,
                assertion_text_mutated: true,
                target_contested: false,
                governance_mutation: false,
                independence_satisfied: false,
                method_integrity_sound: false,
                credential_valid: true,
                has_unknown_fields: false,
                replayability: "exact".to_string(),
            },
            policy_decision: Decision {
                outcome: Outcome::Defer,
                matched_rule_ids: vec![],
                reasons: vec!["human_scientific_judgment".to_string()],
                evaluator: "acceptance-policy-test".to_string(),
                policy_id: "vap_test".to_string(),
            },
            engine_gate: EngineVerdict {
                status: "pass".to_string(),
                new_blocking: Vec::new(),
                new_warnings: Vec::new(),
                forced: false,
                strict: true,
                release_blocking_failed: 0,
                warnings: 0,
            },
            receipt_root,
            event_log_root,
        }
    }

    fn test_route(fixture: &Fixture) -> ReviewRoute<'_> {
        ReviewRoute::test(
            &fixture.policy_context,
            Some(&fixture.policy_decision),
            Some(&fixture.engine_gate),
            "active",
            None,
        )
    }

    fn test_input<'a>(
        fixture: &'a Fixture,
        receipt: ReceiptMaterial<'a>,
        route: ReviewRoute<'a>,
    ) -> DecisionBriefInput<'a> {
        DecisionBriefInput {
            proposal_id: &fixture.proposal_id,
            receipt,
            route,
            observed_at: "2026-07-13T12:36:00Z",
            replay_ok: true,
            publication: None,
        }
    }

    fn decision(outcome: Outcome, reason: &str) -> Decision {
        Decision {
            outcome,
            matched_rule_ids: vec!["rule:test".to_string()],
            reasons: vec![reason.to_string()],
            evaluator: "acceptance-policy-test".to_string(),
            policy_id: "vap_test".to_string(),
        }
    }

    fn refresh_missing_audit(facts: &mut DecisionFacts) {
        facts.unknowns.sort();
        facts.unknowns.dedup();
        let missing_root = typed_root(&facts.unknowns).unwrap();
        facts.audit.missing_root.clone_from(&missing_root);
        facts.audit.missing_truncated = facts.unknowns.len().saturating_sub(MAX_MISSING_FACTS);
        let known = facts
            .known
            .iter_mut()
            .find(|fact| fact.name == "missing_facts_root")
            .unwrap();
        known.value = DecisionFactValue::Root(missing_root);
    }

    #[test]
    fn decision_brief_is_deterministic_complete_and_read_only() {
        let fixture = fixture();
        let before = vela_protocol::canonical::to_canonical_bytes(&fixture.project).unwrap();
        let publication_root = format!("sha256:{}", "e".repeat(64));
        let input = DecisionBriefInput {
            proposal_id: &fixture.proposal_id,
            receipt: ReceiptMaterial::from_receipt(&fixture.receipt),
            route: test_route(&fixture),
            observed_at: "2026-07-13T12:36:00Z",
            replay_ok: true,
            publication: Some(PublicationProjection {
                root: &publication_root,
                state: "committed_local",
            }),
        };
        let first = build_decision_brief(&fixture.project, input).unwrap();
        let second = build_decision_brief(&fixture.project, input).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.schema, DECISION_BRIEF_SCHEMA);
        assert_eq!(first.stability, DECISION_BRIEF_STABILITY);
        assert_eq!(first.change.subject.id, "vf_decision_brief");
        assert_eq!(
            first.change.fixed_base.event_log_root,
            fixture.event_log_root
        );
        assert!(first.change.before.is_none());
        assert_eq!(
            first.change.after.as_ref().unwrap().text,
            "The bounded witness has the declared checksum."
        );
        assert_eq!(first.change.requested_action, "finding.add");

        assert_eq!(first.basis.primary_evidence_roots.len(), 3);
        assert_eq!(first.basis.check_state.gate_status, "needs_verification");
        assert_eq!(first.basis.check_state.durable_verifier_count, 0);
        assert_eq!(first.basis.check_state.producer_reported.len(), 1);
        assert_eq!(
            first.basis.check_state.producer_reported[0].authority,
            "producer"
        );
        assert_eq!(
            first.basis.main_caveat,
            Some("This establishes only the bounded case.".to_string())
        );
        assert_eq!(
            first
                .basis
                .attributed_interpretation
                .as_ref()
                .unwrap()
                .actor,
            "agent:decision-brief-test"
        );

        assert_eq!(first.impact.downstream_effect.changed_findings, 1);
        assert_eq!(first.impact.downstream_effect.downstream_dependents, 0);
        assert!(first.impact.critical_warnings.is_empty());
        assert_eq!(
            first.impact.correction_path.after_acceptance,
            ["finding.retract", "finding.supersede"]
        );
        assert_eq!(first.authority.route, "defer");
        assert_eq!(first.authority.scope, "hypothesis_only");
        assert!(first.accept_ready());
        assert!(first.reject_ready());
        assert!(first.action("skip").is_none());
        assert_eq!(first.authority.actions[1].action, "reject");
        assert_eq!(first.audit.receipt_root, Some(fixture.receipt_root.clone()));
        assert_eq!(
            first.audit.declared_receipt_root,
            Some(fixture.receipt_root.clone())
        );
        assert_eq!(first.audit.publication_root, Some(publication_root));
        assert!(first.audit.proposal_root.starts_with("sha256:"));
        assert!(
            first
                .audit
                .artifact_root
                .as_ref()
                .is_some_and(|root| root.starts_with("sha256:"))
        );
        assert!(first.audit.policy_input_root.starts_with("sha256:"));
        assert!(first.audit.decision_facts_root.starts_with("sha256:"));
        assert!(first.missing.is_empty());
        assert!(first.facet("work_lease").is_none());
        assert!(first.facet("challenge").is_none());
        assert!(first.facet("acceptance_authority").is_some());
        assert_eq!(
            first.facet("publication").unwrap().data["state"],
            json!("committed_local")
        );
        assert!(first.facets.keys().is_sorted());

        let after = vela_protocol::canonical::to_canonical_bytes(&fixture.project).unwrap();
        assert_eq!(before, after, "projection must not mutate canonical state");
        let serialized = serde_json::to_string(&first).unwrap();
        assert!(!serialized.contains("\"trusted\""));
        assert!(!serialized.contains("\"signed\""));
    }

    #[test]
    fn decision_fact_root_binds_publication_and_missing_facts_are_separate() {
        let mut fixture = fixture();
        let first_publication = format!("sha256:{}", "1".repeat(64));
        let second_publication = format!("sha256:{}", "2".repeat(64));
        let first = build_decision_brief(
            &fixture.project,
            DecisionBriefInput {
                proposal_id: &fixture.proposal_id,
                receipt: ReceiptMaterial::from_receipt(&fixture.receipt),
                route: test_route(&fixture),
                observed_at: "2026-07-13T12:36:00Z",
                replay_ok: true,
                publication: Some(PublicationProjection {
                    root: &first_publication,
                    state: "committed_local",
                }),
            },
        )
        .unwrap();
        let second = build_decision_brief(
            &fixture.project,
            DecisionBriefInput {
                proposal_id: &fixture.proposal_id,
                receipt: ReceiptMaterial::from_receipt(&fixture.receipt),
                route: test_route(&fixture),
                observed_at: "2026-07-13T12:36:00Z",
                replay_ok: true,
                publication: Some(PublicationProjection {
                    root: &second_publication,
                    state: "committed_local",
                }),
            },
        )
        .unwrap();
        assert_ne!(
            first.audit.decision_facts_root,
            second.audit.decision_facts_root
        );

        fixture.project.frontier_id = None;
        let missing = build_decision_brief(
            &fixture.project,
            DecisionBriefInput {
                proposal_id: &fixture.proposal_id,
                receipt: ReceiptMaterial::from_receipt(&fixture.receipt),
                route: test_route(&fixture),
                observed_at: "2026-07-13T12:36:00Z",
                replay_ok: true,
                publication: None,
            },
        )
        .unwrap();
        assert_eq!(
            missing.missing,
            [MissingDecisionFact {
                field: "authority.frontier.id".to_string(),
                reason: "frontier_id_absent".to_string(),
            }]
        );
        assert!(missing.facet("publication").is_none());
    }

    #[test]
    fn action_readiness_matrix_fails_closed_without_hiding_review_material() {
        let fixture = fixture();

        let missing = build_decision_brief(
            &fixture.project,
            test_input(
                &fixture,
                ReceiptMaterial::missing("receipt_not_found"),
                test_route(&fixture),
            ),
        )
        .unwrap();
        assert!(!missing.accept_ready());
        assert!(missing.reject_ready());
        assert!(missing.action("skip").is_none());
        assert!(
            missing
                .missing
                .iter()
                .any(|fact| fact.field == "basis.receipt")
        );

        // A legacy whole-receipt root remains visible to a human, but it never
        // becomes verifier or policy authority.
        let legacy = build_decision_brief(
            &fixture.project,
            test_input(
                &fixture,
                ReceiptMaterial::test_legacy(&fixture.receipt),
                test_route(&fixture),
            ),
        )
        .unwrap();
        assert!(legacy.accept_ready());
        assert!(
            legacy
                .impact
                .critical_warnings
                .iter()
                .any(|warning| { warning.code == "legacy_unbound_receipt" })
        );

        let mut mismatch_fixture = self::fixture();
        let proposal = &mut mismatch_fixture.project.proposals[0];
        proposal.payload["vela_submission"]["receipt_root"] =
            json!(format!("sha256:{}", "9".repeat(64)));
        proposal.id = proposals::proposal_id(proposal);
        mismatch_fixture.proposal_id.clone_from(&proposal.id);
        let mismatch = build_decision_brief(
            &mismatch_fixture.project,
            test_input(
                &mismatch_fixture,
                ReceiptMaterial::from_receipt(&mismatch_fixture.receipt),
                test_route(&mismatch_fixture),
            ),
        )
        .unwrap();
        assert!(!mismatch.accept_ready());
        assert!(
            mismatch
                .action("accept")
                .unwrap()
                .reasons
                .iter()
                .any(|reason| { reason == "proposal_receipt_root_mismatch" })
        );

        let broken = build_decision_brief(
            &fixture.project,
            test_input(
                &fixture,
                ReceiptMaterial::from_receipt(&fixture.receipt),
                ReviewRoute::unavailable("broken", "policy snapshot could not be verified"),
            ),
        )
        .unwrap();
        assert_eq!(broken.authority.route, "broken");
        assert!(!broken.accept_ready());
        assert!(broken.reject_ready());
        assert!(
            broken
                .missing
                .iter()
                .any(|fact| { fact.field == "authority.policy_context" })
        );

        let deny_decision = decision(Outcome::Deny, "rule denied this landing");
        let deny_route = ReviewRoute::test(
            &fixture.policy_context,
            Some(&deny_decision),
            Some(&fixture.engine_gate),
            "active",
            None,
        );
        let denied = build_decision_brief(
            &fixture.project,
            test_input(
                &fixture,
                ReceiptMaterial::from_receipt(&fixture.receipt),
                deny_route,
            ),
        )
        .unwrap();
        assert_eq!(denied.authority.route, "deny");
        assert!(!denied.accept_ready());
        assert!(denied.reject_ready());

        let permit_decision = decision(Outcome::Permit, "standing policy permits this landing");
        let permit_route = ReviewRoute::test(
            &fixture.policy_context,
            Some(&permit_decision),
            Some(&fixture.engine_gate),
            "active",
            None,
        );
        let pending_permit = build_decision_brief(
            &fixture.project,
            test_input(
                &fixture,
                ReceiptMaterial::from_receipt(&fixture.receipt),
                permit_route,
            ),
        )
        .unwrap();
        assert_eq!(pending_permit.authority.route, "permit_pending");
        assert!(!pending_permit.accept_ready());
        assert!(
            pending_permit
                .action("accept")
                .unwrap()
                .reasons
                .iter()
                .any(|reason| { reason == "pending_permit_invariant" })
        );
    }

    #[test]
    fn human_only_route_needs_no_fabricated_policy_or_engine_facts() {
        let policy_fixture = fixture();
        let bypass = build_decision_brief(
            &policy_fixture.project,
            test_input(
                &policy_fixture,
                ReceiptMaterial::from_receipt(&policy_fixture.receipt),
                ReviewRoute::human_only("manual", "attempted route downgrade"),
            ),
        )
        .unwrap_err();
        assert!(bypass.contains("require a staged policy route"));

        let mut fixture = fixture();
        let proposal = &mut fixture.project.proposals[0];
        proposal.kind = "finding.note".to_string();
        proposal
            .payload
            .as_object_mut()
            .unwrap()
            .remove("vela_submission");
        proposal.id = proposals::proposal_id(proposal);
        fixture.proposal_id.clone_from(&proposal.id);

        let brief = build_decision_brief(
            &fixture.project,
            test_input(
                &fixture,
                ReceiptMaterial::missing("receipt_not_applicable"),
                ReviewRoute::human_only(
                    "proposal_kind_requires_human_review",
                    "this proposal kind is intentionally reviewed by a human",
                ),
            ),
        )
        .unwrap();

        assert_eq!(brief.authority.route, "defer");
        assert!(brief.accept_ready());
        assert!(brief.reject_ready());
        assert!(brief.missing.is_empty());
        assert!(brief.basis.check_state.engine_status.is_none());
        assert!(brief.facet("acceptance_authority").is_none());
        assert_eq!(
            brief.authority.why_human,
            ["this proposal kind is intentionally reviewed by a human"]
        );
    }

    #[test]
    fn seventeenth_missing_fact_changes_root_but_not_render_bound() {
        let fixture = fixture();
        let mut facts = DecisionFacts::build(
            &fixture.project,
            test_input(
                &fixture,
                ReceiptMaterial::from_receipt(&fixture.receipt),
                test_route(&fixture),
            ),
        )
        .unwrap();
        for index in 0..MAX_MISSING_FACTS {
            push_missing(
                &mut facts.unknowns,
                &format!("missing.{index:02}"),
                "adversarial_fixture",
            );
        }
        refresh_missing_audit(&mut facts);
        let root_at_render_limit = facts.decision_facts_root().unwrap();

        push_missing(
            &mut facts.unknowns,
            "missing.16",
            "must_remain_in_full_fact_set",
        );
        refresh_missing_audit(&mut facts);
        let root_with_seventeenth = facts.decision_facts_root().unwrap();
        assert_ne!(root_at_render_limit, root_with_seventeenth);

        let brief = facts.into_brief().unwrap();
        assert_eq!(brief.missing.len(), MAX_MISSING_FACTS);
        assert_eq!(brief.audit.missing_truncated, 1);
        assert!(!brief.missing.iter().any(|fact| fact.field == "missing.16"));
    }

    #[test]
    fn rendering_limits_do_not_change_decision_facts_root() {
        let fixture = fixture();
        let mut facts = DecisionFacts::build(
            &fixture.project,
            test_input(
                &fixture,
                ReceiptMaterial::from_receipt(&fixture.receipt),
                test_route(&fixture),
            ),
        )
        .unwrap();
        let full_fact_root = facts.decision_facts_root().unwrap();

        facts.change.claim = "different bounded rendering".to_string();
        facts.change.after.as_mut().unwrap().text = "renderer-only text".to_string();
        facts.basis.check_state.engine_status = Some("renderer-only status".to_string());
        facts.basis.check_state.engine_new_blocking = vec!["renderer-only blocker".to_string()];
        facts.basis.check_state.engine_new_warnings = vec!["renderer-only warning".to_string()];
        facts.audit.raw_references.clear();
        facts.audit.truncations.clear();
        for facet in facts.facets.values_mut() {
            facet.data = json!({"truncated": true});
            facet.truncated = true;
        }

        assert_eq!(full_fact_root, facts.decision_facts_root().unwrap());
    }

    #[test]
    fn review_sort_key_uses_canonical_utc() {
        let mut fixture = fixture();
        fixture.project.proposals[0].created_at = "2026-07-13T14:35:00+02:00".to_string();
        let snapshot = build_review_snapshot(
            &fixture.project,
            test_input(
                &fixture,
                ReceiptMaterial::from_receipt(&fixture.receipt),
                test_route(&fixture),
            ),
        )
        .unwrap();
        assert_eq!(
            snapshot.sort_key.created_at,
            "2026-07-13T12:35:00.000000000Z"
        );
    }
}
