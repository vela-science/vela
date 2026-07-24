//! Disposable Era-1 repository-authority transaction writer.
//!
//! This module is a production-shaped writer core with no CLI route. It proves
//! that the proposed authority record, runtime authentication preflight, and
//! existing recoverable frontier transaction can compose without introducing
//! a second journal or a live writer. A later migration slice may expose it
//! only after the ADR 0020 gates pass.

use std::fmt;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;
use vela_authority::CedarEvaluationInput;
use vela_authority::runtime_authentication::{
    AuthenticationAdapter, AuthenticationRequest, AuthorityPreflightFailure, RuntimeSessionState,
    preflight_authority_action,
};
use vela_protocol::authentication::AuthenticationObservationV1;
use vela_protocol::authority::{
    AUTHORITY_MODE, AUTHORITY_PAYLOAD_TYPE_V1, AuthorityEnvelopeV1, AuthorityEventContentV1,
    AuthorityEventV1, AuthorityKeysetV1, AuthorityRecordContentV1, AuthorityRecordV1,
    AuthorizationClaimV1, DelegationClaimV1, DsseSignatureV1, ExecutionClaimV1, ObjectDeltaV1,
    PolicyBundleV1, PrincipalSnapshotV1, SemanticApprovalV1, verify_authority_envelope,
};
use vela_protocol::authority_history::{
    AuthorityHistoryEra, AuthorityHistoryInput, authority_event_log_root, verify_authority_history,
};
use vela_protocol::canonical::to_canonical_bytes;
use vela_protocol::events::{EventKind, StateActor, StateEvent, StateTarget, event_log_hash};
use vela_protocol::principal_capability::HUMAN_ONLY_AUTHORITY_ACTIONS_V1;

use crate::frontier_txn::{
    CanonicalWriteBarrier, ContentDigest, DeltaDraft, FrontierBinding, FrontierTxn,
    FrontierTxnError, FrontierTxnPlan, FrontierTxnPlanSpec, InputBinding, OperationId,
    OperationKind, PlannedWrite, RepoPath, WriteClass,
};

const TRANSACTION_ID_SCHEMA: &str = "vela.authority-transaction-id.internal.v1";
const READ_SET_SCHEMA: &str = "vela.authority-read-set.internal.v1";
const WRITE_SET_SCHEMA: &str = "vela.authority-write-set.internal.v1";
const LAYOUT_SCHEMA: &str = "vela.authority-layout.internal.v1";
const OPERATION_DOMAIN: &str = "authority_transaction";

/// Complete verified-history input for the next transaction.
///
/// The writer re-runs dual-history verification itself. It does not accept a
/// caller-asserted head root or sequence.
#[derive(Debug, Clone)]
pub(crate) struct AuthorityHistorySnapshot {
    pub(crate) frontier_id: String,
    pub(crate) legacy_events: Vec<StateEvent>,
    pub(crate) legacy_actor_registry_bytes: Vec<u8>,
    pub(crate) legacy_active_policy_head_root: String,
    pub(crate) legacy_policy_store_manifest_root: String,
    pub(crate) authority_keyset: AuthorityKeysetV1,
    pub(crate) policy_bundle: PolicyBundleV1,
    pub(crate) authority_events: Vec<AuthorityEventV1>,
    pub(crate) authority_envelopes: Vec<AuthorityEnvelopeV1>,
}

/// Event fields whose transaction and principal attribution are derived by the
/// writer rather than accepted from a caller.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct AuthorityEventDraft {
    pub(crate) kind: EventKind,
    pub(crate) target: StateTarget,
    pub(crate) actor: StateActor,
    pub(crate) timestamp: String,
    pub(crate) reason: String,
    pub(crate) before_hash: String,
    pub(crate) after_hash: String,
    pub(crate) payload: Value,
    pub(crate) caveats: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthorityTransactionRequest {
    pub(crate) history: AuthorityHistorySnapshot,
    pub(crate) intent_digest: String,
    pub(crate) principal: PrincipalSnapshotV1,
    pub(crate) authentication_request: AuthenticationRequest,
    pub(crate) runtime_session_state: RuntimeSessionState,
    pub(crate) authorization_input: CedarEvaluationInput,
    pub(crate) delegation: Option<DelegationClaimV1>,
    pub(crate) semantic_approvals: Vec<SemanticApprovalV1>,
    pub(crate) event_drafts: Vec<AuthorityEventDraft>,
    pub(crate) read_set: Vec<InputBinding>,
    pub(crate) vela_version: String,
    pub(crate) binary_sha256: String,
    pub(crate) recorded_at: String,
}

/// Repository signer boundary. Implementations receive only the canonical,
/// already-validated authority-record payload.
pub(crate) trait RepositoryAuthoritySigner {
    fn sign(
        &mut self,
        payload_type: &str,
        canonical_payload: &[u8],
    ) -> Result<Vec<DsseSignatureV1>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthorityTransactionResult {
    pub(crate) operation_id: String,
    pub(crate) transaction_id: String,
    pub(crate) event_ids: Vec<String>,
    pub(crate) authority_record_id: String,
    pub(crate) authority_record_root: String,
    pub(crate) before_event_log_root: String,
    pub(crate) after_event_log_root: String,
    pub(crate) read_set_root: String,
    pub(crate) write_set_root: String,
}

#[derive(Debug)]
pub(crate) struct PreparedAuthorityTransaction {
    transaction: FrontierTxn,
    pub(crate) result: AuthorityTransactionResult,
    pub(crate) events: Vec<AuthorityEventV1>,
    pub(crate) envelope: AuthorityEnvelopeV1,
}

impl PreparedAuthorityTransaction {
    pub(crate) fn mark_committed(&mut self) -> Result<(), AuthorityTransactionError> {
        self.transaction
            .mark_committed()
            .map_err(AuthorityTransactionError::Transaction)
    }

    pub(crate) fn install(&mut self) -> Result<(), AuthorityTransactionError> {
        self.transaction
            .install()
            .map_err(AuthorityTransactionError::Transaction)
    }

    pub(crate) fn complete(&mut self) -> Result<(), AuthorityTransactionError> {
        self.transaction
            .complete()
            .map_err(AuthorityTransactionError::Transaction)
    }

