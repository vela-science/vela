//! Current repository review decisions.
//!
//! A human supplies the exact semantic command and is authenticated by the
//! local operating-system session. Repository authority signs the covering
//! transaction. No human scientific key is read.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use vela_authority::CedarEvaluationInput;
use vela_authority::runtime_authentication::AuthenticationRequest;
use vela_protocol::authority::{PrincipalSnapshotV1, SemanticApprovalV1};
use vela_protocol::claim_record::ClaimRecordV1;
use vela_protocol::current_repository::{
    ClaimStandingRefV1, CurrentRepositoryV4, RepositoryObjectRefV1,
};
use vela_protocol::events::{EventKind, NULL_HASH, StateActor, StateEvent, StateTarget};
use vela_protocol::principal::PrincipalClass;
use vela_protocol::proposal_v1::ProposalV1;
use vela_protocol::repository_origin::RepositoryOriginV1;
use vela_protocol::submission_v1::SubmissionV1;
use vela_protocol::verification_record::VerificationRecordV1;

use crate::authority_transaction::{
    AuthorityEventDraft, AuthorityObjectDraft, AuthorityTransactionRequest,
    AuthorityTransactionResult, prepare_authority_transaction,
};
use crate::config::git_publish::{
    PublicationState, PublishOptions, exact_publication_preflight, publish_exact_delta,
};
use crate::frontier_txn::{
    ContentDigest, FrontierRecoveryBarrier, FrontierTxn, InputBinding, WriteClass,
};
use crate::repository_authority_provider::SshAgentRepositoryAuthoritySigner;
use crate::repository_ops::publication_delta;

const PLAN_SCHEMA: &str = "vela.current-review-decision.v1";
const PLAN_DOMAIN: &[u8] = b"vela.current-review-decision.v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DecisionAction {
    Accept,
    Reject,
}

impl DecisionAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Reject => "reject",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CurrentReviewDecisionPlan {
    pub(crate) schema: String,
    pub(crate) frontier_id: String,
    pub(crate) frontier_name: String,
    pub(crate) repository_root: String,
    pub(crate) proposal_id: String,
    pub(crate) proposal_root: String,
    pub(crate) claim_id: String,
    pub(crate) claim_root: String,
    pub(crate) submission_root: String,
    pub(crate) verification_set_root: String,
    pub(crate) action: String,
    pub(crate) reason: String,
    pub(crate) principal_id: String,
    pub(crate) observed_at: String,
    pub(crate) authority_event_log_root: String,
    pub(crate) policy_bundle_root: String,
    pub(crate) plan_root: String,
}

pub(crate) struct PreparedCurrentReviewDecision {
    pub(crate) plan: CurrentReviewDecisionPlan,
    pub(crate) repository: CurrentRepositoryV4,
    pub(crate) authority: crate::cli::LoadedRepositoryAuthority,
    pub(crate) proposal_reference: RepositoryObjectRefV1,
    pub(crate) proposal: ProposalV1,
    pub(crate) claim: ClaimRecordV1,
    pub(crate) submission: SubmissionV1,
    pub(crate) verifications: Vec<(String, VerificationRecordV1)>,
    pub(crate) pending_conflicts: Vec<String>,
}

pub(crate) fn read_exact<T>(
    frontier: &Path,
    path: &str,
    expected_root: &str,
    parse: impl FnOnce(&[u8]) -> Result<T, String>,
    canonical_bytes: impl FnOnce(&T) -> Result<Vec<u8>, String>,
) -> Result<T, String> {
    let bytes = fs::read(frontier.join(path)).map_err(|error| format!("read {path}: {error}"))?;
    if format!("sha256:{}", hex::encode(Sha256::digest(bytes.as_slice()))) != expected_root {
        return Err(format!("{path} differs from its declared full root"));
    }
    let value = parse(&bytes)?;
    if canonical_bytes(&value)? != bytes {
        return Err(format!("{path} is not exact canonical JSON"));
    }
    Ok(value)
}

pub(crate) fn claim_for_proposal(
    frontier: &Path,
    repository: &CurrentRepositoryV4,
    proposal: &ProposalV1,
) -> Result<ClaimRecordV1, String> {
    let reference = repository
        .pending_claims
        .iter()
        .chain(&repository.accepted_claims)
        .find(|claim| {
            claim.claim_id == proposal.subject.id && claim.claim_root == proposal.subject.root
        })
        .ok_or_else(|| {
            format!(
                "Proposal {} has no exact current Claim subject",
                proposal.proposal_id
            )
        })?;
    let claim = read_exact(
        frontier,
        &reference.path,
        &reference.claim_root,
        ClaimRecordV1::parse,
        ClaimRecordV1::canonical_bytes,
    )?;
    if claim.claim_id != reference.claim_id {
        return Err("Proposal Claim identity differs from the current repository".into());
    }
    Ok(claim)
}

pub(crate) fn submission_for_proposal(
    frontier: &Path,
    repository: &CurrentRepositoryV4,
    proposal: &ProposalV1,
) -> Result<SubmissionV1, String> {
    let reference = repository
        .submissions
        .iter()
        .find(|submission| {
            submission.id == proposal.producer_package.id
                && submission.root == proposal.producer_package.root
                && submission.path == proposal.producer_package.path
        })
        .ok_or_else(|| {
            format!(
                "Proposal {} has no exact current Submission",
                proposal.proposal_id
            )
        })?;
    let submission = read_exact(
        frontier,
        &reference.path,
        &reference.root,
        SubmissionV1::parse,
        SubmissionV1::canonical_bytes,
    )?;
    if submission.submission_id != reference.id {
        return Err("Proposal Submission identity differs from the current repository".into());
    }
    Ok(submission)
}

