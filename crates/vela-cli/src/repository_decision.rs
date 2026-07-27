//! Repository-authority human review decisions.
//!
//! This is the Era-1 replacement for personal Vela event signing. In the solo
//! local profile, invoking the exact proposal/action/reason command is the
//! semantic approval, the operating-system account supplies authentication,
//! and the repository authority signs the covering transaction. Vela never
//! reads a human scientific key.

use std::fs;
use std::path::Path;

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use vela_authority::CedarEvaluationInput;
use vela_authority::runtime_authentication::{AuthenticationRequest, RuntimeSessionState};
use vela_protocol::authority::{PrincipalSnapshotV1, SemanticApprovalV1};
use vela_protocol::canonical::to_canonical_bytes;
use vela_protocol::events::{EventKind, NULL_HASH, StateActor, StateEvent, StateTarget};
use vela_protocol::principal_capability::PrincipalClass;
use vela_protocol::proposals::StateProposal;

use crate::authority_transaction::{
    AuthorityDerivedDraft, AuthorityEventDraft, AuthorityObjectDraft, AuthorityTransactionRequest,
    AuthorityTransactionResult, execute_authority_transaction,
};
use crate::decision_plan::{DecisionAction, SavedAnswer, decision_read_set};
use crate::frontier_txn::{FrontierTxn, PlannedWrite, WriteClass};
use crate::repository_authority_provider::SshAgentRepositoryAuthoritySigner;
use crate::review_material::ReviewProjection;

const PLAN_SCHEMA: &str = "vela.repository-review-decision.v1";
const PLAN_DOMAIN: &[u8] = b"vela.repository-review-decision.v1\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepositoryReviewDecisionPlan {
    pub(crate) schema: String,
    pub(crate) frontier_id: String,
    pub(crate) frontier_name: String,
    pub(crate) proposal_id: String,
    pub(crate) proposal_root: String,
    pub(crate) decision_facts_root: String,
    pub(crate) decision_bindings_root: String,
    pub(crate) action: String,
    pub(crate) reason: String,
    pub(crate) principal_id: String,
    pub(crate) observed_at: String,
    pub(crate) authority_event_log_root: String,
    pub(crate) policy_bundle_root: String,
    pub(crate) plan_root: String,
}

#[derive(Debug)]
pub(crate) struct PreparedRepositoryReviewDecision {
    pub(crate) plan: RepositoryReviewDecisionPlan,
    pub(crate) review: vela_edge::decision_brief::ReviewSnapshot,
    project: vela_protocol::project::Project,
    authority: crate::cli::LoadedRepositoryAuthority,
    proposal: StateProposal,
}

pub(crate) fn is_repository_authority_frontier(frontier: &Path) -> Result<bool, String> {
    let project = vela_protocol::repo::load_from_path(frontier)?;
    Ok(crate::cli::load_repository_authority(frontier, &project)?.is_some())
}

pub(crate) fn prepare_reject(
    frontier: &Path,
    proposal_id: &str,
    reason: &str,
    observed_at: &str,
) -> Result<PreparedRepositoryReviewDecision, String> {
    prepare_decision(
        frontier,
        proposal_id,
        DecisionAction::Reject,
        reason,
        observed_at,
    )
}

pub(crate) fn prepare_accept(
    frontier: &Path,
    proposal_id: &str,
    reason: &str,
    observed_at: &str,
) -> Result<PreparedRepositoryReviewDecision, String> {
    prepare_decision(
        frontier,
        proposal_id,
        DecisionAction::Accept,
        reason,
        observed_at,
    )
}