    #[cfg(test)]
    fn transaction_mut(&mut self) -> &mut FrontierTxn {
        &mut self.transaction
    }
}

#[derive(Debug)]
pub(crate) enum AuthorityTransactionError {
    Invalid(String),
    History(String),
    Authentication(AuthorityPreflightFailure),
    Signing(String),
    Transaction(FrontierTxnError),
}

impl fmt::Display for AuthorityTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(formatter, "invalid authority transaction: {message}"),
            Self::History(message) => write!(formatter, "invalid authority history: {message}"),
            Self::Authentication(error) => write!(formatter, "{error}"),
            Self::Signing(message) => {
                write!(formatter, "repository authority signing failed: {message}")
            }
            Self::Transaction(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AuthorityTransactionError {}

/// Prepare a signed transaction under an already-held canonical write barrier.
///
/// Authentication, dual-history verification, authorization, transaction
/// construction, and DSSE verification all complete before the first journal
/// byte is created. The returned transaction still has no commit marker and no
/// canonical postimage.
pub(crate) fn prepare_authority_transaction<A, S>(
    barrier: CanonicalWriteBarrier,
    frontier_root: &Path,
    mut request: AuthorityTransactionRequest,
    authentication_adapter: &mut A,
    signer: &mut S,
) -> Result<PreparedAuthorityTransaction, AuthorityTransactionError>
where
    A: AuthenticationAdapter,
    S: RepositoryAuthoritySigner,
{
    validate_request_shape(&request)?;
    let history = verify_authority_history(AuthorityHistoryInput {
        frontier_id: &request.history.frontier_id,
        legacy_events: &request.history.legacy_events,
        legacy_actor_registry_bytes: &request.history.legacy_actor_registry_bytes,
        legacy_active_policy_head_root: &request.history.legacy_active_policy_head_root,
        legacy_policy_store_manifest_root: &request.history.legacy_policy_store_manifest_root,
        authority_keyset: &request.history.authority_keyset,
        policy_bundle: &request.history.policy_bundle,
        authority_events: &request.history.authority_events,
        authority_envelopes: &request.history.authority_envelopes,
    })
    .map_err(AuthorityTransactionError::History)?;
    if history.era != AuthorityHistoryEra::RepositoryAuthority {
        return Err(AuthorityTransactionError::History(
            "the disposable Era-1 writer requires a verified migration bridge".into(),
        ));
    }
    let previous_authority_record_root =
        history.final_authority_record_root.clone().ok_or_else(|| {
            AuthorityTransactionError::History(
                "repository-authority history has no previous record root".into(),
            )
        })?;
    let sequence = u64::try_from(history.authority_record_count + 1)
        .map_err(|_| AuthorityTransactionError::Invalid("authority sequence exceeds u64".into()))?;

    let preflight = preflight_authority_action(
        authentication_adapter,
        &request.authentication_request,
        &request.runtime_session_state,
        &request.authorization_input,
    )
    .map_err(AuthorityTransactionError::Authentication)?;

    validate_preflight_attribution(&request, &preflight.authentication)?;
    validate_semantic_approvals(&request)?;

    let request_root = authorization_request_root(
        &request.authorization_input,
        &preflight.authorization_context,
    )?;
    let entity_snapshot_root = domain_root(
        b"vela.authority-entity-snapshot.internal.v1\0",
        &request.authorization_input.entities,
    )?;
    normalize_read_set(&mut request.read_set)?;
    let authority_keyset_root = request
        .history
        .authority_keyset
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    let policy_bundle_root = request
        .history
        .policy_bundle
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;

    let transaction_id = transaction_id(
        &request,
        &history.final_event_log_root,
        &previous_authority_record_root,
        &request_root,
        &entity_snapshot_root,
        &authority_keyset_root,
        &policy_bundle_root,
    )?;
    let mut events = request
        .event_drafts
        .iter()
        .map(|draft| {
            AuthorityEventV1::new(AuthorityEventContentV1 {
                transaction_id: transaction_id.clone(),
                principal_id: request.principal.principal_id.clone(),
                authority_mode: AUTHORITY_MODE.into(),
                kind: draft.kind.clone(),
                target: draft.target.clone(),
                actor: draft.actor.clone(),
                timestamp: draft.timestamp.clone(),
                reason: draft.reason.clone(),
                before_hash: draft.before_hash.clone(),
                after_hash: draft.after_hash.clone(),
                payload: draft.payload.clone(),
                caveats: draft.caveats.clone(),
            })
            .map_err(AuthorityTransactionError::Invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    events.sort_by(|left, right| left.id.cmp(&right.id));
    if events
        .windows(2)
        .any(|pair| pair[0].id.as_str() == pair[1].id.as_str())
    {
        return Err(AuthorityTransactionError::Invalid(
            "transaction derives duplicate Era-1 event IDs".into(),
        ));
    }

    let mut cumulative_events = request.history.authority_events.iter().collect::<Vec<_>>();
    cumulative_events.extend(events.iter());
    let legacy_root_with_bridge =
        format!("sha256:{}", event_log_hash(&request.history.legacy_events));
    let after_event_log_root =
        authority_event_log_root(&legacy_root_with_bridge, &cumulative_events)
            .map_err(AuthorityTransactionError::History)?;

    let event_ids = events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let object_delta = events
        .iter()
        .map(|event| {
            Ok(ObjectDeltaV1 {
                path: authority_event_path(&event.id),
                before_root: None,
                after_root: Some(event.root().map_err(AuthorityTransactionError::Invalid)?),
                object_kind: "event".into(),
            })
        })
        .collect::<Result<Vec<_>, AuthorityTransactionError>>()?;

    let read_set_root = read_set_root(
        &request,
        &history.final_event_log_root,
        &previous_authority_record_root,
        &authority_keyset_root,
        &policy_bundle_root,
    )?;
    let write_set_root = write_set_root(
        &transaction_id,
        &history.final_event_log_root,
        &after_event_log_root,
        &event_ids,
        &object_delta,
    )?;
    let operation_id = OperationId::derive(OPERATION_DOMAIN, transaction_id.as_bytes());

    let record = AuthorityRecordV1::new(AuthorityRecordContentV1 {
        frontier_id: request.history.frontier_id.clone(),
        sequence,
        previous_authority_record_root: Some(previous_authority_record_root),
        operation_id: operation_id.as_str().into(),
        transaction_id: transaction_id.clone(),
        intent_digest: request.intent_digest.clone(),
        before_event_log_root: history.final_event_log_root.clone(),
        after_event_log_root: after_event_log_root.clone(),
        event_ids: event_ids.clone(),
        object_delta,
        principal: request.principal.clone(),
        authentication: preflight.authentication,
        delegation: request.delegation.clone(),
        authorization: AuthorizationClaimV1 {
            policy_bundle_root,
            request_root: request_root.clone(),
            entity_snapshot_root,
            evaluation: preflight.authorization,
        },
        semantic_approvals: request.semantic_approvals.clone(),
        execution: ExecutionClaimV1 {
            vela_version: request.vela_version.clone(),
            binary_sha256: request.binary_sha256.clone(),
            transaction_read_set_root: read_set_root.clone(),
            transaction_write_set_root: write_set_root.clone(),
            completed_at: request.recorded_at.clone(),
        },
        authority_keyset_root,
        recorded_at: request.recorded_at.clone(),
    })
    .map_err(AuthorityTransactionError::Invalid)?;
    let canonical_record =
        to_canonical_bytes(&record).map_err(AuthorityTransactionError::Invalid)?;
    let signatures = signer
        .sign(AUTHORITY_PAYLOAD_TYPE_V1, &canonical_record)
        .map_err(AuthorityTransactionError::Signing)?;
    let envelope = AuthorityEnvelopeV1::from_record(&record, signatures)
        .map_err(AuthorityTransactionError::Invalid)?;
    let verified = verify_authority_envelope(
        &envelope,
        &request.history.authority_keyset,
        &request.history.frontier_id,
        sequence,
        record.content.previous_authority_record_root.as_deref(),
    )
    .map_err(AuthorityTransactionError::Signing)?;
    if verified.record_root != record.root().map_err(AuthorityTransactionError::Invalid)? {
        return Err(AuthorityTransactionError::Signing(
            "verified envelope root differs from the constructed record".into(),
        ));
    }

    let mut writes = events
        .iter()
        .map(|event| {
            Ok(PlannedWrite::write(
                RepoPath::parse(authority_event_path(&event.id))
                    .map_err(AuthorityTransactionError::Transaction)?,
                WriteClass::Authority,
                to_canonical_bytes(event).map_err(AuthorityTransactionError::Invalid)?,
            ))
        })
        .collect::<Result<Vec<_>, AuthorityTransactionError>>()?;
    writes.push(PlannedWrite::write(
        RepoPath::parse(authority_record_path(&record.record_id))
            .map_err(AuthorityTransactionError::Transaction)?,
        WriteClass::Authority,
        to_canonical_bytes(&envelope).map_err(AuthorityTransactionError::Invalid)?,
    ));
    let draft = DeltaDraft::prepare(frontier_root, writes)
        .map_err(AuthorityTransactionError::Transaction)?;
    if draft.delta.writes().len() != events.len() + 1
        || draft
            .delta
            .writes()
            .iter()
            .any(|write| !matches!(write.preimage, crate::frontier_txn::FileState::Absent))
    {
        return Err(AuthorityTransactionError::Invalid(
            "authority event and record paths must all be new".into(),
        ));
    }

    let legacy_event_root = ContentDigest::parse(format!(
        "sha256:{}",
        event_log_hash(&request.history.legacy_events)
    ))
    .map_err(AuthorityTransactionError::Transaction)?;
    let mut legacy_event_ids = request
        .history
        .legacy_events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    legacy_event_ids.sort();
    if legacy_event_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(AuthorityTransactionError::Invalid(
            "legacy event IDs are not unique".into(),
        ));
    }
    let layout_identity = to_canonical_bytes(&LayoutCommitment {
        schema: LAYOUT_SCHEMA,
        frontier_id: &request.history.frontier_id,
        authority_event_paths: &event_ids
            .iter()
            .map(|event_id| authority_event_path(event_id))
            .collect::<Vec<_>>(),
        authority_record_path: &authority_record_path(&record.record_id),
    })
    .map_err(AuthorityTransactionError::Invalid)?;
    let plan = FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind: OperationKind::Decision,
            operation_id,
            request_root: ContentDigest::parse(request.intent_digest.clone())
                .map_err(AuthorityTransactionError::Transaction)?,
            frontier: FrontierBinding::new(
                frontier_root,
                &request.history.frontier_id,
                &layout_identity,
            )
            .map_err(AuthorityTransactionError::Transaction)?,
            fixed_time: request.recorded_at,
            expected_event_log_root: legacy_event_root.clone(),
            resulting_event_log_root: legacy_event_root,
            resulting_event_ids: legacy_event_ids,
            read_set: request.read_set,
            result: serde_json::json!({
                "schema": "vela.authority-transaction-result.internal.v1",
                "transaction_id": transaction_id,
                "authority_record_id": record.record_id,
                "authority_record_root": verified.record_root,
                "event_ids": event_ids,
            }),
        },
        draft.delta.clone(),
    )
    .map_err(AuthorityTransactionError::Transaction)?;
    let transaction = FrontierTxn::prepare_with_barrier(barrier, plan, draft)
        .map_err(AuthorityTransactionError::Transaction)?;
    let result = AuthorityTransactionResult {
        operation_id: record.content.operation_id.clone(),
        transaction_id: record.content.transaction_id.clone(),
        event_ids: record.content.event_ids.clone(),
        authority_record_id: record.record_id.clone(),
        authority_record_root: verified.record_root,
        before_event_log_root: record.content.before_event_log_root.clone(),
        after_event_log_root: record.content.after_event_log_root.clone(),
        read_set_root,
        write_set_root,
    };
    Ok(PreparedAuthorityTransaction {
        transaction,
        result,
        events,
        envelope,
    })
}

pub(crate) fn execute_authority_transaction<A, S>(
    barrier: CanonicalWriteBarrier,
    frontier_root: &Path,
    request: AuthorityTransactionRequest,
    authentication_adapter: &mut A,
    signer: &mut S,
) -> Result<AuthorityTransactionResult, AuthorityTransactionError>
where
    A: AuthenticationAdapter,
    S: RepositoryAuthoritySigner,
{
    let mut prepared = prepare_authority_transaction(
        barrier,
        frontier_root,
        request,
        authentication_adapter,
        signer,
    )?;
    prepared.mark_committed()?;
    prepared.install()?;
    prepared.complete()?;
    Ok(prepared.result)
}

fn validate_request_shape(
    request: &AuthorityTransactionRequest,
) -> Result<(), AuthorityTransactionError> {
    request
        .history
        .authority_keyset
        .validate()
        .map_err(AuthorityTransactionError::Invalid)?;
    request
        .history
        .policy_bundle
        .validate()
        .map_err(AuthorityTransactionError::Invalid)?;
    if request.history.frontier_id != request.history.authority_keyset.frontier_id
        || request.history.frontier_id != request.history.policy_bundle.frontier_id
    {
        return Err(AuthorityTransactionError::Invalid(
            "history, keyset, and policy bundle name different frontiers".into(),
        ));
    }
    ContentDigest::parse(request.intent_digest.clone())
        .map_err(AuthorityTransactionError::Transaction)?;
    ContentDigest::parse(request.binary_sha256.clone())
        .map_err(AuthorityTransactionError::Transaction)?;
    if request.event_drafts.is_empty()
        || request.vela_version.trim().is_empty()
        || request.recorded_at.trim().is_empty()
        || request.authentication_request.transaction_at != request.recorded_at
    {
        return Err(AuthorityTransactionError::Invalid(
            "event set, version, and exact transaction time are required".into(),
        ));
    }
    for draft in &request.event_drafts {
        if draft.actor.id != request.principal.principal_id
            || draft.timestamp != request.recorded_at
            || draft.reason.trim().is_empty()
        {
            return Err(AuthorityTransactionError::Invalid(
                "event attribution, timestamp, or reason differs from the transaction".into(),
            ));
        }
    }
    Ok(())
}

fn validate_preflight_attribution(
    request: &AuthorityTransactionRequest,
    authentication: &AuthenticationObservationV1,
) -> Result<(), AuthorityTransactionError> {
    if request.principal.principal_id != authentication.principal_id
        || request.principal.principal_class != authentication.principal_class
        || !request
            .principal
            .account_links
            .contains(&authentication.principal_id)
    {
        return Err(AuthorityTransactionError::Invalid(
            "principal snapshot differs from verified authentication".into(),
        ));
    }
    if let Some(delegation) = &request.delegation {
        delegation
            .validate()
            .map_err(AuthorityTransactionError::Invalid)?;
        if delegation.subject_principal_id != authentication.principal_id
            || delegation.current_actor_principal_id != authentication.principal_id
            || delegation.frontier_id != request.history.frontier_id
        {
            return Err(AuthorityTransactionError::Invalid(
                "capability differs from the authenticated transaction".into(),
            ));
        }
    }
    Ok(())
}

fn validate_semantic_approvals(
    request: &AuthorityTransactionRequest,
) -> Result<(), AuthorityTransactionError> {
    let requires_human_approval =
        HUMAN_ONLY_AUTHORITY_ACTIONS_V1.contains(&request.authorization_input.action.as_str());
    if requires_human_approval && request.semantic_approvals.is_empty() {
        return Err(AuthorityTransactionError::Invalid(
            "human-only authority action lacks a semantic approval".into(),
        ));
    }
    for approval in &request.semantic_approvals {
        if approval.principal_id.trim().is_empty()
            || approval.role.trim().is_empty()
            || approval.action != request.authorization_input.action
            || approval.reason.trim().is_empty()
            || approval.approved_at != request.recorded_at
            || approval.intent_digest != request.intent_digest
        {
            return Err(AuthorityTransactionError::Invalid(
                "semantic approval does not bind the exact action, time, and intent".into(),
            ));
        }
    }
    Ok(())
}

fn normalize_read_set(read_set: &mut [InputBinding]) -> Result<(), AuthorityTransactionError> {
    read_set.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.digest.cmp(&right.digest))
    });
    if read_set.windows(2).any(|pair| pair[0].name == pair[1].name) {
        return Err(AuthorityTransactionError::Invalid(
            "authority transaction read-set names must be unique".into(),
        ));
    }
    Ok(())
}