pub(crate) fn exact_verifications(
    frontier: &Path,
    repository: &CurrentRepositoryV4,
    proposal: &ProposalV1,
    claim: &ClaimRecordV1,
    submission: &SubmissionV1,
) -> Result<Vec<(String, VerificationRecordV1)>, String> {
    let mut records = Vec::new();
    for reference in &repository.verifications {
        let record = read_exact(
            frontier,
            &reference.path,
            &reference.root,
            VerificationRecordV1::parse,
            VerificationRecordV1::canonical_bytes,
        )?;
        if crate::current_repository::verification_targets_proposal(proposal, claim, &record)
            && record.subject.submission_id == submission.submission_id
        {
            records.push((reference.root.clone(), record));
        }
    }
    records.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(records)
}

pub(crate) fn verification_set_root(
    records: &[(String, VerificationRecordV1)],
) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        vela_protocol::canonical::sha256_canonical(&json!({
            "schema": "vela.current-verification-set.v1",
            "records": records.iter().map(|(root, record)| json!({
                "verification_record_id": record.verification_record_id,
                "verification_record_root": root,
            })).collect::<Vec<_>>(),
        }))?
    ))
}

pub(crate) fn verification_satisfies_requirement(
    submission: &SubmissionV1,
    requirement: &str,
    record: &VerificationRecordV1,
) -> bool {
    record.scope.property == requirement
        && record.outcome == "pass"
        && record.verifier != submission.provenance.producer
        && record
            .independence
            .declared_independent_of
            .contains(&submission.provenance.producer)
}

pub(crate) fn require_acceptance_evidence(
    submission: &SubmissionV1,
    records: &[(String, VerificationRecordV1)],
) -> Result<(), String> {
    if records
        .iter()
        .any(|(_, record)| matches!(record.outcome.as_str(), "fail" | "error"))
    {
        return Err("current acceptance is blocked by a failing Verification Record".into());
    }
    for requirement in &submission.verification_requirements {
        let satisfying = records.iter().filter(|(_, record)| {
            verification_satisfies_requirement(submission, requirement, record)
        });
        if satisfying.count() == 0 {
            return Err(format!(
                "current acceptance lacks an independent passing Verification Record for {requirement:?}"
            ));
        }
    }
    Ok(())
}

/// Return whether two pending Submissions are alternate semantic renderings
/// of the same exact producer execution.
///
/// A shared artifact is not enough: one run may support multiple Claims. The
/// collision key therefore requires the same producer, source system, run,
/// attempt, requested change, scoped conditions, artifacts, and verifier
/// contract. The assertion may differ because corrected wording is the common
/// reason the same execution is submitted twice.
pub(crate) fn same_exact_producer_execution(left: &SubmissionV1, right: &SubmissionV1) -> bool {
    let Some(left_run) = left.provenance.source_run.as_deref() else {
        return false;
    };
    let Some(right_run) = right.provenance.source_run.as_deref() else {
        return false;
    };
    let Some(left_attempt) = left.provenance.source_attempt.as_deref() else {
        return false;
    };
    let Some(right_attempt) = right.provenance.source_attempt.as_deref() else {
        return false;
    };
    if left_run.is_empty()
        || left_attempt.is_empty()
        || left_run != right_run
        || left_attempt != right_attempt
    {
        return false;
    }

    let mut left_artifacts = left
        .artifacts
        .iter()
        .map(|artifact| (&artifact.kind, &artifact.digest))
        .collect::<Vec<_>>();
    let mut right_artifacts = right
        .artifacts
        .iter()
        .map(|artifact| (&artifact.kind, &artifact.digest))
        .collect::<Vec<_>>();
    left_artifacts.sort();
    right_artifacts.sort();

    !left_artifacts.is_empty()
        && left.provenance.producer == right.provenance.producer
        && left.provenance.source_system == right.provenance.source_system
        && left.requested_change == right.requested_change
        && left.claim.claim_type == right.claim.claim_type
        && left.claim.conditions == right.claim.conditions
        && left_artifacts == right_artifacts
        && left.verification_requirements == right.verification_requirements
        && left.execution_binding == right.execution_binding
}

/// Find unresolved sibling Proposals that bind the same exact producer
/// execution. Accepting either sibling while both remain pending would make a
/// wording retry look like two independent scientific advances.
pub(crate) fn pending_submission_conflicts(
    frontier: &Path,
    repository: &CurrentRepositoryV4,
    proposal: &ProposalV1,
    submission: &SubmissionV1,
) -> Result<Vec<String>, String> {
    let standings =
        crate::current_repository::load_current_proposal_standings(frontier, repository)?;
    let mut conflicts = Vec::new();
    for reference in &repository.proposals {
        if reference.id == proposal.proposal_id || standings.contains_key(&reference.id) {
            continue;
        }
        let sibling = read_exact(
            frontier,
            &reference.path,
            &reference.root,
            ProposalV1::parse,
            ProposalV1::canonical_bytes,
        )?;
        if sibling.action != proposal.action {
            continue;
        }
        let sibling_submission = submission_for_proposal(frontier, repository, &sibling)?;
        if same_exact_producer_execution(submission, &sibling_submission) {
            conflicts.push(sibling.proposal_id);
        }
    }
    conflicts.sort();
    Ok(conflicts)
}

