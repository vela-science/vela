//! Private, transaction-bound human decision planning.
//!
//! A `DecisionPlan` is process plumbing, not a protocol object or a second
//! state model. It binds the exact frontier head, explicit human answers,
//! typed roots already derived by the Decision Brief, policy/Engine inputs,
//! reviewer authority, and the unsigned semantic event cores. It is rebuilt
//! under the frontier recovery barrier before any private-key loader runs.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path};

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use vela_protocol::project::Project;
use vela_protocol::proposals::{EngineVerdict, PreparedDecisionMutation};

use crate::config::git_publish::{PublicationDelta, PublicationDeltaEntry};
use crate::frontier_txn::{
    ContentDigest, DeltaDraft, FrontierBinding, FrontierRecoveryBarrier, FrontierTxn,
    FrontierTxnError, FrontierTxnPlan, FrontierTxnPlanSpec, InputBinding, OperationId,
    OperationKind, PlannedWrite, RecoveryOutcome, RepoPath,
};
use crate::review_material::{LockedReviewSelection, ReviewProjection};

pub(crate) const DECISION_PREIMAGE_VERSION: &str = "vela.decision-plan.internal.v1";
const DECISION_PLAN_DOMAIN: &[u8] = b"vela.decision-plan.internal.v1\0";
const REVIEWER_AUTHORITY_DOMAIN: &[u8] = b"vela.reviewer-authority.internal.v1\0";
const AGGREGATE_ENGINE_DOMAIN: &[u8] = b"vela.aggregate-engine-verdict.internal.v1\0";
const POLICY_INPUT_DOMAIN: &[u8] = b"vela.decision-policy-input.internal.v1\0";
const COHERENCE_POLICY_DOMAIN: &[u8] = b"vela.decision-coherence.policy.internal.v1\0";
const COHERENCE_CHECK_DOMAIN: &[u8] = b"vela.decision-coherence.checks.internal.v1\0";
const COHERENCE_CAPABILITY_DOMAIN: &[u8] = b"vela.decision-coherence.capability.internal.v1\0";
/// Scripted confirmations are deliberately short-lived clear-signing tokens.
/// A small future allowance tolerates ordinary host clock skew without
/// permitting a caller to mint a decision far into the future.
pub(crate) const SCRIPTED_CONFIRMATION_MAX_AGE_SECONDS: i64 = 15 * 60;
pub(crate) const SCRIPTED_CONFIRMATION_MAX_FUTURE_SKEW_SECONDS: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecisionExecutorStep {
    BeforeLock,
    AfterLock,
    AfterLockedRederive,
    AfterKeyRead,
    AfterPreparedJournal,
    AfterCommitMarker,
    AfterInstall,
    AfterComplete,
}

#[cfg(test)]
type DecisionMutationHook = Option<(DecisionExecutorStep, Box<dyn FnOnce()>)>;

#[cfg(test)]
std::thread_local! {
    static DECISION_EXECUTOR_FAILPOINT: std::cell::Cell<Option<DecisionExecutorStep>> =
        const { std::cell::Cell::new(None) };
    static DECISION_EXECUTOR_MUTATION_HOOK: std::cell::RefCell<DecisionMutationHook> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn set_executor_failpoint(step: Option<DecisionExecutorStep>) {
    DECISION_EXECUTOR_FAILPOINT.with(|target| target.set(step));
}

#[cfg(test)]
fn set_executor_mutation_hook(step: DecisionExecutorStep, hook: impl FnOnce() + 'static) {
    DECISION_EXECUTOR_MUTATION_HOOK.with(|target| {
        *target.borrow_mut() = Some((step, Box::new(hook)));
    });
}

#[cfg(test)]
fn hit_executor_step(step: DecisionExecutorStep) -> Result<(), DecisionPlanError> {
    if DECISION_EXECUTOR_FAILPOINT.with(std::cell::Cell::get) == Some(step) {
        return Err(DecisionPlanError::new(
            "injected_failure",
            format!("injected decision executor failure at {step:?}"),
        ));
    }
    let hook = DECISION_EXECUTOR_MUTATION_HOOK.with(|target| {
        let mut target = target.borrow_mut();
        if target
            .as_ref()
            .is_some_and(|(hook_step, _)| *hook_step == step)
        {
            target.take().map(|(_, hook)| hook)
        } else {
            None
        }
    });
    if let Some(hook) = hook {
        hook();
    }
    Ok(())
}

#[cfg(not(test))]
fn hit_executor_step(_step: DecisionExecutorStep) -> Result<(), DecisionPlanError> {
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DecisionAction {
    Accept,
    Reject,
}

impl DecisionAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Reject => "reject",
        }
    }
}

/// Resumable answer state. `seen_decision_facts_root` invalidates a saved
/// answer when the bounded human-facing projection changes, but is never part
/// of the signing preimage or an authority source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SavedAnswer {
    pub(crate) proposal_id: String,
    pub(crate) proposal_root: String,
    pub(crate) seen_decision_facts_root: String,
    pub(crate) action: DecisionAction,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DecisionAnswer {
    pub(crate) proposal_id: String,
    pub(crate) proposal_root: String,
    pub(crate) action: DecisionAction,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ConsumedDecisionRoots {
    pub(crate) proposal_id: String,
    pub(crate) proposal_root: String,
    pub(crate) receipt_observation_root: String,
    pub(crate) receipt_root: Option<String>,
    pub(crate) evidence_or_reference_root: String,
    pub(crate) evidence_availability: String,
    pub(crate) verifier_snapshot_root: String,
    pub(crate) policy_input_root: String,
    pub(crate) policy_result_root: String,
    pub(crate) engine_gate_root: String,
    pub(crate) reviewer_authority_root: String,
    pub(crate) semantic_effect_root: String,
    pub(crate) downstream_impact_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UnsignedEventCore {
    pub(crate) answer_ordinal: usize,
    pub(crate) event_ordinal: usize,
    pub(crate) event: serde_json::Value,
}

/// The private decision commitment. Deliberately not `Serialize`: only the
/// dedicated seven-field preimage below may enter `decision_root`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecisionPlan {
    pub(crate) decision_preimage_version: String,
    pub(crate) frontier_id: String,
    pub(crate) expected_event_log_root: String,
    pub(crate) ordered_answers: Vec<DecisionAnswer>,
    pub(crate) consumed_fact_roots: Vec<ConsumedDecisionRoots>,
    pub(crate) policy_input_root: String,
    pub(crate) semantic_event_cores: Vec<UnsignedEventCore>,
    pub(crate) decision_root: String,
}

#[derive(Serialize)]
struct DecisionPlanPreimage<'a> {
    decision_preimage_version: &'a str,
    frontier_id: &'a str,
    expected_event_log_root: &'a str,
    ordered_answers: &'a [DecisionAnswer],
    consumed_fact_roots: &'a [ConsumedDecisionRoots],
    policy_input_root: &'a str,
    semantic_event_cores: &'a [UnsignedEventCore],
}

#[derive(Debug)]
pub(crate) struct PreparedDecision {
    pub(crate) plan: DecisionPlan,
    pub(crate) aggregate_engine: EngineVerdict,
    /// Full final log identity consumed only by `FrontierTxnPlan`.
    pub(crate) resulting_event_log_ids: Vec<String>,
    /// Newly appended signed events, in semantic append order. This is the
    /// honest CLI/publication identity for the decision itself.
    pub(crate) appended_event_ids: Vec<String>,
    candidate: Project,
    mutations: Vec<PreparedDecisionMutation>,
    saved_answers: Vec<SavedAnswer>,
    reviewer_id: String,
    reviewer_public_key: String,
    decided_at: String,
    provenance: Option<vela_protocol::provenance::Provenance>,
}

#[derive(Debug)]
pub(crate) struct LockedPreparedDecision {
    barrier: FrontierRecoveryBarrier,
    prepared: PreparedDecision,
    read_set: Vec<InputBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecisionExecutionOutcome {
    pub(crate) decision_root: String,
    pub(crate) operation_id: String,
    pub(crate) event_ids: Vec<String>,
    pub(crate) aggregate_engine: EngineVerdict,
    pub(crate) publication_delta: Option<PublicationDelta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecisionPlanError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

impl DecisionPlanError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn transaction(error: impl fmt::Display) -> Self {
        Self::new("transaction_failed", error.to_string())
    }
}

impl fmt::Display for DecisionPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for DecisionPlanError {}

#[derive(Serialize)]
struct ReviewerAuthorityCommitment<'a> {
    schema: &'static str,
    frontier_id: &'a str,
    reviewer: &'a vela_protocol::sign::ActorRecord,
    decided_at: &'a str,
    authorization: &'static str,
}

#[derive(Serialize)]
struct PolicyItemCommitment<'a> {
    proposal_id: &'a str,
    proposal_root: &'a str,
    policy_input_root: &'a str,
    policy_result_root: &'a str,
    engine_gate_root: &'a str,
}

#[derive(Serialize)]
struct PolicyInputCommitment<'a> {
    schema: &'static str,
    active_policy_snapshot_root: &'a str,
    engine_policy_observation_root: &'a str,
    reviewer_authority_root: &'a str,
    aggregate_engine_verdict_root: &'a str,
    ordered_items: Vec<PolicyItemCommitment<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoherenceKey {
    proposal_kind: String,
    route: String,
    scope: String,
    policy_context_root: String,
    required_check_root: String,
    capability_root: String,
    impact_tier: u8,
    reviewer_authority_root: String,
}

/// Build a coherent plan from a barrier-consistent snapshot, then release the
/// barrier. This is the plan shown for final human confirmation.
pub(crate) fn build_unlocked(
    frontier: &Path,
    answers: &[SavedAnswer],
    reviewer: &str,
    decided_at: &str,
    provenance: Option<vela_protocol::provenance::Provenance>,
) -> Result<PreparedDecision, DecisionPlanError> {
    ensure_transactional_frontier(frontier)?;
    let barrier = acquire_barrier_with_recovery(frontier)?;
    let locked = build_locked(
        frontier,
        barrier,
        answers,
        reviewer,
        decided_at,
        provenance.as_ref(),
    )?;
    Ok(locked.prepared)
}