fn transaction_id(
    request: &AuthorityTransactionRequest,
    before_event_log_root: &str,
    previous_authority_record_root: &str,
    authorization_request_root: &str,
    entity_snapshot_root: &str,
    authority_keyset_root: &str,
    policy_bundle_root: &str,
) -> Result<String, AuthorityTransactionError> {
    let root = domain_root(
        b"vela.authority-transaction-id.internal.v1\0",
        &TransactionIdCommitment {
            schema: TRANSACTION_ID_SCHEMA,
            frontier_id: &request.history.frontier_id,
            intent_digest: &request.intent_digest,
            before_event_log_root,
            previous_authority_record_root,
            principal_id: &request.principal.principal_id,
            authorization_request_root,
            entity_snapshot_root,
            authority_keyset_root,
            policy_bundle_root,
            delegation: request.delegation.as_ref(),
            semantic_approvals: &request.semantic_approvals,
            event_drafts: &request.event_drafts,
            read_set: &request.read_set,
            vela_version: &request.vela_version,
            binary_sha256: &request.binary_sha256,
            recorded_at: &request.recorded_at,
        },
    )?;
    Ok(format!(
        "vtx_{}",
        root.strip_prefix("sha256:")
            .expect("domain_root always returns sha256")
    ))
}

fn authorization_request_root(
    input: &CedarEvaluationInput,
    verified_context: &Value,
) -> Result<String, AuthorityTransactionError> {
    domain_root(
        b"vela.authority-authorization-request.internal.v1\0",
        &AuthorizationRequestCommitment {
            schema: "vela.authority-authorization-request.internal.v1",
            principal: &input.principal,
            principal_class: input.principal_class,
            action: &input.action,
            resource: &input.resource,
            context: verified_context,
        },
    )
}