fn load_origin(frontier: &Path) -> Result<RepositoryOriginV1, String> {
    let bytes = fs::read(frontier.join(".vela/origin.json"))
        .map_err(|error| format!("read current repository origin: {error}"))?;
    RepositoryOriginV1::parse(&bytes)
}

pub(crate) fn prepare(
    frontier: &Path,
    proposal_id: &str,
    action: DecisionAction,
    reason: &str,
    observed_at: &str,
) -> Result<PreparedCurrentReviewDecision, String> {
    if reason.trim().is_empty() || reason != reason.trim() {
        return Err("current review reason must be non-empty trimmed text".into());
    }
    DateTime::parse_from_rfc3339(observed_at)
        .map_err(|error| format!("current review observation time is invalid: {error}"))?;
    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    let repository_root = repository.canonical_root()?;
    let origin = load_origin(frontier)?;
    let authority = crate::cli::load_current_repository_authority(frontier, &repository, &origin)?;
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
            "Proposal {proposal_id} already has a current repository Decision"
        ));
    }
    let proposal_reference = repository
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("current repository has no Proposal {proposal_id}"))?
        .clone();
    let proposal = read_exact(
        frontier,
        &proposal_reference.path,
        &proposal_reference.root,
        ProposalV1::parse,
        ProposalV1::canonical_bytes,
    )?;
    let claim = claim_for_proposal(frontier, &repository, &proposal)?;
    let submission = submission_for_proposal(frontier, &repository, &proposal)?;
    let verifications = exact_verifications(frontier, &repository, &proposal, &claim, &submission)?;
    let pending_conflicts =
        pending_submission_conflicts(frontier, &repository, &proposal, &submission)?;
    if action == DecisionAction::Accept {
        require_acceptance_evidence(&submission, &verifications)?;
        if !pending_conflicts.is_empty() {
            return Err(format!(
                "current acceptance is blocked by unresolved sibling Proposal(s) {} from the same exact producer execution; reject or withdraw the obsolete wording first",
                pending_conflicts.join(", ")
            ));
        }
    }
    let profile = vela_protocol::current_repository::CurrentFrontierProfileV2::from_toml_str(
        &fs::read_to_string(frontier.join("frontier.toml"))
            .map_err(|error| format!("read current Frontier Profile: {error}"))?,
    )?;
    let local = crate::cli::local_session(observed_at)?;
    let mut plan = CurrentReviewDecisionPlan {
        schema: PLAN_SCHEMA.into(),
        frontier_id: repository.frontier_id.clone(),
        frontier_name: profile.name,
        repository_root,
        proposal_id: proposal.proposal_id.clone(),
        proposal_root: proposal_reference.root.clone(),
        claim_id: claim.claim_id.clone(),
        claim_root: claim.canonical_root()?,
        submission_root: submission.canonical_root()?,
        verification_set_root: verification_set_root(&verifications)?,
        action: match action {
            DecisionAction::Accept => "review_accept",
            DecisionAction::Reject => "review_reject",
        }
        .into(),
        reason: reason.into(),
        principal_id: local.principal_id,
        observed_at: observed_at.into(),
        authority_event_log_root: authority.verification.final_event_log_root.clone(),
        policy_bundle_root: authority.history.policy_bundle.root()?,
        plan_root: String::new(),
    };
    plan.plan_root = plan_root(&plan)?;
    Ok(PreparedCurrentReviewDecision {
        plan,
        repository,
        authority,
        proposal_reference,
        proposal,
        claim,
        submission,
        verifications,
        pending_conflicts,
    })
}

/// Acquire the frontier write barrier before loading any mutable Decision
/// input, then prepare the complete verified Decision exactly once.
pub(crate) fn prepare_locked(
    frontier: &Path,
    proposal_id: &str,
    action: DecisionAction,
    reason: &str,
    observed_at: &str,
) -> Result<(PreparedCurrentReviewDecision, FrontierRecoveryBarrier), String> {
    let journal_dir = crate::repository_ops::frontier_transaction_journal_dir(frontier)?;
    let barrier = FrontierTxn::acquire_recovery_barrier(frontier, &journal_dir)
        .map_err(|error| error.to_string())?;
    let prepared = prepare(frontier, proposal_id, action, reason, observed_at)?;
    Ok((prepared, barrier))
}

