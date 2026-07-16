//! Pure, read-only inspection of one fully named human decision.
//!
//! This is deliberately narrower than a frontier gate and narrower than a
//! dependency verdict. It proves that one exact review event is
//! content-addressed, signed by the one authorized historical reviewer, linked
//! to one applied proposal, and (when supplied) bound to the canonical private
//! DecisionPlan preimage committed by the event. It never
//! reads a key, path, socket, clock, registry, or hosted service.

use std::collections::BTreeSet;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::events::{self, StateEvent};
use crate::project::Project;

const INSPECTION_SCHEMA: &str = "vela.named-decision-inspection.v0.1";
const DECISION_PREIMAGE_VERSION: &str = "vela.decision-plan.internal.v1";
const DECISION_PLAN_DOMAIN: &[u8] = b"vela.decision-plan.internal.v1\0";
const REVIEWER_AUTHORITY_DOMAIN: &[u8] = b"vela.reviewer-authority.internal.v1\0";
const MAX_PREIMAGE_BYTES: usize = 1024 * 1024;
const MAX_ANSWERS: usize = 64;
const MAX_EVENT_CORES: usize = 512;

/// Stable result for inspecting one exact decision. `verified` means only that
/// the named decision evidence is internally bound; it is not a scientific or
/// dependency verdict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionInspection {
    pub schema: String,
    pub ok: bool,
    pub status: String,
    pub code: String,
    pub detail: String,
    pub decision_event_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_event_content_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_event_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_event_log_root: Option<String>,
}

impl DecisionInspection {
    fn rejected(decision_event_id: &str, code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            schema: INSPECTION_SCHEMA.to_string(),
            ok: false,
            status: "rejected".to_string(),
            code: format!("rejected:{code}"),
            detail: detail.into(),
            decision_event_id: decision_event_id.to_string(),
            decision_event_content_root: None,
            decision_root: None,
            proposal_id: None,
            applied_event_id: None,
            authority_id: None,
            expected_event_log_root: None,
        }
    }

    fn with_context(mut self, context: &DecisionContext) -> Self {
        self.decision_event_content_root = Some(context.content_root.clone());
        self.decision_root = Some(context.decision_root.clone());
        self.proposal_id = Some(context.proposal_id.clone());
        self.applied_event_id = Some(context.applied_event_id.clone());
        self.authority_id = Some(context.authority_id.clone());
        self
    }
}

#[derive(Debug)]
struct DecisionContext {
    content_root: String,
    decision_root: String,
    proposal_id: String,
    applied_event_id: String,
    authority_id: String,
    decision_index: usize,
    applied_index: usize,
}