fn read_set_root(
    request: &AuthorityTransactionRequest,
    current_event_log_root: &str,
    previous_authority_record_root: &str,
    authority_keyset_root: &str,
    policy_bundle_root: &str,
) -> Result<String, AuthorityTransactionError> {
    domain_root(
        b"vela.authority-read-set.internal.v1\0",
        &ReadSetCommitment {
            schema: READ_SET_SCHEMA,
            frontier_id: &request.history.frontier_id,
            current_event_log_root,
            previous_authority_record_root,
            authority_keyset_root,
            policy_bundle_root,
            inputs: &request.read_set,
        },
    )
}

fn write_set_root(
    transaction_id: &str,
    before_event_log_root: &str,
    after_event_log_root: &str,
    event_ids: &[String],
    object_delta: &[ObjectDeltaV1],
) -> Result<String, AuthorityTransactionError> {
    domain_root(
        b"vela.authority-write-set.internal.v1\0",
        &WriteSetCommitment {
            schema: WRITE_SET_SCHEMA,
            transaction_id,
            before_event_log_root,
            after_event_log_root,
            event_ids,
            object_delta,
        },
    )
}

fn domain_root(domain: &[u8], value: &impl Serialize) -> Result<String, AuthorityTransactionError> {
    let canonical = to_canonical_bytes(value).map_err(AuthorityTransactionError::Invalid)?;
    let mut preimage = Vec::with_capacity(domain.len() + canonical.len());
    preimage.extend_from_slice(domain);
    preimage.extend_from_slice(&canonical);
    Ok(ContentDigest::hash(preimage).as_str().to_string())
}