pub(crate) fn next_repository(
    current: &CurrentRepositoryV4,
    proposal: &ProposalV1,
    subject_claim: &ClaimRecordV1,
    claim_root: &str,
    action: DecisionAction,
) -> Result<CurrentRepositoryV4, String> {
    let mut repository = current.clone();
    if action == DecisionAction::Reject {
        if proposal.action != "claim.withdraw" {
            repository.pending_claims.retain(|reference| {
                reference.claim_id != subject_claim.claim_id || reference.claim_root != claim_root
            });
        }
        repository.verify()?;
        return Ok(repository);
    }

    match proposal.action.as_str() {
        "claim.add" | "claim.revise" => {
            let pending = repository
                .pending_claims
                .iter()
                .find(|reference| {
                    reference.claim_id == subject_claim.claim_id
                        && reference.claim_root == claim_root
                })
                .cloned()
                .ok_or_else(|| "accepted Proposal subject is not pending".to_string())?;
            repository.pending_claims.retain(|claim| {
                claim.claim_id != pending.claim_id || claim.claim_root != pending.claim_root
            });
            if proposal.action == "claim.revise" {
                let superseded = subject_claim
                    .relations
                    .iter()
                    .filter(|relation| matches!(relation.kind.as_str(), "corrects" | "supersedes"))
                    .map(|relation| relation.target_claim_id.as_str())
                    .collect::<BTreeSet<_>>();
                if superseded.len() != 1 {
                    return Err(
                        "Claim revision must name exactly one corrected or superseded Claim".into(),
                    );
                }
                repository
                    .accepted_claims
                    .retain(|claim| !superseded.contains(claim.claim_id.as_str()));
            }
            repository.accepted_claims.push(ClaimStandingRefV1 {
                claim_id: pending.claim_id,
                claim_root: pending.claim_root,
                standing: "accepted".into(),
                path: pending.path,
            });
            repository
                .accepted_claims
                .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        }
        "claim.withdraw" => {
            let before = repository.accepted_claims.len();
            repository.accepted_claims.retain(|reference| {
                reference.claim_id != subject_claim.claim_id || reference.claim_root != claim_root
            });
            if repository.accepted_claims.len() + 1 != before {
                return Err("withdrawal Proposal subject is not exactly accepted".into());
            }
        }
        other => return Err(format!("unsupported current Proposal action {other}")),
    }
    repository.verify()?;
    Ok(repository)
}

fn semantic_event_id(draft: &AuthorityEventDraft) -> String {
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
    event.id
}

fn decision_events(
    plan: &CurrentReviewDecisionPlan,
    repository: &CurrentRepositoryV4,
    proposal: &ProposalV1,
    claim: &ClaimRecordV1,
    next_repository_root: &str,
    action: DecisionAction,
    recorded_at: &str,
) -> Result<Vec<AuthorityEventDraft>, String> {
    let actor = StateActor {
        r#type: "human".into(),
        id: plan.principal_id.clone(),
    };
    if action == DecisionAction::Reject {
        return Ok(vec![AuthorityEventDraft {
            kind: EventKind::ReviewRejected,
            target: StateTarget {
                r#type: "proposal".into(),
                id: proposal.proposal_id.clone(),
            },
            actor,
            timestamp: recorded_at.into(),
            reason: plan.reason.clone(),
            before_hash: NULL_HASH.into(),
            after_hash: NULL_HASH.into(),
            payload: json!({
                "proposal_id": proposal.proposal_id,
                "proposal_kind": proposal.action,
                "verdict": "rejected",
                "repository_before": plan.repository_root,
                "repository_after": next_repository_root,
            }),
            caveats: Vec::new(),
        }]);
    }

    let (kind, target, before_hash, after_hash) = match proposal.action.as_str() {
        "claim.add" => (
            EventKind::ClaimAsserted,
            claim.claim_id.clone(),
            NULL_HASH.into(),
            plan.claim_root.clone(),
        ),
        "claim.revise" => {
            let target = claim
                .relations
                .iter()
                .find(|relation| matches!(relation.kind.as_str(), "corrects" | "supersedes"))
                .ok_or_else(|| "Claim revision has no exact predecessor relation".to_string())?
                .target_claim_id
                .clone();
            let before = repository
                .accepted_claims
                .iter()
                .find(|claim| claim.claim_id == target)
                .ok_or_else(|| "Claim revision predecessor is not accepted".to_string())?
                .claim_root
                .clone();
            (
                EventKind::ClaimSuperseded,
                target,
                before,
                plan.claim_root.clone(),
            )
        }
        "claim.withdraw" => (
            EventKind::ClaimRetracted,
            claim.claim_id.clone(),
            plan.claim_root.clone(),
            NULL_HASH.into(),
        ),
        other => return Err(format!("unsupported current Proposal action {other}")),
    };
    let domain = AuthorityEventDraft {
        kind,
        target: StateTarget {
            r#type: "claim".into(),
            id: target,
        },
        actor: actor.clone(),
        timestamp: recorded_at.into(),
        reason: plan.reason.clone(),
        before_hash,
        after_hash,
        payload: json!({
            "claim_id": claim.claim_id,
            "claim_root": plan.claim_root,
            "proposal_id": proposal.proposal_id,
            "repository_before": plan.repository_root,
            "repository_after": next_repository_root,
        }),
        caveats: proposal.caveats.clone(),
    };
    let applied_event_id = semantic_event_id(&domain);
    let review = AuthorityEventDraft {
        kind: EventKind::ReviewAccepted,
        target: StateTarget {
            r#type: "proposal".into(),
            id: proposal.proposal_id.clone(),
        },
        actor,
        timestamp: recorded_at.into(),
        reason: plan.reason.clone(),
        before_hash: NULL_HASH.into(),
        after_hash: NULL_HASH.into(),
        payload: json!({
            "proposal_id": proposal.proposal_id,
            "proposal_kind": proposal.action,
            "verdict": "accepted",
            "applied_event_id": applied_event_id,
            "repository_before": plan.repository_root,
            "repository_after": next_repository_root,
        }),
        caveats: Vec::new(),
    };
    Ok(vec![domain, review])
}