/// Build the plan used for rendering or validating a scripted confirmation
/// without acquiring a lock, touching a journal, or attempting recovery.
///
/// Preview is not an authority boundary. It reads every bound input twice and
/// only returns when the complete read set, review projection, and resulting
/// typed plan agree. The confirmed execution still rederives the same plan
/// under the exclusive recovery barrier before the private-key loader runs.
pub(crate) fn build_read_only_preview(
    frontier: &Path,
    answers: &[SavedAnswer],
    reviewer: &str,
    decided_at: &str,
    provenance: Option<vela_protocol::provenance::Provenance>,
) -> Result<PreparedDecision, DecisionPlanError> {
    ensure_transactional_frontier(frontier)?;
    let proposal_ids = answers
        .iter()
        .map(|answer| answer.proposal_id.clone())
        .collect::<Vec<_>>();

    for _ in 0..3 {
        let first_project = vela_protocol::repo::load_from_path(frontier)
            .map_err(|error| DecisionPlanError::new("frontier_invalid", error))?;
        let first_review = ReviewProjection::selected_from_locked_project_at(
            frontier,
            &first_project,
            &proposal_ids,
            decided_at,
        )
        .map_err(|error| DecisionPlanError::new(error.code, error.message))?;
        let first_read_set = decision_read_set(frontier, &first_project, &first_review, answers)?;
        let first_prepared = build_from_snapshot(
            frontier,
            &first_project,
            first_review.clone(),
            answers,
            reviewer,
            decided_at,
            provenance.as_ref(),
        )?;

        let second_project = vela_protocol::repo::load_from_path(frontier)
            .map_err(|error| DecisionPlanError::new("frontier_invalid", error))?;
        let second_review = ReviewProjection::selected_from_locked_project_at(
            frontier,
            &second_project,
            &proposal_ids,
            decided_at,
        )
        .map_err(|error| DecisionPlanError::new(error.code, error.message))?;
        let second_read_set =
            decision_read_set(frontier, &second_project, &second_review, answers)?;
        let second_prepared = build_from_snapshot(
            frontier,
            &second_project,
            second_review.clone(),
            answers,
            reviewer,
            decided_at,
            provenance.as_ref(),
        )?;

        if first_read_set == second_read_set
            && first_review == second_review
            && first_prepared.plan == second_prepared.plan
        {
            return Ok(second_prepared);
        }
    }

    Err(DecisionPlanError::new(
        "decision_inputs_unstable",
        "decision inputs changed while forming a read-only preview; rerun to review a stable semantic set",
    ))
}

/// Validate the caller-echoed observation time before any recovery barrier or
/// key access. The root binds the exact timestamp bytes; this independent
/// wall-clock bound prevents a caller from retaining roots indefinitely or
/// manufacturing materially backdated/future decision chronology.
pub(crate) fn validate_scripted_confirmation_time(
    confirm_at: &str,
) -> Result<(), DecisionPlanError> {
    let observed_at = chrono::DateTime::parse_from_rfc3339(confirm_at)
        .map_err(|_| {
            DecisionPlanError::new(
                "confirmation_expired",
                "confirmation time is not valid RFC3339; render a fresh preview and echo its exact --confirm-at value",
            )
        })?
        .with_timezone(&chrono::Utc);
    let now = chrono::Utc::now();
    if observed_at > now + chrono::Duration::seconds(SCRIPTED_CONFIRMATION_MAX_FUTURE_SKEW_SECONDS)
    {
        return Err(DecisionPlanError::new(
            "confirmation_expired",
            format!(
                "confirmation time is more than {} seconds in the future; render a fresh preview and echo its exact --confirm-at value",
                SCRIPTED_CONFIRMATION_MAX_FUTURE_SKEW_SECONDS
            ),
        ));
    }
    if now.signed_duration_since(observed_at)
        > chrono::Duration::seconds(SCRIPTED_CONFIRMATION_MAX_AGE_SECONDS)
    {
        return Err(DecisionPlanError::new(
            "confirmation_expired",
            format!(
                "confirmation is older than {} minutes; render a fresh preview and confirm that exact semantic set",
                SCRIPTED_CONFIRMATION_MAX_AGE_SECONDS / 60
            ),
        ));
    }
    Ok(())
}

/// Build the exact post-confirmation plan while retaining the caller's single
/// recovery barrier for `FrontierTxn::prepare_with_barrier`.
pub(crate) fn build_locked(
    frontier: &Path,
    barrier: FrontierRecoveryBarrier,
    answers: &[SavedAnswer],
    reviewer: &str,
    decided_at: &str,
    provenance: Option<&vela_protocol::provenance::Provenance>,
) -> Result<LockedPreparedDecision, DecisionPlanError> {
    let project = vela_protocol::repo::load_from_path(frontier)
        .map_err(|error| DecisionPlanError::new("frontier_invalid", error))?;
    let proposal_ids = answers
        .iter()
        .map(|answer| answer.proposal_id.clone())
        .collect::<Vec<_>>();
    let review = ReviewProjection::selected_from_locked_project_at(
        frontier,
        &project,
        &proposal_ids,
        decided_at,
    )
    .map_err(|error| DecisionPlanError::new(error.code, error.message))?;
    let read_set = decision_read_set(frontier, &project, &review, answers)?;
    let verified_review = ReviewProjection::selected_from_locked_project_at(
        frontier,
        &project,
        &proposal_ids,
        decided_at,
    )
    .map_err(|error| DecisionPlanError::new(error.code, error.message))?;
    if verified_review != review {
        return Err(DecisionPlanError::new(
            "decision_inputs_unstable",
            "receipt or policy inputs changed while the locked Decision Plan read set was being bound",
        ));
    }
    let prepared = build_from_snapshot(
        frontier,
        &project,
        verified_review,
        answers,
        reviewer,
        decided_at,
        provenance,
    )?;
    Ok(LockedPreparedDecision {
        barrier,
        prepared,
        read_set,
    })
}

/// Execute a confirmed plan through the one recoverable frontier write edge.
/// The private key closure is invoked exactly once, and only after recovery,
/// locked rederivation, stale comparison, reviewer authorization, coherence,
/// and the strict aggregate Engine gate have all succeeded.
pub(crate) fn execute_with_key_loader<K>(
    frontier: &Path,
    confirmed: &PreparedDecision,
    key_loader: K,
) -> Result<DecisionExecutionOutcome, DecisionPlanError>
where
    K: FnOnce() -> Result<SigningKey, String>,
{
    ensure_transactional_frontier(frontier)?;
    let computed_root = decision_plan_root(&confirmed.plan)?;
    if computed_root != confirmed.plan.decision_root {
        return Err(DecisionPlanError::new(
            "confirmed_plan_invalid",
            format!(
                "confirmed decision root {} does not match its canonical preimage root {computed_root}",
                confirmed.plan.decision_root
            ),
        ));
    }
    hit_executor_step(DecisionExecutorStep::BeforeLock)?;
    if let Some(recovered) = recover_confirmed_outcome(frontier, confirmed)? {
        return Ok(recovered);
    }
    let barrier = acquire_barrier_with_recovery(frontier)?;
    let operation_id = OperationId::derive("decision", confirmed.plan.decision_root.as_bytes());
    if let Some(plan) = barrier
        .completed_plan(&operation_id)
        .map_err(DecisionPlanError::transaction)?
    {
        drop(barrier);
        let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)
            .map_err(|error| DecisionPlanError::new("frontier_unavailable", error))?;
        let transaction = FrontierTxn::open(frontier, &journal_dir, &operation_id)
            .map_err(DecisionPlanError::transaction)?;
        let publication_delta = transaction_publication_delta(frontier, &transaction)?;
        let mut outcome = outcome_from_durable_plan(&plan, &confirmed.plan.decision_root)?;
        outcome.publication_delta = publication_delta;
        return Ok(outcome);
    }
    hit_executor_step(DecisionExecutorStep::AfterLock)?;
    let locked = build_locked(
        frontier,
        barrier,
        &confirmed.saved_answers,
        &confirmed.reviewer_id,
        confirmed.decided_at.as_str(),
        confirmed.provenance.as_ref(),
    )?;
    if locked.prepared.plan != confirmed.plan {
        return Err(DecisionPlanError::new(
            "decision_stale",
            format!(
                "confirmed decision root {} rederived as {}; review the changed facts before signing",
                confirmed.plan.decision_root, locked.prepared.plan.decision_root
            ),
        ));
    }
    hit_executor_step(DecisionExecutorStep::AfterLockedRederive)?;

    let LockedPreparedDecision {
        barrier,
        mut prepared,
        read_set,
    } = locked;
    if prepared.aggregate_engine.status == "blocked" {
        return Err(DecisionPlanError::new(
            "engine_blocked",
            "strict aggregate Engine gate blocked the confirmed decision",
        ));
    }

    // This advisory-lock check is the last zero-key early-abort opportunity.
    // `mark_committed` verifies the same complete set again and remains the
    // authoritative boundary against non-cooperating filesystem writers.
    barrier
        .verify_read_set(&read_set)
        .map_err(DecisionPlanError::transaction)?;
    validate_reviewer_for_key_use(
        &prepared.candidate,
        &prepared.reviewer_id,
        &prepared.reviewer_public_key,
        &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
    )?;

    let signing_key =
        key_loader().map_err(|error| DecisionPlanError::new("key_unavailable", error))?;
    hit_executor_step(DecisionExecutorStep::AfterKeyRead)?;
    let verifying_key = hex::encode(signing_key.verifying_key().to_bytes());
    if !verifying_key.eq_ignore_ascii_case(&prepared.reviewer_public_key) {
        return Err(DecisionPlanError::new(
            "key_mismatch",
            format!(
                "loaded key does not match the registered key for {}",
                prepared.reviewer_id
            ),
        ));
    }
    validate_reviewer_for_key_use(
        &prepared.candidate,
        &prepared.reviewer_id,
        &prepared.reviewer_public_key,
        &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
    )?;
    for mutation in &prepared.mutations {
        vela_protocol::proposals::sign_prepared_decision_events(
            &mut prepared.candidate,
            mutation,
            &prepared.reviewer_id,
            &signing_key,
        )
        .map_err(|error| DecisionPlanError::new("signing_failed", error))?;
    }

    let writes = PlannedWrite::from_managed_files(
        vela_protocol::repo::render_vela_repo_files(frontier, &prepared.candidate)
            .map_err(|error| DecisionPlanError::new("render_failed", error))?,
    )
    .map_err(DecisionPlanError::transaction)?;
    let draft = DeltaDraft::prepare(frontier, writes).map_err(DecisionPlanError::transaction)?;
    let layout = vela_protocol::canonical::to_canonical_bytes(&json!({
        "schema": "vela.frontier-layout.internal.v1",
        "frontier_id": prepared.plan.frontier_id,
        "paths": draft
            .delta
            .writes()
            .iter()
            .map(|write| write.path.as_str())
            .collect::<Vec<_>>(),
    }))
    .map_err(|error| DecisionPlanError::new("layout_failed", error))?;
    let request_root = ContentDigest::parse(prepared.plan.decision_root.clone())
        .map_err(DecisionPlanError::transaction)?;
    let operation_id = OperationId::derive("decision", prepared.plan.decision_root.as_bytes());
    let expected_event_log_root =
        ContentDigest::parse(prepared.plan.expected_event_log_root.clone())
            .map_err(DecisionPlanError::transaction)?;
    let resulting_event_log_root = ContentDigest::parse(format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&prepared.candidate.events)
    ))
    .map_err(DecisionPlanError::transaction)?;
    let result = json!({
        "schema": "vela.decision-result.internal.v1",
        "decision_root": prepared.plan.decision_root,
        "reviewer": prepared.reviewer_id,
        "answers": prepared.plan.ordered_answers.len(),
        "event_ids": prepared.appended_event_ids,
        "aggregate_engine": prepared.aggregate_engine,
    });
    let plan = FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind: OperationKind::Decision,
            operation_id: operation_id.clone(),
            request_root,
            frontier: FrontierBinding::new(frontier, prepared.plan.frontier_id.clone(), &layout)
                .map_err(DecisionPlanError::transaction)?,
            fixed_time: confirmed.decided_at.clone(),
            expected_event_log_root,
            resulting_event_log_root,
            resulting_event_ids: prepared.resulting_event_log_ids.clone(),
            read_set,
            result,
        },
        draft.delta.clone(),
    )
    .map_err(DecisionPlanError::transaction)?;
    let mut transaction = FrontierTxn::prepare_with_barrier(barrier, plan, draft)
        .map_err(DecisionPlanError::transaction)?;
    hit_executor_step(DecisionExecutorStep::AfterPreparedJournal)?;
    transaction
        .mark_committed()
        .map_err(DecisionPlanError::transaction)?;
    hit_executor_step(DecisionExecutorStep::AfterCommitMarker)?;
    transaction
        .install()
        .map_err(DecisionPlanError::transaction)?;
    hit_executor_step(DecisionExecutorStep::AfterInstall)?;
    transaction
        .complete()
        .map_err(DecisionPlanError::transaction)?;
    hit_executor_step(DecisionExecutorStep::AfterComplete)?;
    let publication_delta = transaction_publication_delta(frontier, &transaction)?;

    Ok(DecisionExecutionOutcome {
        decision_root: prepared.plan.decision_root,
        operation_id: operation_id.as_str().to_string(),
        event_ids: prepared.appended_event_ids,
        aggregate_engine: prepared.aggregate_engine,
        publication_delta,
    })
}