fn authority_event_path(event_id: &str) -> String {
    format!(".vela/authority/events/{event_id}.json")
}

fn authority_record_path(record_id: &str) -> String {
    format!(".vela/authority/records/{record_id}.dsse.json")
}

#[derive(Serialize)]
struct TransactionIdCommitment<'a> {
    schema: &'static str,
    frontier_id: &'a str,
    intent_digest: &'a str,
    before_event_log_root: &'a str,
    previous_authority_record_root: &'a str,
    principal_id: &'a str,
    authorization_request_root: &'a str,
    entity_snapshot_root: &'a str,
    authority_keyset_root: &'a str,
    policy_bundle_root: &'a str,
    delegation: Option<&'a DelegationClaimV1>,
    semantic_approvals: &'a [SemanticApprovalV1],
    event_drafts: &'a [AuthorityEventDraft],
    read_set: &'a [InputBinding],
    vela_version: &'a str,
    binary_sha256: &'a str,
    recorded_at: &'a str,
}

#[derive(Serialize)]
struct AuthorizationRequestCommitment<'a> {
    schema: &'static str,
    principal: &'a str,
    principal_class: vela_protocol::principal_capability::PrincipalClass,
    action: &'a str,
    resource: &'a str,
    context: &'a Value,
}

#[derive(Serialize)]
struct ReadSetCommitment<'a> {
    schema: &'static str,
    frontier_id: &'a str,
    current_event_log_root: &'a str,
    previous_authority_record_root: &'a str,
    authority_keyset_root: &'a str,
    policy_bundle_root: &'a str,
    inputs: &'a [InputBinding],
}

#[derive(Serialize)]
struct WriteSetCommitment<'a> {
    schema: &'static str,
    transaction_id: &'a str,
    before_event_log_root: &'a str,
    after_event_log_root: &'a str,
    event_ids: &'a [String],
    object_delta: &'a [ObjectDeltaV1],
}