type DecisionEventFailure = (&'static str, String);

fn decision_event_failure(
    _decision_event_id: &str,
    code: &'static str,
    detail: impl Into<String>,
) -> DecisionEventFailure {
    (code, detail.into())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct DecisionPlanPreimage {
    decision_preimage_version: String,
    frontier_id: String,
    expected_event_log_root: String,
    ordered_answers: Vec<DecisionAnswer>,
    consumed_fact_roots: Vec<ConsumedDecisionRoots>,
    policy_input_root: String,
    semantic_event_cores: Vec<UnsignedEventCore>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct DecisionAnswer {
    proposal_id: String,
    proposal_root: String,
    action: DecisionAction,
    reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DecisionAction {
    Accept,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ConsumedDecisionRoots {
    proposal_id: String,
    proposal_root: String,
    receipt_observation_root: String,
    receipt_root: Option<String>,
    evidence_or_reference_root: String,
    evidence_availability: String,
    verifier_snapshot_root: String,
    policy_input_root: String,
    policy_result_root: String,
    engine_gate_root: String,
    reviewer_authority_root: String,
    semantic_effect_root: String,
    downstream_impact_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct UnsignedEventCore {
    answer_ordinal: usize,
    event_ordinal: usize,
    event: Value,
}

#[derive(Serialize)]
struct ReviewerAuthorityCommitment<'a> {
    schema: &'static str,
    frontier_id: &'a str,
    reviewer: &'a crate::sign::ActorRecord,
    decided_at: &'a str,
    authorization: &'static str,
}

/// Inspect exactly one full event id and full event-content root.
///
/// The supplied `decision_preimage` must be the canonical JSON bytes of the
/// seven-field internal DecisionPlan preimage. Absence is not invalid evidence:
/// it is the specific historical representability gap this read path exposes.
#[must_use]
pub fn inspect_named_decision(
    project: &Project,
    decision_event_id: &str,
    decision_event_content_root: &str,
    decision_preimage: Option<&[u8]>,
) -> DecisionInspection {
    let context =
        match inspect_decision_event(project, decision_event_id, decision_event_content_root) {
            Ok(context) => context,
            Err((code, detail)) => {
                return DecisionInspection::rejected(decision_event_id, code, detail);
            }
        };

    let Some(preimage_bytes) = decision_preimage else {
        return DecisionInspection {
            schema: INSPECTION_SCHEMA.to_string(),
            ok: false,
            status: "unresolvable".to_string(),
            code: "unresolvable:decision_preimage_unavailable".to_string(),
            detail: "the signed decision retains its DecisionPlan root but not the canonical preimage needed to inspect the facts it consumed".to_string(),
            decision_event_id: decision_event_id.to_string(),
            decision_event_content_root: Some(context.content_root),
            decision_root: Some(context.decision_root),
            proposal_id: Some(context.proposal_id),
            applied_event_id: Some(context.applied_event_id),
            authority_id: Some(context.authority_id),
            expected_event_log_root: None,
        };
    };

    match inspect_preimage(project, decision_event_id, &context, preimage_bytes) {
        Ok(expected_event_log_root) => DecisionInspection {
            schema: INSPECTION_SCHEMA.to_string(),
            ok: true,
            status: "verified".to_string(),
            code: "verified:decision_evidence_bound".to_string(),
            detail: "the exact signed human decision, proposal linkage, historical authority, base event-log root, and retained DecisionPlan preimage agree".to_string(),
            decision_event_id: decision_event_id.to_string(),
            decision_event_content_root: Some(context.content_root),
            decision_root: Some(context.decision_root),
            proposal_id: Some(context.proposal_id),
            applied_event_id: Some(context.applied_event_id),
            authority_id: Some(context.authority_id),
            expected_event_log_root: Some(expected_event_log_root),
        },
        Err((code, detail)) => {
            DecisionInspection::rejected(decision_event_id, code, detail).with_context(&context)
        }
    }
}

fn inspect_decision_event(
    project: &Project,
    decision_event_id: &str,
    asserted_content_root: &str,
) -> Result<DecisionContext, DecisionEventFailure> {
    if !is_event_id(decision_event_id) {
        return Err(decision_event_failure(
            decision_event_id,
            "decision_event_not_unique",
            "decision_event_id must be one complete vev_ content address",
        ));
    }
    if !is_sha256_root(asserted_content_root) {
        return Err(decision_event_failure(
            decision_event_id,
            "decision_content_root_mismatch",
            "decision_event_content_root must be sha256:<64 lowercase hex>",
        ));
    }
    let matches = project
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| event.id == decision_event_id)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(decision_event_failure(
            decision_event_id,
            "decision_event_not_unique",
            format!("expected one exact event; found {}", matches.len()),
        ));
    }
    let (decision_index, event) = matches[0];
    if events::compute_event_id(event) != event.id {
        return Err(decision_event_failure(
            decision_event_id,
            "decision_event_id_mismatch",
            "event id does not rederive from its canonical content",
        ));
    }
    let content_root = event_content_root(event);
    if content_root != asserted_content_root {
        return Err(decision_event_failure(
            decision_event_id,
            "decision_content_root_mismatch",
            format!("asserted {asserted_content_root}; derived {content_root}"),
        ));
    }
    if event.kind.as_str() != events::EVENT_KIND_REVIEW_ACCEPTED
        || event.actor.r#type != "human"
        || crate::events::actor_kind(&event.actor.id) != "human"
    {
        return Err(decision_event_failure(
            decision_event_id,
            "decision_actor_invalid",
            "named decision must be one review.accepted event attributed to a human actor",
        ));
    }
    let actor = match super::validate_human_reviewer_authority_at(
        project,
        &event.actor.id,
        &event.timestamp,
    ) {
        Ok(actor) => actor,
        Err(error) => {
            return Err(decision_event_failure(
                decision_event_id,
                "reviewer_unauthorized",
                error,
            ));
        }
    };
    match crate::sign::verify_event_signature(event, &actor.public_key) {
        Ok(true) => {}
        Ok(false) => {
            return Err(decision_event_failure(
                decision_event_id,
                "decision_signature_invalid",
                "decision signature does not verify under the historical reviewer key",
            ));
        }
        Err(error) => {
            return Err(decision_event_failure(
                decision_event_id,
                "decision_signature_invalid",
                error,
            ));
        }
    }

    let payload = match event.payload.as_object() {
        Some(payload) => payload,
        None => {
            return Err(decision_event_failure(
                decision_event_id,
                "proposal_link_mismatch",
                "review decision payload is not an object",
            ));
        }
    };
    if payload.get("verdict").and_then(Value::as_str) != Some("accepted") {
        return Err(decision_event_failure(
            decision_event_id,
            "proposal_link_mismatch",
            "review.accepted payload does not carry verdict=accepted",
        ));
    }
    let proposal_id = required_string(payload.get("proposal_id"));
    let proposal_kind = required_string(payload.get("proposal_kind"));
    let applied_event_id = required_string(payload.get("applied_event_id"));
    let (proposal_id, proposal_kind, applied_event_id) =
        match (proposal_id, proposal_kind, applied_event_id) {
            (Some(proposal_id), Some(proposal_kind), Some(applied_event_id)) => {
                (proposal_id, proposal_kind, applied_event_id)
            }
            _ => {
                return Err(decision_event_failure(
                    decision_event_id,
                    "proposal_link_mismatch",
                    "review decision omits a complete proposal/applied-event link",
                ));
            }
        };
    if event.target.r#type != "proposal" || event.target.id != proposal_id {
        return Err(decision_event_failure(
            decision_event_id,
            "proposal_link_mismatch",
            "review target and payload proposal_id disagree",
        ));
    }
    let proposals = project
        .proposals
        .iter()
        .filter(|proposal| proposal.id == proposal_id)
        .collect::<Vec<_>>();
    if proposals.len() != 1 {
        return Err(decision_event_failure(
            decision_event_id,
            "proposal_link_mismatch",
            format!("expected one linked proposal; found {}", proposals.len()),
        ));
    }
    let proposal = proposals[0];
    if super::proposal_id(proposal) != proposal.id
        || proposal.kind != proposal_kind
        || proposal.status != "applied"
        || proposal.reviewed_by.as_deref() != Some(event.actor.id.as_str())
        || proposal.reviewed_at.as_deref() != Some(event.timestamp.as_str())
        || proposal.decision_reason.as_deref() != Some(event.reason.as_str())
    {
        return Err(decision_event_failure(
            decision_event_id,
            "proposal_link_mismatch",
            "linked proposal does not rederive or match the accepted review metadata",
        ));
    }
    if proposal.applied_event_id.as_deref() != Some(applied_event_id.as_str()) {
        return Err(decision_event_failure(
            decision_event_id,
            "applied_event_link_mismatch",
            "proposal and review decision disagree on the applied event",
        ));
    }

    let applied_matches = project
        .events
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.id == applied_event_id)
        .collect::<Vec<_>>();
    if applied_matches.len() != 1 {
        return Err(decision_event_failure(
            decision_event_id,
            "applied_event_link_mismatch",
            format!(
                "expected one applied event; found {}",
                applied_matches.len()
            ),
        ));
    }
    let (applied_index, applied) = applied_matches[0];
    if events::compute_event_id(applied) != applied.id
        || applied.payload.get("proposal_id").and_then(Value::as_str) != Some(proposal_id.as_str())
        || applied.actor != event.actor
        || applied.timestamp != event.timestamp
        || (applied.id != event.id && applied.target != proposal.target)
    {
        return Err(decision_event_failure(
            decision_event_id,
            "applied_event_link_mismatch",
            "applied event does not rederive or match the proposal, reviewer, timestamp, and target",
        ));
    }

    let decision_roots = match decision_root_refs(event) {
        Ok(roots) => roots,
        Err(error) => {
            return Err(decision_event_failure(
                decision_event_id,
                "decision_root_not_unique",
                error,
            ));
        }
    };
    if decision_roots.len() != 1 {
        return Err(decision_event_failure(
            decision_event_id,
            "decision_root_not_unique",
            format!(
                "expected one canonical decision-root input reference; found {}",
                decision_roots.len()
            ),
        ));
    }

    Ok(DecisionContext {
        content_root,
        decision_root: decision_roots[0].clone(),
        proposal_id,
        applied_event_id,
        authority_id: event.actor.id.clone(),
        decision_index,
        applied_index,
    })
}

fn inspect_preimage(
    project: &Project,
    decision_event_id: &str,
    context: &DecisionContext,
    bytes: &[u8],
) -> Result<String, (&'static str, String)> {
    if bytes.len() > MAX_PREIMAGE_BYTES {
        return Err((
            "decision_preimage_oversized",
            format!(
                "preimage is {} bytes; limit is {MAX_PREIMAGE_BYTES}",
                bytes.len()
            ),
        ));
    }
    let preimage: DecisionPlanPreimage = serde_json::from_slice(bytes).map_err(|error| {
        (
            "decision_preimage_invalid",
            format!("preimage is not strict typed JSON: {error}"),
        )
    })?;
    if preimage.decision_preimage_version != DECISION_PREIMAGE_VERSION {
        return Err((
            "decision_preimage_version",
            format!(
                "unsupported decision preimage version {}",
                preimage.decision_preimage_version
            ),
        ));
    }
    if preimage.ordered_answers.len() != 1
        || preimage.consumed_fact_roots.len() != 1
        || preimage.ordered_answers.len() > MAX_ANSWERS
        || preimage.semantic_event_cores.is_empty()
        || preimage.semantic_event_cores.len() > MAX_EVENT_CORES
    {
        return Err((
            "decision_preimage_scope",
            "named-decision inspection requires exactly one answer, one consumed-root set, and a bounded non-empty event-core set".to_string(),
        ));
    }
    let canonical = crate::canonical::to_canonical_bytes(&preimage).map_err(|error| {
        (
            "decision_preimage_invalid",
            format!("preimage canonicalization failed: {error}"),
        )
    })?;
    if canonical != bytes {
        return Err((
            "decision_preimage_noncanonical",
            "retained preimage bytes must equal their canonical JSON encoding".to_string(),
        ));
    }
    validate_preimage_roots(&preimage)?;
    let mut digest = Sha256::new();
    digest.update(DECISION_PLAN_DOMAIN);
    digest.update(&canonical);
    let derived_root = format!("sha256:{}", hex::encode(digest.finalize()));
    if derived_root != context.decision_root {
        return Err((
            "decision_preimage_root_mismatch",
            format!(
                "signed event binds {}; supplied preimage derives {derived_root}",
                context.decision_root
            ),
        ));
    }
    if preimage.frontier_id != project.frontier_id() {
        return Err((
            "decision_frontier_mismatch",
            "DecisionPlan frontier_id does not match the inspected frontier".to_string(),
        ));
    }

    let answer = &preimage.ordered_answers[0];
    let consumed = &preimage.consumed_fact_roots[0];
    let proposal = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == context.proposal_id)
        .expect("proposal uniqueness was checked before preimage inspection");
    if answer.action != DecisionAction::Accept
        || answer.proposal_id != context.proposal_id
        || consumed.proposal_id != context.proposal_id
        || answer.proposal_root != consumed.proposal_root
        || answer.reason != proposal.decision_reason.as_deref().unwrap_or_default()
    {
        return Err((
            "decision_answer_mismatch",
            "DecisionPlan answer and consumed roots do not match the accepted proposal".to_string(),
        ));
    }
    let mut pending = proposal.clone();
    pending.status = "pending_review".to_string();
    pending.reviewed_by = None;
    pending.reviewed_at = None;
    pending.decision_reason = None;
    pending.applied_event_id = None;
    let proposal_root = crate::canonical::sha256_canonical(&pending)
        .map(|digest| format!("sha256:{digest}"))
        .map_err(|error| ("decision_proposal_root_mismatch", error))?;
    if answer.proposal_root != proposal_root {
        return Err((
            "decision_proposal_root_mismatch",
            format!(
                "DecisionPlan proposal root is {}; reconstructed pending proposal root is {proposal_root}",
                answer.proposal_root
            ),
        ));
    }

    let decision = &project.events[context.decision_index];
    let actor = project
        .actors
        .iter()
        .find(|actor| actor.id == context.authority_id)
        .expect("actor uniqueness was checked before preimage inspection");
    let historical_actor = actor_at_decision(actor, &decision.timestamp)?;
    let authority_root = reviewer_authority_root(
        &project.frontier_id(),
        &historical_actor,
        &decision.timestamp,
    )?;
    if consumed.reviewer_authority_root != authority_root {
        return Err((
            "decision_authority_root_mismatch",
            format!(
                "DecisionPlan authority root is {}; reconstructed root is {authority_root}",
                consumed.reviewer_authority_root
            ),
        ));
    }

    let matched = match_event_cores(project, &preimage.semantic_event_cores)?;
    if !matched.contains(&context.decision_index) || !matched.contains(&context.applied_index) {
        return Err((
            "decision_event_core_mismatch",
            "DecisionPlan semantic cores do not contain the named decision and applied event"
                .to_string(),
        ));
    }
    for index in &matched {
        let event = &project.events[*index];
        if event.actor != decision.actor || event.timestamp != decision.timestamp {
            return Err((
                "decision_event_core_mismatch",
                "DecisionPlan event set does not share one reviewer and fixed decision timestamp"
                    .to_string(),
            ));
        }
    }

    let decision_time = parse_time(&decision.timestamp, "decision timestamp")?;
    let mut historical_events = Vec::new();
    for (index, event) in project.events.iter().enumerate() {
        if matched.contains(&index) {
            continue;
        }
        let timestamp = parse_time(&event.timestamp, "event timestamp")?;
        if timestamp < decision_time {
            historical_events.push(event.clone());
        } else if timestamp == decision_time {
            return Err((
                "decision_historical_head_ambiguous",
                format!(
                    "unmatched event {} shares the fixed decision timestamp",
                    event.id
                ),
            ));
        }
    }
    let historical_root = format!("sha256:{}", events::event_log_hash(&historical_events));
    if preimage.expected_event_log_root != historical_root {
        return Err((
            "decision_event_log_root_mismatch",
            format!(
                "DecisionPlan expected {}; reconstructed historical head is {historical_root}",
                preimage.expected_event_log_root
            ),
        ));
    }

    let _ = decision_event_id;
    Ok(preimage.expected_event_log_root)
}