fn prepare_decision(
    frontier: &Path,
    proposal_id: &str,
    action: DecisionAction,
    reason: &str,
    observed_at: &str,
) -> Result<PreparedRepositoryReviewDecision, String> {
    if reason.trim().is_empty() {
        return Err("repository-authority review reason must not be empty".into());
    }
    DateTime::parse_from_rfc3339(observed_at)
        .map_err(|error| format!("repository review observation time is invalid: {error}"))?;
    let project = vela_protocol::repo::load_from_path(frontier)?;
    let authority = crate::cli::load_repository_authority(frontier, &project)?
        .ok_or_else(|| "frontier has no verified repository-authority history".to_string())?;
    if authority.history.authority_events.iter().any(|event| {
        event.content.target.r#type == "proposal"
            && event.content.target.id == proposal_id
            && matches!(
                event.content.kind,
                EventKind::ReviewAccepted
                    | EventKind::ReviewRejected
                    | EventKind::ReviewRevisionRequested
            )
    }) {
        return Err(format!(
            "proposal {proposal_id} already has a repository-authority review decision"
        ));
    }
    let proposal = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .cloned()
        .ok_or_else(|| format!("proposal {proposal_id} was not found"))?;
    if proposal.status != "pending_review" || proposal.applied_event_id.is_some() {
        return Err(format!(
            "proposal {proposal_id} is no longer pending and unapplied"
        ));
    }
    let review = ReviewProjection::one_at(frontier, proposal_id, observed_at)
        .map_err(|error| error.to_string())?;
    let action_ready = match action {
        DecisionAction::Accept => review.brief.accept_ready(),
        DecisionAction::Reject => review.brief.reject_ready(),
    };
    if !action_ready {
        let reason = review
            .brief
            .action(action.as_str())
            .map(|action| action.reasons.join("; "))
            .filter(|unavailable| !unavailable.is_empty())
            .unwrap_or_else(|| format!("{} is unavailable for this proposal", action.as_str()));
        return Err(reason);
    }
    let local = crate::cli::local_session(observed_at)?;
    if action == DecisionAction::Accept {
        let (candidate, _) =
            vela_protocol::proposals::prepare_repository_authority_accept_candidate_at(
                &project,
                proposal_id,
                &local.principal_id,
                reason,
                None,
                observed_at,
            )?;
        let aggregate_engine = vela_protocol::proposals::strict_engine_verdict_for_candidate(
            &project,
            &candidate,
            frontier,
            std::slice::from_ref(&proposal.kind),
        );
        if aggregate_engine.status == "blocked" {
            return Err(format!(
                "strict aggregate Engine gate found {} new blocking failure(s) and {} new warning(s): blocking={:?}; warnings={:?}",
                aggregate_engine.new_blocking.len(),
                aggregate_engine.new_warnings.len(),
                aggregate_engine.new_blocking,
                aggregate_engine.new_warnings,
            ));
        }
    }
    let policy_bundle_root = authority.history.policy_bundle.root()?;
    let bindings = &review.decision_bindings;
    let decision_bindings_root = format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "proposal_root": bindings.proposal_root,
            "receipt_observation_root": bindings.receipt_observation_root,
            "receipt_root": bindings.receipt_root,
            "evidence_or_reference_root": bindings.evidence_or_reference_root,
            "evidence_availability": bindings.evidence_availability,
            "verifier_snapshot_root": bindings.verifier_snapshot_root,
            "policy_input_root": bindings.policy_input_root,
            "policy_result_root": bindings.policy_result_root,
            "engine_gate_root": bindings.engine_gate_root,
            "semantic_effect_root": bindings.semantic_effect_root,
            "downstream_impact_root": bindings.downstream_impact_root,
        }))?
    );
    let mut plan = RepositoryReviewDecisionPlan {
        schema: PLAN_SCHEMA.into(),
        frontier_id: project.frontier_id(),
        frontier_name: project.project.name.clone(),
        proposal_id: proposal_id.into(),
        proposal_root: review.decision_bindings.proposal_root.clone(),
        decision_facts_root: review.brief.audit.decision_facts_root.clone(),
        decision_bindings_root,
        action: match action {
            DecisionAction::Accept => "review_accept",
            DecisionAction::Reject => "review_reject",
        }
        .into(),
        reason: reason.into(),
        principal_id: local.principal_id,
        observed_at: observed_at.into(),
        authority_event_log_root: authority.verification.final_event_log_root.clone(),
        policy_bundle_root,
        plan_root: String::new(),
    };
    plan.plan_root = plan_root(&plan)?;
    Ok(PreparedRepositoryReviewDecision {
        plan,
        review,
        project,
        authority,
        proposal,
    })
}