#[derive(Serialize)]
struct LayoutCommitment<'a> {
    schema: &'static str,
    frontier_id: &'a str,
    authority_event_paths: &'a [String],
    authority_record_path: &'a str,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;
    use tempfile::TempDir;
    use vela_authority::runtime_authentication::{AuthenticationFailure, LocalOsSession};
    use vela_protocol::authentication::{
        AUTHENTICATION_OBSERVATION_SCHEMA_V1, AuthenticationAssurance, AuthenticationMethod,
    };
    use vela_protocol::authority::{
        AUTHORITY_KEY_ALGORITHM, AUTHORITY_KEY_PURPOSE, AUTHORITY_KEYSET_SCHEMA_V1, AuthorityKeyV1,
        CEDAR_ENGINE, CEDAR_ENGINE_VERSION, CEDAR_PROFILE_V1, CedarDecision, CedarEvaluation,
        DsseSignatureV1, POLICY_BUNDLE_SCHEMA_V1, SemanticApprovalV1, dsse_pae,
    };
    use vela_protocol::authority_history::{
        AUTHORITY_MIGRATION_ACTION, AUTHORITY_MODEL_MIGRATION_SCHEMA_V1, AuthorityModelMigrationV1,
    };
    use vela_protocol::canonical::to_canonical_bytes;
    use vela_protocol::events::{
        EVENT_SCHEMA, NULL_HASH, StateActor, StateEvent, StateTarget, compute_event_id,
    };
    use vela_protocol::principal_capability::PrincipalClass;
    use vela_protocol::sign::{ActorRecord, sign_event};

    use super::*;
    use crate::frontier_txn::{FrontierTxnStep, RecoveryOutcome};

    const FRONTIER_ID: &str = "vfr_0123456789abcdef";
    const LEGACY_ACTOR: &str = "reviewer:legacy";
    const REPOSITORY_PRINCIPAL: &str = "local:device-1|uid:501";
    const RECORDED_AT: &str = "2026-07-24T12:05:00Z";

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn canonical_root(value: &impl Serialize) -> String {
        ContentDigest::hash(to_canonical_bytes(value).unwrap())
            .as_str()
            .to_string()
    }

    struct TestSigner {
        key: SigningKey,
        calls: usize,
        fail: bool,
    }

    impl RepositoryAuthoritySigner for TestSigner {
        fn sign(
            &mut self,
            payload_type: &str,
            canonical_payload: &[u8],
        ) -> Result<Vec<DsseSignatureV1>, String> {
            self.calls += 1;
            if self.fail {
                return Err("injected signer refusal".into());
            }
            let signature = self.key.sign(&dsse_pae(payload_type, canonical_payload));
            Ok(vec![DsseSignatureV1 {
                keyid: "repository-key-1".into(),
                sig: BASE64_STANDARD.encode(signature.to_bytes()),
            }])
        }
    }

    struct CancelledAdapter;

    impl AuthenticationAdapter for CancelledAdapter {
        fn observe(
            &mut self,
            _request: &AuthenticationRequest,
        ) -> Result<AuthenticationObservationV1, AuthenticationFailure> {
            Err(AuthenticationFailure::Cancelled)
        }
    }

    struct Fixture {
        temporary: TempDir,
        request: AuthorityTransactionRequest,
        repository_key: SigningKey,
        actor_registry_bytes: Vec<u8>,
    }

    impl Fixture {
        fn journal_dir(&self) -> std::path::PathBuf {
            self.temporary.path().join(".vela/operation-journals")
        }

        fn barrier(&self) -> CanonicalWriteBarrier {
            FrontierTxn::acquire_write_barrier_for_test(self.temporary.path(), &self.journal_dir())
                .unwrap()
        }

        fn adapter(&self) -> LocalOsSession {
            LocalOsSession {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                issuer: "device-1".into(),
                subject: "uid:501".into(),
                session_root: root('8'),
                authenticated_at: "2026-07-24T12:00:00Z".into(),
                expires_at: "2026-07-24T13:00:00Z".into(),
                recovery_recent: false,
            }
        }

        fn signer(&self) -> TestSigner {
            TestSigner {
                key: self.repository_key.clone(),
                calls: 0,
                fail: false,
            }
        }
    }

    fn fixture() -> Fixture {
        let temporary = TempDir::new().unwrap();
        let root_path = temporary.path();
        fs::create_dir_all(root_path.join(".vela/events")).unwrap();

        let legacy_key = SigningKey::from_bytes(&[11; 32]);
        let repository_key = SigningKey::from_bytes(&[12; 32]);
        let actor_registry_bytes = serde_json::to_vec_pretty(&vec![ActorRecord {
            id: LEGACY_ACTOR.into(),
            public_key: hex::encode(legacy_key.verifying_key().to_bytes()),
            algorithm: "ed25519".into(),
            created_at: "2026-07-01T00:00:00Z".into(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        }])
        .unwrap();
        fs::write(root_path.join(".vela/actors.json"), &actor_registry_bytes).unwrap();

        let keyset = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            generation: 1,
            threshold: 1,
            keys: vec![AuthorityKeyV1 {
                key_id: "repository-key-1".into(),
                algorithm: AUTHORITY_KEY_ALGORITHM.into(),
                public_key: hex::encode(repository_key.verifying_key().to_bytes()),
                valid_from_sequence: 1,
                valid_through_sequence: None,
                purpose: AUTHORITY_KEY_PURPOSE.into(),
            }],
            previous_keyset_root: None,
            activation_record_root: None,
        };
        let policy_bundle = PolicyBundleV1 {
            schema: POLICY_BUNDLE_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            cedar_schema_root: root('a'),
            policies_root: root('b'),
            entities_root: root('c'),
            tests_root: root('d'),
            engine: CEDAR_ENGINE.into(),
            engine_version: CEDAR_ENGINE_VERSION.into(),
            restricted_profile: CEDAR_PROFILE_V1.into(),
            previous_bundle_root: None,
            authority_summary: "Repository authority may reject one exact proposal.".into(),
        };

        let mut genesis = StateEvent {
            schema: EVENT_SCHEMA.into(),
            id: String::new(),
            kind: EventKind::FrontierCreated,
            target: StateTarget {
                r#type: "frontier".into(),
                id: FRONTIER_ID.into(),
            },
            actor: StateActor {
                r#type: "system".into(),
                id: "vela:init".into(),
            },
            timestamp: "2026-07-01T00:00:00Z".into(),
            reason: "Create the disposable writer fixture.".into(),
            before_hash: NULL_HASH.into(),
            after_hash: NULL_HASH.into(),
            payload: json!({}),
            caveats: Vec::new(),
            signature: None,
        };
        genesis.id = compute_event_id(&genesis);
        let legacy_root = format!("sha256:{}", event_log_hash(&[genesis.clone()]));
        let migration_payload = AuthorityModelMigrationV1 {
            schema: AUTHORITY_MODEL_MIGRATION_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            legacy_event_log_root: legacy_root.clone(),
            legacy_actor_registry_root: ContentDigest::hash(&actor_registry_bytes).as_str().into(),
            legacy_active_policy_head_root: root('3'),
            legacy_policy_store_manifest_root: root('4'),
            new_authority_keyset_root: keyset.root().unwrap(),
            new_policy_bundle_root: policy_bundle.root().unwrap(),
            new_principal_id: REPOSITORY_PRINCIPAL.into(),
            minimum_writer_version: "0.930.0".into(),
            reason: "Move the disposable fixture to repository authority.".into(),
        };
        let mut migration = StateEvent {
            schema: EVENT_SCHEMA.into(),
            id: String::new(),
            kind: EventKind::AuthorityModelMigrated,
            target: StateTarget {
                r#type: "frontier".into(),
                id: FRONTIER_ID.into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: LEGACY_ACTOR.into(),
            },
            timestamp: "2026-07-24T12:00:00Z".into(),
            reason: migration_payload.reason.clone(),
            before_hash: NULL_HASH.into(),
            after_hash: NULL_HASH.into(),
            payload: serde_json::to_value(&migration_payload).unwrap(),
            caveats: vec!["Historical events remain byte-identical.".into()],
            signature: None,
        };
        migration.id = compute_event_id(&migration);
        migration.signature = Some(sign_event(&migration, &legacy_key).unwrap());
        let legacy_events = vec![genesis, migration.clone()];
        for event in &legacy_events {
            fs::write(
                root_path
                    .join(".vela/events")
                    .join(format!("{}.json", event.id)),
                to_canonical_bytes(event).unwrap(),
            )
            .unwrap();
        }
        let legacy_root_with_bridge = format!("sha256:{}", event_log_hash(&legacy_events));

        let first_record = AuthorityRecordV1::new(AuthorityRecordContentV1 {
            frontier_id: FRONTIER_ID.into(),
            sequence: 1,
            previous_authority_record_root: None,
            operation_id: "vop_migration".into(),
            transaction_id: "vtx_migration".into(),
            intent_digest: root('e'),
            before_event_log_root: legacy_root,
            after_event_log_root: legacy_root_with_bridge,
            event_ids: vec![migration.id.clone()],
            object_delta: vec![ObjectDeltaV1 {
                path: format!(".vela/events/{}.json", migration.id),
                before_root: None,
                after_root: Some(canonical_root(&migration)),
                object_kind: "event".into(),
            }],
            principal: PrincipalSnapshotV1 {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                principal_class: PrincipalClass::Human,
                display_name: Some("Repository administrator".into()),
                affiliation: None,
                account_links: vec![REPOSITORY_PRINCIPAL.into()],
            },
            authentication: AuthenticationObservationV1 {
                schema: AUTHENTICATION_OBSERVATION_SCHEMA_V1.into(),
                principal_id: REPOSITORY_PRINCIPAL.into(),
                principal_class: PrincipalClass::Human,
                issuer: "device-1".into(),
                subject: "uid:501".into(),
                method: AuthenticationMethod::LocalOsSession,
                assurance: AuthenticationAssurance::LocalSession,
                session_root: root('7'),
                authenticated_at: "2026-07-24T12:00:00Z".into(),
                observed_at: "2026-07-24T12:00:00Z".into(),
                expires_at: "2026-07-24T13:00:00Z".into(),
                user_presence: false,
                user_verification: false,
                recovery_recent: false,
                revocation_ref: None,
            },
            delegation: None,
            authorization: AuthorizationClaimV1 {
                policy_bundle_root: policy_bundle.root().unwrap(),
                request_root: root('5'),
                entity_snapshot_root: root('6'),
                evaluation: CedarEvaluation {
                    engine: CEDAR_ENGINE.into(),
                    engine_version: CEDAR_ENGINE_VERSION.into(),
                    profile: CEDAR_PROFILE_V1.into(),
                    valid: true,
                    decision: CedarDecision::Allow,
                    automatic_permit: false,
                    determining_policies: vec!["permit_repository_admin".into()],
                    diagnostics: Vec::new(),
                },
            },
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: LEGACY_ACTOR.into(),
                role: "frontier_administrator".into(),
                action: AUTHORITY_MIGRATION_ACTION.into(),
                reason: migration_payload.reason,
                approved_at: "2026-07-24T12:00:00Z".into(),
                intent_digest: root('e'),
            }],
            execution: ExecutionClaimV1 {
                vela_version: "0.930.0-rc.1".into(),
                binary_sha256: root('8'),
                transaction_read_set_root: root('9'),
                transaction_write_set_root: root('0'),
                completed_at: "2026-07-24T12:00:01Z".into(),
            },
            authority_keyset_root: keyset.root().unwrap(),
            recorded_at: "2026-07-24T12:00:01Z".into(),
        })
        .unwrap();
        let first_payload = to_canonical_bytes(&first_record).unwrap();
        let first_envelope = AuthorityEnvelopeV1 {
            payload_type: AUTHORITY_PAYLOAD_TYPE_V1.into(),
            payload: BASE64_STANDARD.encode(&first_payload),
            signatures: vec![DsseSignatureV1 {
                keyid: "repository-key-1".into(),
                sig: BASE64_STANDARD.encode(
                    repository_key
                        .sign(&dsse_pae(AUTHORITY_PAYLOAD_TYPE_V1, &first_payload))
                        .to_bytes(),
                ),
            }],
        };

        let authorization_input = CedarEvaluationInput {
            schema: r#"
                entity Human;
                entity Proposal;
                action "review_reject" appliesTo {
                    principal: Human,
                    resource: Proposal,
                    context: {
                        exact: Bool,
                        authentication: {
                            method: String,
                            assurance: String,
                            authenticated_at: String,
                            observed_at: String,
                            expires_at: String,
                            user_presence: Bool,
                            user_verification: Bool,
                            recovery_recent: Bool
                        }
                    }
                };
            "#
            .into(),
            policies: r#"
                permit(principal, action, resource)
                when { context.exact };
            "#
            .into(),
            entities: json!([
                {
                    "uid": {"type": "Human", "id": REPOSITORY_PRINCIPAL},
                    "attrs": {},
                    "parents": []
                },
                {
                    "uid": {"type": "Proposal", "id": "vpr_0123456789abcdef"},
                    "attrs": {},
                    "parents": []
                }
            ]),
            principal: format!(r#"Human::"{REPOSITORY_PRINCIPAL}""#),
            principal_class: PrincipalClass::Human,
            action: "review_reject".into(),
            resource: r#"Proposal::"vpr_0123456789abcdef""#.into(),
            context: json!({"exact": true}),
        };
        let actor_input = InputBinding::existing_file(
            root_path,
            RepoPath::parse(".vela/actors.json".to_string()).unwrap(),
        )
        .unwrap();
        let request = AuthorityTransactionRequest {
            history: AuthorityHistorySnapshot {
                frontier_id: FRONTIER_ID.into(),
                legacy_events,
                legacy_actor_registry_bytes: actor_registry_bytes.clone(),
                legacy_active_policy_head_root: root('3'),
                legacy_policy_store_manifest_root: root('4'),
                authority_keyset: keyset,
                policy_bundle,
                authority_events: Vec::new(),
                authority_envelopes: vec![first_envelope],
            },
            intent_digest: root('2'),
            principal: PrincipalSnapshotV1 {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                principal_class: PrincipalClass::Human,
                display_name: Some("Repository administrator".into()),
                affiliation: None,
                account_links: vec![REPOSITORY_PRINCIPAL.into()],
            },
            authentication_request: AuthenticationRequest {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                principal_class: PrincipalClass::Human,
                transaction_at: RECORDED_AT.into(),
            },
            runtime_session_state: RuntimeSessionState::default(),
            authorization_input,
            delegation: None,
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                role: "frontier_administrator".into(),
                action: "review_reject".into(),
                reason: "Reject the disposable fixture proposal.".into(),
                approved_at: RECORDED_AT.into(),
                intent_digest: root('2'),
            }],
            event_drafts: vec![AuthorityEventDraft {
                kind: EventKind::ReviewRejected,
                target: StateTarget {
                    r#type: "proposal".into(),
                    id: "vpr_0123456789abcdef".into(),
                },
                actor: StateActor {
                    r#type: "human".into(),
                    id: REPOSITORY_PRINCIPAL.into(),
                },
                timestamp: RECORDED_AT.into(),
                reason: "Reject the disposable fixture proposal.".into(),
                before_hash: root('f'),
                after_hash: root('f'),
                payload: json!({"proposal_id": "vpr_0123456789abcdef"}),
                caveats: Vec::new(),
            }],
            read_set: vec![actor_input],
            vela_version: "0.930.0-rc.1".into(),
            binary_sha256: root('1'),
            recorded_at: RECORDED_AT.into(),
        };

        Fixture {
            temporary,
            request,
            repository_key,
            actor_registry_bytes,
        }
    }

    fn verify_installed_history(
        fixture: &Fixture,
        events: &[AuthorityEventV1],
        envelope: &AuthorityEnvelopeV1,
    ) {
        let mut envelopes = fixture.request.history.authority_envelopes.clone();
        envelopes.push(envelope.clone());
        let result = verify_authority_history(AuthorityHistoryInput {
            frontier_id: FRONTIER_ID,
            legacy_events: &fixture.request.history.legacy_events,
            legacy_actor_registry_bytes: &fixture.actor_registry_bytes,
            legacy_active_policy_head_root: &fixture.request.history.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &fixture
                .request
                .history
                .legacy_policy_store_manifest_root,
            authority_keyset: &fixture.request.history.authority_keyset,
            policy_bundle: &fixture.request.history.policy_bundle,
            authority_events: events,
            authority_envelopes: &envelopes,
        })
        .unwrap();
        assert_eq!(result.authority_event_count, 1);
        assert_eq!(result.authority_record_count, 2);
    }

    fn authority_store_absent(fixture: &Fixture) -> bool {
        !fixture.temporary.path().join(".vela/authority").exists()
    }

    fn prepared_journal_absent(fixture: &Fixture) -> bool {
        !fixture.journal_dir().join("frontier").exists()
    }

    #[test]
    fn disposable_writer_installs_one_exact_transaction_and_replays_offline() {
        let fixture = fixture();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = execute_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            fixture.request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        assert_eq!(signer.calls, 1);
        assert_eq!(result.event_ids.len(), 1);
        assert!(result.transaction_id.starts_with("vtx_"));

        let event_bytes = fs::read(
            fixture
                .temporary
                .path()
                .join(authority_event_path(&result.event_ids[0])),
        )
        .unwrap();
        let event: AuthorityEventV1 = serde_json::from_slice(&event_bytes).unwrap();
        assert_eq!(event_bytes, to_canonical_bytes(&event).unwrap());
        let envelope_bytes = fs::read(
            fixture
                .temporary
                .path()
                .join(authority_record_path(&result.authority_record_id)),
        )
        .unwrap();
        let envelope: AuthorityEnvelopeV1 = serde_json::from_slice(&envelope_bytes).unwrap();
        assert_eq!(envelope_bytes, to_canonical_bytes(&envelope).unwrap());

        verify_installed_history(&fixture, &[event], &envelope);
    }

    #[test]
    fn authentication_and_signer_failures_create_no_canonical_or_prepared_bytes() {
        let fixture_one = fixture();
        let mut cancelled = CancelledAdapter;
        let mut signer = fixture_one.signer();
        let error = prepare_authority_transaction(
            fixture_one.barrier(),
            fixture_one.temporary.path(),
            fixture_one.request.clone(),
            &mut cancelled,
            &mut signer,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AuthorityTransactionError::Authentication(AuthorityPreflightFailure::Authentication(
                AuthenticationFailure::Cancelled
            ))
        ));
        assert_eq!(signer.calls, 0);
        assert!(authority_store_absent(&fixture_one));
        assert!(prepared_journal_absent(&fixture_one));

        let fixture_two = fixture();
        let mut adapter = fixture_two.adapter();
        let mut signer = fixture_two.signer();
        signer.fail = true;
        let error = prepare_authority_transaction(
            fixture_two.barrier(),
            fixture_two.temporary.path(),
            fixture_two.request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(matches!(error, AuthorityTransactionError::Signing(_)));
        assert_eq!(signer.calls, 1);
        assert!(authority_store_absent(&fixture_two));
        assert!(prepared_journal_absent(&fixture_two));
    }

    #[test]
    fn history_and_policy_substitution_fail_before_signing_or_journaling() {
        let fixture_one = fixture();
        let mut request = fixture_one.request.clone();
        request.history.authority_envelopes[0].payload.push('A');
        let mut adapter = fixture_one.adapter();
        let mut signer = fixture_one.signer();
        let error = prepare_authority_transaction(
            fixture_one.barrier(),
            fixture_one.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(matches!(error, AuthorityTransactionError::History(_)));
        assert_eq!(signer.calls, 0);
        assert!(authority_store_absent(&fixture_one));
        assert!(prepared_journal_absent(&fixture_one));

        let fixture_two = fixture();
        let mut request = fixture_two.request.clone();
        request.authorization_input.policies = "forbid(principal, action, resource);".into();
        let mut adapter = fixture_two.adapter();
        let mut signer = fixture_two.signer();
        let error = prepare_authority_transaction(
            fixture_two.barrier(),
            fixture_two.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AuthorityTransactionError::Authentication(
                AuthorityPreflightFailure::AuthorizationDenied
            )
        ));
        assert_eq!(signer.calls, 0);
        assert!(authority_store_absent(&fixture_two));
        assert!(prepared_journal_absent(&fixture_two));
    }

    #[test]
    fn stale_read_set_aborts_before_commit_marker_and_installs_nothing() {
        let fixture = fixture();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            fixture.request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        assert!(authority_store_absent(&fixture));
        fs::write(
            fixture.temporary.path().join(".vela/actors.json"),
            b"tampered after semantic approval",
        )
        .unwrap();
        let error = prepared.mark_committed().unwrap_err();
        assert!(matches!(
            error,
            AuthorityTransactionError::Transaction(FrontierTxnError::StaleInput { .. })
        ));
        assert!(authority_store_absent(&fixture));
        assert!(!prepared.transaction.plan().operation_id.as_str().is_empty());
        assert!(matches!(
            prepared.transaction.recovery_state(),
            crate::frontier_txn::RecoveryState::Aborted
        ));
    }

    #[test]
    fn transaction_identity_binds_read_set_and_execution_pin() {
        let fixture_one = fixture();
        let mut adapter = fixture_one.adapter();
        let mut signer = fixture_one.signer();
        let baseline = prepare_authority_transaction(
            fixture_one.barrier(),
            fixture_one.temporary.path(),
            fixture_one.request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap()
        .result
        .transaction_id;

        let fixture_two = fixture();
        let mut request = fixture_two.request.clone();
        request.read_set[0].digest = ContentDigest::parse(root('a')).unwrap();
        let mut adapter = fixture_two.adapter();
        let mut signer = fixture_two.signer();
        let changed_read_set = prepare_authority_transaction(
            fixture_two.barrier(),
            fixture_two.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap()
        .result
        .transaction_id;

        let fixture_three = fixture();
        let mut request = fixture_three.request.clone();
        request.binary_sha256 = root('b');
        let mut adapter = fixture_three.adapter();
        let mut signer = fixture_three.signer();
        let changed_binary = prepare_authority_transaction(
            fixture_three.barrier(),
            fixture_three.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap()
        .result
        .transaction_id;

        assert_ne!(baseline, changed_read_set);
        assert_ne!(baseline, changed_binary);
    }

    #[test]
    fn committed_partial_install_recovers_without_authentication_or_resigning() {
        let fixture = fixture();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            fixture.request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        let result = prepared.result.clone();
        let events = prepared.events.clone();
        let envelope = prepared.envelope.clone();
        prepared.mark_committed().unwrap();
        let error = prepared
            .transaction_mut()
            .install_at_failpoint(FrontierTxnStep::BeforeInstallWrite { index: 1 })
            .unwrap_err();
        assert!(matches!(
            error,
            FrontierTxnError::InjectedFailure {
                step: FrontierTxnStep::BeforeInstallWrite { index: 1 }
            }
        ));
        drop(prepared);
        let operation_id = OperationId::parse(result.operation_id).unwrap();
        assert_eq!(
            FrontierTxn::recover(
                fixture.temporary.path(),
                &fixture.journal_dir(),
                &operation_id,
            )
            .unwrap(),
            RecoveryOutcome::Completed
        );
        assert_eq!(signer.calls, 1);
        verify_installed_history(&fixture, &events, &envelope);
    }
}