fn match_event_cores(
    project: &Project,
    cores: &[UnsignedEventCore],
) -> Result<BTreeSet<usize>, (&'static str, String)> {
    let mut matched = BTreeSet::new();
    for (ordinal, core) in cores.iter().enumerate() {
        if core.answer_ordinal != 0 || core.event_ordinal != ordinal {
            return Err((
                "decision_event_core_mismatch",
                "semantic event cores must be the contiguous ordered set for answer zero"
                    .to_string(),
            ));
        }
        let candidates = project
            .events
            .iter()
            .enumerate()
            .filter(|(index, event)| {
                !matched.contains(index) && normalize_event_core(event) == core.event
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err((
                "decision_event_core_mismatch",
                format!(
                    "semantic event core {ordinal} matched {} canonical events",
                    candidates.len()
                ),
            ));
        }
        matched.insert(candidates[0]);
    }
    Ok(matched)
}

fn normalize_event_core(event: &StateEvent) -> Value {
    let mut value = serde_json::to_value(event).expect("StateEvent serialization is infallible");
    let object = value
        .as_object_mut()
        .expect("StateEvent serializes as an object");
    object.insert("id".to_string(), Value::String(String::new()));
    object.insert("signature".to_string(), Value::Null);
    if let Some(payload) = object.get_mut("payload").and_then(Value::as_object_mut)
        && let Some(provenance) = payload.get_mut("provenance").and_then(Value::as_object_mut)
    {
        if let Some(input_refs) = provenance
            .get_mut("input_refs")
            .and_then(Value::as_array_mut)
        {
            input_refs.retain(|reference| {
                reference.as_str().is_none_or(|reference| {
                    !reference.starts_with(crate::provenance::DECISION_ROOT_INPUT_REF_PREFIX)
                })
            });
            if input_refs.is_empty() {
                provenance.remove("input_refs");
            }
        }
        if provenance.is_empty() {
            payload.remove("provenance");
        }
    }
    value
}

fn validate_preimage_roots(preimage: &DecisionPlanPreimage) -> Result<(), (&'static str, String)> {
    if !is_sha256_root(&preimage.expected_event_log_root)
        || !is_sha256_root(&preimage.policy_input_root)
    {
        return Err((
            "decision_preimage_invalid",
            "top-level DecisionPlan roots must be sha256:<64 lowercase hex>".to_string(),
        ));
    }
    for answer in &preimage.ordered_answers {
        if !is_sha256_root(&answer.proposal_root)
            || answer.proposal_id.is_empty()
            || answer.reason.is_empty()
            || answer.reason.len() > 64 * 1024
        {
            return Err((
                "decision_preimage_invalid",
                "DecisionPlan answer fields are malformed or oversized".to_string(),
            ));
        }
    }
    for roots in &preimage.consumed_fact_roots {
        let required = [
            &roots.proposal_root,
            &roots.receipt_observation_root,
            &roots.evidence_or_reference_root,
            &roots.verifier_snapshot_root,
            &roots.policy_input_root,
            &roots.policy_result_root,
            &roots.engine_gate_root,
            &roots.reviewer_authority_root,
            &roots.semantic_effect_root,
            &roots.downstream_impact_root,
        ];
        if required.into_iter().any(|root| !is_sha256_root(root))
            || roots
                .receipt_root
                .as_deref()
                .is_some_and(|root| !is_sha256_root(root))
            || roots.evidence_availability.is_empty()
            || roots.evidence_availability.len() > 1024
        {
            return Err((
                "decision_preimage_invalid",
                "DecisionPlan consumed-root fields are malformed or oversized".to_string(),
            ));
        }
    }
    Ok(())
}

fn actor_at_decision(
    actor: &crate::sign::ActorRecord,
    decided_at: &str,
) -> Result<crate::sign::ActorRecord, (&'static str, String)> {
    let decision_time = parse_time(decided_at, "decision timestamp")?;
    let mut historical = actor.clone();
    if let Some(revoked_at) = actor.revoked_at.as_deref() {
        let revoked = parse_time(revoked_at, "reviewer revocation timestamp")?;
        if revoked > decision_time {
            historical.revoked_at = None;
            historical.revoked_reason = None;
        }
    }
    Ok(historical)
}

fn reviewer_authority_root(
    frontier_id: &str,
    actor: &crate::sign::ActorRecord,
    decided_at: &str,
) -> Result<String, (&'static str, String)> {
    let value = ReviewerAuthorityCommitment {
        schema: "vela.reviewer-authority.internal.v1",
        frontier_id,
        reviewer: actor,
        decided_at,
        authorization: "authorized",
    };
    let bytes = crate::canonical::to_canonical_bytes(&value)
        .map_err(|error| ("decision_authority_root_mismatch", error))?;
    let mut digest = Sha256::new();
    digest.update(REVIEWER_AUTHORITY_DOMAIN);
    digest.update(bytes);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn decision_root_refs(event: &StateEvent) -> Result<Vec<String>, String> {
    let Some(provenance) = event.payload.get("provenance") else {
        return Ok(Vec::new());
    };
    let object = provenance
        .as_object()
        .ok_or_else(|| "decision provenance must be an object".to_string())?;
    let Some(input_refs) = object.get("input_refs") else {
        return Ok(Vec::new());
    };
    let input_refs = input_refs
        .as_array()
        .ok_or_else(|| "decision provenance.input_refs must be an array".to_string())?;
    let mut roots = Vec::new();
    for reference in input_refs {
        let reference = reference
            .as_str()
            .ok_or_else(|| "decision provenance.input_refs must contain strings".to_string())?;
        let Some(root) = reference.strip_prefix(crate::provenance::DECISION_ROOT_INPUT_REF_PREFIX)
        else {
            continue;
        };
        let expected = crate::provenance::decision_root_input_ref(root)?;
        if expected != reference {
            return Err("decision root reference is not canonical".to_string());
        }
        roots.push(root.to_string());
    }
    Ok(roots)
}

fn event_content_root(event: &StateEvent) -> String {
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(events::event_content_preimage_bytes(event)))
    )
}

