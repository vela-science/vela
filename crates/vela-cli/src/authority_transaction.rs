//! Repository-authority transaction writer.
//!
//! The writer composes authority records, runtime authentication, and the
//! existing recoverable repository transaction without introducing a second
//! journal. Fresh repository initialization and exact attributed Decisions use
//! this same core.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use vela_authority::runtime_authentication::{
    AuthenticationAdapter, AuthenticationRequest, AuthorityPreflightFailure, RuntimeSessionState,
    preflight_authority_action,
};
use vela_protocol::authentication::AuthenticationObservationV1;
use vela_protocol::authority::{
    AUTHORITY_MODE, AUTHORITY_PAYLOAD_TYPE_V1, AuthorityEnvelopeV1, AuthorityEventContentV1,
    AuthorityEventV1, AuthorityKeysetV1, AuthorityRecordContentV1, AuthorityRecordV1,
    AuthorizationClaimV1, DsseSignatureV1, ExecutionClaimV1, ObjectDeltaV1, PrincipalSnapshotV1,
    SemanticApprovalV1, authority_envelope, verify_authority_envelope,
};
use vela_protocol::authority_history::{
    AUTHORITY_INITIALIZE_ACTION, AUTHORITY_INITIALIZED_EVENT_KIND, AuthorityHistoryEra,
    AuthorityHistoryInput, AuthorityHistoryVerification, AuthorityInitializationV1,
    authority_event_log_root, verify_authority_history,
};
use vela_protocol::authorization::{
    AuthorityActionV1, AuthorizationModelV1, AuthorizationRequestV1,
};
use vela_protocol::canonical::sha256_root;
use vela_protocol::canonical::to_canonical_bytes;
#[cfg(test)]
use vela_protocol::events::event_log_hash;
use vela_protocol::events::{EventKind, StateActor, StateTarget};

use vela_repository::{
    CanonicalWriteBarrier, ContentDigest, DeltaDraft, InputBinding, OperationId, OperationKind,
    PlannedWrite, RepoPath, RepositoryBinding, RepositoryTxn, RepositoryTxnError,
    RepositoryTxnPlan, RepositoryTxnPlanSpec, WriteClass,
};

const TRANSACTION_ID_SCHEMA: &str = "vela.authority-transaction-id.internal.v1";
const READ_SET_SCHEMA: &str = "vela.authority-read-set.internal.v1";
const WRITE_SET_SCHEMA: &str = "vela.authority-write-set.internal.v1";
pub(crate) const RESULT_SCHEMA: &str = "vela.authority-transaction-result.internal.v1";
pub(crate) const OPERATION_DOMAIN: &str = "authority_transaction";
pub(crate) const REPOSITORY_OPERATION_KIND: &str = "decision";

/// Complete verified-history input for the next transaction.
///
/// The writer re-runs repository-origin verification itself. It does not accept a
/// caller-asserted head root or sequence.
#[derive(Debug, Clone)]
pub(crate) struct AuthorityHistorySnapshot {
    pub(crate) repository_id: String,
    pub(crate) initial_event_log_root: String,
    pub(crate) initial_actor_registry_root: String,
    pub(crate) authority_keyset: AuthorityKeysetV1,
    pub(crate) authorization_model: AuthorizationModelV1,
    pub(crate) retained_authority_keysets: Vec<AuthorityKeysetV1>,
    pub(crate) retained_authorization_models: Vec<AuthorizationModelV1>,
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

/// Canonical non-event postimage covered by the same authority transaction.
///
/// `None` deletes an existing object. The writer derives before/after roots
/// from the held repository and refuses no-ops, duplicate paths, private
/// coordination, retired protocol paths, or covering-record paths.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct AuthorityObjectDraft {
    pub(crate) path: String,
    pub(crate) object_kind: String,
    pub(crate) class: WriteClass,
    pub(crate) postimage: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthorityTransactionRequest {
    pub(crate) history: AuthorityHistorySnapshot,
    pub(crate) intent_digest: String,
    pub(crate) principal: PrincipalSnapshotV1,
    pub(crate) authentication_request: AuthenticationRequest,
    /// The request the closed evaluator decides, minus the two fields the
    /// preflight fills from the verified session: `recovery_recent` and
    /// `authentication_root`.
    pub(crate) authorization_request: AuthorizationRequestV1,
    pub(crate) semantic_approvals: Vec<SemanticApprovalV1>,
    pub(crate) event_drafts: Vec<AuthorityEventDraft>,
    pub(crate) object_drafts: Vec<AuthorityObjectDraft>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DurableAuthorityTransactionResult {
    schema: String,
    result: AuthorityTransactionResult,
}

#[derive(Debug)]
pub(crate) struct PreparedAuthorityTransaction {
    transaction: RepositoryTxn,
    pub(crate) result: AuthorityTransactionResult,
    #[cfg(test)]
    pub(crate) events: Vec<AuthorityEventV1>,
    #[cfg(test)]
    pub(crate) envelope: AuthorityEnvelopeV1,
}

impl PreparedAuthorityTransaction {
    pub(crate) fn resolved_public_writes(
        &self,
    ) -> Result<Vec<vela_repository::ResolvedWrite>, AuthorityTransactionError> {
        self.transaction
            .resolved_public_writes()
            .map_err(AuthorityTransactionError::Transaction)
    }

    pub(crate) fn canonical_delta_root(&self) -> &str {
        self.transaction.canonical_delta_root()
    }

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

    pub(crate) fn retire_completed_recovery_blobs(
        &mut self,
    ) -> Result<usize, AuthorityTransactionError> {
        self.transaction
            .retire_completed_recovery_blobs()
            .map_err(AuthorityTransactionError::Transaction)
    }

    #[cfg(test)]
    fn transaction_mut(&mut self) -> &mut RepositoryTxn {
        &mut self.transaction
    }
}

#[derive(Debug)]
pub(crate) enum AuthorityTransactionError {
    Invalid(String),
    History(String),
    Authentication(AuthorityPreflightFailure),
    Signing(String),
    Transaction(RepositoryTxnError),
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
/// Authentication, repository-origin verification, authorization, transaction
/// construction, and DSSE verification all complete before the first journal
/// byte is created. The returned transaction still has no commit marker and no
/// canonical postimage.
pub(crate) fn prepare_authority_transaction<A, S>(
    barrier: CanonicalWriteBarrier,
    repository_root: &Path,
    mut request: AuthorityTransactionRequest,
    authentication_adapter: &mut A,
    signer: &mut S,
) -> Result<PreparedAuthorityTransaction, AuthorityTransactionError>
where
    A: AuthenticationAdapter,
    S: RepositoryAuthoritySigner + ?Sized,
{
    if request.event_drafts.is_empty() && request.object_drafts.is_empty() {
        return Err(AuthorityTransactionError::Invalid(
            "authority transaction intent changes no event or object".into(),
        ));
    }
    normalize_authority_snapshots(repository_root, &mut request)?;
    normalize_object_drafts(repository_root, &mut request)?;
    validate_request_shape(&request)?;
    let history = verify_authority_history(AuthorityHistoryInput {
        repository_id: &request.history.repository_id,
        initial_event_log_root: &request.history.initial_event_log_root,
        initial_actor_registry_root: &request.history.initial_actor_registry_root,
        authority_keysets: &request.history.retained_authority_keysets,
        authorization_models: &request.history.retained_authorization_models,
        authority_events: &request.history.authority_events,
        authority_envelopes: &request.history.authority_envelopes,
    })
    .map_err(AuthorityTransactionError::History)?;
    bind_repository_authority_history(repository_root, &mut request)?;
    let fresh_initialization = history.era == AuthorityHistoryEra::Uninitialized;
    if fresh_initialization {
        validate_fresh_initialization_request(&request)?;
    }
    let previous_authority_record_root = history.final_authority_record_root.clone();
    if !fresh_initialization && previous_authority_record_root.is_none() {
        return Err(AuthorityTransactionError::History(
            "repository-authority history has no previous record root".into(),
        ));
    }
    let sequence = u64::try_from(history.authority_record_count + 1)
        .map_err(|_| AuthorityTransactionError::Invalid("authority sequence exceeds u64".into()))?;
    if fresh_initialization {
        if sequence != 1 {
            return Err(AuthorityTransactionError::History(
                "fresh authority initialization must create sequence 1".into(),
            ));
        }
    } else {
        validate_active_authority_snapshots(&request, &history)?;
    }

    let preflight = preflight_authority_action(
        authentication_adapter,
        &request.authentication_request,
        &RuntimeSessionState::default(),
        &request.history.authorization_model,
        &request.authorization_request,
    )
    .map_err(AuthorityTransactionError::Authentication)?;

    validate_preflight_attribution(&request, &preflight.authentication)?;
    validate_semantic_approvals(&request)?;

    /* The request has its own root. It used to be a Cedar principal, action,
    resource and free-form context hashed under an internal domain tag,
    because there was no typed request to take a root over. */
    let request_root = preflight
        .request
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    normalize_read_set(&mut request.read_set)?;
    let authority_keyset_root = request
        .history
        .authority_keyset
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    let model_root = request
        .history
        .authorization_model
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;

    let transaction_id = transaction_id(
        &request,
        &history.final_event_log_root,
        previous_authority_record_root.as_deref(),
        &request_root,
        &authority_keyset_root,
        &model_root,
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
            "transaction derives duplicate authority event IDs".into(),
        ));
    }
    validate_semantic_event_links(&request.history, &events)?;

    let mut cumulative_events = request.history.authority_events.iter().collect::<Vec<_>>();
    cumulative_events.extend(events.iter());
    let after_event_log_root = if events.is_empty() {
        history.final_event_log_root.clone()
    } else {
        authority_event_log_root(&request.history.initial_event_log_root, &cumulative_events)
            .map_err(AuthorityTransactionError::History)?
    };

    let event_ids = events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let mut content_writes = events
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
    content_writes.extend(
        request
            .object_drafts
            .iter()
            .map(authority_object_planned_write)
            .collect::<Result<Vec<_>, _>>()?,
    );
    let unsigned_draft = DeltaDraft::prepare(repository_root, content_writes.clone())
        .map_err(AuthorityTransactionError::Transaction)?;
    let full_object_delta =
        authority_object_delta(&unsigned_draft, &events, &request.object_drafts)?;
    let record_object_delta = full_object_delta.clone();

    let read_set_root = read_set_root(
        &request,
        &history.final_event_log_root,
        previous_authority_record_root.as_deref(),
        &authority_keyset_root,
        &model_root,
    )?;
    let write_set_root = write_set_root(
        &transaction_id,
        &history.final_event_log_root,
        &after_event_log_root,
        &event_ids,
        &full_object_delta,
    )?;
    let operation_id = OperationId::derive(OPERATION_DOMAIN, transaction_id.as_bytes());

    let record = AuthorityRecordV1::new(AuthorityRecordContentV1 {
        repository_id: request.history.repository_id.clone(),
        sequence,
        previous_authority_record_root,
        operation_id: operation_id.as_str().into(),
        transaction_id: transaction_id.clone(),
        intent_digest: request.intent_digest.clone(),
        before_event_log_root: history.final_event_log_root.clone(),
        after_event_log_root: after_event_log_root.clone(),
        event_ids: event_ids.clone(),
        object_delta: record_object_delta.clone(),
        principal: request.principal.clone(),
        authentication: preflight.authentication,
        delegation: None,
        authorization: AuthorizationClaimV1 {
            model_root: model_root.clone(),
            request: preflight.request,
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
        authority_keyset_root: authority_keyset_root.clone(),
        recorded_at: request.recorded_at.clone(),
    })
    .map_err(AuthorityTransactionError::Invalid)?;
    let canonical_record =
        to_canonical_bytes(&record).map_err(AuthorityTransactionError::Invalid)?;
    let signatures = signer
        .sign(AUTHORITY_PAYLOAD_TYPE_V1, &canonical_record)
        .map_err(AuthorityTransactionError::Signing)?;
    let envelope =
        authority_envelope(&record, signatures).map_err(AuthorityTransactionError::Invalid)?;
    let verified = verify_authority_envelope(
        &envelope,
        &request.history.authority_keyset,
        &request.history.repository_id,
        sequence,
        record.content.previous_authority_record_root.as_deref(),
    )
    .map_err(AuthorityTransactionError::Signing)?;
    if verified.record_root != record.root().map_err(AuthorityTransactionError::Invalid)? {
        return Err(AuthorityTransactionError::Signing(
            "verified envelope root differs from the constructed record".into(),
        ));
    }
    let mut candidate_keysets = request.history.retained_authority_keysets.clone();
    if fresh_initialization {
        candidate_keysets.push(request.history.authority_keyset.clone());
    }
    let mut candidate_policies = request.history.retained_authorization_models.clone();
    if fresh_initialization {
        candidate_policies.push(request.history.authorization_model.clone());
    }
    let mut candidate_events = request.history.authority_events.clone();
    candidate_events.extend(events.iter().cloned());
    let mut candidate_envelopes = request.history.authority_envelopes.clone();
    candidate_envelopes.push(envelope.clone());
    let candidate_history = verify_authority_history(AuthorityHistoryInput {
        repository_id: &request.history.repository_id,
        initial_event_log_root: &request.history.initial_event_log_root,
        initial_actor_registry_root: &request.history.initial_actor_registry_root,
        authority_keysets: &candidate_keysets,
        authorization_models: &candidate_policies,
        authority_events: &candidate_events,
        authority_envelopes: &candidate_envelopes,
    })
    .map_err(AuthorityTransactionError::History)?;
    if candidate_history.final_authority_record_root.as_deref()
        != Some(verified.record_root.as_str())
        || candidate_history.final_authority_keyset_root.as_deref()
            != Some(authority_keyset_root.as_str())
        || candidate_history.final_authorization_model_root.as_deref() != Some(model_root.as_str())
    {
        return Err(AuthorityTransactionError::History(
            "candidate transaction does not produce the exact expected authority head".into(),
        ));
    }

    let mut writes = content_writes;
    writes.push(PlannedWrite::write(
        RepoPath::parse(authority_record_path(&record.record_id))
            .map_err(AuthorityTransactionError::Transaction)?,
        WriteClass::Authority,
        to_canonical_bytes(&envelope).map_err(AuthorityTransactionError::Invalid)?,
    ));
    let draft = DeltaDraft::prepare(repository_root, writes)
        .map_err(AuthorityTransactionError::Transaction)?;
    let record_path = authority_record_path(&record.record_id);
    let record_writes = draft
        .delta
        .writes()
        .iter()
        .filter(|write| write.path.as_str() == record_path)
        .collect::<Vec<_>>();
    let content_delta = draft
        .delta
        .writes()
        .iter()
        .filter(|write| write.path.as_str() != record_path)
        .cloned()
        .collect::<Vec<_>>();
    if record_writes.len() != 1
        || !matches!(
            record_writes[0].preimage,
            vela_repository::FileState::Absent
        )
        || content_delta != unsigned_draft.delta.writes()
        || record.content.object_delta != record_object_delta
    {
        return Err(AuthorityTransactionError::Invalid(
            "covering record must be new and its signed commitments must cover the exact journal delta"
                .into(),
        ));
    }

    let result = AuthorityTransactionResult {
        operation_id: record.content.operation_id.clone(),
        transaction_id: record.content.transaction_id.clone(),
        event_ids: record.content.event_ids.clone(),
        authority_record_id: record.record_id.clone(),
        authority_record_root: verified.record_root.clone(),
        before_event_log_root: record.content.before_event_log_root.clone(),
        after_event_log_root: record.content.after_event_log_root.clone(),
        read_set_root,
        write_set_root,
    };
    let plan = RepositoryTxnPlan::new(
        RepositoryTxnPlanSpec {
            kind: OperationKind::new(REPOSITORY_OPERATION_KIND)
                .map_err(AuthorityTransactionError::Transaction)?,
            operation_id,
            request_root: ContentDigest::parse(request.intent_digest.clone())
                .map_err(AuthorityTransactionError::Transaction)?,
            repository: RepositoryBinding::new(repository_root, &request.history.repository_id)
                .map_err(AuthorityTransactionError::Transaction)?,
            fixed_time: request.recorded_at,
            read_set: request.read_set,
            result: serde_json::to_value(DurableAuthorityTransactionResult {
                schema: RESULT_SCHEMA.into(),
                result: result.clone(),
            })
            .map_err(|error| AuthorityTransactionError::Invalid(error.to_string()))?,
        },
        draft.delta.clone(),
    )
    .map_err(AuthorityTransactionError::Transaction)?;
    let transaction = RepositoryTxn::prepare_with_barrier(barrier, plan, draft)
        .map_err(AuthorityTransactionError::Transaction)?;
    Ok(PreparedAuthorityTransaction {
        transaction,
        result,
        #[cfg(test)]
        events,
        #[cfg(test)]
        envelope,
    })
}

pub(crate) fn execute_authority_transaction<A, S>(
    barrier: CanonicalWriteBarrier,
    repository_root: &Path,
    request: AuthorityTransactionRequest,
    authentication_adapter: &mut A,
    signer: &mut S,
) -> Result<AuthorityTransactionResult, AuthorityTransactionError>
where
    A: AuthenticationAdapter,
    S: RepositoryAuthoritySigner + ?Sized,
{
    let mut prepared = prepare_authority_transaction(
        barrier,
        repository_root,
        request,
        authentication_adapter,
        signer,
    )?;
    prepared.mark_committed()?;
    prepared.install()?;
    #[cfg(feature = "test-support")]
    if std::env::var_os("VELA_TEST_INTERRUPT_INIT_AFTER_INSTALLED").is_some() {
        std::process::exit(86);
    }
    prepared.complete()?;
    Ok(prepared.result)
}

fn validate_semantic_event_links(
    history: &AuthorityHistorySnapshot,
    events: &[AuthorityEventV1],
) -> Result<(), AuthorityTransactionError> {
    let mut semantic_ids = BTreeMap::new();
    for event in history.authority_events.iter().chain(events) {
        if matches!(event.content.kind, EventKind::Other(_)) {
            continue;
        }
        let semantic_id = if event.content.kind == EventKind::TargetClaimed {
            event.id.clone()
        } else {
            event
                .semantic_event_id()
                .map_err(AuthorityTransactionError::Invalid)?
        };
        if semantic_ids
            .insert(semantic_id.clone(), event.content.kind.clone())
            .is_some()
        {
            return Err(AuthorityTransactionError::Invalid(format!(
                "semantic event identity {semantic_id} occurs more than once in authority history"
            )));
        }
    }
    for review in events
        .iter()
        .filter(|event| event.content.kind == EventKind::ReviewAccepted)
    {
        let applied = review
            .content
            .payload
            .get("applied_event_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                AuthorityTransactionError::Invalid(format!(
                    "review.accepted event {} lacks payload.applied_event_id",
                    review.id
                ))
            })?;
        let matching_current = events
            .iter()
            .filter(|candidate| {
                candidate.content.kind != EventKind::ReviewAccepted
                    && candidate
                        .semantic_event_id()
                        .is_ok_and(|semantic_id| semantic_id == applied)
            })
            .count();
        if matching_current != 1 {
            return Err(AuthorityTransactionError::Invalid(format!(
                "review.accepted event {} must link exactly one scientific event in the same authority transaction",
                review.id
            )));
        }
        if semantic_ids.get(applied).is_none_or(|kind| {
            matches!(
                kind,
                EventKind::ReviewAccepted
                    | EventKind::ReviewRejected
                    | EventKind::ReviewRevisionRequested
            )
        }) {
            return Err(AuthorityTransactionError::Invalid(format!(
                "review.accepted event {} does not link a scientific domain event",
                review.id
            )));
        }
    }
    Ok(())
}