pub(crate) fn execute_reject(
    frontier: &Path,
    expected: &RepositoryReviewDecisionPlan,
) -> Result<AuthorityTransactionResult, String> {
    if expected.action != "review_reject" {
        return Err("repository rejection plan carries another action".into());
    }
    execute_decision(frontier, expected, DecisionAction::Reject)
}

pub(crate) fn execute_accept(
    frontier: &Path,
    expected: &RepositoryReviewDecisionPlan,
) -> Result<AuthorityTransactionResult, String> {
    if expected.action != "review_accept" {
        return Err("repository acceptance plan carries another action".into());
    }
    execute_decision(frontier, expected, DecisionAction::Accept)
}

fn execute_decision(
    frontier: &Path,
    expected: &RepositoryReviewDecisionPlan,
    action: DecisionAction,
) -> Result<AuthorityTransactionResult, String> {
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)?;
    let barrier = FrontierTxn::acquire_repository_authority_write_barrier(frontier, &journal_dir)
        .map_err(|error| error.to_string())?;
    let locked = prepare_decision(
        frontier,
        &expected.proposal_id,
        action,
        &expected.reason,
        &expected.observed_at,
    )?;
    if locked.plan != *expected {
        return Err(
            "repository review facts changed while acquiring the authority barrier; no authority signature was requested"
                .into(),
        );
    }

    let recorded_at =
        crate::cli::canonical_whole_second_time("review decision", &locked.plan.observed_at)?;
    let local = crate::cli::local_session(&recorded_at)?;
    if local.principal_id != locked.plan.principal_id {
        return Err("local operating-system principal changed before review execution".into());
    }
    let mut authentication = local;

    let authorization = CedarEvaluationInput {
        schema: locked.authority.policy_material.schema.clone(),
        policies: locked.authority.policy_material.policies.clone(),
        entities: locked.authority.policy_material.entities.clone(),
        principal: format!(
            "Human::{}",
            serde_json::to_string(&locked.plan.principal_id)
                .expect("serializing a principal string cannot fail")
        ),
        principal_class: PrincipalClass::Human,
        action: locked.plan.action.clone(),
        resource: format!(
            "Proposal::{}",
            serde_json::to_string(&locked.plan.proposal_id)
                .expect("serializing a proposal string cannot fail")
        ),
        context: json!({"exact": true}),
    };
    let (event_drafts, object_drafts, derived_drafts) = match action {
        DecisionAction::Reject => rejection_drafts(frontier, &locked, &recorded_at)?,
        DecisionAction::Accept => acceptance_drafts(frontier, &locked, &recorded_at)?,
    };
    let answer = SavedAnswer {
        proposal_id: locked.plan.proposal_id.clone(),
        proposal_root: locked.plan.proposal_root.clone(),
        seen_decision_facts_root: locked.plan.decision_facts_root.clone(),
        action,
        reason: locked.plan.reason.clone(),
    };
    let selection = ReviewProjection::selected_from_locked_project_at(
        frontier,
        &locked.project,
        std::slice::from_ref(&locked.plan.proposal_id),
        &locked.plan.observed_at,
    )
    .map_err(|error| error.to_string())?;
    let read_set = decision_read_set(
        frontier,
        &locked.project,
        &selection,
        std::slice::from_ref(&answer),
    )
    .map_err(|error| error.to_string())?;
    let (key_id, public_key) = crate::cli::active_repository_key(&locked.authority)?;
    let mut signer = SshAgentRepositoryAuthoritySigner::from_environment(key_id, &public_key)?;
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let binary_sha256 = crate::authority_transaction::execution_binary_sha256(&executable)?;
    let result = execute_authority_transaction(
        barrier,
        frontier,
        AuthorityTransactionRequest {
            history: locked.authority.history,
            intent_digest: locked.plan.plan_root.clone(),
            principal: PrincipalSnapshotV1 {
                principal_id: locked.plan.principal_id.clone(),
                principal_class: PrincipalClass::Human,
                display_name: Some("Frontier reviewer".into()),
                affiliation: None,
                account_links: vec![locked.plan.principal_id.clone()],
            },
            authentication_request: AuthenticationRequest {
                principal_id: locked.plan.principal_id.clone(),
                principal_class: PrincipalClass::Human,
                transaction_at: recorded_at.clone(),
            },
            runtime_session_state: RuntimeSessionState::default(),
            authorization_input: authorization,
            delegation: None,
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: locked.plan.principal_id.clone(),
                role: "frontier_reviewer".into(),
                action: locked.plan.action.clone(),
                reason: locked.plan.reason.clone(),
                approved_at: recorded_at.clone(),
                intent_digest: locked.plan.plan_root.clone(),
            }],
            event_drafts,
            object_drafts,
            derived_drafts,
            next_authority_keyset: None,
            next_policy_bundle: None,
            next_policy_material: None,
            read_set,
            vela_version: env!("CARGO_PKG_VERSION").into(),
            binary_sha256,
            recorded_at,
        },
        &mut authentication,
        &mut signer,
    )
    .map_err(|error| error.to_string())?;
    Ok(result)
}