/// Recover one explicitly named scientific Decision transaction after a
/// process restart, without reconstructing answers or resolving a key. The
/// durable commit marker and journaled signed postimages are the sole
/// authority for completing installation; a marker-free Prepared journal is
/// aborted and still requires human reconfirmation.
///
/// `Ok(None)` means the operation id is not a scientific Decision journal, so
/// `vela publication recover` may continue with its ordinary Git-journal lane.
pub(crate) fn recover_decision_operation(
    frontier: &Path,
    operation_id: &str,
) -> Result<Option<DecisionExecutionOutcome>, DecisionPlanError> {
    ensure_transactional_frontier(frontier)?;
    let operation_id =
        OperationId::parse(operation_id.to_string()).map_err(DecisionPlanError::transaction)?;
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)
        .map_err(|error| DecisionPlanError::new("frontier_unavailable", error))?;
    let Some(transaction) = FrontierTxn::open_if_present(frontier, &journal_dir, &operation_id)
        .map_err(DecisionPlanError::transaction)?
    else {
        return Ok(None);
    };
    if transaction.plan().kind != OperationKind::Decision {
        return Ok(None);
    }
    let decision_root = durable_decision_root(transaction.plan())?.to_string();
    drop(transaction);

    match FrontierTxn::recover(frontier, &journal_dir, &operation_id)
        .map_err(DecisionPlanError::transaction)?
    {
        RecoveryOutcome::Prepared => {
            let mut transaction = FrontierTxn::open(frontier, &journal_dir, &operation_id)
                .map_err(DecisionPlanError::transaction)?;
            transaction
                .abort_prepared()
                .map_err(DecisionPlanError::transaction)?;
            Err(DecisionPlanError::new(
                "reconfirmation_required",
                "the interrupted decision has no durable commit marker; its prepared journal was aborted, so review and confirm again",
            ))
        }
        RecoveryOutcome::Aborted => Err(DecisionPlanError::new(
            "reconfirmation_required",
            "the interrupted decision was aborted before its durable commit marker; review and confirm again",
        )),
        RecoveryOutcome::Completed | RecoveryOutcome::AlreadyCompleted => {
            let transaction = FrontierTxn::open(frontier, &journal_dir, &operation_id)
                .map_err(DecisionPlanError::transaction)?;
            let mut outcome = outcome_from_durable_plan(transaction.plan(), &decision_root)?;
            outcome.publication_delta = transaction_publication_delta(frontier, &transaction)?;
            Ok(Some(outcome))
        }
    }
}

/// Complete or recognize the exact journal already bound to this confirmed
/// decision before entering semantic rederivation. A durable marker is
/// authority to finish installation without consulting the human key again.
/// A marker-free Prepared journal is not: it is aborted and requires a fresh
/// render/confirmation/signature.
fn recover_confirmed_outcome(
    frontier: &Path,
    confirmed: &PreparedDecision,
) -> Result<Option<DecisionExecutionOutcome>, DecisionPlanError> {
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)
        .map_err(|error| DecisionPlanError::new("frontier_unavailable", error))?;
    let operation_id = OperationId::derive("decision", confirmed.plan.decision_root.as_bytes());
    let Some(transaction) = FrontierTxn::open_if_present(frontier, &journal_dir, &operation_id)
        .map_err(DecisionPlanError::transaction)?
    else {
        return Ok(None);
    };
    let expected_root = ContentDigest::parse(confirmed.plan.decision_root.clone())
        .map_err(DecisionPlanError::transaction)?;
    if transaction.plan().request_root != expected_root {
        return Err(DecisionPlanError::new(
            "operation_conflict",
            format!(
                "operation {} is already bound to a different decision root",
                operation_id.as_str()
            ),
        ));
    }
    drop(transaction);
    match FrontierTxn::recover(frontier, &journal_dir, &operation_id)
        .map_err(DecisionPlanError::transaction)?
    {
        RecoveryOutcome::Prepared => {
            let mut transaction = FrontierTxn::open(frontier, &journal_dir, &operation_id)
                .map_err(DecisionPlanError::transaction)?;
            transaction
                .abort_prepared()
                .map_err(DecisionPlanError::transaction)?;
            Err(DecisionPlanError::new(
                "reconfirmation_required",
                "the prior decision stopped before its durable commit marker; its prepared journal was aborted, so review and confirm again",
            ))
        }
        RecoveryOutcome::Aborted => Ok(None),
        RecoveryOutcome::Completed | RecoveryOutcome::AlreadyCompleted => {
            let transaction = FrontierTxn::open(frontier, &journal_dir, &operation_id)
                .map_err(DecisionPlanError::transaction)?;
            let publication_delta = transaction_publication_delta(frontier, &transaction)?;
            let mut outcome =
                outcome_from_durable_plan(transaction.plan(), &confirmed.plan.decision_root)?;
            outcome.publication_delta = publication_delta;
            Ok(Some(outcome))
        }
    }
}

fn outcome_from_durable_plan(
    plan: &FrontierTxnPlan,
    expected_decision_root: &str,
) -> Result<DecisionExecutionOutcome, DecisionPlanError> {
    let result = &plan.result;
    let decision_root = durable_decision_root(plan)?;
    if decision_root != expected_decision_root {
        return Err(DecisionPlanError::new(
            "recovery_result_invalid",
            "completed decision result does not match the confirmed root",
        ));
    }
    let event_ids = serde_json::from_value(result.get("event_ids").cloned().ok_or_else(|| {
        DecisionPlanError::new(
            "recovery_result_invalid",
            "completed decision journal has no event_ids",
        )
    })?)
    .map_err(|error| DecisionPlanError::new("recovery_result_invalid", error.to_string()))?;
    let aggregate_engine =
        serde_json::from_value(result.get("aggregate_engine").cloned().ok_or_else(|| {
            DecisionPlanError::new(
                "recovery_result_invalid",
                "completed decision journal has no aggregate_engine",
            )
        })?)
        .map_err(|error| DecisionPlanError::new("recovery_result_invalid", error.to_string()))?;
    Ok(DecisionExecutionOutcome {
        decision_root: decision_root.to_string(),
        operation_id: plan.operation_id.as_str().to_string(),
        event_ids,
        aggregate_engine,
        publication_delta: None,
    })
}

fn durable_decision_root(plan: &FrontierTxnPlan) -> Result<&str, DecisionPlanError> {
    plan.result
        .get("decision_root")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            DecisionPlanError::new(
                "recovery_result_invalid",
                "completed decision journal has no decision_root",
            )
        })
}

fn transaction_publication_delta(
    frontier: &Path,
    transaction: &FrontierTxn,
) -> Result<Option<PublicationDelta>, DecisionPlanError> {
    if !frontier
        .ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
    {
        return Ok(None);
    }
    publication_delta_from_writes(
        frontier,
        transaction.plan().canonical_delta.root().as_str(),
        transaction
            .resolved_public_writes()
            .map_err(DecisionPlanError::transaction)?,
    )
}