fn is_event_id(value: &str) -> bool {
    value.len() == 20
        && value.starts_with("vev_")
        && value[4..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_sha256_root(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn required_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 4096)
        .map(ToString::to_string)
}

fn parse_time(
    value: &str,
    label: &'static str,
) -> Result<DateTime<FixedOffset>, (&'static str, String)> {
    DateTime::parse_from_rfc3339(value).map_err(|error| {
        (
            "decision_timestamp_invalid",
            format!("{label} is not RFC3339: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use serde_json::json;

    const REVIEWER: &str = "reviewer:decision-inspection-test-fixture";
    const DECIDED_AT: &str = "2099-07-15T12:00:00Z";

    struct Fixture {
        project: Project,
        event_id: String,
        content_root: String,
        preimage: Vec<u8>,
        key: SigningKey,
    }

    #[derive(serde::Deserialize)]
    struct StaticFixtureDocument {
        event_id: String,
        content_root: String,
        preimage: Value,
        project: Project,
        test_key_seed_hex: String,
    }

    struct VectorFixture {
        event_id: String,
        content_root: String,
        preimage: Option<Value>,
        project: Project,
        key: SigningKey,
    }

    #[derive(serde::Deserialize)]
    struct RegisteredVectors {
        cases: Vec<RegisteredVector>,
    }

    #[derive(serde::Deserialize)]
    struct RegisteredVector {
        id: String,
        mutation: String,
        expected: String,
    }

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn build_fixture() -> Fixture {
        let key = SigningKey::from_bytes(&[73_u8; 32]);
        let mut project = crate::project::assemble(
            "decision-inspection",
            vec![crate::proposals::tests::finding("vf_decision")],
            0,
            0,
            "test fixture",
        );
        project.actors.push(crate::sign::ActorRecord {
            id: REVIEWER.to_string(),
            public_key: crate::sign::pubkey_hex(&key),
            algorithm: "ed25519".to_string(),
            created_at: "2020-01-01T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        let proposal = super::super::new_proposal_at(
            "finding.note",
            crate::events::StateTarget {
                r#type: "finding".to_string(),
                id: "vf_decision".to_string(),
            },
            "agent:decision-inspection-test-fixture",
            "agent",
            "record exact decision evidence",
            json!({"text": "bounded fixture note"}),
            Vec::new(),
            Vec::new(),
            "2099-07-15T11:00:00Z",
        );
        let proposal_id = proposal.id.clone();
        super::super::insert_pending_in_frontier(&mut project, proposal).unwrap();
        let pending = project
            .proposals
            .iter()
            .find(|proposal| proposal.id == proposal_id)
            .unwrap();
        let proposal_root = format!(
            "sha256:{}",
            crate::canonical::sha256_canonical(pending).unwrap()
        );
        let expected_event_log_root =
            format!("sha256:{}", crate::events::event_log_hash(&project.events));
        let mut prepared = super::super::prepare_proposal_accept_in_memory_at(
            &mut project,
            &proposal_id,
            REVIEWER,
            "fixture acceptance",
            None,
            DECIDED_AT,
        )
        .unwrap();
        let semantic_event_cores = prepared
            .appended_event_ids()
            .iter()
            .enumerate()
            .map(|(event_ordinal, id)| UnsignedEventCore {
                answer_ordinal: 0,
                event_ordinal,
                event: normalize_event_core(
                    project.events.iter().find(|event| event.id == *id).unwrap(),
                ),
            })
            .collect();
        let authority_root =
            reviewer_authority_root(&project.frontier_id(), &project.actors[0], DECIDED_AT)
                .unwrap();
        let preimage_value = DecisionPlanPreimage {
            decision_preimage_version: DECISION_PREIMAGE_VERSION.to_string(),
            frontier_id: project.frontier_id(),
            expected_event_log_root,
            ordered_answers: vec![DecisionAnswer {
                proposal_id: proposal_id.clone(),
                proposal_root: proposal_root.clone(),
                action: DecisionAction::Accept,
                reason: "fixture acceptance".to_string(),
            }],
            consumed_fact_roots: vec![ConsumedDecisionRoots {
                proposal_id,
                proposal_root,
                receipt_observation_root: root('1'),
                receipt_root: Some(root('2')),
                evidence_or_reference_root: root('3'),
                evidence_availability: "available".to_string(),
                verifier_snapshot_root: root('4'),
                policy_input_root: root('5'),
                policy_result_root: root('6'),
                engine_gate_root: root('7'),
                reviewer_authority_root: authority_root,
                semantic_effect_root: root('8'),
                downstream_impact_root: root('9'),
            }],
            policy_input_root: root('a'),
            semantic_event_cores,
        };
        let preimage = crate::canonical::to_canonical_bytes(&preimage_value).unwrap();
        let mut digest = Sha256::new();
        digest.update(DECISION_PLAN_DOMAIN);
        digest.update(&preimage);
        let decision_root = format!("sha256:{}", hex::encode(digest.finalize()));
        super::super::bind_decision_root_to_prepared(&mut project, &mut prepared, &decision_root)
            .unwrap();
        super::super::sign_prepared_decision_events(&mut project, &prepared, REVIEWER, &key)
            .unwrap();
        let event_id = prepared.decision_event_id().to_string();
        let content_root = event_content_root(
            project
                .events
                .iter()
                .find(|event| event.id == event_id)
                .unwrap(),
        );
        Fixture {
            project,
            event_id,
            content_root,
            preimage,
            key,
        }
    }

    fn static_fixture() -> VectorFixture {
        let document: StaticFixtureDocument = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../research/verifiable-composition/fixtures/decision-inspection/fixture.json"
        )))
        .unwrap();
        let key_bytes: [u8; 32] = hex::decode(document.test_key_seed_hex)
            .unwrap()
            .try_into()
            .unwrap();
        VectorFixture {
            event_id: document.event_id,
            content_root: document.content_root,
            preimage: Some(document.preimage),
            project: document.project,
            key: SigningKey::from_bytes(&key_bytes),
        }
    }

    fn vector_preimage(fixture: &VectorFixture) -> Option<Vec<u8>> {
        fixture
            .preimage
            .as_ref()
            .map(|value| crate::canonical::to_canonical_bytes(value).unwrap())
    }

    fn readdress_decision(fixture: &mut VectorFixture) {
        let index = fixture
            .project
            .events
            .iter()
            .position(|event| event.id == fixture.event_id)
            .unwrap();
        fixture.project.events[index].id =
            crate::events::compute_event_id(&fixture.project.events[index]);
        fixture.project.events[index].signature =
            Some(crate::sign::sign_event(&fixture.project.events[index], &fixture.key).unwrap());
        fixture.event_id = fixture.project.events[index].id.clone();
        fixture.content_root = event_content_root(&fixture.project.events[index]);
    }

    fn mutate_vector(fixture: &mut VectorFixture, mutation: &str) {
        let forged = root('f');
        match mutation {
            "none" => {}
            "remove_preimage" => fixture.preimage = None,
            "decision_event_id" => fixture.event_id = "vev_0000000000000000".to_string(),
            "decision_content_root" => fixture.content_root = forged,
            "event_id_rederivation" => {
                let event = fixture
                    .project
                    .events
                    .iter_mut()
                    .find(|event| event.id == fixture.event_id)
                    .unwrap();
                event.id = "vev_0000000000000000".to_string();
                fixture.event_id = event.id.clone();
            }
            "actor_remove" => fixture.project.actors.clear(),
            "actor_duplicate" => fixture
                .project
                .actors
                .push(fixture.project.actors[0].clone()),
            "actor_agent" => {
                let event = fixture
                    .project
                    .events
                    .iter_mut()
                    .find(|event| event.id == fixture.event_id)
                    .unwrap();
                event.actor.id = "agent:decision-inspection-test".to_string();
                readdress_decision(fixture);
            }
            "actor_namespace" => {
                let id = "scientist:decision-inspection-test".to_string();
                fixture.project.actors[0].id = id.clone();
                fixture
                    .project
                    .events
                    .iter_mut()
                    .find(|event| event.id == fixture.event_id)
                    .unwrap()
                    .actor
                    .id = id;
                readdress_decision(fixture);
            }
            "actor_registered_after" => {
                fixture.project.actors[0].created_at = "2100-01-01T00:00:00Z".to_string();
            }
            "actor_revoked_at" => {
                fixture.project.actors[0].revoked_at = Some(DECIDED_AT.to_string());
                fixture.project.actors[0].revoked_reason = Some("fixed hostile vector".to_string());
            }
            "actor_revoked_after" => {
                fixture.project.actors[0].revoked_at = Some("2100-01-01T00:00:00Z".to_string());
                fixture.project.actors[0].revoked_reason =
                    Some("fixed valid historical vector".to_string());
            }
            "actor_public_key" => fixture.project.actors[0].public_key = "00".repeat(32),
            "decision_signature" => {
                fixture
                    .project
                    .events
                    .iter_mut()
                    .find(|event| event.id == fixture.event_id)
                    .unwrap()
                    .signature = Some(format!("v1:{}", "00".repeat(64)));
            }
            "decision_root_remove" => {
                fixture
                    .project
                    .events
                    .iter_mut()
                    .find(|event| event.id == fixture.event_id)
                    .unwrap()
                    .payload["provenance"]["input_refs"] = json!([]);
                readdress_decision(fixture);
            }
            "decision_root_duplicate" => {
                let event = fixture
                    .project
                    .events
                    .iter_mut()
                    .find(|event| event.id == fixture.event_id)
                    .unwrap();
                let reference = event.payload["provenance"]["input_refs"][0].clone();
                event.payload["provenance"]["input_refs"]
                    .as_array_mut()
                    .unwrap()
                    .push(reference);
                readdress_decision(fixture);
            }
            "proposal_link" => {
                fixture
                    .project
                    .events
                    .iter_mut()
                    .find(|event| event.id == fixture.event_id)
                    .unwrap()
                    .target
                    .id = "vpr_0000000000000000".to_string();
                readdress_decision(fixture);
            }
            "applied_event_link" => {
                fixture
                    .project
                    .events
                    .iter_mut()
                    .find(|event| event.id == fixture.event_id)
                    .unwrap()
                    .payload["applied_event_id"] = json!("vev_0000000000000000");
                readdress_decision(fixture);
            }
            "preimage_version" => {
                fixture.preimage.as_mut().unwrap()["decision_preimage_version"] =
                    json!("vela.decision-plan.internal.v2");
            }
            "preimage_reason" => {
                fixture.preimage.as_mut().unwrap()["ordered_answers"][0]["reason"] =
                    json!("tampered");
            }
            "preimage_event_log_root" => {
                fixture.preimage.as_mut().unwrap()["expected_event_log_root"] = json!(forged);
            }
            "preimage_proposal_root" => {
                fixture.preimage.as_mut().unwrap()["ordered_answers"][0]["proposal_root"] =
                    json!(forged.clone());
                fixture.preimage.as_mut().unwrap()["consumed_fact_roots"][0]["proposal_root"] =
                    json!(forged);
            }
            "preimage_receipt_root" => {
                fixture.preimage.as_mut().unwrap()["consumed_fact_roots"][0]["receipt_root"] =
                    json!(forged);
            }
            "preimage_verifier_root" => {
                fixture.preimage.as_mut().unwrap()["consumed_fact_roots"][0]["verifier_snapshot_root"] =
                    json!(forged);
            }
            "preimage_policy_root" => {
                fixture.preimage.as_mut().unwrap()["consumed_fact_roots"][0]["policy_input_root"] =
                    json!(forged);
            }
            "preimage_authority_root" => {
                fixture.preimage.as_mut().unwrap()["consumed_fact_roots"][0]["reviewer_authority_root"] =
                    json!(forged);
            }
            "preimage_impact_root" => {
                fixture.preimage.as_mut().unwrap()["consumed_fact_roots"][0]["downstream_impact_root"] =
                    json!(forged);
            }
            "post_decision_attachment" => {
                let mut later = fixture.project.events[0].clone();
                later.kind = crate::events::EventKind::from("verifier_attachment.added");
                later.target = crate::events::StateTarget {
                    r#type: "verifier_attachment".to_string(),
                    id: "vva_postdecision000".to_string(),
                };
                later.timestamp = "2100-01-02T00:00:00Z".to_string();
                later.reason = "post-decision verifier attachment".to_string();
                later.payload = json!({"attachment_id": "vva_postdecision000"});
                later.signature = None;
                later.id = crate::events::compute_event_id(&later);
                fixture.project.events.push(later);
            }
            other => panic!("unimplemented registered mutation {other}"),
        }
    }

    fn inspect(fixture: &Fixture, preimage: Option<&[u8]>) -> DecisionInspection {
        inspect_named_decision(
            &fixture.project,
            &fixture.event_id,
            &fixture.content_root,
            preimage,
        )
    }

    #[test]
    fn decision_inspection_verifies_only_the_exact_retained_preimage() {
        let fixture = build_fixture();
        let result = inspect(&fixture, Some(&fixture.preimage));
        assert!(result.ok, "{result:#?}");
        assert_eq!(result.code, "verified:decision_evidence_bound");
    }

    #[test]
    fn decision_inspection_root_only_is_specifically_unresolvable() {
        let fixture = build_fixture();
        let result = inspect(&fixture, None);
        assert!(!result.ok);
        assert_eq!(result.code, "unresolvable:decision_preimage_unavailable");
    }

    #[test]
    fn decision_inspection_rejects_tampered_preimage_and_wrong_named_roots() {
        let fixture = build_fixture();
        let mut value: Value = serde_json::from_slice(&fixture.preimage).unwrap();
        value["consumed_fact_roots"][0]["receipt_root"] = Value::String(root('f'));
        let tampered = crate::canonical::to_canonical_bytes(&value).unwrap();
        assert_eq!(
            inspect(&fixture, Some(&tampered)).code,
            "rejected:decision_preimage_root_mismatch"
        );
        assert_eq!(
            inspect_named_decision(
                &fixture.project,
                &fixture.event_id,
                &root('f'),
                Some(&fixture.preimage),
            )
            .code,
            "rejected:decision_content_root_mismatch"
        );
    }

    #[test]
    fn decision_inspection_rejects_duplicate_and_historically_invalid_actors() {
        let fixture = build_fixture();
        let mut duplicate = fixture.project;
        duplicate.actors.push(duplicate.actors[0].clone());
        assert_eq!(
            inspect_named_decision(
                &duplicate,
                &fixture.event_id,
                &fixture.content_root,
                Some(&fixture.preimage),
            )
            .code,
            "rejected:reviewer_unauthorized"
        );

        let fixture = build_fixture();
        let mut late = fixture.project;
        late.actors[0].created_at = "2100-01-01T00:00:00Z".to_string();
        assert_eq!(
            inspect_named_decision(
                &late,
                &fixture.event_id,
                &fixture.content_root,
                Some(&fixture.preimage),
            )
            .code,
            "rejected:reviewer_unauthorized"
        );
    }

    #[test]
    fn decision_inspection_respects_revocation_time() {
        let fixture = build_fixture();
        let mut after = fixture.project;
        after.actors[0].revoked_at = Some("2100-01-01T00:00:00Z".to_string());
        after.actors[0].revoked_reason = Some("test rotation".to_string());
        let result = inspect_named_decision(
            &after,
            &fixture.event_id,
            &fixture.content_root,
            Some(&fixture.preimage),
        );
        assert!(result.ok, "{result:#?}");

        let fixture = build_fixture();
        let mut at = fixture.project;
        at.actors[0].revoked_at = Some(DECIDED_AT.to_string());
        assert_eq!(
            inspect_named_decision(
                &at,
                &fixture.event_id,
                &fixture.content_root,
                Some(&fixture.preimage),
            )
            .code,
            "rejected:reviewer_unauthorized"
        );
    }

    #[test]
    fn decision_inspection_rejects_signature_and_event_id_tampering() {
        let fixture = build_fixture();
        let mut signature = fixture.project;
        let decision_index = signature
            .events
            .iter()
            .position(|event| event.id == fixture.event_id)
            .unwrap();
        signature.events[decision_index].signature = Some(format!("v1:{}", "0".repeat(128)));
        assert_eq!(
            inspect_named_decision(
                &signature,
                &fixture.event_id,
                &fixture.content_root,
                Some(&fixture.preimage),
            )
            .code,
            "rejected:decision_signature_invalid"
        );

        let fixture = build_fixture();
        let mut event_id = fixture.project;
        let index = event_id
            .events
            .iter()
            .position(|event| event.id == fixture.event_id)
            .unwrap();
        event_id.events[index].id = "vev_0000000000000000".to_string();
        assert_eq!(
            inspect_named_decision(
                &event_id,
                "vev_0000000000000000",
                &fixture.content_root,
                Some(&fixture.preimage),
            )
            .code,
            "rejected:decision_event_id_mismatch"
        );
    }

    #[test]
    fn decision_inspection_rejects_noncanonical_or_oversized_preimages() {
        let fixture = build_fixture();
        let mut whitespace = fixture.preimage.clone();
        whitespace.push(b'\n');
        assert_eq!(
            inspect(&fixture, Some(&whitespace)).code,
            "rejected:decision_preimage_noncanonical"
        );
        let oversized = vec![b' '; MAX_PREIMAGE_BYTES + 1];
        assert_eq!(
            inspect(&fixture, Some(&oversized)).code,
            "rejected:decision_preimage_oversized"
        );
    }

    #[test]
    fn decision_inspection_is_read_only() {
        let fixture = build_fixture();
        let before = crate::canonical::to_canonical_bytes(&fixture.project).unwrap();
        let _ = inspect(&fixture, Some(&fixture.preimage));
        let after = crate::canonical::to_canonical_bytes(&fixture.project).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn decision_inspection_test_fixture_uses_only_the_fixed_test_key() {
        let fixture = build_fixture();
        let decision = fixture
            .project
            .events
            .iter()
            .find(|event| event.id == fixture.event_id)
            .unwrap();
        assert_eq!(
            fixture.project.actors[0].public_key,
            crate::sign::pubkey_hex(&fixture.key)
        );
        assert!(
            crate::sign::verify_event_signature(decision, &fixture.project.actors[0].public_key)
                .unwrap()
        );
    }

    #[test]
    fn decision_inspection_registered_vectors_match_python_classifications() {
        let vectors: RegisteredVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../research/verifiable-composition/vectors/decision-evidence-cases.json"
        )))
        .unwrap();
        assert_eq!(vectors.cases.len(), 28);
        for vector in vectors.cases {
            let mut fixture = static_fixture();
            mutate_vector(&mut fixture, &vector.mutation);
            let preimage = vector_preimage(&fixture);
            let result = inspect_named_decision(
                &fixture.project,
                &fixture.event_id,
                &fixture.content_root,
                preimage.as_deref(),
            );
            assert_eq!(
                result.code, vector.expected,
                "registered vector {} diverged",
                vector.id
            );
        }
    }
}