fn rejection_drafts(
    frontier: &Path,
    locked: &PreparedRepositoryReviewDecision,
    recorded_at: &str,
) -> Result<
    (
        Vec<AuthorityEventDraft>,
        Vec<AuthorityObjectDraft>,
        Vec<AuthorityDerivedDraft>,
    ),
    String,
> {
    let mut rejected = locked.proposal.clone();
    rejected.status = "rejected".into();
    rejected.reviewed_by = Some(locked.plan.principal_id.clone());
    rejected.reviewed_at = Some(recorded_at.into());
    rejected.decision_reason = Some(locked.plan.reason.clone());
    rejected.applied_event_id = None;
    let event_drafts = vec![AuthorityEventDraft {
        kind: EventKind::ReviewRejected,
        target: StateTarget {
            r#type: "proposal".into(),
            id: locked.plan.proposal_id.clone(),
        },
        actor: StateActor {
            r#type: "human".into(),
            id: locked.plan.principal_id.clone(),
        },
        timestamp: recorded_at.into(),
        reason: locked.plan.reason.clone(),
        before_hash: NULL_HASH.into(),
        after_hash: NULL_HASH.into(),
        payload: json!({
            "proposal_id": locked.plan.proposal_id,
            "proposal_kind": locked.proposal.kind,
            "verdict": "rejected",
            "provenance": {
                "input_refs": [
                    format!("urn:vela:decision-root:{}", locked.plan.plan_root)
                ]
            }
        }),
        caveats: Vec::new(),
    }];
    let encoded = serde_json::to_value(&locked.project).map_err(|error| error.to_string())?;
    let mut candidate: vela_protocol::project::Project =
        serde_json::from_value(encoded).map_err(|error| error.to_string())?;
    let proposal = candidate
        .proposals
        .iter_mut()
        .find(|proposal| proposal.id == locked.plan.proposal_id)
        .ok_or_else(|| {
            format!(
                "proposal {} disappeared while preparing rejection",
                locked.plan.proposal_id
            )
        })?;
    *proposal = rejected;
    let current = event_drafts
        .iter()
        .map(semantic_event_from_draft)
        .collect::<Vec<_>>();
    candidate.events = candidate_events_with_current_semantics(locked, &current)?;
    vela_protocol::project::recompute_stats(&mut candidate);
    let drafts = changed_candidate_drafts(frontier, &candidate)?;
    Ok((event_drafts, drafts.objects, drafts.derived))
}