fn validate_fresh_initialization_request(
    request: &AuthorityTransactionRequest,
) -> Result<(), AuthorityTransactionError> {
    if !request.history.authority_events.is_empty()
        || !request.history.authority_envelopes.is_empty()
        || !request.history.retained_authority_keysets.is_empty()
        || !request.history.retained_authorization_models.is_empty()
        || request.event_drafts.len() != 1
    {
        return Err(AuthorityTransactionError::History(
            "fresh authority initialization requires an empty authority store and one initialization event"
                .into(),
        ));
    }
    let draft = &request.event_drafts[0];
    let payload: AuthorityInitializationV1 = serde_json::from_value(draft.payload.clone())
        .map_err(|error| {
            AuthorityTransactionError::Invalid(format!(
                "fresh authority initialization payload is invalid: {error}"
            ))
        })?;
    payload
        .validate()
        .map_err(AuthorityTransactionError::Invalid)?;
    let keyset_root = request
        .history
        .authority_keyset
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    let policy_root = request
        .history
        .authorization_model
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    let mut mismatches = Vec::new();
    for (matches, field) in [
        (
            draft.kind.as_str() == AUTHORITY_INITIALIZED_EVENT_KIND,
            "event_kind",
        ),
        (
            draft.target.id == request.history.repository_id,
            "target_repository",
        ),
        (draft.actor.r#type == "human", "actor_type"),
        (
            draft.actor.id == request.principal.principal_id,
            "actor_principal",
        ),
        (
            draft.before_hash == vela_protocol::events::NULL_HASH,
            "before_root",
        ),
        (
            draft.after_hash == vela_protocol::events::NULL_HASH,
            "after_root",
        ),
        (
            payload.repository_id == request.history.repository_id,
            "payload_repository",
        ),
        (
            payload.new_principal_id == request.principal.principal_id,
            "payload_principal",
        ),
        (
            payload.new_authority_keyset_root == keyset_root,
            "authority_keyset_root",
        ),
        (
            payload.new_authorization_model_root == policy_root,
            "model_root",
        ),
        (
            payload.initial_event_log_root == request.history.initial_event_log_root,
            "initial_event_log_root",
        ),
        (
            payload.initial_actor_registry_root == request.history.initial_actor_registry_root,
            "initial_actor_registry_root",
        ),
        (payload.reason == draft.reason, "reason"),
        (
            request.authorization_request.action == AuthorityActionV1::AuthorityInitialize,
            "authorization_action",
        ),
        (
            request.semantic_approvals.iter().any(|approval| {
                approval.action == AUTHORITY_INITIALIZE_ACTION
                    && approval.principal_id == request.principal.principal_id
                    && approval.reason == draft.reason
                    && approval.intent_digest == request.intent_digest
            }),
            "semantic_approval",
        ),
    ] {
        if !matches {
            mismatches.push(field);
        }
    }
    if !mismatches.is_empty() {
        return Err(AuthorityTransactionError::Invalid(format!(
            "fresh authority initialization request does not bind its exact principal, snapshots, action, and reason: {}",
            mismatches.join(", ")
        )));
    }
    Ok(())
}

fn validate_active_authority_snapshots(
    request: &AuthorityTransactionRequest,
    history: &AuthorityHistoryVerification,
) -> Result<(), AuthorityTransactionError> {
    let keyset_root = request
        .history
        .authority_keyset
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    let policy_root = request
        .history
        .authorization_model
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    if history.final_authority_keyset_root.as_deref() != Some(keyset_root.as_str())
        || history.final_authorization_model_root.as_deref() != Some(policy_root.as_str())
    {
        return Err(AuthorityTransactionError::History(
            "caller active authority snapshots differ from the verified history head".into(),
        ));
    }
    Ok(())
}

fn normalize_authority_snapshots(
    repository_root: &Path,
    request: &mut AuthorityTransactionRequest,
) -> Result<(), AuthorityTransactionError> {
    if request.object_drafts.iter().any(|draft| {
        draft.path.starts_with(".vela/authority/keysets/")
            || draft.path.starts_with(".vela/authority/models/")
    }) {
        return Err(AuthorityTransactionError::Invalid(
            "authority keyset and authorization-model snapshots are derived from the verified history, not caller object drafts"
                .into(),
        ));
    }

    let mut snapshots = BTreeMap::<String, (&'static str, Vec<u8>)>::new();
    for keyset in request
        .history
        .retained_authority_keysets
        .iter()
        .chain(std::iter::once(&request.history.authority_keyset))
    {
        let root = keyset.root().map_err(AuthorityTransactionError::Invalid)?;
        snapshots.insert(
            authority_keyset_path(&root)?,
            (
                "authority_keyset",
                to_canonical_bytes(keyset).map_err(AuthorityTransactionError::Invalid)?,
            ),
        );
    }
    for bundle in request
        .history
        .retained_authorization_models
        .iter()
        .chain(std::iter::once(&request.history.authorization_model))
    {
        let root = bundle.root().map_err(AuthorityTransactionError::Invalid)?;
        snapshots.insert(
            authority_model_path(&root)?,
            (
                "authorization_model",
                to_canonical_bytes(bundle).map_err(AuthorityTransactionError::Invalid)?,
            ),
        );
    }
    /* The Cedar schema, policy text and entity snapshot were written into
    `.vela/authority/policy-material/` alongside every model, because a bundle
    named them by root and a replaying reader had to re-run them. The model is
    the policy now: it is one canonical object, retained above with the
    keysets, and there is nothing else to snapshot. */

    for directory in [
        ".vela/authority/keysets",
        ".vela/authority/models",
        ".vela/authority/policy-material/schema",
        ".vela/authority/policy-material/policies",
        ".vela/authority/policy-material/entities",
    ] {
        let binding = InputBinding::current_directory(
            repository_root,
            RepoPath::parse(directory.to_string())
                .map_err(AuthorityTransactionError::Transaction)?,
        )
        .map_err(AuthorityTransactionError::Transaction)?;
        merge_input_binding(&mut request.read_set, binding)?;
    }

    for (path_text, (object_kind, bytes)) in snapshots {
        let path =
            RepoPath::parse(path_text.clone()).map_err(AuthorityTransactionError::Transaction)?;
        let absolute = repository_root.join(path.as_str());
        match fs::symlink_metadata(&absolute) {
            Ok(_) => {
                let binding = InputBinding::exact_file(repository_root, path, &bytes)
                    .map_err(AuthorityTransactionError::Transaction)?;
                merge_input_binding(&mut request.read_set, binding)?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                if authority_history_retains_path(&request.history, &path_text)? {
                    return Err(AuthorityTransactionError::History(format!(
                        "retained authority policy material {path_text} is missing"
                    )));
                }
                let binding = InputBinding::absent_file(repository_root, path)
                    .map_err(AuthorityTransactionError::Transaction)?;
                merge_input_binding(&mut request.read_set, binding)?;
                request.object_drafts.push(AuthorityObjectDraft {
                    path: path_text,
                    object_kind: object_kind.into(),
                    class: WriteClass::Authority,
                    postimage: Some(bytes),
                });
            }
            Err(error) => {
                return Err(AuthorityTransactionError::Transaction(
                    RepositoryTxnError::Io(format!(
                        "inspect authority snapshot {}: {error}",
                        absolute.display()
                    )),
                ));
            }
        }
    }
    Ok(())
}

fn authority_history_retains_path(
    history: &AuthorityHistorySnapshot,
    path: &str,
) -> Result<bool, AuthorityTransactionError> {
    let mut retained = false;
    for envelope in &history.authority_envelopes {
        let payload = BASE64_STANDARD.decode(&envelope.payload).map_err(|error| {
            AuthorityTransactionError::History(format!(
                "authority envelope payload is not base64: {error}"
            ))
        })?;
        let record = serde_json::from_slice::<AuthorityRecordV1>(&payload).map_err(|error| {
            AuthorityTransactionError::History(format!(
                "authority envelope record JSON is invalid: {error}"
            ))
        })?;
        if let Some(delta) = record
            .content
            .object_delta
            .iter()
            .find(|delta| delta.path == path)
        {
            retained = delta.after_root.is_some();
        }
    }
    Ok(retained)
}

fn normalize_object_drafts(
    repository_root: &Path,
    request: &mut AuthorityTransactionRequest,
) -> Result<(), AuthorityTransactionError> {
    request
        .object_drafts
        .sort_by(|left, right| left.path.cmp(&right.path));
    let mut previous_path: Option<&str> = None;
    let mut object_inputs = Vec::with_capacity(request.object_drafts.len());
    for draft in &request.object_drafts {
        if previous_path == Some(draft.path.as_str()) {
            return Err(AuthorityTransactionError::Invalid(format!(
                "duplicate authority object path {}",
                draft.path
            )));
        }
        previous_path = Some(&draft.path);
        if draft.object_kind.trim().is_empty() {
            return Err(AuthorityTransactionError::Invalid(format!(
                "authority object {} has an empty kind",
                draft.path
            )));
        }
        let path =
            RepoPath::parse(draft.path.clone()).map_err(AuthorityTransactionError::Transaction)?;
        validate_authority_object_path(&path, draft.class)?;
        if let Some(bytes) = &draft.postimage
            && draft.path.ends_with(".json")
        {
            let value = serde_json::from_slice::<Value>(bytes).map_err(|error| {
                AuthorityTransactionError::Invalid(format!(
                    "authority object {} is not valid JSON: {error}",
                    draft.path
                ))
            })?;
            let canonical =
                to_canonical_bytes(&value).map_err(AuthorityTransactionError::Invalid)?;
            if canonical != *bytes {
                return Err(AuthorityTransactionError::Invalid(format!(
                    "authority object {} is not canonical JSON",
                    draft.path
                )));
            }
        }
        object_inputs.push(
            InputBinding::current_file(repository_root, path)
                .map_err(AuthorityTransactionError::Transaction)?,
        );
    }
    for binding in object_inputs {
        merge_input_binding(&mut request.read_set, binding)?;
    }
    Ok(())
}

fn bind_repository_authority_history(
    repository_root: &Path,
    request: &mut AuthorityTransactionRequest,
) -> Result<(), AuthorityTransactionError> {
    let mut bindings = Vec::new();
    let mut authority_event_paths = Vec::new();
    for event in &request.history.authority_events {
        let path = RepoPath::parse(authority_event_path(&event.id))
            .map_err(AuthorityTransactionError::Transaction)?;
        bindings.push(exact_verified_json_input(repository_root, &path, event)?);
        authority_event_paths.push(path);
    }
    let mut authority_record_paths = Vec::new();
    for envelope in &request.history.authority_envelopes {
        let payload = BASE64_STANDARD.decode(&envelope.payload).map_err(|error| {
            AuthorityTransactionError::History(format!(
                "authority envelope payload is not base64: {error}"
            ))
        })?;
        let record = serde_json::from_slice::<AuthorityRecordV1>(&payload).map_err(|error| {
            AuthorityTransactionError::History(format!(
                "authority envelope record JSON is invalid: {error}"
            ))
        })?;
        let path = RepoPath::parse(authority_record_path(&record.record_id))
            .map_err(AuthorityTransactionError::Transaction)?;
        bindings.push(exact_verified_json_input(repository_root, &path, envelope)?);
        authority_record_paths.push(path);
    }
    bindings.push(
        InputBinding::absent_file(
            repository_root,
            RepoPath::parse(".vela/actors.json".to_string())
                .map_err(AuthorityTransactionError::Transaction)?,
        )
        .map_err(AuthorityTransactionError::Transaction)?,
    );
    for (directory, paths) in [
        (".vela/events", Vec::new()),
        (".vela/authority/events", authority_event_paths),
        (".vela/authority/records", authority_record_paths),
    ] {
        bindings.push(
            InputBinding::exact_directory(
                repository_root,
                RepoPath::parse(directory.to_string())
                    .map_err(AuthorityTransactionError::Transaction)?,
                &paths,
            )
            .map_err(AuthorityTransactionError::Transaction)?,
        );
    }
    for binding in bindings {
        merge_input_binding(&mut request.read_set, binding)?;
    }
    Ok(())
}

fn exact_verified_json_input<T>(
    repository_root: &Path,
    path: &RepoPath,
    expected: &T,
) -> Result<InputBinding, AuthorityTransactionError>
where
    T: Serialize + serde::de::DeserializeOwned,
{
    const MAX_AUTHORITY_MEMBER_BYTES: u64 = 16 * 1024 * 1024;
    let bytes = crate::bounded_file::read_bounded_repository_file(
        repository_root,
        Path::new(path.as_str()),
        MAX_AUTHORITY_MEMBER_BYTES,
        path.as_str(),
    )
    .map_err(|error| {
        AuthorityTransactionError::History(format!(
            "read retained authority-history member {}: {error}",
            path.as_str()
        ))
    })?;
    let decoded: T = serde_json::from_slice(&bytes).map_err(|error| {
        AuthorityTransactionError::History(format!(
            "decode retained authority-history member {}: {error}",
            path.as_str()
        ))
    })?;
    let expected_canonical =
        to_canonical_bytes(expected).map_err(AuthorityTransactionError::Invalid)?;
    let decoded_canonical =
        to_canonical_bytes(&decoded).map_err(AuthorityTransactionError::Invalid)?;
    if decoded_canonical != expected_canonical {
        return Err(AuthorityTransactionError::History(format!(
            "retained authority-history member {} differs from the verified object",
            path.as_str()
        )));
    }
    InputBinding::exact_file(repository_root, path.clone(), &bytes)
        .map_err(AuthorityTransactionError::Transaction)
}

fn merge_input_binding(
    read_set: &mut Vec<InputBinding>,
    binding: InputBinding,
) -> Result<(), AuthorityTransactionError> {
    if let Some(existing) = read_set.iter().find(|input| input.name == binding.name) {
        if existing.digest == binding.digest {
            return Ok(());
        }
        return Err(AuthorityTransactionError::Invalid(format!(
            "authority transaction input {} conflicts with the verified repository history",
            binding.name
        )));
    }
    read_set.push(binding);
    Ok(())
}

fn validate_authority_object_path(
    path: &RepoPath,
    class: WriteClass,
) -> Result<(), AuthorityTransactionError> {
    let value = path.as_str();
    if value.starts_with(".vela/authority/events/") || value.starts_with(".vela/authority/records/")
    {
        return Err(AuthorityTransactionError::Invalid(format!(
            "authority object drafts cannot replace covering-record path {value}"
        )));
    }
    // Retirement is the verifier's judgement, so ask the verifier. Restating
    // the set here is what let `.vela/events/` be refused while
    // `.vela/findings/`, `.vela/artifacts/`, `.vela/policies/` and
    // `.vela/actors.json` stayed writable, which is a repository the writer
    // accepts and replay then rejects.
    if crate::repository::is_retired_path(value) {
        return Err(AuthorityTransactionError::Invalid(format!(
            "authority object drafts cannot write retired protocol path {value}"
        )));
    }
    let valid = match class {
        WriteClass::Authority => value.starts_with(".vela/authority/"),
        WriteClass::PublicReview => {
            value.starts_with(".vela/proposals/") || value.starts_with("records/")
        }
        WriteClass::CanonicalEvidence => {
            (value.starts_with(".vela/")
                && !value.starts_with(".vela/authority/")
                && !value.starts_with(".vela/proposals/"))
                || value.starts_with("records/claims/")
                || value.starts_with("records/artifacts/")
        }
    };
    if !valid {
        return Err(AuthorityTransactionError::Invalid(format!(
            "authority object path {value} is incompatible with write class {class:?}"
        )));
    }
    Ok(())
}

fn authority_object_planned_write(
    draft: &AuthorityObjectDraft,
) -> Result<PlannedWrite, AuthorityTransactionError> {
    let path =
        RepoPath::parse(draft.path.clone()).map_err(AuthorityTransactionError::Transaction)?;
    Ok(match &draft.postimage {
        Some(bytes) => PlannedWrite::write(path, draft.class, bytes.clone()),
        None => PlannedWrite::delete(path, draft.class),
    })
}

fn authority_object_delta(
    draft: &DeltaDraft,
    events: &[AuthorityEventV1],
    objects: &[AuthorityObjectDraft],
) -> Result<Vec<ObjectDeltaV1>, AuthorityTransactionError> {
    let mut kinds = events
        .iter()
        .map(|event| (authority_event_path(&event.id), "event".to_string()))
        .collect::<BTreeMap<_, _>>();
    for object in objects {
        if kinds
            .insert(object.path.clone(), object.object_kind.clone())
            .is_some()
        {
            return Err(AuthorityTransactionError::Invalid(format!(
                "duplicate authority object path {}",
                object.path
            )));
        }
    }
    if draft.delta.writes().len() != kinds.len() {
        return Err(AuthorityTransactionError::Invalid(
            "every requested authority object must change exactly one canonical path".into(),
        ));
    }
    draft
        .delta
        .writes()
        .iter()
        .map(|write| {
            let object_kind = kinds.get(write.path.as_str()).cloned().ok_or_else(|| {
                AuthorityTransactionError::Invalid(format!(
                    "journal delta contains uncovered authority object {}",
                    write.path.as_str()
                ))
            })?;
            let before_root = file_state_root(&write.preimage);
            let after_root = file_state_root(&write.postimage);
            if before_root == after_root {
                return Err(AuthorityTransactionError::Invalid(format!(
                    "authority object {} changes no bytes",
                    write.path.as_str()
                )));
            }
            Ok(ObjectDeltaV1 {
                path: write.path.as_str().to_string(),
                before_root,
                after_root,
                object_kind,
            })
        })
        .collect()
}

fn file_state_root(state: &vela_repository::FileState) -> Option<String> {
    match state {
        vela_repository::FileState::Absent => None,
        vela_repository::FileState::File { digest, .. } => Some(digest.as_str().to_string()),
    }
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
        .authorization_model
        .validate()
        .map_err(AuthorityTransactionError::Invalid)?;
    if request.history.repository_id != request.history.authority_keyset.repository_id
        || request.history.repository_id != request.history.authorization_model.repository_id
    {
        return Err(AuthorityTransactionError::Invalid(
            "history, keyset, and authorization model name different repositories".into(),
        ));
    }
    /* The binary used to carry its own copy of the Cedar schema, policy text
    and entity snapshot, and this compared their roots against the retained
    bundle — the pin that made editing one character of `entity Frontier`
    fail every later authority write on that repository. There is no runtime
    policy text to drift now: the request names the model root it was decided
    under, and the request is checked against it. */
    request
        .authorization_request
        .validate()
        .map_err(AuthorityTransactionError::Invalid)?;
    let retained_model_root = request
        .history
        .authorization_model
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    if request.authorization_request.model_root != retained_model_root
        || request.authorization_request.repository_id != request.history.repository_id
    {
        return Err(AuthorityTransactionError::Invalid(
            "the authorization request does not bind the retained authorization model".into(),
        ));
    }
    ContentDigest::parse(request.intent_digest.clone())
        .map_err(AuthorityTransactionError::Transaction)?;
    ContentDigest::parse(request.binary_sha256.clone())
        .map_err(AuthorityTransactionError::Transaction)?;
    if (request.event_drafts.is_empty() && request.object_drafts.is_empty())
        || request.vela_version.trim().is_empty()
        || request.recorded_at.trim().is_empty()
        || request.authentication_request.transaction_at != request.recorded_at
    {
        return Err(AuthorityTransactionError::Invalid(
            "a changed event or object, version, and exact transaction time are required".into(),
        ));
    }
    for draft in &request.event_drafts {
        if !event_performer_matches_authority_principal(request, draft)
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

fn event_performer_matches_authority_principal(
    request: &AuthorityTransactionRequest,
    draft: &AuthorityEventDraft,
) -> bool {
    if draft.actor.id == request.principal.principal_id {
        return true;
    }
    if !matches!(
        request.authorization_request.action,
        AuthorityActionV1::ReviewAccept | AuthorityActionV1::ReviewReject
    ) || !matches!(draft.actor.r#type.as_str(), "human" | "agent")
    {
        return false;
    }
    let Some(provenance) = draft.payload.get("decision_performer") else {
        return false;
    };
    provenance.get("schema").and_then(serde_json::Value::as_str)
        == Some("vela.decision-performer.v1")
        && provenance
            .get("actor_id")
            .and_then(serde_json::Value::as_str)
            == Some(draft.actor.id.as_str())
        && provenance
            .get("actor_class")
            .and_then(serde_json::Value::as_str)
            == Some(draft.actor.r#type.as_str())
        && provenance
            .get("authority_principal_id")
            .and_then(serde_json::Value::as_str)
            == Some(request.principal.principal_id.as_str())
        && provenance.get("session_ref").is_some_and(|value| {
            value.is_null()
                || value.as_str().is_some_and(|reference| {
                    !reference.trim().is_empty()
                        && reference == reference.trim()
                        && reference.len() <= 2048
                        && !reference.chars().any(char::is_control)
                })
        })
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
    Ok(())
}

fn validate_semantic_approvals(
    request: &AuthorityTransactionRequest,
) -> Result<(), AuthorityTransactionError> {
    let action = serde_json::to_value(request.authorization_request.action)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .ok_or_else(|| {
            AuthorityTransactionError::Invalid("authorization action does not name itself".into())
        })?;
    if request.semantic_approvals.is_empty() {
        return Err(AuthorityTransactionError::Invalid(
            "authority action lacks a semantic approval".into(),
        ));
    }
    for approval in &request.semantic_approvals {
        if approval.principal_id.trim().is_empty()
            || approval.role.trim().is_empty()
            || approval.action != action
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
    previous_authority_record_root: Option<&str>,
    authorization_request_root: &str,
    authority_keyset_root: &str,
    model_root: &str,
) -> Result<String, AuthorityTransactionError> {
    let root = domain_root(
        b"vela.authority-transaction-id.internal.v1\0",
        &TransactionIdCommitment {
            schema: TRANSACTION_ID_SCHEMA,
            repository_id: &request.history.repository_id,
            intent_digest: &request.intent_digest,
            before_event_log_root,
            previous_authority_record_root,
            principal_id: &request.principal.principal_id,
            authorization_request_root,
            authority_keyset_root,
            model_root,
            delegation: None,
            semantic_approvals: &request.semantic_approvals,
            event_drafts: &request.event_drafts,
            object_drafts: &request.object_drafts,
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

fn read_set_root(
    request: &AuthorityTransactionRequest,
    current_event_log_root: &str,
    previous_authority_record_root: Option<&str>,
    authority_keyset_root: &str,
    model_root: &str,
) -> Result<String, AuthorityTransactionError> {
    authority_read_set_root_for_inputs(
        &request.history.repository_id,
        current_event_log_root,
        previous_authority_record_root,
        authority_keyset_root,
        model_root,
        &request.read_set,
    )
}

pub(crate) fn authority_read_set_root_for_inputs(
    repository_id: &str,
    current_event_log_root: &str,
    previous_authority_record_root: Option<&str>,
    authority_keyset_root: &str,
    model_root: &str,
    inputs: &[InputBinding],
) -> Result<String, AuthorityTransactionError> {
    domain_root(
        b"vela.authority-read-set.internal.v1\0",
        &ReadSetCommitment {
            schema: READ_SET_SCHEMA,
            repository_id,
            current_event_log_root,
            previous_authority_record_root,
            authority_keyset_root,
            model_root,
            inputs,
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

pub(crate) fn authority_event_path(event_id: &str) -> String {
    format!(".vela/authority/events/{event_id}.json")
}

pub(crate) fn authority_record_path(record_id: &str) -> String {
    format!(".vela/authority/records/{record_id}.dsse.json")
}

pub(crate) fn authority_keyset_path(root: &str) -> Result<String, AuthorityTransactionError> {
    Ok(format!(
        ".vela/authority/keysets/{}.json",
        authority_snapshot_stem(root)?
    ))
}

pub(crate) fn authority_model_path(root: &str) -> Result<String, AuthorityTransactionError> {
    Ok(format!(
        ".vela/authority/models/{}.json",
        authority_snapshot_stem(root)?
    ))
}

fn authority_snapshot_stem(root: &str) -> Result<&str, AuthorityTransactionError> {
    ContentDigest::parse(root.to_string()).map_err(AuthorityTransactionError::Transaction)?;
    root.strip_prefix("sha256:")
        .ok_or_else(|| AuthorityTransactionError::Invalid("snapshot root lacks sha256 tag".into()))
}

#[derive(Serialize)]
struct TransactionIdCommitment<'a> {
    schema: &'static str,
    repository_id: &'a str,
    intent_digest: &'a str,
    before_event_log_root: &'a str,
    previous_authority_record_root: Option<&'a str>,
    principal_id: &'a str,
    authorization_request_root: &'a str,
    authority_keyset_root: &'a str,
    model_root: &'a str,
    delegation: Option<&'a Value>,
    semantic_approvals: &'a [SemanticApprovalV1],
    event_drafts: &'a [AuthorityEventDraft],
    object_drafts: &'a [AuthorityObjectDraft],
    read_set: &'a [InputBinding],
    vela_version: &'a str,
    binary_sha256: &'a str,
    recorded_at: &'a str,
}

#[derive(Serialize)]
struct ReadSetCommitment<'a> {
    schema: &'static str,
    repository_id: &'a str,
    current_event_log_root: &'a str,
    previous_authority_record_root: Option<&'a str>,
    authority_keyset_root: &'a str,
    model_root: &'a str,
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

pub(crate) fn execution_binary_sha256(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read execution binary {}: {error}", path.display()))?;
    Ok(sha256_root(&bytes))
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
        DsseSignatureV1, SemanticApprovalV1,
    };
    use vela_protocol::authorization::{
        AUTHORIZATION_MODEL_SCHEMA_V1, AUTHORIZATION_PROFILE_V1, AUTHORIZATION_REQUEST_SCHEMA_V1,
        AuthorityMemberV1, AuthorityResourceTypeV1, AuthorityRoleV1, AuthorizationResourceV1,
    };
    use vela_protocol::dsse::pae;

    use vela_protocol::authority_history::{
        AUTHORITY_INITIALIZATION_SCHEMA_V1, AUTHORITY_INITIALIZE_ACTION,
        AUTHORITY_INITIALIZED_EVENT_KIND, AuthorityInitializationV1, authority_event_log_root,
    };
    use vela_protocol::canonical::to_canonical_bytes;
    use vela_protocol::events::{
        EVENT_SCHEMA, NULL_HASH, StateActor, StateEvent, StateTarget, compute_event_id,
    };
    use vela_protocol::principal::PrincipalClass;

    use super::*;
    use vela_repository::{
        RecoveryOutcome, RepositoryTxnStep, TransactionAuthorization,
        TransactionAuthorizationContext,
    };

    const REPOSITORY_ID: &str = "01234567-89ab-4def-8123-456789abcdef";
    const REPOSITORY_PRINCIPAL: &str = "local:device-1|uid:501";
    const RECORDED_AT: &str = "2026-07-24T12:05:00Z";

    #[derive(Debug, Default)]
    struct TestTransactionAuthorization(Option<ContentDigest>);

    impl TransactionAuthorization for TestTransactionAuthorization {
        fn bind_plan(
            &mut self,
            context: &mut TransactionAuthorizationContext<'_>,
        ) -> Result<(), RepositoryTxnError> {
            if let Some(expected) = &self.0 {
                if expected != context.plan_root() {
                    return Err(RepositoryTxnError::WriteAuthorization(
                        "test authorization plan changed after binding".into(),
                    ));
                }
            } else {
                self.0 = Some(context.plan_root().clone());
            }
            Ok(())
        }

        fn revalidate_for_marker(
            &self,
            context: &mut TransactionAuthorizationContext<'_>,
        ) -> Result<(), RepositoryTxnError> {
            if self.0.as_ref() != Some(context.plan_root()) {
                return Err(RepositoryTxnError::WriteAuthorization(
                    "test authorization plan changed before marker".into(),
                ));
            }
            Ok(())
        }
    }

    fn test_transaction_authorization() -> Box<dyn TransactionAuthorization> {
        Box::<TestTransactionAuthorization>::default()
    }

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn canonical_root(value: &impl Serialize) -> String {
        ContentDigest::hash(to_canonical_bytes(value).unwrap())
            .as_str()
            .to_string()
    }

    /// Where a review object actually lives, matching what `submission`
    /// writes. `.vela/proposals/` is retired, so fixtures that used it were
    /// exercising a path the verifier refuses.
    fn proposal_object_path(postimage: &[u8]) -> String {
        crate::submission::rooted_path(
            "records/proposals/sha256",
            ContentDigest::hash(postimage).as_str(),
        )
        .unwrap()
    }

    /// The proposal a review fixture decides.
    const FIXTURE_PROPOSAL: &str = "vpr_0123456789abcdef";

    /// The model a fixture repository is initialized with: one human holding
    /// both roles. This was a Cedar schema declaring six actions and their
    /// authentication context, a policy text, an entity snapshot, and a
    /// hand-written bundle binding all three by root — about a hundred and
    /// eighty lines saying what these six say.
    fn fixture_authorization_model() -> AuthorizationModelV1 {
        AuthorizationModelV1 {
            schema: AUTHORIZATION_MODEL_SCHEMA_V1.into(),
            profile: AUTHORIZATION_PROFILE_V1.into(),
            repository_id: REPOSITORY_ID.into(),
            members: vec![
                AuthorityMemberV1 {
                    principal_id: REPOSITORY_PRINCIPAL.into(),
                    principal_class: PrincipalClass::Human,
                    role: AuthorityRoleV1::Administrator,
                },
                AuthorityMemberV1 {
                    principal_id: REPOSITORY_PRINCIPAL.into(),
                    principal_class: PrincipalClass::Human,
                    role: AuthorityRoleV1::Reviewer,
                },
            ],
            previous_model_root: None,
        }
    }

    fn fixture_authorization_request(
        model: &AuthorizationModelV1,
        action: AuthorityActionV1,
    ) -> AuthorizationRequestV1 {
        let resource = match action.required_resource_type() {
            AuthorityResourceTypeV1::Repository => AuthorizationResourceV1 {
                repository_id: REPOSITORY_ID.into(),
                resource_type: AuthorityResourceTypeV1::Repository,
                resource_id: REPOSITORY_ID.into(),
            },
            AuthorityResourceTypeV1::Proposal => AuthorizationResourceV1 {
                repository_id: REPOSITORY_ID.into(),
                resource_type: AuthorityResourceTypeV1::Proposal,
                resource_id: FIXTURE_PROPOSAL.into(),
            },
        };
        AuthorizationRequestV1 {
            schema: AUTHORIZATION_REQUEST_SCHEMA_V1.into(),
            profile: AUTHORIZATION_PROFILE_V1.into(),
            model_root: model.root().unwrap(),
            repository_id: REPOSITORY_ID.into(),
            principal_id: REPOSITORY_PRINCIPAL.into(),
            principal_class: PrincipalClass::Human,
            action,
            resource,
            authentication_root: root('9'),
            transaction_read_set_root: root('e'),
            intent_digest: root('a'),
            recovery_recent: false,
        }
    }

    struct TestSigner {
        key: SigningKey,
        key_id: String,
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
            let signature = self.key.sign(&pae(payload_type, canonical_payload));
            Ok(vec![DsseSignatureV1 {
                keyid: self.key_id.clone(),
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
    }

    impl Fixture {
        fn journal_dir(&self) -> std::path::PathBuf {
            self.temporary.path().join(".vela/operation-journals")
        }

        fn barrier(&self) -> CanonicalWriteBarrier {
            RepositoryTxn::acquire_recovery_barrier(self.temporary.path(), &self.journal_dir())
                .unwrap()
                .authorize(test_transaction_authorization())
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
                key_id: "repository-key-1".into(),
                calls: 0,
                fail: false,
            }
        }

        fn prepare(
            &self,
            request: AuthorityTransactionRequest,
            adapter: &mut LocalOsSession,
            signer: &mut TestSigner,
        ) -> Result<PreparedAuthorityTransaction, AuthorityTransactionError> {
            prepare_authority_transaction(
                self.barrier(),
                self.temporary.path(),
                request,
                adapter,
                signer,
            )
        }

        fn execute(
            &self,
            request: AuthorityTransactionRequest,
            adapter: &mut LocalOsSession,
            signer: &mut TestSigner,
        ) -> Result<AuthorityTransactionResult, AuthorityTransactionError> {
            execute_authority_transaction(
                self.barrier(),
                self.temporary.path(),
                request,
                adapter,
                signer,
            )
        }
    }

    fn fixture() -> Fixture {
        let temporary = TempDir::new().unwrap();
        let root_path = temporary.path();
        fs::create_dir_all(root_path.join(".vela/events")).unwrap();

        let repository_key = SigningKey::from_bytes(&[12; 32]);
        fs::write(root_path.join(".vela/input.json"), b"{\"fixture\":true}\n").unwrap();

        let authorization_model = fixture_authorization_model();
        let authorization_request =
            fixture_authorization_request(&authorization_model, AuthorityActionV1::ReviewReject);
        let keyset = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            repository_id: REPOSITORY_ID.into(),
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
            closed: false,
        };
        let initial_event_log_root = format!("sha256:{}", event_log_hash(&[]));
        let initial_actor_registry_root = sha256_root(&[]);
        let keyset_root = keyset.root().unwrap();
        let model_root = authorization_model.root().unwrap();
        let initialization = AuthorityInitializationV1 {
            schema: AUTHORITY_INITIALIZATION_SCHEMA_V1.into(),
            repository_id: REPOSITORY_ID.into(),
            initial_event_log_root: initial_event_log_root.clone(),
            initial_actor_registry_root: initial_actor_registry_root.clone(),
            new_authority_keyset_root: keyset.root().unwrap(),
            new_authorization_model_root: authorization_model.root().unwrap(),
            new_principal_id: REPOSITORY_PRINCIPAL.into(),
            minimum_writer_version: "0.930.0".into(),
            reason: "Initialize the disposable repository authority fixture.".into(),
        };
        let initialization_event = AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "vtx_initialization".into(),
            principal_id: REPOSITORY_PRINCIPAL.into(),
            authority_mode: AUTHORITY_MODE.into(),
            kind: EventKind::Other(AUTHORITY_INITIALIZED_EVENT_KIND.into()),
            target: StateTarget {
                r#type: "repository".into(),
                id: REPOSITORY_ID.into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: REPOSITORY_PRINCIPAL.into(),
            },
            timestamp: "2026-07-24T12:00:00Z".into(),
            reason: initialization.reason.clone(),
            before_hash: NULL_HASH.into(),
            after_hash: NULL_HASH.into(),
            payload: serde_json::to_value(&initialization).unwrap(),
            caveats: Vec::new(),
        })
        .unwrap();
        let initialization_intent = canonical_root(&initialization);
        let initialization_event_root = initialization_event.root().unwrap();
        let initialized_event_log_root =
            authority_event_log_root(&initial_event_log_root, &[&initialization_event]).unwrap();

        let first_record = AuthorityRecordV1::new(AuthorityRecordContentV1 {
            repository_id: REPOSITORY_ID.into(),
            sequence: 1,
            previous_authority_record_root: None,
            operation_id: "vop_initialization".into(),
            transaction_id: "vtx_initialization".into(),
            intent_digest: initialization_intent.clone(),
            before_event_log_root: initial_event_log_root.clone(),
            after_event_log_root: initialized_event_log_root,
            event_ids: vec![initialization_event.id.clone()],
            object_delta: vec![
                ObjectDeltaV1 {
                    path: authority_event_path(&initialization_event.id),
                    before_root: None,
                    after_root: Some(initialization_event_root),
                    object_kind: "event".into(),
                },
                ObjectDeltaV1 {
                    path: authority_keyset_path(&keyset_root).unwrap(),
                    before_root: None,
                    after_root: Some(keyset_root.clone()),
                    object_kind: "authority_keyset".into(),
                },
                ObjectDeltaV1 {
                    path: authority_model_path(&model_root).unwrap(),
                    before_root: None,
                    after_root: Some(model_root.clone()),
                    object_kind: "authorization_model".into(),
                },
            ],
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
            authorization: {
                /* The evaluation is the evaluator's own output over the
                retained request. Replay recomputes it, so a fixture that
                asserted its own answer would prove nothing. */
                let initialize_request = fixture_authorization_request(
                    &authorization_model,
                    AuthorityActionV1::AuthorityInitialize,
                );
                AuthorizationClaimV1 {
                    model_root: authorization_model.root().unwrap(),
                    evaluation: vela_protocol::authorization::evaluate_authorization_v1(
                        &authorization_model,
                        &initialize_request,
                    )
                    .unwrap(),
                    request: initialize_request,
                }
            },
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                role: "repository_administrator".into(),
                action: AUTHORITY_INITIALIZE_ACTION.into(),
                reason: initialization.reason,
                approved_at: "2026-07-24T12:00:00Z".into(),
                intent_digest: initialization_intent,
            }],
            execution: ExecutionClaimV1 {
                vela_version: "0.930.0-rc.1".into(),
                binary_sha256: root('8'),
                transaction_read_set_root: root('9'),
                transaction_write_set_root: root('0'),
                completed_at: "2026-07-24T12:00:01Z".into(),
            },
            authority_keyset_root: keyset_root.clone(),
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
                        .sign(&pae(AUTHORITY_PAYLOAD_TYPE_V1, &first_payload))
                        .to_bytes(),
                ),
            }],
        };
        fs::create_dir_all(root_path.join(".vela/authority/events")).unwrap();
        fs::create_dir_all(root_path.join(".vela/authority/records")).unwrap();
        fs::write(
            root_path.join(authority_event_path(&initialization_event.id)),
            to_canonical_bytes(&initialization_event).unwrap(),
        )
        .unwrap();
        fs::write(
            root_path.join(authority_record_path(&first_record.record_id)),
            to_canonical_bytes(&first_envelope).unwrap(),
        )
        .unwrap();
        let keyset_path = authority_keyset_path(&keyset_root).unwrap();
        let policy_path = authority_model_path(&model_root).unwrap();
        fs::create_dir_all(
            root_path
                .join(&keyset_path)
                .parent()
                .expect("keyset snapshot parent"),
        )
        .unwrap();
        fs::create_dir_all(
            root_path
                .join(&policy_path)
                .parent()
                .expect("policy snapshot parent"),
        )
        .unwrap();
        fs::write(
            root_path.join(keyset_path),
            to_canonical_bytes(&keyset).unwrap(),
        )
        .unwrap();
        fs::write(
            root_path.join(policy_path),
            to_canonical_bytes(&authorization_model).unwrap(),
        )
        .unwrap();

        let fixture_input = InputBinding::current_file(
            root_path,
            RepoPath::parse(".vela/input.json".to_string()).unwrap(),
        )
        .unwrap();
        let request = AuthorityTransactionRequest {
            history: AuthorityHistorySnapshot {
                repository_id: REPOSITORY_ID.into(),
                initial_event_log_root,
                initial_actor_registry_root,
                authority_keyset: keyset.clone(),
                authorization_model: authorization_model.clone(),
                retained_authority_keysets: vec![keyset],
                retained_authorization_models: vec![authorization_model],
                authority_events: vec![initialization_event],
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
            authorization_request,
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                role: "repository_administrator".into(),
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
            object_drafts: Vec::new(),
            read_set: vec![fixture_input],
            vela_version: "0.930.0-rc.1".into(),
            binary_sha256: root('1'),
            recorded_at: RECORDED_AT.into(),
        };

        Fixture {
            temporary,
            request,
            repository_key,
        }
    }

    fn verify_installed_history(
        fixture: &Fixture,
        events: &[AuthorityEventV1],
        envelope: &AuthorityEnvelopeV1,
    ) {
        let mut envelopes = fixture.request.history.authority_envelopes.clone();
        envelopes.push(envelope.clone());
        let mut authority_events = fixture.request.history.authority_events.clone();
        authority_events.extend_from_slice(events);
        let result = verify_authority_history(AuthorityHistoryInput {
            repository_id: REPOSITORY_ID,
            initial_event_log_root: &fixture.request.history.initial_event_log_root,
            initial_actor_registry_root: &fixture.request.history.initial_actor_registry_root,
            authority_keysets: &fixture.request.history.retained_authority_keysets,
            authorization_models: &fixture.request.history.retained_authorization_models,
            authority_events: &authority_events,
            authority_envelopes: &envelopes,
        })
        .unwrap();
        assert_eq!(result.authority_event_count, authority_events.len());
        assert_eq!(result.authority_record_count, 2);
    }

    fn verified_fixture_history(fixture: &Fixture) -> AuthorityHistoryVerification {
        verify_authority_history(AuthorityHistoryInput {
            repository_id: REPOSITORY_ID,
            initial_event_log_root: &fixture.request.history.initial_event_log_root,
            initial_actor_registry_root: &fixture.request.history.initial_actor_registry_root,
            authority_keysets: &fixture.request.history.retained_authority_keysets,
            authorization_models: &fixture.request.history.retained_authorization_models,
            authority_events: &fixture.request.history.authority_events,
            authority_envelopes: &fixture.request.history.authority_envelopes,
        })
        .unwrap()
    }

    fn authority_transaction_postimages_absent(fixture: &Fixture) -> bool {
        let event_count = fs::read_dir(fixture.temporary.path().join(".vela/authority/events"))
            .map(|entries| entries.count())
            .unwrap_or_default();
        let record_count = fs::read_dir(fixture.temporary.path().join(".vela/authority/records"))
            .unwrap()
            .count();
        let keyset_path =
            authority_keyset_path(&fixture.request.history.authority_keyset.root().unwrap())
                .unwrap();
        let policy_path =
            authority_model_path(&fixture.request.history.authorization_model.root().unwrap())
                .unwrap();
        let keyset_unchanged =
            fs::read(fixture.temporary.path().join(keyset_path)).is_ok_and(|bytes| {
                bytes == to_canonical_bytes(&fixture.request.history.authority_keyset).unwrap()
            });
        let policy_unchanged =
            fs::read(fixture.temporary.path().join(policy_path)).is_ok_and(|bytes| {
                bytes == to_canonical_bytes(&fixture.request.history.authorization_model).unwrap()
            });
        event_count == 1 && record_count == 1 && keyset_unchanged && policy_unchanged
    }

    #[test]
    fn fresh_initialization_binds_repository_origin_roots() {
        let mut fixture = fixture();
        let archived_event_root = root('a');
        let archived_actor_root = root('b');
        let reason = "Adopt the current repository origin roots.";
        let keyset_root = fixture.request.history.authority_keyset.root().unwrap();
        let policy_root = fixture.request.history.authorization_model.root().unwrap();

        fixture.request.history.initial_event_log_root = archived_event_root.clone();
        fixture.request.history.initial_actor_registry_root = archived_actor_root.clone();
        fixture.request.history.retained_authority_keysets.clear();
        fixture
            .request
            .history
            .retained_authorization_models
            .clear();
        fixture.request.history.authority_events.clear();
        fixture.request.history.authority_envelopes.clear();
        fixture.request.authorization_request = fixture_authorization_request(
            &fixture.request.history.authorization_model,
            AuthorityActionV1::AuthorityInitialize,
        );
        fixture.request.semantic_approvals = vec![SemanticApprovalV1 {
            principal_id: REPOSITORY_PRINCIPAL.into(),
            role: "repository_administrator".into(),
            action: AUTHORITY_INITIALIZE_ACTION.into(),
            reason: reason.into(),
            approved_at: RECORDED_AT.into(),
            intent_digest: fixture.request.intent_digest.clone(),
        }];
        let initialization = AuthorityInitializationV1 {
            schema: AUTHORITY_INITIALIZATION_SCHEMA_V1.into(),
            repository_id: REPOSITORY_ID.into(),
            initial_event_log_root: archived_event_root.clone(),
            initial_actor_registry_root: archived_actor_root,
            new_authority_keyset_root: keyset_root,
            new_authorization_model_root: policy_root,
            new_principal_id: REPOSITORY_PRINCIPAL.into(),
            minimum_writer_version: "0.940.9".into(),
            reason: reason.into(),
        };
        fixture.request.event_drafts = vec![AuthorityEventDraft {
            kind: EventKind::Other(AUTHORITY_INITIALIZED_EVENT_KIND.into()),
            target: StateTarget {
                r#type: "repository".into(),
                id: REPOSITORY_ID.into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: REPOSITORY_PRINCIPAL.into(),
            },
            timestamp: RECORDED_AT.into(),
            reason: reason.into(),
            before_hash: NULL_HASH.into(),
            after_hash: NULL_HASH.into(),
            payload: serde_json::to_value(&initialization).unwrap(),
            caveats: Vec::new(),
        }];

        validate_fresh_initialization_request(&fixture.request).unwrap();

        let mut stale = fixture.request.clone();
        stale.event_drafts[0].payload["initial_event_log_root"] =
            serde_json::Value::String(root('c'));
        let error = validate_fresh_initialization_request(&stale).unwrap_err();
        assert!(error.to_string().contains("initial_event_log_root"));

        let history = verify_authority_history(AuthorityHistoryInput {
            repository_id: &fixture.request.history.repository_id,
            initial_event_log_root: &fixture.request.history.initial_event_log_root,
            initial_actor_registry_root: &fixture.request.history.initial_actor_registry_root,
            authority_keysets: &fixture.request.history.retained_authority_keysets,
            authorization_models: &fixture.request.history.retained_authorization_models,
            authority_events: &fixture.request.history.authority_events,
            authority_envelopes: &fixture.request.history.authority_envelopes,
        })
        .unwrap();
        assert_eq!(history.final_event_log_root, archived_event_root);

        /* An empty authority store on disk as well as in the request: with
        compaction gone there is no origin that carries keyset and policy
        snapshots forward, so record 1 installs both and its object delta has
        to contain them. */
        for directory in [
            ".vela/events",
            ".vela/authority/events",
            ".vela/authority/records",
            ".vela/authority/keysets",
            ".vela/authority/models",
        ] {
            fs::remove_dir_all(fixture.temporary.path().join(directory)).unwrap();
        }
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = fixture
            .execute(fixture.request.clone(), &mut adapter, &mut signer)
            .unwrap();
        assert_eq!(result.before_event_log_root, archived_event_root);
        assert_eq!(signer.calls, 1);
    }

    fn prepared_journal_absent(fixture: &Fixture) -> bool {
        !fixture.journal_dir().join("repository_path").exists()
    }

    #[test]
    fn authority_transaction_accepts_a_reusable_trait_object_signer() {
        let fixture = self::fixture();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let injected: &mut dyn RepositoryAuthoritySigner = &mut signer;
        let result = execute_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            fixture.request.clone(),
            &mut adapter,
            injected,
        )
        .unwrap();

        assert_eq!(signer.calls, 1);
        assert_eq!(result.event_ids.len(), 1);
    }

    #[test]
    fn disposable_writer_installs_one_exact_transaction_and_replays_offline() {
        let fixture = self::fixture();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = fixture
            .execute(fixture.request.clone(), &mut adapter, &mut signer)
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
        let keyset_path =
            authority_keyset_path(&fixture.request.history.authority_keyset.root().unwrap())
                .unwrap();
        let policy_path =
            authority_model_path(&fixture.request.history.authorization_model.root().unwrap())
                .unwrap();
        assert_eq!(
            fs::read(fixture.temporary.path().join(keyset_path)).unwrap(),
            to_canonical_bytes(&fixture.request.history.authority_keyset).unwrap()
        );
        assert_eq!(
            fs::read(fixture.temporary.path().join(policy_path)).unwrap(),
            to_canonical_bytes(&fixture.request.history.authorization_model).unwrap()
        );
        verify_installed_history(&fixture, &[event], &envelope);
    }

    fn acceptance_request(fixture: &Fixture) -> (AuthorityTransactionRequest, StateEvent, Vec<u8>) {
        let proposal_id = "vpr_0123456789abcdef";
        let reason = "Accept the exact bounded scientific transition.";
        let mut semantic_domain = StateEvent {
            schema: EVENT_SCHEMA.into(),
            id: String::new(),
            kind: EventKind::ClaimNoted,
            target: StateTarget {
                r#type: "finding".into(),
                id: "vf_0123456789abcdef".into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: REPOSITORY_PRINCIPAL.into(),
            },
            timestamp: RECORDED_AT.into(),
            reason: reason.into(),
            before_hash: root('a'),
            after_hash: root('b'),
            payload: json!({"annotation": "exact bounded result"}),
            caveats: Vec::new(),
            signature: None,
        };
        semantic_domain.id = compute_event_id(&semantic_domain);
        let proposal_postimage = to_canonical_bytes(&json!({
            "schema": "fixture.proposal.v1",
            "id": proposal_id,
            "status": "applied",
            "applied_event_id": semantic_domain.id.clone(),
        }))
        .unwrap();

        let mut request = fixture.request.clone();
        request.intent_digest = root('e');
        request.authorization_request = fixture_authorization_request(
            &request.history.authorization_model,
            AuthorityActionV1::ReviewAccept,
        );
        request.semantic_approvals[0].action = "review_accept".into();
        request.semantic_approvals[0].reason = reason.into();
        request.semantic_approvals[0].intent_digest = root('e');
        request.event_drafts = vec![
            AuthorityEventDraft {
                kind: semantic_domain.kind.clone(),
                target: semantic_domain.target.clone(),
                actor: semantic_domain.actor.clone(),
                timestamp: semantic_domain.timestamp.clone(),
                reason: semantic_domain.reason.clone(),
                before_hash: semantic_domain.before_hash.clone(),
                after_hash: semantic_domain.after_hash.clone(),
                payload: semantic_domain.payload.clone(),
                caveats: semantic_domain.caveats.clone(),
            },
            AuthorityEventDraft {
                kind: EventKind::ReviewAccepted,
                target: StateTarget {
                    r#type: "proposal".into(),
                    id: proposal_id.into(),
                },
                actor: semantic_domain.actor.clone(),
                timestamp: RECORDED_AT.into(),
                reason: reason.into(),
                before_hash: NULL_HASH.into(),
                after_hash: NULL_HASH.into(),
                payload: json!({
                    "proposal_id": proposal_id,
                    "proposal_kind": "claim.note",
                    "verdict": "accepted",
                    "applied_event_id": semantic_domain.id.clone(),
                }),
                caveats: Vec::new(),
            },
        ];
        request.object_drafts = vec![AuthorityObjectDraft {
            path: proposal_object_path(&proposal_postimage),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(proposal_postimage.clone()),
        }];
        (request, semantic_domain, proposal_postimage)
    }

    #[test]
    fn acceptance_transaction_covers_domain_review_and_proposal_postimage() {
        let fixture = self::fixture();
        let (request, semantic_domain, proposal_postimage) = acceptance_request(&fixture);
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = fixture.execute(request, &mut adapter, &mut signer).unwrap();
        assert_eq!(result.event_ids.len(), 2);
        let events = result
            .event_ids
            .iter()
            .map(|event_id| {
                serde_json::from_slice::<AuthorityEventV1>(
                    &fs::read(
                        fixture
                            .temporary
                            .path()
                            .join(authority_event_path(event_id)),
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let domain = events
            .iter()
            .find(|event| event.content.kind == EventKind::ClaimNoted)
            .unwrap();
        let review = events
            .iter()
            .find(|event| event.content.kind == EventKind::ReviewAccepted)
            .unwrap();
        assert_eq!(domain.semantic_event_id().unwrap(), semantic_domain.id);
        assert_eq!(
            review.content.payload["applied_event_id"],
            json!(semantic_domain.id)
        );
        assert_eq!(
            fs::read(
                fixture
                    .temporary
                    .path()
                    .join(proposal_object_path(&proposal_postimage))
            )
            .unwrap(),
            proposal_postimage
        );
        let envelope: AuthorityEnvelopeV1 = serde_json::from_slice(
            &fs::read(
                fixture
                    .temporary
                    .path()
                    .join(authority_record_path(&result.authority_record_id)),
            )
            .unwrap(),
        )
        .unwrap();
        verify_installed_history(&fixture, &events, &envelope);
    }

    #[test]
    fn acceptance_transaction_rejects_missing_or_cross_transaction_domain_link_before_signing() {
        for mutation in ["missing", "cross_transaction"] {
            let fixture = self::fixture();
            let (mut request, _, _) = acceptance_request(&fixture);
            let review = request
                .event_drafts
                .iter_mut()
                .find(|event| event.kind == EventKind::ReviewAccepted)
                .unwrap();
            if mutation == "missing" {
                review
                    .payload
                    .as_object_mut()
                    .unwrap()
                    .remove("applied_event_id");
            } else {
                review.payload["applied_event_id"] = json!(root('9'));
            }

            let mut adapter = fixture.adapter();
            let mut signer = fixture.signer();
            let error = fixture
                .prepare(request, &mut adapter, &mut signer)
                .unwrap_err();
            assert!(
                error.to_string().contains(if mutation == "missing" {
                    "lacks payload.applied_event_id"
                } else {
                    "must link exactly one scientific event"
                }),
                "{mutation}: {error}"
            );
            assert_eq!(signer.calls, 0);
            assert!(prepared_journal_absent(&fixture));
        }
    }

    #[test]
    fn authority_history_rejects_duplicate_semantic_event_identity() {
        let fixture = self::fixture();
        let (_, semantic_domain, _) = acceptance_request(&fixture);
        let authority_event = |transaction_id: &str| {
            AuthorityEventV1::new(AuthorityEventContentV1 {
                transaction_id: transaction_id.into(),
                principal_id: REPOSITORY_PRINCIPAL.into(),
                authority_mode: AUTHORITY_MODE.into(),
                kind: semantic_domain.kind.clone(),
                target: semantic_domain.target.clone(),
                actor: semantic_domain.actor.clone(),
                timestamp: semantic_domain.timestamp.clone(),
                reason: semantic_domain.reason.clone(),
                before_hash: semantic_domain.before_hash.clone(),
                after_hash: semantic_domain.after_hash.clone(),
                payload: semantic_domain.payload.clone(),
                caveats: semantic_domain.caveats.clone(),
            })
            .unwrap()
        };
        let mut history = fixture.request.history.clone();
        history.authority_events.push(authority_event("vtx_prior"));
        let error =
            validate_semantic_event_links(&history, &[authority_event("vtx_current")]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("occurs more than once in authority history"),
            "{error}"
        );
    }

    #[test]
    fn the_retained_authorization_model_cannot_disappear_or_change() {
        for mutation in ["delete", "alter"] {
            let fixture = fixture();
            let mut adapter = fixture.adapter();
            let mut signer = fixture.signer();
            let result = fixture
                .execute(fixture.request.clone(), &mut adapter, &mut signer)
                .unwrap();
            assert_eq!(signer.calls, 1);
            // Completed journals are ignored operational state and are absent
            // in a clean clone. The signed authority record must therefore
            // independently make retained material loss fail closed.
            fs::remove_dir_all(fixture.journal_dir()).unwrap();

            let event: AuthorityEventV1 = serde_json::from_slice(
                &fs::read(
                    fixture
                        .temporary
                        .path()
                        .join(authority_event_path(&result.event_ids[0])),
                )
                .unwrap(),
            )
            .unwrap();
            let envelope: AuthorityEnvelopeV1 = serde_json::from_slice(
                &fs::read(
                    fixture
                        .temporary
                        .path()
                        .join(authority_record_path(&result.authority_record_id)),
                )
                .unwrap(),
            )
            .unwrap();
            let mut request = fixture.request.clone();
            request.history.authority_events.push(event);
            request.history.authority_envelopes.push(envelope);
            /* The Cedar schema, policy text and entity snapshot used to be
            the retained material this protects. The model is the only policy
            object now, and losing or editing it has to fail the same way. */
            let model_path =
                authority_model_path(&request.history.authorization_model.root().unwrap()).unwrap();
            let absolute = fixture.temporary.path().join(&model_path);
            if mutation == "delete" {
                fs::remove_file(&absolute).unwrap();
            } else {
                fs::write(&absolute, b"a tampered authorization model").unwrap();
            }

            let mut adapter = fixture.adapter();
            let mut signer = fixture.signer();
            let error = fixture
                .prepare(request, &mut adapter, &mut signer)
                .unwrap_err();
            if mutation == "delete" {
                assert!(
                    matches!(error, AuthorityTransactionError::History(_)),
                    "{error:?}"
                );
            } else {
                assert!(
                    matches!(
                        error,
                        AuthorityTransactionError::Transaction(
                            RepositoryTxnError::StaleInput { .. }
                        )
                    ),
                    "{error:?}"
                );
            }
            assert_eq!(signer.calls, 0);
        }
    }

    #[test]
    fn authority_snapshot_paths_and_store_membership_fail_closed() {
        let fixture = fixture();
        let keyset_path =
            authority_keyset_path(&fixture.request.history.authority_keyset.root().unwrap())
                .unwrap();
        let mut request = fixture.request.clone();
        request.object_drafts = vec![AuthorityObjectDraft {
            path: keyset_path.clone(),
            object_kind: "caller_substitution".into(),
            class: WriteClass::Authority,
            postimage: Some(to_canonical_bytes(&json!({"wrong": true})).unwrap()),
        }];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = fixture
            .prepare(request, &mut adapter, &mut signer)
            .unwrap_err();
        assert!(matches!(error, AuthorityTransactionError::Invalid(_)));
        assert_eq!(signer.calls, 0);

        let fixture = self::fixture();
        let keyset_path =
            authority_keyset_path(&fixture.request.history.authority_keyset.root().unwrap())
                .unwrap();
        fs::create_dir_all(fixture.temporary.path().join(".vela/authority/keysets")).unwrap();
        fs::write(
            fixture.temporary.path().join(&keyset_path),
            to_canonical_bytes(&json!({"wrong": true})).unwrap(),
        )
        .unwrap();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = fixture
            .prepare(fixture.request.clone(), &mut adapter, &mut signer)
            .unwrap_err();
        assert!(
            matches!(
                error,
                AuthorityTransactionError::Transaction(RepositoryTxnError::StaleInput { .. })
            ),
            "{error:?}"
        );
        assert_eq!(signer.calls, 0);

        let fixture = self::fixture();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = fixture
            .prepare(fixture.request.clone(), &mut adapter, &mut signer)
            .unwrap();
        fs::create_dir_all(fixture.temporary.path().join(".vela/authority/keysets")).unwrap();
        fs::write(
            fixture
                .temporary
                .path()
                .join(".vela/authority/keysets/unexpected.json"),
            b"{}\n",
        )
        .unwrap();
        let error = prepared.mark_committed().unwrap_err();
        assert!(
            matches!(
                error,
                AuthorityTransactionError::Transaction(RepositoryTxnError::StaleSnapshot { .. })
            ),
            "{error:?}"
        );
        assert_eq!(signer.calls, 1);
        assert_eq!(
            fs::read_dir(fixture.temporary.path().join(".vela/authority/events"))
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            fs::read_dir(fixture.temporary.path().join(".vela/authority/records"))
                .unwrap()
                .count(),
            1
        );
        let policy_path =
            authority_model_path(&fixture.request.history.authorization_model.root().unwrap())
                .unwrap();
        assert_eq!(
            fs::read(fixture.temporary.path().join(policy_path)).unwrap(),
            to_canonical_bytes(&fixture.request.history.authorization_model).unwrap()
        );
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
        assert!(authority_transaction_postimages_absent(&fixture_one));
        assert!(prepared_journal_absent(&fixture_one));

        let fixture_two = fixture();
        let mut adapter = fixture_two.adapter();
        let mut signer = fixture_two.signer();
        signer.fail = true;
        let error = fixture_two
            .prepare(fixture_two.request.clone(), &mut adapter, &mut signer)
            .unwrap_err();
        assert!(matches!(error, AuthorityTransactionError::Signing(_)));
        assert_eq!(signer.calls, 1);
        assert!(authority_transaction_postimages_absent(&fixture_two));
        assert!(prepared_journal_absent(&fixture_two));
    }

    #[test]
    fn history_and_policy_substitution_fail_before_signing_or_journaling() {
        let fixture_one = fixture();
        let mut request = fixture_one.request.clone();
        request.history.authority_envelopes[0].payload.push('A');
        let mut adapter = fixture_one.adapter();
        let mut signer = fixture_one.signer();
        let error = fixture_one
            .prepare(request, &mut adapter, &mut signer)
            .unwrap_err();
        assert!(matches!(error, AuthorityTransactionError::History(_)));
        assert_eq!(signer.calls, 0);
        assert!(authority_transaction_postimages_absent(&fixture_one));
        assert!(prepared_journal_absent(&fixture_one));

        let fixture_two = fixture();
        let mut request = fixture_two.request.clone();
        /* A request the model does not authorize. Cedar's version of this
        was a `forbid` policy overriding the permit; the closed profile has
        no policy text, so the request simply names a principal the model has
        never heard of. */
        request.authorization_request.model_root = root('7');
        let mut adapter = fixture_two.adapter();
        let mut signer = fixture_two.signer();
        let error = fixture_two
            .prepare(request, &mut adapter, &mut signer)
            .unwrap_err();
        assert!(
            matches!(error, AuthorityTransactionError::Invalid(_)),
            "{error:?}"
        );
        assert_eq!(signer.calls, 0);
        assert!(authority_transaction_postimages_absent(&fixture_two));
        assert!(prepared_journal_absent(&fixture_two));
    }

    #[test]
    fn stale_read_set_aborts_before_commit_marker_and_installs_nothing() {
        let fixture = fixture();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = fixture
            .prepare(fixture.request.clone(), &mut adapter, &mut signer)
            .unwrap();
        assert!(authority_transaction_postimages_absent(&fixture));
        fs::write(
            fixture.temporary.path().join(".vela/actors.json"),
            b"tampered after semantic approval",
        )
        .unwrap();
        let error = prepared.mark_committed().unwrap_err();
        assert!(matches!(
            error,
            AuthorityTransactionError::Transaction(RepositoryTxnError::StaleInput { .. })
        ));
        assert!(authority_transaction_postimages_absent(&fixture));
        assert!(matches!(
            prepared.transaction.recovery_state(),
            vela_repository::RecoveryState::Aborted
        ));
    }

    #[test]
    fn authority_object_drift_after_signing_aborts_before_commit_marker() {
        let fixture = fixture();
        fs::create_dir_all(fixture.temporary.path().join(".vela/evidence")).unwrap();
        let path = fixture
            .temporary
            .path()
            .join(".vela/evidence/existing.json");
        fs::write(&path, to_canonical_bytes(&json!({"version": 1})).unwrap()).unwrap();
        let mut request = fixture.request.clone();
        request.object_drafts = vec![AuthorityObjectDraft {
            path: ".vela/evidence/existing.json".into(),
            object_kind: "evidence".into(),
            class: WriteClass::CanonicalEvidence,
            postimage: Some(to_canonical_bytes(&json!({"version": 2})).unwrap()),
        }];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = fixture.prepare(request, &mut adapter, &mut signer).unwrap();
        fs::write(&path, to_canonical_bytes(&json!({"version": 3})).unwrap()).unwrap();
        let error = prepared.mark_committed().unwrap_err();
        assert!(matches!(
            error,
            AuthorityTransactionError::Transaction(RepositoryTxnError::StaleInput { .. })
        ));
        assert_eq!(signer.calls, 1);
        assert!(authority_transaction_postimages_absent(&fixture));
    }

    #[test]
    fn current_claim_and_artifact_paths_are_canonical_evidence() {
        for path in [
            "records/claims/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.json",
            "records/artifacts/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            validate_authority_object_path(
                &RepoPath::parse(path.to_string()).unwrap(),
                WriteClass::CanonicalEvidence,
            )
            .unwrap();
        }
        let proposal = RepoPath::parse(
            "records/proposals/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.json"
                .to_string(),
        )
        .unwrap();
        assert!(
            validate_authority_object_path(&proposal, WriteClass::CanonicalEvidence).is_err(),
            "review objects must not be relabeled canonical evidence"
        );
    }

    #[test]
    fn retired_paths_the_verifier_refuses_are_refused_by_the_writer() {
        for path in [
            ".vela/actors.json",
            ".vela/events/ve_fixture.json",
            ".vela/findings/vf_fixture.json",
            ".vela/proposals/vpr_fixture.json",
            ".vela/artifacts/va_fixture",
            ".vela/policies/active.json",
            "records/receipts/sha256/fixture.json",
            "records/review/pending.json",
            "records/decision-evidence/fixture.json",
            "records/vrc_fixture.json",
        ] {
            assert!(
                crate::repository::is_retired_path(path),
                "{path} is no longer retired; this test is stale"
            );
            let parsed = RepoPath::parse(path.to_string()).unwrap();
            for class in [
                WriteClass::Authority,
                WriteClass::PublicReview,
                WriteClass::CanonicalEvidence,
            ] {
                assert!(
                    validate_authority_object_path(&parsed, class).is_err(),
                    "{path} was admitted as {class:?}; the verifier would then refuse the repository"
                );
            }
        }
    }

    #[test]
    fn repository_history_membership_refuses_stale_forks_and_marker_time_additions() {
        let fixture = fixture();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        fixture
            .execute(fixture.request.clone(), &mut adapter, &mut signer)
            .unwrap();

        let mut adapter = fixture.adapter();
        let mut stale_signer = fixture.signer();
        let error = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            fixture.request.clone(),
            &mut adapter,
            &mut stale_signer,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AuthorityTransactionError::Transaction(RepositoryTxnError::CorruptPlan(message))
                if message.contains("membership differs")
        ));
        assert_eq!(stale_signer.calls, 0);

        let fresh = self::fixture();
        let mut adapter = fresh.adapter();
        let mut signer = fresh.signer();
        let mut prepared = fresh
            .prepare(fresh.request.clone(), &mut adapter, &mut signer)
            .unwrap();
        fs::write(
            fresh
                .temporary
                .path()
                .join(".vela/authority/records/unexpected.dsse.json"),
            b"unexpected",
        )
        .unwrap();
        let error = prepared.mark_committed().unwrap_err();
        assert!(matches!(
            error,
            AuthorityTransactionError::Transaction(RepositoryTxnError::StaleSnapshot { .. })
        ));
        assert_eq!(signer.calls, 1);
        fs::remove_file(
            fresh
                .temporary
                .path()
                .join(".vela/authority/records/unexpected.dsse.json"),
        )
        .unwrap();
        assert!(authority_transaction_postimages_absent(&fresh));
    }

    #[test]
    fn repository_history_bytes_fail_closed_before_authentication_or_signing() {
        let fixture = self::fixture();
        let event_path = fixture.temporary.path().join(format!(
            ".vela/authority/events/{}.json",
            fixture.request.history.authority_events[0].id
        ));
        fs::write(event_path, b"tampered").unwrap();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = fixture
            .prepare(fixture.request.clone(), &mut adapter, &mut signer)
            .unwrap_err();
        assert!(
            matches!(error, AuthorityTransactionError::History(_)),
            "{error}"
        );
        assert_eq!(signer.calls, 0);
        assert!(authority_transaction_postimages_absent(&fixture));
        assert!(prepared_journal_absent(&fixture));

        let missing = self::fixture();
        let payload = BASE64_STANDARD
            .decode(&missing.request.history.authority_envelopes[0].payload)
            .unwrap();
        let record: AuthorityRecordV1 = serde_json::from_slice(&payload).unwrap();
        fs::remove_file(
            missing
                .temporary
                .path()
                .join(authority_record_path(&record.record_id)),
        )
        .unwrap();
        let mut adapter = missing.adapter();
        let mut signer = missing.signer();
        let error = missing
            .prepare(missing.request.clone(), &mut adapter, &mut signer)
            .unwrap_err();
        assert!(
            matches!(error, AuthorityTransactionError::History(_)),
            "{error}"
        );
        assert_eq!(signer.calls, 0);
        assert!(prepared_journal_absent(&missing));
    }

    #[test]
    fn authority_transaction_covers_create_update_delete_and_distinct_write_classes() {
        let fixture = fixture();
        fs::create_dir_all(fixture.temporary.path().join(".vela/evidence")).unwrap();
        let old_update = to_canonical_bytes(&json!({"version": 1})).unwrap();
        let old_delete = to_canonical_bytes(&json!({"delete": true})).unwrap();
        fs::write(
            fixture.temporary.path().join(".vela/evidence/update.json"),
            &old_update,
        )
        .unwrap();
        fs::write(
            fixture.temporary.path().join(".vela/evidence/delete.json"),
            &old_delete,
        )
        .unwrap();
        let mut request = fixture.request.clone();
        let new_update = to_canonical_bytes(&json!({"version": 2})).unwrap();
        let new_proposal = to_canonical_bytes(&json!({"proposal": "pending"})).unwrap();
        let new_authority = to_canonical_bytes(&json!({"decision": "recorded"})).unwrap();
        request.object_drafts = vec![
            AuthorityObjectDraft {
                path: proposal_object_path(&new_proposal),
                object_kind: "proposal".into(),
                class: WriteClass::PublicReview,
                postimage: Some(new_proposal.clone()),
            },
            AuthorityObjectDraft {
                path: ".vela/evidence/update.json".into(),
                object_kind: "evidence".into(),
                class: WriteClass::CanonicalEvidence,
                postimage: Some(new_update.clone()),
            },
            AuthorityObjectDraft {
                path: ".vela/evidence/delete.json".into(),
                object_kind: "evidence".into(),
                class: WriteClass::CanonicalEvidence,
                postimage: None,
            },
            AuthorityObjectDraft {
                path: ".vela/authority/decisions/fixture.json".into(),
                object_kind: "authority_decision".into(),
                class: WriteClass::Authority,
                postimage: Some(new_authority.clone()),
            },
        ];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = fixture.prepare(request, &mut adapter, &mut signer).unwrap();
        let payload = BASE64_STANDARD.decode(&prepared.envelope.payload).unwrap();
        let record: AuthorityRecordV1 = serde_json::from_slice(&payload).unwrap();
        /* Five, not eight: the three Cedar policy-material snapshots that
        every transaction used to backfill are gone with the policy text. */
        assert_eq!(record.content.object_delta.len(), 5);
        assert!(!record.content.object_delta.iter().any(|delta| {
            delta.object_kind == "authority_keyset" || delta.object_kind == "authorization_model"
        }));
        assert!(record.content.object_delta.iter().any(|delta| {
            delta.path == ".vela/evidence/update.json"
                && delta.before_root == Some(ContentDigest::hash(&old_update).as_str().to_string())
                && delta.after_root == Some(ContentDigest::hash(&new_update).as_str().to_string())
        }));
        assert!(record.content.object_delta.iter().any(|delta| {
            delta.path == ".vela/evidence/delete.json"
                && delta.before_root == Some(ContentDigest::hash(&old_delete).as_str().to_string())
                && delta.after_root.is_none()
        }));
        prepared.mark_committed().unwrap();
        prepared.install().unwrap();
        prepared.complete().unwrap();
        assert_eq!(
            fs::read(
                fixture
                    .temporary
                    .path()
                    .join(proposal_object_path(&new_proposal))
            )
            .unwrap(),
            new_proposal
        );
        assert_eq!(
            fs::read(fixture.temporary.path().join(".vela/evidence/update.json")).unwrap(),
            new_update
        );
        assert!(
            !fixture
                .temporary
                .path()
                .join(".vela/evidence/delete.json")
                .exists()
        );
        assert_eq!(signer.calls, 1);
    }

    #[test]
    fn object_only_pending_submission_advances_authority_not_scientific_events() {
        let fixture = fixture();
        let mut request = fixture.request.clone();
        request.event_drafts.clear();
        let pending_bytes = to_canonical_bytes(&json!({"standing": "pending"})).unwrap();
        request.object_drafts = vec![AuthorityObjectDraft {
            path: proposal_object_path(&pending_bytes),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(pending_bytes.clone()),
        }];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = fixture.execute(request, &mut adapter, &mut signer).unwrap();

        assert!(result.event_ids.is_empty());
        assert_eq!(result.before_event_log_root, result.after_event_log_root);
        assert_eq!(
            fs::read(
                fixture
                    .temporary
                    .path()
                    .join(proposal_object_path(&pending_bytes))
            )
            .unwrap(),
            pending_bytes
        );
        let mut envelopes = fixture.request.history.authority_envelopes.clone();
        let envelope: AuthorityEnvelopeV1 = serde_json::from_slice(
            &fs::read(
                fixture
                    .temporary
                    .path()
                    .join(authority_record_path(&result.authority_record_id)),
            )
            .unwrap(),
        )
        .unwrap();
        envelopes.push(envelope);
        let verification = verify_authority_history(AuthorityHistoryInput {
            repository_id: REPOSITORY_ID,
            initial_event_log_root: &fixture.request.history.initial_event_log_root,
            initial_actor_registry_root: &fixture.request.history.initial_actor_registry_root,
            authority_keysets: &fixture.request.history.retained_authority_keysets,
            authorization_models: &fixture.request.history.retained_authorization_models,
            authority_events: &fixture.request.history.authority_events,
            authority_envelopes: &envelopes,
        })
        .unwrap();
        assert_eq!(verification.authority_event_count, 1);
        assert_eq!(verification.authority_record_count, 2);
    }

    #[test]
    fn invalid_or_noop_object_drafts_fail_before_repository_signing() {
        let fixture = fixture();
        let mut request = fixture.request.clone();
        request.event_drafts.clear();
        request.object_drafts.clear();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = fixture
            .prepare(request, &mut adapter, &mut signer)
            .unwrap_err();
        assert!(matches!(error, AuthorityTransactionError::Invalid(_)));
        assert_eq!(signer.calls, 0);
        assert!(prepared_journal_absent(&fixture));

        let fixture = self::fixture();
        let mut request = fixture.request.clone();
        let uncanonical = b"{ \"not\": \"canonical\" }".to_vec();
        request.object_drafts = vec![AuthorityObjectDraft {
            path: proposal_object_path(&uncanonical),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(uncanonical),
        }];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = fixture
            .prepare(request, &mut adapter, &mut signer)
            .unwrap_err();
        assert!(matches!(error, AuthorityTransactionError::Invalid(_)));
        assert_eq!(signer.calls, 0);

        let fixture = self::fixture();
        let existing = fs::read(fixture.temporary.path().join(".vela/input.json")).unwrap();
        let mut request = fixture.request.clone();
        request.object_drafts = vec![AuthorityObjectDraft {
            path: ".vela/input.json".into(),
            object_kind: "evidence".into(),
            class: WriteClass::CanonicalEvidence,
            postimage: Some(existing),
        }];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = fixture
            .prepare(request, &mut adapter, &mut signer)
            .unwrap_err();
        assert!(matches!(error, AuthorityTransactionError::Invalid(_)));
        assert_eq!(signer.calls, 0);
        assert!(prepared_journal_absent(&fixture));
    }

    #[test]
    fn transaction_identity_binds_read_set_and_execution_pin() {
        let fixture_one = fixture();
        let mut adapter = fixture_one.adapter();
        let mut signer = fixture_one.signer();
        let baseline = fixture_one
            .prepare(fixture_one.request.clone(), &mut adapter, &mut signer)
            .unwrap()
            .result
            .transaction_id;

        let fixture_two = fixture();
        let mut request = fixture_two.request.clone();
        request.read_set[0].digest = ContentDigest::parse(root('a')).unwrap();
        let mut adapter = fixture_two.adapter();
        let mut signer = fixture_two.signer();
        let changed_read_set = fixture_two
            .prepare(request, &mut adapter, &mut signer)
            .unwrap()
            .result
            .transaction_id;

        let fixture_three = fixture();
        let mut request = fixture_three.request.clone();
        request.binary_sha256 = root('b');
        let mut adapter = fixture_three.adapter();
        let mut signer = fixture_three.signer();
        let changed_binary = fixture_three
            .prepare(request, &mut adapter, &mut signer)
            .unwrap()
            .result
            .transaction_id;

        assert_ne!(baseline, changed_read_set);
        assert_ne!(baseline, changed_binary);
    }

    #[test]
    fn committed_partial_install_recovers_without_authentication_or_resigning() {
        let fixture = fixture();
        let mut request = fixture.request.clone();
        let proposal_bytes = to_canonical_bytes(&json!({"proposal": "recoverable"})).unwrap();
        request.object_drafts = vec![AuthorityObjectDraft {
            path: proposal_object_path(&proposal_bytes),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(proposal_bytes.clone()),
        }];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = fixture.prepare(request, &mut adapter, &mut signer).unwrap();
        let result = prepared.result.clone();
        let events = prepared.events.clone();
        let envelope = prepared.envelope.clone();
        prepared.mark_committed().unwrap();
        let error = prepared
            .transaction_mut()
            .install_at_failpoint(RepositoryTxnStep::BeforeInstallWrite { index: 1 })
            .unwrap_err();
        assert!(matches!(
            error,
            RepositoryTxnError::InjectedFailure {
                step: RepositoryTxnStep::BeforeInstallWrite { index: 1 }
            }
        ));
        drop(prepared);
        let operation_id = OperationId::parse(result.operation_id).unwrap();
        assert_eq!(
            RepositoryTxn::recover(
                fixture.temporary.path(),
                &fixture.journal_dir(),
                &operation_id,
                REPOSITORY_ID,
            )
            .unwrap()
            .outcome,
            RecoveryOutcome::Completed
        );
        assert_eq!(signer.calls, 1);
        assert_eq!(
            fs::read(
                fixture
                    .temporary
                    .path()
                    .join(proposal_object_path(&proposal_bytes))
            )
            .unwrap(),
            proposal_bytes
        );
        verify_installed_history(&fixture, &events, &envelope);
    }

    #[test]
    fn committed_object_only_partial_install_recovers_without_resigning() {
        let fixture = fixture();
        let mut request = fixture.request.clone();
        request.event_drafts.clear();
        let proposal_bytes =
            to_canonical_bytes(&json!({"proposal": "object-only-recovery"})).unwrap();
        request.object_drafts = vec![AuthorityObjectDraft {
            path: proposal_object_path(&proposal_bytes),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(proposal_bytes.clone()),
        }];
        let before_event_root = verified_fixture_history(&fixture).final_event_log_root;
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = fixture.prepare(request, &mut adapter, &mut signer).unwrap();
        assert!(prepared.result.event_ids.is_empty());
        assert_eq!(prepared.result.before_event_log_root, before_event_root);
        assert_eq!(prepared.result.after_event_log_root, before_event_root);
        let result = prepared.result.clone();
        let envelope = prepared.envelope.clone();
        prepared.mark_committed().unwrap();
        let error = prepared
            .transaction_mut()
            .install_at_failpoint(RepositoryTxnStep::BeforeInstallWrite { index: 1 })
            .unwrap_err();
        assert!(matches!(
            error,
            RepositoryTxnError::InjectedFailure {
                step: RepositoryTxnStep::BeforeInstallWrite { index: 1 }
            }
        ));
        drop(prepared);

        let operation_id = OperationId::parse(result.operation_id).unwrap();
        assert_eq!(
            RepositoryTxn::recover(
                fixture.temporary.path(),
                &fixture.journal_dir(),
                &operation_id,
                REPOSITORY_ID,
            )
            .unwrap()
            .outcome,
            RecoveryOutcome::Completed
        );
        assert_eq!(signer.calls, 1);
        assert_eq!(
            fs::read(
                fixture
                    .temporary
                    .path()
                    .join(proposal_object_path(&proposal_bytes))
            )
            .unwrap(),
            proposal_bytes
        );
        verify_installed_history(&fixture, &[], &envelope);
    }
}