pub(crate) fn execute_prepared(
    frontier: &Path,
    prepared: PreparedCurrentReviewDecision,
    recovery_barrier: FrontierRecoveryBarrier,
    action: DecisionAction,
) -> Result<AuthorityTransactionResult, String> {
    let expected = &prepared.plan;
    let expected_action = match action {
        DecisionAction::Accept => "review_accept",
        DecisionAction::Reject => "review_reject",
    };
    if expected.action != expected_action {
        return Err("current review plan carries another action".into());
    }
    let barrier = recovery_barrier
        .authorize_verified_repository_authority(&prepared.repository, &prepared.authority)
        .map_err(|error| error.to_string())?;
    let recorded_at =
        crate::cli::canonical_whole_second_time("current review decision", &expected.observed_at)?;
    let local = crate::cli::local_session(&recorded_at)?;
    if local.principal_id != expected.principal_id {
        return Err("local operating-system principal changed before review execution".into());
    }
    let next = next_repository(
        &prepared.repository,
        &prepared.proposal,
        &prepared.claim,
        &prepared.plan.claim_root,
        action,
    )?;
    let next_root = next.canonical_root()?;
    let derived = crate::current_submission::rebind_target_index(frontier, &next)?;
    let events = decision_events(
        &prepared.plan,
        &prepared.repository,
        &prepared.proposal,
        &prepared.claim,
        &next_root,
        action,
        &recorded_at,
    )?;
    next.verify()?;

    let authorization = CedarEvaluationInput {
        schema: prepared.authority.policy_material.schema.clone(),
        policies: prepared.authority.policy_material.policies.clone(),
        entities: prepared.authority.policy_material.entities.clone(),
        principal: format!(
            "Human::{}",
            serde_json::to_string(&expected.principal_id).expect("principal serializes")
        ),
        principal_class: PrincipalClass::Human,
        action: expected.action.clone(),
        resource: format!(
            "Proposal::{}",
            serde_json::to_string(&expected.proposal_id).expect("Proposal ID serializes")
        ),
        context: json!({"exact": true}),
    };
    let mut read_set = vec![
        InputBinding {
            name: "current_repository_before".into(),
            digest: ContentDigest::parse(expected.repository_root.clone())
                .map_err(|error| error.to_string())?,
        },
        InputBinding {
            name: "proposal".into(),
            digest: ContentDigest::parse(expected.proposal_root.clone())
                .map_err(|error| error.to_string())?,
        },
        InputBinding {
            name: "claim".into(),
            digest: ContentDigest::parse(expected.claim_root.clone())
                .map_err(|error| error.to_string())?,
        },
        InputBinding {
            name: "submission".into(),
            digest: ContentDigest::parse(expected.submission_root.clone())
                .map_err(|error| error.to_string())?,
        },
        InputBinding {
            name: "verification_set".into(),
            digest: ContentDigest::parse(expected.verification_set_root.clone())
                .map_err(|error| error.to_string())?,
        },
    ];
    read_set.sort_by(|left, right| left.name.cmp(&right.name));
    let (key_id, public_key) =
        crate::repository_ops::active_repository_signing_key(&prepared.authority)?;
    let mut signer = SshAgentRepositoryAuthoritySigner::from_environment(key_id, &public_key)?;
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let binary_sha256 = crate::authority_transaction::execution_binary_sha256(&executable)?;
    let mut authentication = local;
    let mut transaction = prepare_authority_transaction(
        barrier,
        frontier,
        AuthorityTransactionRequest {
            history: prepared.authority.history,
            intent_digest: expected.plan_root.clone(),
            principal: PrincipalSnapshotV1 {
                principal_id: expected.principal_id.clone(),
                principal_class: PrincipalClass::Human,
                display_name: Some("Frontier reviewer".into()),
                affiliation: None,
                account_links: vec![expected.principal_id.clone()],
            },
            authentication_request: AuthenticationRequest {
                principal_id: expected.principal_id.clone(),
                principal_class: PrincipalClass::Human,
                transaction_at: recorded_at.clone(),
            },
            authorization_input: authorization,
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: expected.principal_id.clone(),
                role: "frontier_reviewer".into(),
                action: expected.action.clone(),
                reason: expected.reason.clone(),
                approved_at: recorded_at.clone(),
                intent_digest: expected.plan_root.clone(),
            }],
            event_drafts: events,
            object_drafts: vec![AuthorityObjectDraft {
                path: ".vela/repository.json".into(),
                object_kind: "repository_manifest".into(),
                class: WriteClass::CanonicalEvidence,
                postimage: Some(next.canonical_bytes()?),
            }],
            derived_drafts: derived,
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
    let result = transaction.result.clone();
    let public = transaction
        .resolved_public_writes()
        .map_err(|error| error.to_string())?;
    let delta = publication_delta(frontier, transaction.canonical_delta_root(), public)?
        .ok_or_else(|| "review Decision produced no exact Git delta".to_string())?;
    let preflight = exact_publication_preflight(frontier, &delta, &PublishOptions::local())
        .map_err(|outcome| {
            format!("review Decision Git preflight failed before installation: {outcome:?}")
        })?;
    transaction
        .mark_committed()
        .map_err(|error| error.to_string())?;
    transaction.install().map_err(|error| error.to_string())?;
    transaction.complete().map_err(|error| error.to_string())?;
    if let Err(error) =
        crate::current_repository::verify_current_repository_allow_derived_drift_at(frontier)
    {
        return Err(format!(
            "repository-authority transaction committed as record {} but postcondition verification failed: {error}; do not retry the Decision",
            result.authority_record_id
        ));
    }
    let publication = publish_exact_delta(
        frontier,
        &format!("review {}", action.as_str()),
        std::slice::from_ref(&expected.proposal_id),
        &delta,
        preflight,
    )
    .map_err(|error| {
        format!(
            "review Decision committed as record {} but exact Git publication failed: {error}; do not retry the Decision",
            result.authority_record_id
        )
    })?;
    if !matches!(
        publication.state,
        PublicationState::Unchanged { .. } | PublicationState::CommittedLocal { .. }
    ) {
        return Err(format!(
            "review Decision committed as record {} but Git publication is incomplete: {publication:?}; do not retry the Decision",
            result.authority_record_id
        ));
    }
    crate::current_repository::verify_current_repository_at(frontier, true).map_err(|error| {
        format!(
            "review Decision was published but strict verification failed: {error}; do not retry the Decision"
        )
    })?;
    if let Err(error) = transaction.retire_completed_recovery_blobs() {
        crate::ui::warn_nonfatal(&format!(
            "review Decision {} was published and verified, but private recovery blob cleanup failed: {error}",
            result.operation_id
        ));
    }
    Ok(result)
}