fn acceptance_drafts(
    frontier: &Path,
    locked: &PreparedRepositoryReviewDecision,
    recorded_at: &str,
) -> Result<
    (
        Vec<AuthorityEventDraft>,
        Vec<AuthorityObjectDraft>,
        Vec<AuthorityDerivedDraft>,
    ),
    String,
> {
    let (mut candidate, mut prepared) =
        vela_protocol::proposals::prepare_repository_authority_accept_candidate_at(
            &locked.project,
            &locked.plan.proposal_id,
            &locked.plan.principal_id,
            &locked.plan.reason,
            None,
            recorded_at,
        )?;
    vela_protocol::proposals::bind_decision_root_to_prepared(
        &mut candidate,
        &mut prepared,
        &locked.plan.plan_root,
    )?;
    let aggregate_engine = vela_protocol::proposals::strict_engine_verdict_for_candidate(
        &locked.project,
        &candidate,
        frontier,
        std::slice::from_ref(&locked.proposal.kind),
    );
    if aggregate_engine.status == "blocked" {
        return Err(format!(
            "strict aggregate Engine gate changed after protected approval: {} new blocking failure(s), {} new warning(s)",
            aggregate_engine.new_blocking.len(),
            aggregate_engine.new_warnings.len()
        ));
    }
    let appended = candidate
        .events
        .get(locked.project.events.len()..)
        .ok_or_else(|| "repository acceptance derived an invalid event range".to_string())?;
    let review_count = appended
        .iter()
        .filter(|event| event.kind == EventKind::ReviewAccepted)
        .count();
    if appended.len() < 2 || review_count != 1 {
        return Err(
            "repository acceptance must derive scientific domain event(s) and exactly one review.accepted"
                .into(),
        );
    }
    let event_drafts = appended
        .iter()
        .map(authority_event_draft_from_semantic)
        .collect::<Result<Vec<_>, _>>()?;
    let current = appended.to_vec();
    candidate.events = candidate_events_with_current_semantics(locked, &current)?;
    vela_protocol::project::recompute_stats(&mut candidate);
    let drafts = changed_candidate_drafts(frontier, &candidate)?;
    let object_drafts = drafts.objects;
    if !object_drafts.iter().any(|draft| {
        draft.path == format!(".vela/proposals/{}.json", locked.plan.proposal_id)
            && draft.class == WriteClass::PublicReview
    }) {
        return Err("repository acceptance lacks the exact proposal postimage".into());
    }
    Ok((event_drafts, object_drafts, drafts.derived))
}

fn authority_event_draft_from_semantic(event: &StateEvent) -> Result<AuthorityEventDraft, String> {
    if event.signature.is_some() {
        return Err(format!(
            "repository-authority semantic event {} unexpectedly carries a legacy signature",
            event.id
        ));
    }
    Ok(AuthorityEventDraft {
        kind: event.kind.clone(),
        target: event.target.clone(),
        actor: event.actor.clone(),
        timestamp: event.timestamp.clone(),
        reason: event.reason.clone(),
        before_hash: event.before_hash.clone(),
        after_hash: event.after_hash.clone(),
        payload: event.payload.clone(),
        caveats: event.caveats.clone(),
    })
}