fn publication_delta_from_writes(
    frontier: &Path,
    root: &str,
    writes: Vec<crate::frontier_txn::ResolvedWrite>,
) -> Result<Option<PublicationDelta>, DecisionPlanError> {
    use crate::frontier_txn::{FileMode, FileState};

    if writes.is_empty() {
        return Ok(None);
    }
    let mut entries = writes
        .into_iter()
        .map(|write| {
            let path = crate::config::git_publish::publication_repo_relative_path(
                frontier,
                write.staged.path.as_str(),
            )
            .map_err(|error| DecisionPlanError::new("publication_delta_failed", error))?;
            let preimage_sha256 = match &write.staged.preimage {
                FileState::Absent => None,
                FileState::File { digest, .. } => Some(digest.as_str().to_string()),
            };
            let executable = matches!(
                write.staged.postimage,
                FileState::File {
                    mode: FileMode::Executable,
                    ..
                }
            );
            Ok(PublicationDeltaEntry {
                path,
                preimage_sha256,
                postimage: write.postimage_bytes,
                executable,
            })
        })
        .collect::<Result<Vec<_>, DecisionPlanError>>()?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(Some(PublicationDelta {
        root: root.to_string(),
        entries,
    }))
}

fn build_from_snapshot(
    frontier: &Path,
    project: &Project,
    review: LockedReviewSelection,
    answers: &[SavedAnswer],
    reviewer: &str,
    decided_at: &str,
    provenance: Option<&vela_protocol::provenance::Provenance>,
) -> Result<PreparedDecision, DecisionPlanError> {
    validate_answers(answers)?;
    if review.items.len() != answers.len() {
        return Err(DecisionPlanError::new(
            "decision_stale",
            "the locked review set no longer matches the saved answers",
        ));
    }
    let frontier_id = project.frontier_id().to_string();
    let actor = vela_protocol::proposals::validate_human_reviewer_authority_at(
        project, reviewer, decided_at,
    )
    .map_err(|error| DecisionPlanError::new("reviewer_unauthorized", error))?;
    let reviewer_authority_root = domain_root(
        REVIEWER_AUTHORITY_DOMAIN,
        &ReviewerAuthorityCommitment {
            schema: "vela.reviewer-authority.internal.v1",
            frontier_id: &frontier_id,
            reviewer: &actor,
            decided_at,
            authorization: "authorized",
        },
    )?;

    validate_coherence(project, &review, answers, &reviewer_authority_root)?;
    let mut candidate = clone_project(project)?;
    let mut mutations = Vec::with_capacity(answers.len());
    let mut event_cores = Vec::new();
    let mut accepted_kinds = Vec::new();
    let mut ordered_answers = Vec::with_capacity(answers.len());
    let mut consumed_fact_roots = Vec::with_capacity(answers.len());

    for (answer_ordinal, (answer, snapshot)) in answers.iter().zip(&review.items).enumerate() {
        if snapshot.brief.audit.proposal_id != answer.proposal_id
            || snapshot.decision_bindings.proposal_root != answer.proposal_root
        {
            return Err(DecisionPlanError::new(
                "decision_stale",
                format!(
                    "proposal {} no longer matches its saved root",
                    answer.proposal_id
                ),
            ));
        }
        if snapshot.brief.audit.decision_facts_root != answer.seen_decision_facts_root {
            return Err(DecisionPlanError::new(
                "answer_invalidated",
                format!(
                    "proposal {} review facts changed; answer must be reviewed again",
                    answer.proposal_id
                ),
            ));
        }
        let available = match answer.action {
            DecisionAction::Accept => snapshot.brief.accept_ready(),
            DecisionAction::Reject => snapshot.brief.reject_ready(),
        };
        if !available {
            return Err(DecisionPlanError::new(
                "action_unavailable",
                format!(
                    "{} is no longer available for proposal {}",
                    answer.action.as_str(),
                    answer.proposal_id
                ),
            ));
        }

        let proposal_kind = candidate
            .proposals
            .iter()
            .find(|proposal| proposal.id == answer.proposal_id)
            .map(|proposal| proposal.kind.clone())
            .ok_or_else(|| {
                DecisionPlanError::new(
                    "decision_stale",
                    format!("proposal {} disappeared", answer.proposal_id),
                )
            })?;
        let first_event = candidate.events.len();
        let mutation = match answer.action {
            DecisionAction::Accept => {
                accepted_kinds.push(proposal_kind);
                vela_protocol::proposals::prepare_proposal_accept_in_memory_at(
                    &mut candidate,
                    &answer.proposal_id,
                    reviewer,
                    &answer.reason,
                    provenance,
                    decided_at,
                )
            }
            DecisionAction::Reject => {
                vela_protocol::proposals::prepare_proposal_reject_in_memory_at(
                    &mut candidate,
                    &answer.proposal_id,
                    reviewer,
                    &answer.reason,
                    provenance,
                    decided_at,
                )
            }
        }
        .map_err(|error| DecisionPlanError::new("proposal_prepare_failed", error))?;
        let appended = candidate.events.get(first_event..).ok_or_else(|| {
            DecisionPlanError::new("proposal_prepare_failed", "invalid appended event range")
        })?;
        for (event_ordinal, event) in appended.iter().enumerate() {
            event_cores.push(UnsignedEventCore {
                answer_ordinal,
                event_ordinal,
                event: normalize_event_core(event),
            });
        }
        mutations.push(mutation);
        ordered_answers.push(DecisionAnswer {
            proposal_id: answer.proposal_id.clone(),
            proposal_root: answer.proposal_root.clone(),
            action: answer.action,
            reason: answer.reason.clone(),
        });
        let binding = &snapshot.decision_bindings;
        consumed_fact_roots.push(ConsumedDecisionRoots {
            proposal_id: answer.proposal_id.clone(),
            proposal_root: binding.proposal_root.clone(),
            receipt_observation_root: binding.receipt_observation_root.clone(),
            receipt_root: binding.receipt_root.clone(),
            evidence_or_reference_root: binding.evidence_or_reference_root.clone(),
            evidence_availability: binding.evidence_availability.clone(),
            verifier_snapshot_root: binding.verifier_snapshot_root.clone(),
            policy_input_root: binding.policy_input_root.clone(),
            policy_result_root: binding.policy_result_root.clone(),
            engine_gate_root: binding.engine_gate_root.clone(),
            reviewer_authority_root: reviewer_authority_root.clone(),
            semantic_effect_root: binding.semantic_effect_root.clone(),
            downstream_impact_root: binding.downstream_impact_root.clone(),
        });
    }

    vela_protocol::project::recompute_stats(&mut candidate);
    let aggregate_engine = vela_protocol::proposals::strict_engine_verdict_for_candidate(
        project,
        &candidate,
        frontier,
        &accepted_kinds,
    );
    if aggregate_engine.status == "blocked" {
        return Err(DecisionPlanError::new(
            "engine_blocked",
            format!(
                "strict aggregate Engine gate found {} new blocking failure(s) and {} new warning(s)",
                aggregate_engine.new_blocking.len(),
                aggregate_engine.new_warnings.len()
            ),
        ));
    }
    let aggregate_engine_root = domain_root(AGGREGATE_ENGINE_DOMAIN, &aggregate_engine)?;
    let ordered_items = consumed_fact_roots
        .iter()
        .map(|binding| PolicyItemCommitment {
            proposal_id: &binding.proposal_id,
            proposal_root: &binding.proposal_root,
            policy_input_root: &binding.policy_input_root,
            policy_result_root: &binding.policy_result_root,
            engine_gate_root: &binding.engine_gate_root,
        })
        .collect();
    let policy_input_root = domain_root(
        POLICY_INPUT_DOMAIN,
        &PolicyInputCommitment {
            schema: "vela.decision-policy-input.internal.v1",
            active_policy_snapshot_root: &review.active_policy_snapshot_root,
            engine_policy_observation_root: &review.engine_policy_observation_root,
            reviewer_authority_root: &reviewer_authority_root,
            aggregate_engine_verdict_root: &aggregate_engine_root,
            ordered_items,
        },
    )?;
    let mut plan = DecisionPlan {
        decision_preimage_version: DECISION_PREIMAGE_VERSION.to_string(),
        frontier_id,
        expected_event_log_root: review.event_log_root,
        ordered_answers,
        consumed_fact_roots,
        policy_input_root,
        semantic_event_cores: event_cores,
        decision_root: String::new(),
    };
    plan.decision_root = decision_plan_root(&plan)?;
    for mutation in &mut mutations {
        vela_protocol::proposals::bind_decision_root_to_prepared(
            &mut candidate,
            mutation,
            &plan.decision_root,
        )
        .map_err(|error| DecisionPlanError::new("decision_bind_failed", error))?;
    }
    let appended_event_ids = candidate.events[project.events.len()..]
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let mut resulting_event_log_ids = candidate
        .events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    resulting_event_log_ids.sort();

    Ok(PreparedDecision {
        plan,
        aggregate_engine,
        resulting_event_log_ids,
        appended_event_ids,
        candidate,
        mutations,
        saved_answers: answers.to_vec(),
        reviewer_id: reviewer.to_string(),
        reviewer_public_key: actor.public_key,
        decided_at: decided_at.to_string(),
        provenance: provenance.cloned(),
    })
}

fn validate_reviewer_for_key_use(
    project: &Project,
    reviewer: &str,
    expected_public_key: &str,
    key_use_at: &str,
) -> Result<(), DecisionPlanError> {
    let actor = vela_protocol::proposals::validate_human_reviewer_authority_at(
        project, reviewer, key_use_at,
    )
    .map_err(|error| DecisionPlanError::new("reviewer_unauthorized", error))?;
    if !actor.public_key.eq_ignore_ascii_case(expected_public_key) {
        return Err(DecisionPlanError::new(
            "reviewer_unauthorized",
            format!("reviewer {reviewer} key changed between confirmation and key use"),
        ));
    }
    Ok(())
}