fn plan_root(plan: &CurrentReviewDecisionPlan) -> Result<String, String> {
    let mut value = serde_json::to_value(plan).map_err(|error| error.to_string())?;
    value
        .as_object_mut()
        .ok_or_else(|| "current review plan must be an object".to_string())?
        .insert("plan_root".into(), serde_json::Value::String(String::new()));
    let mut digest = Sha256::new();
    digest.update(PLAN_DOMAIN);
    digest.update(vela_protocol::canonical::to_canonical_bytes(&value)?);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ed25519_dalek::SigningKey;
    use vela_protocol::claim_record::{ClaimAssertion, ClaimRelation, ClaimSource};
    use vela_protocol::current_repository::CURRENT_REPOSITORY_SCHEMA_V4;
    use vela_protocol::identity::{ActorClass, IdentityBinding, IdentityBindingDraft};
    use vela_protocol::proposal_v1::{ProposalProducerPackage, ProposalSubject};
    use vela_protocol::submission_v1::{
        RequestedChange, SubmissionArtifact, SubmissionClaim, SubmissionDraft, SubmissionProvenance,
    };
    use vela_protocol::verification_record::{
        IndependenceDisclosure, VerificationMethod, VerificationRecordDraft, VerificationScope,
        VerificationSubject,
    };

    use super::*;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn repository() -> CurrentRepositoryV4 {
        CurrentRepositoryV4 {
            schema: CURRENT_REPOSITORY_SCHEMA_V4.into(),
            frontier_id: "vfr_0123456789abcdef".into(),
            profile_root: root('a'),
            origin_id: "vro_0123456789abcdef".into(),
            origin_root: root('b'),
            accepted_claims: Vec::new(),
            pending_claims: Vec::new(),
            proposals: Vec::new(),
            proposal_withdrawals: Vec::new(),
            submissions: Vec::new(),
            verifications: Vec::new(),
            artifacts: Vec::new(),
            authority_keyset_root: root('c'),
            authority_policy_root: root('d'),
        }
    }

    fn claim(text: &str, revision: u32, relations: Vec<ClaimRelation>) -> ClaimRecordV1 {
        ClaimRecordV1::build(
            revision,
            ClaimAssertion {
                text: text.into(),
                kind: "computational".into(),
            },
            vec!["Fixture domain.".into()],
            Vec::new(),
            vec![ClaimSource {
                kind: "fixture".into(),
                title: "Decision fixture".into(),
                locator: None,
                authors: vec!["Fixture author".into()],
                year: Some(2026),
            }],
            relations,
            "2026-07-27T00:00:00Z".into(),
            BTreeMap::new(),
        )
        .unwrap()
    }

    fn standing(claim: &ClaimRecordV1, status: &str) -> ClaimStandingRefV1 {
        ClaimStandingRefV1 {
            claim_id: claim.claim_id.clone(),
            claim_root: claim.canonical_root().unwrap(),
            standing: status.into(),
            path: format!(
                "records/claims/sha256/{}.json",
                claim
                    .canonical_root()
                    .unwrap()
                    .trim_start_matches("sha256:")
            ),
        }
    }

    fn proposal(action: &str, claim: &ClaimRecordV1) -> ProposalV1 {
        ProposalV1::build(
            action.into(),
            ProposalSubject {
                kind: "claim".into(),
                id: claim.claim_id.clone(),
                root: claim.canonical_root().unwrap(),
            },
            "agent:producer-fixture".into(),
            "2026-07-27T00:00:01Z".into(),
            "Request the exact fixture transition.".into(),
            ProposalProducerPackage {
                kind: "submission_v1".into(),
                id: "vsb_fixture".into(),
                root: root('e'),
                path: format!("records/submissions/sha256/{}.json", "e".repeat(64)),
            },
            vec!["Fixture caveat.".into()],
        )
        .unwrap()
    }

    fn submission() -> SubmissionV1 {
        let key = SigningKey::from_bytes(&[61_u8; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "agent:producer-fixture".into(),
                actor_class: ActorClass::Agent,
                created_at: "2026-07-27T00:00:00Z".into(),
            },
            &key,
        )
        .unwrap();
        SubmissionV1::build(
            SubmissionDraft {
                claim: SubmissionClaim {
                    assertion: "The fixture has a bounded witness.".into(),
                    claim_type: "computational".into(),
                    conditions: vec!["Fixture domain.".into()],
                },
                artifacts: vec![SubmissionArtifact {
                    kind: "witness".into(),
                    path: "witness.json".into(),
                    digest: root('f'),
                }],
                caveats: vec!["Not an unrestricted result.".into()],
                replayability: "exact".into(),
                producer_checks: Vec::new(),
                verification_requirements: vec!["Replay the frozen verifier.".into()],
                requested_change: RequestedChange {
                    kind: "add_claim".into(),
                    target: None,
                },
                provenance: SubmissionProvenance {
                    producer: "agent:producer-fixture".into(),
                    source_system: "fixture".into(),
                    source_attempt: None,
                    source_run: Some("run_fixture".into()),
                    emitted_at: "2026-07-27T00:00:00Z".into(),
                },
                execution_binding: None,
            },
            identity,
            &key,
        )
        .unwrap()
    }

    fn verification(
        submission: &SubmissionV1,
        property: &str,
        outcome: &str,
        verifier: &str,
        declared_independent: bool,
    ) -> VerificationRecordV1 {
        let key = SigningKey::from_bytes(&[62_u8; 32]);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: verifier.into(),
                actor_class: ActorClass::Org,
                created_at: "2026-07-27T00:00:00Z".into(),
            },
            &key,
        )
        .unwrap();
        VerificationRecordV1::build(
            VerificationRecordDraft {
                subject: VerificationSubject {
                    claim_id: format!("vcl_{}", "a".repeat(64)),
                    artifact_ids: vec!["a".repeat(64)],
                    submission_id: submission.submission_id.clone(),
                    submission_root: submission.canonical_root().unwrap(),
                    proposal_id: "vpr_fixture".into(),
                },
                method: VerificationMethod {
                    profile: "fixture-v1".into(),
                    implementation: "fixture-verifier".into(),
                    environment_root: root('9'),
                },
                scope: VerificationScope {
                    property: property.into(),
                    does_not_establish: vec!["Scientific acceptance.".into()],
                },
                outcome: outcome.into(),
                verifier: verifier.into(),
                independence: IndependenceDisclosure {
                    declared_independent_of: if declared_independent {
                        vec![submission.provenance.producer.clone()]
                    } else {
                        Vec::new()
                    },
                    shared_dependencies: Vec::new(),
                },
                output_artifact_ids: Vec::new(),
                started_at: "2026-07-27T00:00:02Z".into(),
                completed_at: "2026-07-27T00:00:03Z".into(),
            },
            identity,
            &key,
        )
        .unwrap()
    }

    fn plan() -> CurrentReviewDecisionPlan {
        let mut plan = CurrentReviewDecisionPlan {
            schema: PLAN_SCHEMA.into(),
            frontier_id: "vfr_0123456789abcdef".into(),
            frontier_name: "Fixture frontier".into(),
            repository_root: root('1'),
            proposal_id: "vpr_fixture".into(),
            proposal_root: root('2'),
            claim_id: format!("vcl_{}", "3".repeat(64)),
            claim_root: root('4'),
            submission_root: root('5'),
            verification_set_root: root('6'),
            action: "review_accept".into(),
            reason: "Accept the exact fixture evidence.".into(),
            principal_id: "local:fixture".into(),
            observed_at: "2026-07-27T00:00:04Z".into(),
            authority_event_log_root: root('7'),
            policy_bundle_root: root('8'),
            plan_root: String::new(),
        };
        plan.plan_root = plan_root(&plan).unwrap();
        plan
    }

    #[test]
    fn acceptance_requires_exact_independent_passing_evidence() {
        let submission = submission();
        let passing = verification(
            &submission,
            "Replay the frozen verifier.",
            "pass",
            "service:independent-verifier",
            true,
        );
        assert!(verification_satisfies_requirement(
            &submission,
            "Replay the frozen verifier.",
            &passing
        ));
        assert!(require_acceptance_evidence(&submission, &[(root('1'), passing.clone())]).is_ok());

        let wrong_property = verification(
            &submission,
            "Inspect another property.",
            "pass",
            "service:independent-verifier",
            true,
        );
        assert!(!verification_satisfies_requirement(
            &submission,
            "Replay the frozen verifier.",
            &wrong_property
        ));
        assert!(require_acceptance_evidence(&submission, &[(root('2'), wrong_property)]).is_err());
        let producer_record = verification(
            &submission,
            "Replay the frozen verifier.",
            "pass",
            &submission.provenance.producer,
            false,
        );
        assert!(!verification_satisfies_requirement(
            &submission,
            "Replay the frozen verifier.",
            &producer_record
        ));
        assert!(require_acceptance_evidence(&submission, &[(root('3'), producer_record)]).is_err());
        let undeclared = verification(
            &submission,
            "Replay the frozen verifier.",
            "pass",
            "service:independent-verifier",
            false,
        );
        assert!(!verification_satisfies_requirement(
            &submission,
            "Replay the frozen verifier.",
            &undeclared
        ));
        assert!(require_acceptance_evidence(&submission, &[(root('4'), undeclared)]).is_err());
        let failing = verification(
            &submission,
            "Replay the frozen verifier.",
            "fail",
            "service:independent-verifier",
            true,
        );
        assert!(
            require_acceptance_evidence(&submission, &[(root('1'), passing), (root('5'), failing)])
                .is_err()
        );
    }

    #[test]
    fn execution_collision_requires_the_complete_exact_attempt_identity() {
        let mut original = submission();
        original.provenance.source_attempt = Some("attempt_fixture".into());
        let mut refined_wording = original.clone();
        refined_wording.claim.assertion = "A more precise bounded fixture result.".into();
        assert!(same_exact_producer_execution(&original, &refined_wording));

        let mut another_attempt = refined_wording.clone();
        another_attempt.provenance.source_attempt = Some("attempt_other".into());
        assert!(!same_exact_producer_execution(&original, &another_attempt));

        let mut another_artifact = refined_wording;
        another_artifact.artifacts[0].digest = root('b');
        assert!(!same_exact_producer_execution(&original, &another_artifact));
    }

    #[test]
    fn add_accepts_and_rejects_without_legacy_state() {
        let subject = claim("A new bounded result.", 1, Vec::new());
        let proposal = proposal("claim.add", &subject);
        let mut current = repository();
        current
            .pending_claims
            .push(standing(&subject, "pending_review"));

        let accepted = next_repository(
            &current,
            &proposal,
            &subject,
            &subject.canonical_root().unwrap(),
            DecisionAction::Accept,
        )
        .unwrap();
        assert!(accepted.pending_claims.is_empty());
        assert_eq!(accepted.accepted_claims.len(), 1);

        let rejected = next_repository(
            &current,
            &proposal,
            &subject,
            &subject.canonical_root().unwrap(),
            DecisionAction::Reject,
        )
        .unwrap();
        assert!(rejected.pending_claims.is_empty());
        assert!(rejected.accepted_claims.is_empty());
    }

    #[test]
    fn revise_replaces_exactly_one_predecessor() {
        let original = claim("Original bounded result.", 1, Vec::new());
        let replacement = claim(
            "Corrected bounded result.",
            2,
            vec![ClaimRelation {
                kind: "corrects".into(),
                target_claim_id: original.claim_id.clone(),
            }],
        );
        let proposal = proposal("claim.revise", &replacement);
        let mut current = repository();
        current
            .accepted_claims
            .push(standing(&original, "accepted"));
        current
            .pending_claims
            .push(standing(&replacement, "pending_review"));

        let next = next_repository(
            &current,
            &proposal,
            &replacement,
            &replacement.canonical_root().unwrap(),
            DecisionAction::Accept,
        )
        .unwrap();
        assert_eq!(next.accepted_claims.len(), 1);
        assert_eq!(next.accepted_claims[0].claim_id, replacement.claim_id);
        assert!(next.pending_claims.is_empty());
    }

    #[test]
    fn withdrawal_accepts_only_the_exact_accepted_claim() {
        let subject = claim("Accepted bounded result.", 1, Vec::new());
        let proposal = proposal("claim.withdraw", &subject);
        let mut current = repository();
        current.accepted_claims.push(standing(&subject, "accepted"));

        let rejected = next_repository(
            &current,
            &proposal,
            &subject,
            &subject.canonical_root().unwrap(),
            DecisionAction::Reject,
        )
        .unwrap();
        assert_eq!(rejected.accepted_claims.len(), 1);

        let accepted = next_repository(
            &current,
            &proposal,
            &subject,
            &subject.canonical_root().unwrap(),
            DecisionAction::Accept,
        )
        .unwrap();
        assert!(accepted.accepted_claims.is_empty());
    }

    #[test]
    fn decision_plan_root_binds_action_reason_time_and_exact_roots() {
        let baseline = plan();
        for mutate in [
            |plan: &mut CurrentReviewDecisionPlan| plan.action = "review_reject".into(),
            |plan: &mut CurrentReviewDecisionPlan| plan.reason = "Another reason.".into(),
            |plan: &mut CurrentReviewDecisionPlan| plan.observed_at = "2026-07-27T00:00:05Z".into(),
            |plan: &mut CurrentReviewDecisionPlan| plan.claim_root = root('9'),
        ] {
            let mut changed = baseline.clone();
            mutate(&mut changed);
            assert_ne!(plan_root(&changed).unwrap(), baseline.plan_root);
        }
    }

    #[test]
    fn acceptance_links_one_domain_event_to_one_review_event() {
        let subject = claim("A new bounded result.", 1, Vec::new());
        let proposal = proposal("claim.add", &subject);
        let plan = CurrentReviewDecisionPlan {
            claim_id: subject.claim_id.clone(),
            claim_root: subject.canonical_root().unwrap(),
            proposal_id: proposal.proposal_id.clone(),
            ..plan()
        };
        let events = decision_events(
            &plan,
            &repository(),
            &proposal,
            &subject,
            &root('0'),
            DecisionAction::Accept,
            "2026-07-27T00:00:05Z",
        )
        .unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EventKind::ClaimAsserted);
        assert_eq!(events[1].kind, EventKind::ReviewAccepted);
        assert_eq!(
            events[1].payload["applied_event_id"],
            semantic_event_id(&events[0])
        );

        let rejection = decision_events(
            &plan,
            &repository(),
            &proposal,
            &subject,
            &root('0'),
            DecisionAction::Reject,
            "2026-07-27T00:00:05Z",
        )
        .unwrap();
        assert_eq!(rejection.len(), 1);
        assert_eq!(rejection[0].kind, EventKind::ReviewRejected);
    }
}