fn candidate_events_with_current_semantics(
    locked: &PreparedRepositoryReviewDecision,
    current: &[StateEvent],
) -> Result<Vec<StateEvent>, String> {
    let mut events = vela_protocol::reducer::semantic_event_union(
        &locked.project,
        &locked.authority.history.authority_events,
    )?;
    let mut ids = events
        .iter()
        .map(|event| event.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for event in current {
        if !ids.insert(event.id.clone()) {
            return Err(format!(
                "repository decision would duplicate semantic event {}",
                event.id
            ));
        }
        events.push(event.clone());
    }
    Ok(events)
}

fn semantic_event_from_draft(draft: &AuthorityEventDraft) -> StateEvent {
    let mut event = StateEvent {
        schema: vela_protocol::events::EVENT_SCHEMA.into(),
        id: String::new(),
        kind: draft.kind.clone(),
        target: draft.target.clone(),
        actor: draft.actor.clone(),
        timestamp: draft.timestamp.clone(),
        reason: draft.reason.clone(),
        before_hash: draft.before_hash.clone(),
        after_hash: draft.after_hash.clone(),
        payload: draft.payload.clone(),
        caveats: draft.caveats.clone(),
        signature: None,
    };
    event.id = vela_protocol::events::compute_event_id(&event);
    event
}

struct CandidateDrafts {
    objects: Vec<AuthorityObjectDraft>,
    derived: Vec<AuthorityDerivedDraft>,
}

fn changed_candidate_drafts(
    frontier: &Path,
    candidate: &vela_protocol::project::Project,
) -> Result<CandidateDrafts, String> {
    let planned = PlannedWrite::from_managed_files(vela_protocol::repo::render_vela_repo_files(
        frontier, candidate,
    )?)
    .map_err(|error| error.to_string())?;
    let mut objects = Vec::new();
    let mut derived = Vec::new();
    for write in planned {
        let (path, class, postimage) = write
            .into_authority_object_parts()
            .map_err(|error| error.to_string())?;
        if path.starts_with(".vela/events/")
            || matches!(
                class,
                WriteClass::Authority | WriteClass::PrivateCoordination
            )
        {
            continue;
        }
        let absolute = frontier.join(&path);
        let existing = match fs::read(&absolute) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(format!(
                    "read candidate preimage {}: {error}",
                    absolute.display()
                ));
            }
        };
        if semantically_equal_postimage(existing.as_deref(), postimage.as_deref(), &path)? {
            continue;
        }
        if class == WriteClass::Derived {
            derived.push(AuthorityDerivedDraft { path, postimage });
        } else {
            let postimage = match postimage {
                Some(bytes) if path.ends_with(".json") => {
                    let value: serde_json::Value =
                        serde_json::from_slice(&bytes).map_err(|error| {
                            format!("candidate object {path} is invalid JSON: {error}")
                        })?;
                    Some(to_canonical_bytes(&value)?)
                }
                other => other,
            };
            objects.push(AuthorityObjectDraft {
                object_kind: authority_object_kind(&path).into(),
                path,
                class,
                postimage,
            });
        }
    }
    Ok(CandidateDrafts { objects, derived })
}

fn semantically_equal_postimage(
    existing: Option<&[u8]>,
    candidate: Option<&[u8]>,
    path: &str,
) -> Result<bool, String> {
    match (existing, candidate) {
        (None, None) => Ok(true),
        (Some(left), Some(right)) if path.ends_with(".json") => {
            let left: serde_json::Value = serde_json::from_slice(left)
                .map_err(|error| format!("existing object {path} is invalid JSON: {error}"))?;
            let right: serde_json::Value = serde_json::from_slice(right)
                .map_err(|error| format!("candidate object {path} is invalid JSON: {error}"))?;
            Ok(left == right)
        }
        (Some(left), Some(right)) => Ok(left == right),
        _ => Ok(false),
    }
}

fn authority_object_kind(path: &str) -> &'static str {
    if path.starts_with(".vela/proposals/") {
        "proposal"
    } else if path.starts_with(".vela/findings/") {
        "finding"
    } else if path.starts_with(".vela/artifacts/") {
        "artifact"
    } else if path.starts_with(".vela/verifier-attachments/") {
        "verifier_attachment"
    } else if path.starts_with(".vela/attempts/") {
        "attempt"
    } else {
        "canonical_evidence"
    }
}

fn plan_root(plan: &RepositoryReviewDecisionPlan) -> Result<String, String> {
    let mut value = serde_json::to_value(plan).map_err(|error| error.to_string())?;
    value
        .as_object_mut()
        .ok_or_else(|| "repository review plan must be an object".to_string())?
        .insert("plan_root".into(), serde_json::Value::String(String::new()));
    let mut digest = Sha256::new();
    digest.update(PLAN_DOMAIN);
    digest.update(to_canonical_bytes(&value)?);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}