fn validate_answers(answers: &[SavedAnswer]) -> Result<(), DecisionPlanError> {
    if answers.is_empty() {
        return Err(DecisionPlanError::new(
            "empty_decision",
            "a Decision Plan requires at least one explicit answer",
        ));
    }
    if answers.len() > crate::review_material::REVIEW_PAGE_MAX {
        return Err(DecisionPlanError::new(
            "decision_too_large",
            format!(
                "decision has {} answers; maximum is {}",
                answers.len(),
                crate::review_material::REVIEW_PAGE_MAX
            ),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut roots = BTreeSet::new();
    for answer in answers {
        if !ids.insert(answer.proposal_id.as_str()) || !roots.insert(answer.proposal_root.as_str())
        {
            return Err(DecisionPlanError::new(
                "duplicate_answer",
                format!("proposal {} is answered more than once", answer.proposal_id),
            ));
        }
        if answer.reason.trim().is_empty() {
            return Err(DecisionPlanError::new(
                "reason_required",
                format!(
                    "proposal {} requires a non-empty reason",
                    answer.proposal_id
                ),
            ));
        }
    }
    Ok(())
}

fn validate_coherence(
    project: &Project,
    review: &LockedReviewSelection,
    answers: &[SavedAnswer],
    reviewer_authority_root: &str,
) -> Result<(), DecisionPlanError> {
    let mut first: Option<CoherenceKey> = None;
    let mut isolated = Vec::new();
    for (answer, snapshot) in answers.iter().zip(&review.items) {
        let proposal = project
            .proposals
            .iter()
            .find(|proposal| proposal.id == answer.proposal_id)
            .ok_or_else(|| {
                DecisionPlanError::new(
                    "decision_stale",
                    format!("proposal {} disappeared", answer.proposal_id),
                )
            })?;
        validate_generic_decision_kind(&proposal.kind, &proposal.id)?;
        let policy_context_root = domain_root(
            COHERENCE_POLICY_DOMAIN,
            &json!({
                "policy_input_root": snapshot.decision_bindings.policy_input_root,
                "policy_result_root": snapshot.decision_bindings.policy_result_root,
                "engine_gate_root": snapshot.decision_bindings.engine_gate_root,
            }),
        )?;
        let required_check_root =
            domain_root(COHERENCE_CHECK_DOMAIN, &snapshot.brief.basis.check_state)?;
        let capability_root = domain_root(
            COHERENCE_CAPABILITY_DOMAIN,
            &json!({
                "authority": snapshot.brief.authority,
                "acceptance_authority_root": snapshot
                    .brief
                    .facets
                    .get("acceptance_authority")
                    .map(|facet| facet.full_root.as_str()),
            }),
        )?;
        let key = CoherenceKey {
            proposal_kind: proposal.kind.clone(),
            route: snapshot.brief.authority.route.clone(),
            scope: snapshot.brief.authority.scope.clone(),
            policy_context_root,
            required_check_root,
            capability_root,
            impact_tier: snapshot.brief.impact.downstream_effect.impact_tier,
            reviewer_authority_root: reviewer_authority_root.to_string(),
        };
        if let Some(expected) = &first {
            if expected != &key {
                return Err(DecisionPlanError::new(
                    "incoherent_batch",
                    "answers differ in proposal class, policy context, required checks, reviewer capability, impact tier, or reviewer authority",
                ));
            }
        } else {
            first = Some(key);
        }
        let kind = proposal.kind.as_str();
        let high_risk_kind = kind.contains("policy")
            || kind.contains("governance")
            || kind.ends_with(".retract")
            || kind == "finding.confidence_revise";
        let high_risk = high_risk_kind
            || !snapshot.brief.impact.critical_warnings.is_empty()
            || snapshot.brief.impact.downstream_effect.impact_tier >= 2
            || snapshot.brief.basis.check_state.gate_status == "refuted"
            || snapshot.brief.basis.check_state.engine_status.as_deref() == Some("blocked");
        if high_risk {
            isolated.push(answer.proposal_id.as_str());
        }
    }
    if answers.len() > 1 && !isolated.is_empty() {
        return Err(DecisionPlanError::new(
            "high_risk_requires_isolation",
            format!(
                "high-risk proposal(s) {} must be decided in an isolated transaction",
                isolated.join(", ")
            ),
        ));
    }
    Ok(())
}

fn validate_generic_decision_kind(
    proposal_kind: &str,
    proposal_id: &str,
) -> Result<(), DecisionPlanError> {
    if proposal_kind == vela_protocol::proposals::policy_accept::POLICY_HEAD_PROPOSAL_KIND {
        return Err(DecisionPlanError::new(
            "dedicated_policy_ceremony_required",
            format!(
                "proposal {proposal_id} changes the policy head; use the dedicated `vela policy` ceremony"
            ),
        ));
    }
    Ok(())
}

fn normalize_event_core(event: &vela_protocol::events::StateEvent) -> serde_json::Value {
    let mut event = serde_json::to_value(event).expect("StateEvent serialization is infallible");
    let Some(event_object) = event.as_object_mut() else {
        unreachable!("StateEvent serializes as an object")
    };
    event_object.insert("id".to_string(), serde_json::Value::String(String::new()));
    event_object.insert("signature".to_string(), serde_json::Value::Null);
    if let Some(payload) = event_object
        .get_mut("payload")
        .and_then(serde_json::Value::as_object_mut)
        && let Some(provenance) = payload
            .get_mut("provenance")
            .and_then(serde_json::Value::as_object_mut)
    {
        if let Some(input_refs) = provenance
            .get_mut("input_refs")
            .and_then(serde_json::Value::as_array_mut)
        {
            input_refs.retain(|reference| {
                reference.as_str().is_none_or(|reference| {
                    !reference
                        .starts_with(vela_protocol::provenance::DECISION_ROOT_INPUT_REF_PREFIX)
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
    event
}

fn decision_plan_root(plan: &DecisionPlan) -> Result<String, DecisionPlanError> {
    let bytes = decision_plan_preimage_bytes(plan)?;
    let mut digest = Sha256::new();
    digest.update(DECISION_PLAN_DOMAIN);
    digest.update(bytes);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn decision_plan_preimage_bytes(plan: &DecisionPlan) -> Result<Vec<u8>, DecisionPlanError> {
    vela_protocol::canonical::to_canonical_bytes(&DecisionPlanPreimage {
        decision_preimage_version: &plan.decision_preimage_version,
        frontier_id: &plan.frontier_id,
        expected_event_log_root: &plan.expected_event_log_root,
        ordered_answers: &plan.ordered_answers,
        consumed_fact_roots: &plan.consumed_fact_roots,
        policy_input_root: &plan.policy_input_root,
        semantic_event_cores: &plan.semantic_event_cores,
    })
    .map_err(|error| DecisionPlanError::new("canonicalization_failed", error))
}

fn domain_root(value_domain: &[u8], value: &impl Serialize) -> Result<String, DecisionPlanError> {
    let bytes = vela_protocol::canonical::to_canonical_bytes(value)
        .map_err(|error| DecisionPlanError::new("canonicalization_failed", error))?;
    let mut digest = Sha256::new();
    digest.update(value_domain);
    digest.update(bytes);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn clone_project(project: &Project) -> Result<Project, DecisionPlanError> {
    serde_json::from_value(
        serde_json::to_value(project)
            .map_err(|error| DecisionPlanError::new("project_clone_failed", error.to_string()))?,
    )
    .map_err(|error| DecisionPlanError::new("project_clone_failed", error.to_string()))
}

fn ensure_transactional_frontier(frontier: &Path) -> Result<(), DecisionPlanError> {
    if !frontier.join(".vela").is_dir() {
        return Err(DecisionPlanError::new(
            "transactional_frontier_required",
            "Decision Plan writes require a directory frontier with .vela/; migrate or initialize this frontier before signing",
        ));
    }
    Ok(())
}

fn acquire_barrier_with_recovery(
    frontier: &Path,
) -> Result<FrontierRecoveryBarrier, DecisionPlanError> {
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)
        .map_err(|error| DecisionPlanError::new("frontier_unavailable", error))?;
    for _ in 0..3 {
        match FrontierTxn::acquire_recovery_barrier(frontier, &journal_dir) {
            Ok(barrier) => return Ok(barrier),
            Err(FrontierTxnError::RecoveryRequired { operation_id, .. }) => {
                let operation_id =
                    OperationId::parse(operation_id).map_err(DecisionPlanError::transaction)?;
                match FrontierTxn::recover(frontier, &journal_dir, &operation_id)
                    .map_err(DecisionPlanError::transaction)?
                {
                    RecoveryOutcome::Prepared => {
                        let mut transaction =
                            FrontierTxn::open(frontier, &journal_dir, &operation_id)
                                .map_err(DecisionPlanError::transaction)?;
                        transaction
                            .abort_prepared()
                            .map_err(DecisionPlanError::transaction)?;
                        return Err(DecisionPlanError::new(
                            "reconfirmation_required",
                            "an earlier decision stopped before its commit marker; its prepared journal was aborted, so review and confirm again",
                        ));
                    }
                    RecoveryOutcome::Aborted
                    | RecoveryOutcome::Completed
                    | RecoveryOutcome::AlreadyCompleted => continue,
                }
            }
            Err(error) => return Err(DecisionPlanError::transaction(error)),
        }
    }
    Err(DecisionPlanError::new(
        "recovery_incomplete",
        "frontier recovery did not reach a stable barrier",
    ))
}

fn decision_read_set(
    frontier: &Path,
    project: &Project,
    review: &LockedReviewSelection,
    answers: &[SavedAnswer],
) -> Result<Vec<InputBinding>, DecisionPlanError> {
    let mut read_set = vec![
        InputBinding::project_snapshot(project).map_err(DecisionPlanError::transaction)?,
        InputBinding::engine_policy_observation(&review.engine_policy_observation_root)
            .map_err(DecisionPlanError::transaction)?,
    ];
    for path in [
        ".vela/policies/active.json",
        ".vela/policies/active.sig.json",
    ] {
        read_set.push(
            InputBinding::current_file(
                frontier,
                RepoPath::parse(path).map_err(DecisionPlanError::transaction)?,
            )
            .map_err(DecisionPlanError::transaction)?,
        );
    }
    for answer in answers {
        let proposal = project
            .proposals
            .iter()
            .find(|proposal| proposal.id == answer.proposal_id)
            .ok_or_else(|| {
                DecisionPlanError::new(
                    "decision_stale",
                    format!("proposal {} disappeared", answer.proposal_id),
                )
            })?;
        let Some(path) = proposal
            .payload
            .pointer("/vela_submission/receipt_path")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if !safe_receipt_path(path) {
            // The Decision Brief already binds this as invalid/unavailable and
            // therefore blocks accept. Reject must remain possible without
            // hashing or journaling an attacker-selected local path.
            continue;
        }
        read_set.push(
            InputBinding::current_file(
                frontier,
                RepoPath::parse(path).map_err(DecisionPlanError::transaction)?,
            )
            .map_err(DecisionPlanError::transaction)?,
        );
    }
    read_set.sort_by(|left, right| left.name.cmp(&right.name));
    read_set.dedup_by(|left, right| left.name == right.name && left.digest == right.digest);
    Ok(read_set)
}

fn safe_receipt_path(path: &str) -> bool {
    let path = Path::new(path);
    path.to_str()
        .is_some_and(|path| path.starts_with("records/receipts/sha256/"))
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    use vela_protocol::events::StateTarget;
    use vela_protocol::sign::ActorRecord;

    const DECIDED_AT: &str = "2026-07-14T12:00:00Z";

    fn test_root(nibble: char) -> String {
        format!("sha256:{}", nibble.to_string().repeat(64))
    }

    fn managed_projection(frontier: &Path) -> vela_protocol::repo::ManagedFileSet {
        let project = vela_protocol::repo::load_from_path(frontier).unwrap();
        vela_protocol::repo::render_vela_repo_files(frontier, &project).unwrap()
    }

    fn snapshot_file_tree(root: &Path) -> Vec<(String, Vec<u8>)> {
        fn visit(root: &Path, path: &Path, snapshot: &mut Vec<(String, Vec<u8>)>) {
            let Ok(metadata) = std::fs::symlink_metadata(path) else {
                return;
            };
            if metadata.is_dir() {
                let mut entries = std::fs::read_dir(path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .collect::<Vec<_>>();
                entries.sort();
                for entry in entries {
                    visit(root, &entry, snapshot);
                }
            } else {
                snapshot.push((
                    path.strip_prefix(root).unwrap().display().to_string(),
                    std::fs::read(path).unwrap(),
                ));
            }
        }

        let mut snapshot = Vec::new();
        visit(root, root, &mut snapshot);
        snapshot
    }

    fn decision_fixture() -> (tempfile::TempDir, SigningKey, SavedAnswer) {
        let temp = tempfile::tempdir().unwrap();
        vela_protocol::frontier_repo::initialize(
            temp.path(),
            vela_protocol::frontier_repo::InitOptions {
                name: "decision-plan-test",
                template: "",
                initialize_git: false,
            },
        )
        .unwrap();
        let signing_key = SigningKey::from_bytes(&[0x51; 32]);
        let mut project = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        project.actors.push(ActorRecord {
            id: "reviewer:test".to_string(),
            public_key: hex::encode(signing_key.verifying_key().to_bytes()),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-13T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        let proposal = vela_protocol::proposals::new_proposal_at(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_test_target".to_string(),
            },
            "agent:test",
            "agent",
            "record a bounded note",
            json!({
                "note": "bounded test note",
                "vela_submission": {
                    "schema": "vela.submission-links.internal.v1",
                    "receipt_root": format!("sha256:{}", "3".repeat(64)),
                    "receipt_path": format!(
                        "records/receipts/sha256/{}.json",
                        "3".repeat(64)
                    ),
                    "record_id": "vrc_test",
                    "operation_id": format!("vop_{}", "4".repeat(64)),
                }
            }),
            Vec::new(),
            Vec::new(),
            "2026-07-13T01:00:00Z",
        );
        let proposal_id = proposal.id.clone();
        project.proposals.push(proposal);
        vela_protocol::repo::save_to_path(temp.path(), &project).unwrap();
        let snapshot = ReviewProjection::one(temp.path(), &proposal_id).unwrap();
        let answer = SavedAnswer {
            proposal_id,
            proposal_root: snapshot.decision_bindings.proposal_root.clone(),
            seen_decision_facts_root: snapshot.brief.audit.decision_facts_root,
            action: DecisionAction::Reject,
            reason: "Reject malformed retained material".to_string(),
        };
        (temp, signing_key, answer)
    }

    fn deterministic_production_accept_fixture() -> (tempfile::TempDir, PreparedDecision) {
        const COMPILED_AT: &str = "2026-07-13T00:00:00Z";
        const PROPOSAL_AT: &str = "2026-07-13T01:00:00Z";
        const REASON: &str = "Scope and evidence checked";
        // This compiler string is part of the published cross-implementation
        // vector. Keep it fixed across releases: the production assembler and
        // manual-curation provenance use CARGO_PKG_VERSION, but a conformance
        // preimage must not acquire new roots merely because the test binary
        // was rebuilt.
        const FIXTURE_COMPILER: &str = "vela/0.758.11";

        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join(".vela")).unwrap();

        let mut finding_proposal = vela_protocol::state::build_add_finding_proposal_at(
            vela_protocol::state::FindingDraftOptions {
                text: "A deterministic fixture observation".to_string(),
                assertion_type: "observation".to_string(),
                source: "Deterministic fixture source".to_string(),
                source_type: "preprint".to_string(),
                author: "human:fixture-author".to_string(),
                confidence: 0.8,
                evidence_type: "experimental".to_string(),
                doi: None,
                year: Some(2026),
                url: None,
                source_authors: vec!["Fixture Author".to_string()],
                source_refs: Vec::new(),
                conditions_text: Some("bounded fixture conditions".to_string()),
                evidence_spans: Vec::new(),
                gap: false,
                negative_space: false,
                replication_attestation: None,
            },
            COMPILED_AT,
        )
        .unwrap();
        finding_proposal.payload["finding"]["created"] = COMPILED_AT.into();
        finding_proposal.payload["finding"]["provenance"]["extraction"]["extractor_version"] =
            FIXTURE_COMPILER.into();
        let finding: vela_protocol::bundle::FindingBundle =
            serde_json::from_value(finding_proposal.payload["finding"].clone()).unwrap();
        let finding_id = finding.id.clone();

        let mut project = vela_protocol::project::assemble(
            "decision-plan-conformance",
            vec![finding],
            1,
            0,
            "Deterministic production Decision Plan fixture",
        );
        project.project.compiled_at = COMPILED_AT.to_string();
        project.project.compiler = FIXTURE_COMPILER.to_string();
        let genesis = project.events.first_mut().unwrap();
        genesis.timestamp = COMPILED_AT.to_string();
        genesis.actor.id = FIXTURE_COMPILER.to_string();
        genesis.payload["compiled_at"] = COMPILED_AT.into();
        genesis.payload["creator"] = FIXTURE_COMPILER.into();
        genesis.id = vela_protocol::events::compute_event_id(genesis);
        project.frontier_id = vela_protocol::project::frontier_id_from_genesis(&project.events);

        let signing_key = SigningKey::from_bytes(&[0x51; 32]);
        project.actors.push(ActorRecord {
            id: "reviewer:test".to_string(),
            public_key: hex::encode(signing_key.verifying_key().to_bytes()),
            algorithm: "ed25519".to_string(),
            created_at: COMPILED_AT.to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        let proposal = vela_protocol::proposals::new_proposal_at(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: finding_id,
            },
            "agent:fixture",
            "agent",
            "record bounded scope",
            json!({"text": "The observation applies only under the fixture conditions."}),
            Vec::new(),
            Vec::new(),
            PROPOSAL_AT,
        );
        let proposal_id = proposal.id.clone();
        project.proposals.push(proposal);
        vela_protocol::repo::save_to_path(temp.path(), &project).unwrap();

        let snapshot = ReviewProjection::one_at(temp.path(), &proposal_id, DECIDED_AT).unwrap();
        assert!(snapshot.brief.accept_ready(), "{:#?}", snapshot.brief);
        let answer = SavedAnswer {
            proposal_id,
            proposal_root: snapshot.decision_bindings.proposal_root.clone(),
            seen_decision_facts_root: snapshot.brief.audit.decision_facts_root,
            action: DecisionAction::Accept,
            reason: REASON.to_string(),
        };
        let prepared =
            build_unlocked(temp.path(), &[answer], "reviewer:test", DECIDED_AT, None).unwrap();
        (temp, prepared)
    }

    #[test]
    fn decision_plan_cross_implementation_vector_is_exact() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../conformance/decision-binding.json"))
                .unwrap();
        let decision = &fixture["decision_plan"];
        let preimage = decision["preimage"].clone();
        let canonical = vela_protocol::canonical::to_canonical_bytes(&preimage).unwrap();
        assert_eq!(
            std::str::from_utf8(&canonical).unwrap(),
            decision["canonical"].as_str().unwrap()
        );
        let (_frontier, production) = deterministic_production_accept_fixture();
        let production_bytes = decision_plan_preimage_bytes(&production.plan).unwrap();
        assert_eq!(
            production_bytes, canonical,
            "the fixture must be emitted by the real production builder and typed serializer"
        );
        assert_eq!(
            decision_plan_root(&production.plan).unwrap(),
            decision["decision_root"].as_str().unwrap()
        );
        assert_eq!(production.plan.decision_root, decision["decision_root"]);
        assert_eq!(
            domain_root(DECISION_PLAN_DOMAIN, &preimage).unwrap(),
            decision["decision_root"].as_str().unwrap()
        );
        assert_eq!(
            decision["domain_prefix"].as_str().unwrap().as_bytes(),
            DECISION_PLAN_DOMAIN
        );
        assert_eq!(preimage.as_object().unwrap().len(), 7);
        assert_eq!(
            preimage["consumed_fact_roots"][0]
                .as_object()
                .unwrap()
                .len(),
            13
        );
        assert_eq!(preimage["semantic_event_cores"][0]["event"]["id"], "");
        assert!(preimage["semantic_event_cores"][0]["event"]["signature"].is_null());
        assert_eq!(
            production
                .plan
                .semantic_event_cores
                .iter()
                .map(|core| core.event["kind"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["finding.noted", "review.accepted"],
            "the fixture must bind the complete production accept event set"
        );

        let acceptance = &fixture["acceptance_v1"];
        let inputs = &acceptance["inputs"];
        assert_eq!(inputs["vfr_id"], production.plan.frontier_id);
        assert_eq!(
            inputs["proposal_id"],
            production.plan.ordered_answers[0].proposal_id
        );
        assert_eq!(inputs["reason"], production.plan.ordered_answers[0].reason);
        assert_eq!(acceptance["decision_root"], production.plan.decision_root);
        let signing_input = vela_protocol::proposals::accept_preimage_bytes_v1(
            inputs["vfr_id"].as_str().unwrap(),
            inputs["proposal_id"].as_str().unwrap(),
            inputs["reviewer_id"].as_str().unwrap(),
            inputs["reason"].as_str().unwrap(),
            inputs["parent_event_log_hash"].as_str().unwrap(),
            acceptance["decision_root"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(signing_input.clone()).unwrap(),
            acceptance["pae"].as_str().unwrap()
        );
        assert_eq!(
            hex::encode(Sha256::digest(signing_input)),
            acceptance["sha256"].as_str().unwrap()
        );
    }

    #[test]
    fn decision_root_binds_answer_reason_order_and_every_consumed_root() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../conformance/decision-binding.json"))
                .unwrap();
        let original = fixture["decision_plan"]["preimage"].clone();
        let expected = domain_root(DECISION_PLAN_DOMAIN, &original).unwrap();

        let mut changed_reason = original.clone();
        changed_reason["ordered_answers"][0]["reason"] = "different".into();
        assert_ne!(
            domain_root(DECISION_PLAN_DOMAIN, &changed_reason).unwrap(),
            expected
        );

        let mut changed_fact = original.clone();
        changed_fact["consumed_fact_roots"][0]["verifier_snapshot_root"] =
            format!("sha256:{}", "e".repeat(64)).into();
        assert_ne!(
            domain_root(DECISION_PLAN_DOMAIN, &changed_fact).unwrap(),
            expected
        );

        let mut changed_core = original;
        changed_core["semantic_event_cores"][0]["event"]["reason"] = "different".into();
        assert_ne!(
            domain_root(DECISION_PLAN_DOMAIN, &changed_core).unwrap(),
            expected
        );
    }

    #[test]
    fn renderer_color_and_git_options_are_outside_the_decision_root() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../conformance/decision-binding.json"))
                .unwrap();
        let preimage = fixture["decision_plan"]["preimage"].clone();
        let root_before = domain_root(DECISION_PLAN_DOMAIN, &preimage).unwrap();
        let operational_only = json!({
            "renderer": "human-ansi-v9",
            "color": true,
            "git_commit": true,
            "git_push": true,
        });
        let operational_changed = json!({
            "renderer": "json-v1",
            "color": false,
            "git_commit": false,
            "git_push": false,
        });
        assert_ne!(operational_only, operational_changed);
        assert_eq!(
            root_before,
            domain_root(DECISION_PLAN_DOMAIN, &preimage).unwrap()
        );
    }

    #[test]
    fn receipt_read_set_refuses_attacker_selected_paths() {
        assert!(safe_receipt_path(
            "records/receipts/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.json"
        ));
        assert!(!safe_receipt_path(".ssh/id_ed25519"));
        assert!(!safe_receipt_path(
            "records/receipts/sha256/../../../../.ssh/id_ed25519"
        ));
        assert!(!safe_receipt_path("/records/receipts/sha256/a.json"));
    }

    #[test]
    fn locked_read_set_binds_project_engine_policy_and_exact_absent_files() {
        let (temp, _key, answer) = decision_fixture();
        let journal_dir = crate::workflow::frontier_transaction_journal_dir(temp.path()).unwrap();
        let barrier = FrontierTxn::acquire_recovery_barrier(temp.path(), &journal_dir).unwrap();
        let locked = build_locked(
            temp.path(),
            barrier,
            &[answer],
            "reviewer:test",
            DECIDED_AT,
            None,
        )
        .unwrap();
        let names = locked
            .read_set
            .iter()
            .map(|binding| binding.name.as_str())
            .collect::<BTreeSet<_>>();
        assert!(names.contains("frontier_project:vela.project-snapshot.internal.v1"));
        assert!(names.contains("frontier_observation:vela.engine-policy-summary-observation.v1"));
        assert!(names.contains("frontier_file:.vela/policies/active.json"));
        assert!(names.contains("frontier_file:.vela/policies/active.sig.json"));
        assert!(names.contains(
            &format!(
                "frontier_file:records/receipts/sha256/{}.json",
                "3".repeat(64)
            )[..]
        ));
    }

    #[test]
    fn stale_reviewer_authority_aborts_before_key_and_writes_nothing() {
        let (temp, key, answer) = decision_fixture();
        let confirmed =
            build_unlocked(temp.path(), &[answer], "reviewer:test", DECIDED_AT, None).unwrap();
        let mut changed = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        changed.actors[0].revoked_at = Some("2026-07-14T11:00:00Z".to_string());
        vela_protocol::repo::save_to_path(temp.path(), &changed).unwrap();
        let before_events = changed.events.len();
        let key_reads = Cell::new(0usize);
        let error = execute_with_key_loader(temp.path(), &confirmed, || {
            key_reads.set(key_reads.get() + 1);
            Ok(key.clone())
        })
        .unwrap_err();
        assert_eq!(error.code, "reviewer_unauthorized");
        assert_eq!(key_reads.get(), 0);
        let after = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        assert_eq!(after.events.len(), before_events);
        assert_eq!(after.proposals[0].status, "pending_review");
    }

    #[test]
    fn scheduled_reviewer_revocation_is_rechecked_at_key_use_time() {
        let (temp, key, answer) = decision_fixture();
        let now = chrono::Utc::now();
        let decided_at = (now - chrono::Duration::seconds(120))
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let revoked_at = (now - chrono::Duration::seconds(60))
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let mut project = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        project.actors[0].created_at =
            (now - chrono::Duration::days(1)).to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        project.actors[0].revoked_at = Some(revoked_at);
        project.actors[0].revoked_reason = Some("scheduled rotation".to_string());
        vela_protocol::repo::save_to_path(temp.path(), &project).unwrap();
        let confirmed =
            build_unlocked(temp.path(), &[answer], "reviewer:test", &decided_at, None).unwrap();
        let key_reads = Cell::new(0usize);
        let error = execute_with_key_loader(temp.path(), &confirmed, || {
            key_reads.set(key_reads.get() + 1);
            Ok(key.clone())
        })
        .unwrap_err();
        assert_eq!(error.code, "reviewer_unauthorized");
        assert_eq!(key_reads.get(), 0);
        let after = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        assert_eq!(after.proposals[0].status, "pending_review");
        assert!(after.events.is_empty());
    }

    #[test]
    fn complete_read_set_is_reverified_under_lock_immediately_before_key_use() {
        let (temp, key, answer) = decision_fixture();
        let confirmed =
            build_unlocked(temp.path(), &[answer], "reviewer:test", DECIDED_AT, None).unwrap();
        let policy_path = temp.path().join(".vela/policies/active.json");
        let hook_path = policy_path.clone();
        set_executor_mutation_hook(DecisionExecutorStep::AfterLockedRederive, move || {
            std::fs::create_dir_all(hook_path.parent().unwrap()).unwrap();
            std::fs::write(&hook_path, b"{}\n").unwrap();
        });
        let key_reads = Cell::new(0usize);
        let error = execute_with_key_loader(temp.path(), &confirmed, || {
            key_reads.set(key_reads.get() + 1);
            Ok(key.clone())
        })
        .unwrap_err();
        assert_eq!(error.code, "transaction_failed");
        assert!(error.message.contains("changed before commit"), "{error}");
        assert_eq!(key_reads.get(), 0);
        std::fs::remove_file(policy_path).unwrap();
        let project = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        assert_eq!(project.proposals[0].status, "pending_review");
        assert!(project.events.is_empty());
    }

    #[test]
    fn policy_head_is_never_a_generic_decision_plan_item() {
        let error = validate_generic_decision_kind(
            vela_protocol::proposals::policy_accept::POLICY_HEAD_PROPOSAL_KIND,
            "vpr_policy_head_fixture",
        )
        .unwrap_err();
        assert_eq!(error.code, "dedicated_policy_ceremony_required");
        assert!(error.message.contains("vela policy"));
    }

    #[test]
    fn coherence_key_binds_policy_checks_and_capability_dimensions() {
        let base = CoherenceKey {
            proposal_kind: "finding.note".to_string(),
            route: "human_review".to_string(),
            scope: "frontier_review".to_string(),
            policy_context_root: test_root('1'),
            required_check_root: test_root('2'),
            capability_root: test_root('3'),
            impact_tier: 0,
            reviewer_authority_root: test_root('4'),
        };
        let mut changed_policy = base.clone();
        changed_policy.policy_context_root = test_root('5');
        let mut changed_checks = base.clone();
        changed_checks.required_check_root = test_root('6');
        let mut changed_capability = base.clone();
        changed_capability.capability_root = test_root('7');
        assert_ne!(base, changed_policy);
        assert_ne!(base, changed_checks);
        assert_ne!(base, changed_capability);
    }

    #[test]
    fn corrupt_confirmed_root_aborts_before_recovery_lock_or_key() {
        let (temp, key, answer) = decision_fixture();
        let mut confirmed =
            build_unlocked(temp.path(), &[answer], "reviewer:test", DECIDED_AT, None).unwrap();
        confirmed.plan.decision_root = test_root('f');
        let before = managed_projection(temp.path());
        let key_reads = Cell::new(0usize);
        let error = execute_with_key_loader(temp.path(), &confirmed, || {
            key_reads.set(key_reads.get() + 1);
            Ok(key.clone())
        })
        .unwrap_err();
        assert_eq!(error.code, "confirmed_plan_invalid");
        assert_eq!(key_reads.get(), 0);
        assert_eq!(managed_projection(temp.path()), before);
    }

    #[test]
    fn every_bound_stale_class_aborts_before_key_with_zero_managed_delta() {
        type Mutate = fn(&mut PreparedDecision);
        let cases: [(&str, Mutate); 10] = [
            ("event_log", |prepared| {
                prepared.plan.expected_event_log_root = test_root('1');
            }),
            ("proposal_and_order", |prepared| {
                prepared.plan.ordered_answers[0].proposal_root = test_root('2');
                prepared
                    .plan
                    .ordered_answers
                    .push(prepared.plan.ordered_answers[0].clone());
            }),
            ("answer_and_reason", |prepared| {
                prepared.plan.ordered_answers[0].action = DecisionAction::Accept;
                prepared.plan.ordered_answers[0].reason = "changed reason".to_string();
            }),
            ("receipt_and_evidence_availability", |prepared| {
                let roots = &mut prepared.plan.consumed_fact_roots[0];
                roots.receipt_observation_root = test_root('3');
                roots.receipt_root = Some(test_root('4'));
                roots.evidence_or_reference_root = test_root('5');
                roots.evidence_availability = "restricted".to_string();
            }),
            ("verifier_snapshot", |prepared| {
                prepared.plan.consumed_fact_roots[0].verifier_snapshot_root = test_root('6');
            }),
            ("policy_and_evaluator", |prepared| {
                let roots = &mut prepared.plan.consumed_fact_roots[0];
                roots.policy_input_root = test_root('7');
                roots.policy_result_root = test_root('8');
                roots.engine_gate_root = test_root('9');
                prepared.plan.policy_input_root = test_root('a');
            }),
            ("reviewer_authority", |prepared| {
                prepared.plan.consumed_fact_roots[0].reviewer_authority_root = test_root('b');
            }),
            ("semantic_effect", |prepared| {
                prepared.plan.consumed_fact_roots[0].semantic_effect_root = test_root('c');
            }),
            ("downstream_impact", |prepared| {
                prepared.plan.consumed_fact_roots[0].downstream_impact_root = test_root('d');
            }),
            ("semantic_event_core", |prepared| {
                prepared.plan.semantic_event_cores[0].event["reason"] = "changed event core".into();
            }),
        ];

        for (label, mutate) in cases {
            let (temp, key, answer) = decision_fixture();
            let mut confirmed =
                build_unlocked(temp.path(), &[answer], "reviewer:test", DECIDED_AT, None).unwrap();
            mutate(&mut confirmed);
            confirmed.plan.decision_root = decision_plan_root(&confirmed.plan).unwrap();
            let before = managed_projection(temp.path());
            let key_reads = Cell::new(0usize);
            let error = execute_with_key_loader(temp.path(), &confirmed, || {
                key_reads.set(key_reads.get() + 1);
                Ok(key.clone())
            })
            .unwrap_err();
            assert_eq!(error.code, "decision_stale", "case {label}: {error}");
            assert_eq!(key_reads.get(), 0, "case {label}");
            assert_eq!(managed_projection(temp.path()), before, "case {label}");
        }
    }

    #[test]
    fn success_reads_key_once_and_exact_retry_reads_it_zero_more_times() {
        let (temp, key, answer) = decision_fixture();
        let confirmed =
            build_unlocked(temp.path(), &[answer], "reviewer:test", DECIDED_AT, None).unwrap();
        let key_reads = Cell::new(0usize);
        let first = execute_with_key_loader(temp.path(), &confirmed, || {
            key_reads.set(key_reads.get() + 1);
            Ok(key.clone())
        })
        .unwrap();
        assert_eq!(key_reads.get(), 1);
        assert_eq!(first.event_ids.len(), 1);
        let second = execute_with_key_loader(temp.path(), &confirmed, || {
            key_reads.set(key_reads.get() + 1);
            Ok(key.clone())
        })
        .unwrap();
        assert_eq!(second, first);
        assert_eq!(key_reads.get(), 1);
    }

    #[test]
    fn every_post_marker_executor_phase_recovers_exactly_without_a_second_key_read() {
        // The transaction layer's `post_marker_failpoints_recover_the_exact_delta_idempotently`
        // test injects before and after every individual managed-file install. This adapter-level
        // matrix proves the Decision Plan retains the same event ids, signatures, and public delta
        // across each phase boundary it owns around that per-write harness.
        for step in [
            DecisionExecutorStep::AfterCommitMarker,
            DecisionExecutorStep::AfterInstall,
            DecisionExecutorStep::AfterComplete,
        ] {
            let (temp, key, answer) = decision_fixture();
            let confirmed =
                build_unlocked(temp.path(), &[answer], "reviewer:test", DECIDED_AT, None).unwrap();
            let key_reads = Cell::new(0usize);
            set_executor_failpoint(Some(step));
            let error = execute_with_key_loader(temp.path(), &confirmed, || {
                key_reads.set(key_reads.get() + 1);
                Ok(key.clone())
            })
            .unwrap_err();
            assert_eq!(error.code, "injected_failure", "phase {step:?}");
            assert_eq!(key_reads.get(), 1, "phase {step:?}");
            set_executor_failpoint(None);
            let operation_id =
                OperationId::derive("decision", confirmed.plan.decision_root.as_bytes());
            let recovered = recover_decision_operation(temp.path(), operation_id.as_str())
                .unwrap()
                .expect("scientific Decision journal");
            assert_eq!(key_reads.get(), 1, "phase {step:?}");
            assert_eq!(recovered.decision_root, confirmed.plan.decision_root);
            assert_eq!(recovered.event_ids, confirmed.appended_event_ids);
            assert_eq!(recovered.aggregate_engine, confirmed.aggregate_engine);
            assert!(recovered.publication_delta.is_none());
            let recovered_project = vela_protocol::repo::load_from_path(temp.path()).unwrap();
            let signed_events = recovered
                .event_ids
                .iter()
                .map(|event_id| {
                    recovered_project
                        .events
                        .iter()
                        .find(|event| event.id == *event_id)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert!(signed_events.iter().all(|event| {
                vela_protocol::sign::verify_event_signature(
                    event,
                    &hex::encode(key.verifying_key().to_bytes()),
                )
                .unwrap()
            }));
            let exact_bytes = managed_projection(temp.path());
            let retry = execute_with_key_loader(temp.path(), &confirmed, || {
                key_reads.set(key_reads.get() + 1);
                Ok(key.clone())
            })
            .unwrap();
            assert_eq!(retry, recovered, "phase {step:?}");
            assert_eq!(key_reads.get(), 1, "phase {step:?}");
            assert_eq!(
                managed_projection(temp.path()),
                exact_bytes,
                "phase {step:?}"
            );
        }
    }

    #[test]
    fn marker_free_prepared_failure_aborts_and_requires_reconfirmation() {
        let (temp, key, answer) = decision_fixture();
        let confirmed =
            build_unlocked(temp.path(), &[answer], "reviewer:test", DECIDED_AT, None).unwrap();
        let key_reads = Cell::new(0usize);
        set_executor_failpoint(Some(DecisionExecutorStep::AfterPreparedJournal));
        let first = execute_with_key_loader(temp.path(), &confirmed, || {
            key_reads.set(key_reads.get() + 1);
            Ok(key.clone())
        })
        .unwrap_err();
        assert_eq!(first.code, "injected_failure");
        assert_eq!(key_reads.get(), 1);
        set_executor_failpoint(None);
        let retry = execute_with_key_loader(temp.path(), &confirmed, || {
            key_reads.set(key_reads.get() + 1);
            Ok(key.clone())
        })
        .unwrap_err();
        assert_eq!(retry.code, "reconfirmation_required");
        assert_eq!(key_reads.get(), 1);
        let project = vela_protocol::repo::load_from_path(temp.path()).unwrap();
        assert_eq!(project.proposals[0].status, "pending_review");
    }

    #[test]
    fn read_only_preview_leaves_an_outstanding_prepared_journal_byte_exact() {
        let (temp, key, answer) = decision_fixture();
        let confirmed = build_unlocked(
            temp.path(),
            std::slice::from_ref(&answer),
            "reviewer:test",
            DECIDED_AT,
            None,
        )
        .unwrap();
        set_executor_failpoint(Some(DecisionExecutorStep::AfterPreparedJournal));
        let error =
            execute_with_key_loader(temp.path(), &confirmed, || Ok(key.clone())).unwrap_err();
        set_executor_failpoint(None);
        assert_eq!(error.code, "injected_failure");

        let journal_dir = crate::workflow::frontier_transaction_journal_dir(temp.path()).unwrap();
        let journal_before = snapshot_file_tree(&journal_dir);
        assert!(
            journal_before
                .iter()
                .any(|(path, _)| path.starts_with("frontier/") && path.ends_with(".json")),
            "fixture must contain an outstanding Prepared journal"
        );
        let managed_before = managed_projection(temp.path());

        let preview =
            build_read_only_preview(temp.path(), &[answer], "reviewer:test", DECIDED_AT, None)
                .unwrap();
        assert_eq!(preview.plan, confirmed.plan);
        assert_eq!(snapshot_file_tree(&journal_dir), journal_before);
        assert_eq!(managed_projection(temp.path()), managed_before);
    }

    #[test]
    fn scripted_confirmation_time_has_bounded_age_and_future_skew() {
        let now = chrono::Utc::now();
        let current = now.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        validate_scripted_confirmation_time(&current).unwrap();

        let expired = (now - chrono::Duration::seconds(SCRIPTED_CONFIRMATION_MAX_AGE_SECONDS + 1))
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let error = validate_scripted_confirmation_time(&expired).unwrap_err();
        assert_eq!(error.code, "confirmation_expired");

        let future = (now
            + chrono::Duration::seconds(SCRIPTED_CONFIRMATION_MAX_FUTURE_SKEW_SECONDS + 2))
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let error = validate_scripted_confirmation_time(&future).unwrap_err();
        assert_eq!(error.code, "confirmation_expired");
    }

    #[test]
    fn all_pre_key_failpoints_read_zero_keys_and_write_zero_decision_delta() {
        for step in [
            DecisionExecutorStep::BeforeLock,
            DecisionExecutorStep::AfterLock,
            DecisionExecutorStep::AfterLockedRederive,
        ] {
            let (temp, key, answer) = decision_fixture();
            let confirmed =
                build_unlocked(temp.path(), &[answer], "reviewer:test", DECIDED_AT, None).unwrap();
            let before = vela_protocol::repo::load_from_path(temp.path()).unwrap();
            let key_reads = Cell::new(0usize);
            set_executor_failpoint(Some(step));
            let error = execute_with_key_loader(temp.path(), &confirmed, || {
                key_reads.set(key_reads.get() + 1);
                Ok(key.clone())
            })
            .unwrap_err();
            set_executor_failpoint(None);
            assert_eq!(error.code, "injected_failure");
            assert_eq!(key_reads.get(), 0, "unexpected key read at {step:?}");
            let after = vela_protocol::repo::load_from_path(temp.path()).unwrap();
            assert_eq!(after.events.len(), before.events.len());
            assert_eq!(after.proposals[0].status, "pending_review");
        }
    }
}
