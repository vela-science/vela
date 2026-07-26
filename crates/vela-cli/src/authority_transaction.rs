//! Era-1 repository-authority transaction writer.
//!
//! The writer composes authority records, runtime authentication, retained
//! Cedar material, and the existing recoverable Frontier transaction without
//! introducing a second journal. Routine work leases and the narrow governed
//! work-policy rotation use this same core.

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
use vela_authority::{CedarEvaluationInput, CedarPolicyMaterial};
use vela_protocol::authentication::AuthenticationObservationV1;
use vela_protocol::authority::{
    AUTHORITY_MODE, AUTHORITY_PAYLOAD_TYPE_V1, AuthorityEnvelopeV1, AuthorityEventContentV1,
    AuthorityEventV1, AuthorityKeysetV1, AuthorityRecordContentV1, AuthorityRecordV1,
    AuthorizationClaimV1, DelegationClaimV1, DsseSignatureV1, ExecutionClaimV1, ObjectDeltaV1,
    PolicyBundleV1, PrincipalSnapshotV1, SemanticApprovalV1, verify_authority_envelope,
    verify_authority_keyset_transition, verify_policy_bundle_transition,
};
use vela_protocol::authority_history::{
    AUTHORITY_CLOSE_ACTION, AUTHORITY_CLOSED_EVENT_KIND, AUTHORITY_ROTATE_ACTION, AuthorityCloseV1,
    AuthorityHistoryEra, AuthorityHistoryInput, AuthorityHistoryVerification, POLICY_ROTATE_ACTION,
    authority_event_log_root, verify_authority_history,
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
const LAYOUT_SCHEMA: &str = "vela.authority-layout.internal.v2";
const RESULT_SCHEMA: &str = "vela.authority-transaction-result.internal.v1";
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
    pub(crate) retained_authority_keysets: Vec<AuthorityKeysetV1>,
    pub(crate) retained_policy_bundles: Vec<PolicyBundleV1>,
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
/// from the held Frontier and refuses no-ops, duplicate paths, derived views,
/// private coordination, legacy events, or covering-record paths.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct AuthorityObjectDraft {
    pub(crate) path: String,
    pub(crate) object_kind: String,
    pub(crate) class: WriteClass,
    pub(crate) postimage: Option<Vec<u8>>,
}

/// Non-authoritative materialized postimage installed by the same recoverable
/// transaction as its covering canonical change.
///
/// Derived bytes are bound to the journal and its active recovery checks, but
/// do not enter the signed scientific object delta or authority write-set
/// root. `None` deletes an obsolete derived file.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AuthorityDerivedDraft {
    pub(crate) path: String,
    pub(crate) postimage: Option<Vec<u8>>,
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
    pub(crate) object_drafts: Vec<AuthorityObjectDraft>,
    pub(crate) derived_drafts: Vec<AuthorityDerivedDraft>,
    pub(crate) next_authority_keyset: Option<AuthorityKeysetV1>,
    pub(crate) next_policy_bundle: Option<PolicyBundleV1>,
    pub(crate) next_policy_material: Option<CedarPolicyMaterial>,
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
    transaction: FrontierTxn,
    pub(crate) result: AuthorityTransactionResult,
    pub(crate) events: Vec<AuthorityEventV1>,
    pub(crate) envelope: AuthorityEnvelopeV1,
}

impl PreparedAuthorityTransaction {
    pub(crate) fn resolved_public_writes(
        &self,
    ) -> Result<Vec<crate::frontier_txn::ResolvedWrite>, AuthorityTransactionError> {
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
    if request.event_drafts.is_empty()
        && request.object_drafts.is_empty()
        && request.next_authority_keyset.is_none()
        && request.next_policy_bundle.is_none()
    {
        return Err(AuthorityTransactionError::Invalid(
            "authority transaction intent changes no event, object, keyset, or policy bundle"
                .into(),
        ));
    }
    if request
        .next_authority_keyset
        .as_ref()
        .is_some_and(|keyset| keyset.closed)
        && !request.object_drafts.is_empty()
    {
        return Err(AuthorityTransactionError::Invalid(
            "authority close cannot include additional object drafts".into(),
        ));
    }
    normalize_authority_snapshots(frontier_root, &mut request)?;
    normalize_object_drafts(frontier_root, &mut request)?;
    normalize_derived_drafts(frontier_root, &mut request)?;
    validate_request_shape(&request)?;
    let history = verify_authority_history(AuthorityHistoryInput {
        frontier_id: &request.history.frontier_id,
        legacy_events: &request.history.legacy_events,
        legacy_actor_registry_bytes: &request.history.legacy_actor_registry_bytes,
        legacy_active_policy_head_root: &request.history.legacy_active_policy_head_root,
        legacy_policy_store_manifest_root: &request.history.legacy_policy_store_manifest_root,
        authority_keysets: &request.history.retained_authority_keysets,
        policy_bundles: &request.history.retained_policy_bundles,
        authority_events: &request.history.authority_events,
        authority_envelopes: &request.history.authority_envelopes,
    })
    .map_err(AuthorityTransactionError::History)?;
    bind_repository_authority_history(frontier_root, &mut request)?;
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
    validate_active_authority_snapshots(&request, &history)?;
    validate_requested_rotation(&request, sequence, &previous_authority_record_root)?;

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
    validate_semantic_event_links(&request.history, &events)?;

    let mut cumulative_events = request.history.authority_events.iter().collect::<Vec<_>>();
    cumulative_events.extend(events.iter());
    let legacy_root_with_bridge =
        format!("sha256:{}", event_log_hash(&request.history.legacy_events));
    let after_event_log_root = if events.is_empty() {
        history.final_event_log_root.clone()
    } else {
        authority_event_log_root(&legacy_root_with_bridge, &cumulative_events)
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
    let unsigned_draft = DeltaDraft::prepare(frontier_root, content_writes.clone())
        .map_err(AuthorityTransactionError::Transaction)?;
    let object_delta = authority_object_delta(&unsigned_draft, &events, &request.object_drafts)?;

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
    let mut candidate_keysets = request.history.retained_authority_keysets.clone();
    if let Some(next) = &request.next_authority_keyset {
        candidate_keysets.push(next.clone());
    }
    let mut candidate_policies = request.history.retained_policy_bundles.clone();
    if let Some(next) = &request.next_policy_bundle {
        candidate_policies.push(next.clone());
    }
    let mut candidate_events = request.history.authority_events.clone();
    candidate_events.extend(events.iter().cloned());
    let mut candidate_envelopes = request.history.authority_envelopes.clone();
    candidate_envelopes.push(envelope.clone());
    let candidate_history = verify_authority_history(AuthorityHistoryInput {
        frontier_id: &request.history.frontier_id,
        legacy_events: &request.history.legacy_events,
        legacy_actor_registry_bytes: &request.history.legacy_actor_registry_bytes,
        legacy_active_policy_head_root: &request.history.legacy_active_policy_head_root,
        legacy_policy_store_manifest_root: &request.history.legacy_policy_store_manifest_root,
        authority_keysets: &candidate_keysets,
        policy_bundles: &candidate_policies,
        authority_events: &candidate_events,
        authority_envelopes: &candidate_envelopes,
    })
    .map_err(AuthorityTransactionError::History)?;
    let expected_keyset_root = request
        .next_authority_keyset
        .as_ref()
        .unwrap_or(&request.history.authority_keyset)
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    let expected_policy_root = request
        .next_policy_bundle
        .as_ref()
        .unwrap_or(&request.history.policy_bundle)
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    if candidate_history.final_authority_record_root.as_deref()
        != Some(verified.record_root.as_str())
        || candidate_history.final_authority_keyset_root.as_deref()
            != Some(expected_keyset_root.as_str())
        || candidate_history.final_policy_bundle_root.as_deref()
            != Some(expected_policy_root.as_str())
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
    writes.extend(
        request
            .derived_drafts
            .iter()
            .map(authority_derived_planned_write)
            .collect::<Result<Vec<_>, _>>()?,
    );
    let draft = DeltaDraft::prepare(frontier_root, writes)
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
        .filter(|write| write.path.as_str() != record_path && write.class != WriteClass::Derived)
        .cloned()
        .collect::<Vec<_>>();
    if record_writes.len() != 1
        || !matches!(
            record_writes[0].preimage,
            crate::frontier_txn::FileState::Absent
        )
        || content_delta != unsigned_draft.delta.writes()
    {
        return Err(AuthorityTransactionError::Invalid(
            "covering record must be new and the signed object delta must equal the journal delta"
                .into(),
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
    let layout_identity = to_canonical_bytes(&LayoutCommitment {
        schema: LAYOUT_SCHEMA,
        frontier_id: &request.history.frontier_id,
        authority_event_paths: &event_ids
            .iter()
            .map(|event_id| authority_event_path(event_id))
            .collect::<Vec<_>>(),
        object_paths: &request
            .object_drafts
            .iter()
            .map(|draft| draft.path.clone())
            .collect::<Vec<_>>(),
        derived_paths: &request
            .derived_drafts
            .iter()
            .map(|draft| draft.path.clone())
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
            result: serde_json::to_value(DurableAuthorityTransactionResult {
                schema: RESULT_SCHEMA.into(),
                result: result.clone(),
            })
            .map_err(|error| AuthorityTransactionError::Invalid(error.to_string()))?,
        },
        draft.delta.clone(),
    )
    .map_err(AuthorityTransactionError::Transaction)?;
    let transaction = FrontierTxn::prepare_with_barrier(barrier, plan, draft)
        .map_err(AuthorityTransactionError::Transaction)?;
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

/// Return one already-completed authority result without authenticating or
/// signing again.
///
/// The caller must retain the complete result from the first execution. A
/// short ID, intent digest, or transaction ID alone is insufficient. The
/// recovery barrier verifies the completed marker, blobs, canonical
/// postimages, and historical event commitment before this function compares
/// the durable result byte-for-byte.
pub(crate) fn retry_completed_authority_transaction(
    barrier: &CanonicalWriteBarrier,
    expected: &AuthorityTransactionResult,
) -> Result<AuthorityTransactionResult, AuthorityTransactionError> {
    let operation_id = OperationId::parse(expected.operation_id.as_str())
        .map_err(AuthorityTransactionError::Transaction)?;
    let plan = barrier
        .completed_plan(&operation_id)
        .map_err(AuthorityTransactionError::Transaction)?
        .ok_or_else(|| {
            AuthorityTransactionError::Invalid(format!(
                "authority transaction {} is not completed",
                expected.operation_id
            ))
        })?;
    let durable: DurableAuthorityTransactionResult = serde_json::from_value(plan.result.clone())
        .map_err(|error| {
            AuthorityTransactionError::Invalid(format!(
                "completed authority transaction result is invalid: {error}"
            ))
        })?;
    if durable.schema != RESULT_SCHEMA || durable.result != *expected {
        return Err(AuthorityTransactionError::Invalid(
            "completed authority transaction does not match the exact retained result".into(),
        ));
    }
    Ok(durable.result)
}

fn validate_semantic_event_links(
    history: &AuthorityHistorySnapshot,
    events: &[AuthorityEventV1],
) -> Result<(), AuthorityTransactionError> {
    let mut semantic_ids = history
        .legacy_events
        .iter()
        .map(|event| (event.id.clone(), event.kind.clone()))
        .collect::<BTreeMap<_, _>>();
    for event in history.authority_events.iter().chain(events) {
        if matches!(event.content.kind, EventKind::Other(_)) {
            continue;
        }
        let semantic_id = if event.content.kind == EventKind::AttemptClaimed {
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
                "semantic event identity {semantic_id} occurs more than once across the dual log"
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
        .policy_bundle
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    if history.final_authority_keyset_root.as_deref() != Some(keyset_root.as_str())
        || history.final_policy_bundle_root.as_deref() != Some(policy_root.as_str())
    {
        return Err(AuthorityTransactionError::History(
            "caller active authority snapshots differ from the verified history head".into(),
        ));
    }
    Ok(())
}

fn validate_requested_rotation(
    request: &AuthorityTransactionRequest,
    record_sequence: u64,
    previous_authority_record_root: &str,
) -> Result<(), AuthorityTransactionError> {
    if request.next_authority_keyset.is_some() && request.next_policy_bundle.is_some() {
        return Err(AuthorityTransactionError::Invalid(
            "one authority transaction may rotate either the keyset or the policy, not both".into(),
        ));
    }
    let activation_sequence = record_sequence
        .checked_add(1)
        .ok_or_else(|| AuthorityTransactionError::Invalid("rotation sequence overflows".into()))?;
    if let Some(next) = &request.next_authority_keyset {
        verify_authority_keyset_transition(
            &request.history.authority_keyset,
            next,
            activation_sequence,
            previous_authority_record_root,
        )
        .map_err(AuthorityTransactionError::Invalid)?;
        let required_action = if next.closed {
            AUTHORITY_CLOSE_ACTION
        } else {
            AUTHORITY_ROTATE_ACTION
        };
        if !request
            .semantic_approvals
            .iter()
            .any(|approval| approval.action == required_action)
        {
            return Err(AuthorityTransactionError::Invalid(format!(
                "authority transition lacks {required_action} semantic approval"
            )));
        }
        if next.closed {
            validate_requested_close(
                request,
                next,
                record_sequence,
                previous_authority_record_root,
            )?;
        }
    }
    if let Some(next) = &request.next_policy_bundle {
        verify_policy_bundle_transition(&request.history.policy_bundle, next)
            .map_err(AuthorityTransactionError::Invalid)?;
        if !request
            .semantic_approvals
            .iter()
            .any(|approval| approval.action == POLICY_ROTATE_ACTION)
        {
            return Err(AuthorityTransactionError::Invalid(
                "policy rotation lacks policy_rotate semantic approval".into(),
            ));
        }
    }
    Ok(())
}

fn validate_requested_close(
    request: &AuthorityTransactionRequest,
    closed_keyset: &AuthorityKeysetV1,
    record_sequence: u64,
    previous_authority_record_root: &str,
) -> Result<(), AuthorityTransactionError> {
    if request.event_drafts.len() != 1 {
        return Err(AuthorityTransactionError::Invalid(
            "authority close must contain exactly one event".into(),
        ));
    }
    let draft = &request.event_drafts[0];
    if draft.kind.as_str() != AUTHORITY_CLOSED_EVENT_KIND
        || draft.target.r#type != "frontier"
        || draft.target.id != request.history.frontier_id
        || draft.actor.r#type != "human"
        || draft.before_hash != draft.after_hash
    {
        return Err(AuthorityTransactionError::Invalid(
            "authority close event shape is invalid".into(),
        ));
    }
    let payload: AuthorityCloseV1 =
        serde_json::from_value(draft.payload.clone()).map_err(|error| {
            AuthorityTransactionError::Invalid(format!(
                "authority close payload is invalid: {error}"
            ))
        })?;
    payload
        .validate()
        .map_err(AuthorityTransactionError::Invalid)?;
    let closed_root = closed_keyset
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    let current_keyset_root = request
        .history
        .authority_keyset
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    let current_policy_root = request
        .history
        .policy_bundle
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    if payload.frontier_id != request.history.frontier_id
        || payload.last_trusted_sequence != record_sequence.saturating_sub(1)
        || payload.last_trusted_authority_record_root != previous_authority_record_root
        || payload.previous_authority_keyset_root != current_keyset_root
        || payload.closed_authority_keyset_root != closed_root
        || payload.policy_bundle_root != current_policy_root
        || payload.reason != draft.reason
    {
        return Err(AuthorityTransactionError::Invalid(
            "authority close payload does not match the exact terminal transition".into(),
        ));
    }
    Ok(())
}

fn normalize_authority_snapshots(
    frontier_root: &Path,
    request: &mut AuthorityTransactionRequest,
) -> Result<(), AuthorityTransactionError> {
    if request.object_drafts.iter().any(|draft| {
        draft.path.starts_with(".vela/authority/keysets/")
            || draft.path.starts_with(".vela/authority/policies/")
            || draft.path.starts_with(".vela/authority/policy-material/")
    }) {
        return Err(AuthorityTransactionError::Invalid(
            "authority keyset, policy, and policy-material snapshots are derived from the verified history, not caller object drafts"
                .into(),
        ));
    }
    if request.next_policy_bundle.is_some() != request.next_policy_material.is_some() {
        return Err(AuthorityTransactionError::Invalid(
            "a policy rotation requires both the next bundle manifest and its exact Cedar material"
                .into(),
        ));
    }

    let mut snapshots = BTreeMap::<String, (&'static str, Vec<u8>)>::new();
    for keyset in request
        .history
        .retained_authority_keysets
        .iter()
        .chain(request.next_authority_keyset.iter())
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
        .retained_policy_bundles
        .iter()
        .chain(request.next_policy_bundle.iter())
    {
        let root = bundle.root().map_err(AuthorityTransactionError::Invalid)?;
        snapshots.insert(
            authority_policy_path(&root)?,
            (
                "policy_bundle",
                to_canonical_bytes(bundle).map_err(AuthorityTransactionError::Invalid)?,
            ),
        );
    }
    let current_material = CedarPolicyMaterial::from_evaluation(&request.authorization_input);
    current_material
        .validate_against(&request.history.policy_bundle)
        .map_err(AuthorityTransactionError::Invalid)?;
    let terminal_close = request
        .next_authority_keyset
        .as_ref()
        .is_some_and(|keyset| keyset.closed);
    // The terminal close contract intentionally permits only the close event
    // and closed-keyset snapshot. A closed repository needs no later policy
    // execution, so do not opportunistically backfill policy material there.
    if !terminal_close {
        for (path, object_kind, bytes) in
            authority_policy_material_snapshots(&request.history.policy_bundle, &current_material)?
        {
            insert_authority_snapshot(&mut snapshots, path, object_kind, bytes)?;
        }
    }
    if let (Some(bundle), Some(material)) = (
        request.next_policy_bundle.as_ref(),
        request.next_policy_material.as_ref(),
    ) {
        material
            .validate_against(bundle)
            .map_err(AuthorityTransactionError::Invalid)?;
        for (path, object_kind, bytes) in authority_policy_material_snapshots(bundle, material)? {
            insert_authority_snapshot(&mut snapshots, path, object_kind, bytes)?;
        }
    }

    for directory in [
        ".vela/authority/keysets",
        ".vela/authority/policies",
        ".vela/authority/policy-material/schema",
        ".vela/authority/policy-material/policies",
        ".vela/authority/policy-material/entities",
    ] {
        let binding = InputBinding::current_directory(
            frontier_root,
            RepoPath::parse(directory.to_string())
                .map_err(AuthorityTransactionError::Transaction)?,
        )
        .map_err(AuthorityTransactionError::Transaction)?;
        merge_input_binding(&mut request.read_set, binding)?;
    }

    for (path_text, (object_kind, bytes)) in snapshots {
        let path =
            RepoPath::parse(path_text.clone()).map_err(AuthorityTransactionError::Transaction)?;
        let absolute = frontier_root.join(path.as_str());
        match fs::symlink_metadata(&absolute) {
            Ok(_) => {
                let binding = InputBinding::exact_file(frontier_root, path, &bytes)
                    .map_err(AuthorityTransactionError::Transaction)?;
                merge_input_binding(&mut request.read_set, binding)?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                if authority_history_retains_path(&request.history, &path_text)? {
                    return Err(AuthorityTransactionError::History(format!(
                        "retained authority policy material {path_text} is missing"
                    )));
                }
                let binding = InputBinding::absent_file(frontier_root, path)
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
                    FrontierTxnError::Io(format!(
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

fn insert_authority_snapshot(
    snapshots: &mut BTreeMap<String, (&'static str, Vec<u8>)>,
    path: String,
    object_kind: &'static str,
    bytes: Vec<u8>,
) -> Result<(), AuthorityTransactionError> {
    if let Some((existing_kind, existing_bytes)) = snapshots.get(&path) {
        if *existing_kind != object_kind || *existing_bytes != bytes {
            return Err(AuthorityTransactionError::Invalid(format!(
                "authority snapshot path {path} has conflicting canonical bytes"
            )));
        }
        return Ok(());
    }
    snapshots.insert(path, (object_kind, bytes));
    Ok(())
}

fn normalize_object_drafts(
    frontier_root: &Path,
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
            InputBinding::current_file(frontier_root, path)
                .map_err(AuthorityTransactionError::Transaction)?,
        );
    }
    for binding in object_inputs {
        merge_input_binding(&mut request.read_set, binding)?;
    }
    Ok(())
}

fn normalize_derived_drafts(
    frontier_root: &Path,
    request: &mut AuthorityTransactionRequest,
) -> Result<(), AuthorityTransactionError> {
    request
        .derived_drafts
        .sort_by(|left, right| left.path.cmp(&right.path));
    let mut previous_path: Option<&str> = None;
    let mut derived_inputs = Vec::with_capacity(request.derived_drafts.len());
    for draft in &request.derived_drafts {
        if previous_path == Some(draft.path.as_str()) {
            return Err(AuthorityTransactionError::Invalid(format!(
                "duplicate derived postimage path {}",
                draft.path
            )));
        }
        previous_path = Some(&draft.path);
        if request
            .object_drafts
            .iter()
            .any(|object| object.path == draft.path)
        {
            return Err(AuthorityTransactionError::Invalid(format!(
                "derived postimage path {} overlaps an authority object",
                draft.path
            )));
        }
        let path =
            RepoPath::parse(draft.path.clone()).map_err(AuthorityTransactionError::Transaction)?;
        validate_authority_derived_path(&path)?;
        derived_inputs.push(
            InputBinding::current_file(frontier_root, path)
                .map_err(AuthorityTransactionError::Transaction)?,
        );
    }
    for binding in derived_inputs {
        merge_input_binding(&mut request.read_set, binding)?;
    }
    Ok(())
}

fn bind_repository_authority_history(
    frontier_root: &Path,
    request: &mut AuthorityTransactionRequest,
) -> Result<(), AuthorityTransactionError> {
    let mut bindings = Vec::new();
    let mut legacy_event_paths = Vec::new();
    for event in &request.history.legacy_events {
        let path = RepoPath::parse(format!(".vela/events/{}.json", event.id))
            .map_err(AuthorityTransactionError::Transaction)?;
        bindings.push(exact_verified_json_input(frontier_root, &path, event)?);
        legacy_event_paths.push(path);
    }
    let mut authority_event_paths = Vec::new();
    for event in &request.history.authority_events {
        let path = RepoPath::parse(authority_event_path(&event.id))
            .map_err(AuthorityTransactionError::Transaction)?;
        bindings.push(exact_verified_json_input(frontier_root, &path, event)?);
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
        bindings.push(exact_verified_json_input(frontier_root, &path, envelope)?);
        authority_record_paths.push(path);
    }
    bindings.push(
        InputBinding::exact_file(
            frontier_root,
            RepoPath::parse(".vela/actors.json".to_string())
                .map_err(AuthorityTransactionError::Transaction)?,
            &request.history.legacy_actor_registry_bytes,
        )
        .map_err(AuthorityTransactionError::Transaction)?,
    );
    for (directory, paths) in [
        (".vela/events", legacy_event_paths),
        (".vela/authority/events", authority_event_paths),
        (".vela/authority/records", authority_record_paths),
    ] {
        bindings.push(
            InputBinding::exact_directory(
                frontier_root,
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
    frontier_root: &Path,
    path: &RepoPath,
    expected: &T,
) -> Result<InputBinding, AuthorityTransactionError>
where
    T: Serialize + serde::de::DeserializeOwned,
{
    const MAX_AUTHORITY_MEMBER_BYTES: u64 = 16 * 1024 * 1024;
    let bytes = crate::bounded_file::read_bounded_frontier_file(
        frontier_root,
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
    InputBinding::exact_file(frontier_root, path.clone(), &bytes)
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
    if value.starts_with(".vela/events/")
        || value.starts_with(".vela/authority/events/")
        || value.starts_with(".vela/authority/records/")
    {
        return Err(AuthorityTransactionError::Invalid(format!(
            "authority object drafts cannot replace event or covering-record path {value}"
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
                || value.starts_with("records/artifacts/")
        }
        WriteClass::Derived | WriteClass::PrivateCoordination => false,
    };
    if !valid {
        return Err(AuthorityTransactionError::Invalid(format!(
            "authority object path {value} is incompatible with write class {class:?}"
        )));
    }
    Ok(())
}

fn validate_authority_derived_path(path: &RepoPath) -> Result<(), AuthorityTransactionError> {
    let value = path.as_str();
    if authority_derived_path(value) {
        return Ok(());
    }
    Err(AuthorityTransactionError::Invalid(format!(
        "authority transaction derived path {value} is not a Vela-owned materialized view"
    )))
}

pub(crate) fn authority_derived_path(value: &str) -> bool {
    value == "frontier.json"
        || value == "vela.lock"
        || value == ".vela/proof-state.json"
        || value.starts_with("proof/")
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

fn authority_derived_planned_write(
    draft: &AuthorityDerivedDraft,
) -> Result<PlannedWrite, AuthorityTransactionError> {
    let path =
        RepoPath::parse(draft.path.clone()).map_err(AuthorityTransactionError::Transaction)?;
    Ok(match &draft.postimage {
        Some(bytes) => PlannedWrite::write(path, WriteClass::Derived, bytes.clone()),
        None => PlannedWrite::delete(path, WriteClass::Derived),
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

fn file_state_root(state: &crate::frontier_txn::FileState) -> Option<String> {
    match state {
        crate::frontier_txn::FileState::Absent => None,
        crate::frontier_txn::FileState::File { digest, .. } => Some(digest.as_str().to_string()),
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
    let authorization_schema_root =
        ContentDigest::hash(request.authorization_input.schema.as_bytes());
    let authorization_policies_root =
        ContentDigest::hash(request.authorization_input.policies.as_bytes());
    let authorization_entities = to_canonical_bytes(&request.authorization_input.entities)
        .map_err(AuthorityTransactionError::Invalid)?;
    let authorization_entities_root = ContentDigest::hash(authorization_entities);
    if request.history.policy_bundle.cedar_schema_root != authorization_schema_root.as_str()
        || request.history.policy_bundle.policies_root != authorization_policies_root.as_str()
        || request.history.policy_bundle.entities_root != authorization_entities_root.as_str()
    {
        return Err(AuthorityTransactionError::Invalid(
            "runtime Cedar schema, policies, or entities differ from the retained policy bundle"
                .into(),
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

pub(crate) fn authority_policy_path(root: &str) -> Result<String, AuthorityTransactionError> {
    Ok(format!(
        ".vela/authority/policies/{}.json",
        authority_snapshot_stem(root)?
    ))
}

pub(crate) type AuthorityPolicyMaterialSnapshot = (String, &'static str, Vec<u8>);

pub(crate) fn authority_policy_material_snapshots(
    bundle: &PolicyBundleV1,
    material: &CedarPolicyMaterial,
) -> Result<Vec<AuthorityPolicyMaterialSnapshot>, AuthorityTransactionError> {
    material
        .validate_against(bundle)
        .map_err(AuthorityTransactionError::Invalid)?;
    let entities = material
        .canonical_entities()
        .map_err(AuthorityTransactionError::Invalid)?;
    let [schema_path, policies_path, entities_path] = authority_policy_material_paths(bundle)?;
    Ok(vec![
        (
            schema_path,
            "cedar_schema",
            material.schema.as_bytes().to_vec(),
        ),
        (
            policies_path,
            "cedar_policies",
            material.policies.as_bytes().to_vec(),
        ),
        (entities_path, "cedar_entities", entities),
    ])
}

pub(crate) fn authority_policy_material_paths(
    bundle: &PolicyBundleV1,
) -> Result<[String; 3], AuthorityTransactionError> {
    Ok([
        format!(
            ".vela/authority/policy-material/schema/{}.cedarschema",
            authority_snapshot_stem(&bundle.cedar_schema_root)?
        ),
        format!(
            ".vela/authority/policy-material/policies/{}.cedar",
            authority_snapshot_stem(&bundle.policies_root)?
        ),
        format!(
            ".vela/authority/policy-material/entities/{}.json",
            authority_snapshot_stem(&bundle.entities_root)?
        ),
    ])
}

fn authority_snapshot_stem(root: &str) -> Result<&str, AuthorityTransactionError> {
    ContentDigest::parse(root.to_string()).map_err(AuthorityTransactionError::Transaction)?;
    root.strip_prefix("sha256:")
        .ok_or_else(|| AuthorityTransactionError::Invalid("snapshot root lacks sha256 tag".into()))
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
    object_drafts: &'a [AuthorityObjectDraft],
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
    object_paths: &'a [String],
    derived_paths: &'a [String],
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

    fn fixture_authorization_input() -> CedarEvaluationInput {
        CedarEvaluationInput {
            schema: r#"
                entity Human;
                entity Proposal;
                entity Frontier;
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
                action "review_accept" appliesTo {
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
                action "authority_rotate" appliesTo {
                    principal: Human,
                    resource: Frontier,
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
                action "authority_close" appliesTo {
                    principal: Human,
                    resource: Frontier,
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
                action "policy_rotate" appliesTo {
                    principal: Human,
                    resource: Frontier,
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
                },
                {
                    "uid": {"type": "Frontier", "id": FRONTIER_ID},
                    "attrs": {},
                    "parents": []
                }
            ]),
            principal: format!(r#"Human::"{REPOSITORY_PRINCIPAL}""#),
            principal_class: PrincipalClass::Human,
            action: "review_reject".into(),
            resource: r#"Proposal::"vpr_0123456789abcdef""#.into(),
            context: json!({"exact": true}),
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
            let signature = self.key.sign(&dsse_pae(payload_type, canonical_payload));
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
                key_id: "repository-key-1".into(),
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
        fs::write(root_path.join(".vela/input.json"), b"{\"fixture\":true}\n").unwrap();

        let authorization_input = fixture_authorization_input();
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
            closed: false,
        };
        let policy_bundle = PolicyBundleV1 {
            schema: POLICY_BUNDLE_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            cedar_schema_root: ContentDigest::hash(authorization_input.schema.as_bytes())
                .as_str()
                .into(),
            policies_root: ContentDigest::hash(authorization_input.policies.as_bytes())
                .as_str()
                .into(),
            entities_root: ContentDigest::hash(
                to_canonical_bytes(&authorization_input.entities).unwrap(),
            )
            .as_str()
            .into(),
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
        let migration_intent = canonical_root(&migration);
        let keyset_root = keyset.root().unwrap();
        let policy_bundle_root = policy_bundle.root().unwrap();
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
            intent_digest: migration_intent.clone(),
            before_event_log_root: legacy_root,
            after_event_log_root: legacy_root_with_bridge,
            event_ids: vec![migration.id.clone()],
            object_delta: vec![
                ObjectDeltaV1 {
                    path: format!(".vela/events/{}.json", migration.id),
                    before_root: None,
                    after_root: Some(migration_intent.clone()),
                    object_kind: "event".into(),
                },
                ObjectDeltaV1 {
                    path: authority_keyset_path(&keyset_root).unwrap(),
                    before_root: None,
                    after_root: Some(keyset_root.clone()),
                    object_kind: "authority_keyset".into(),
                },
                ObjectDeltaV1 {
                    path: authority_policy_path(&policy_bundle_root).unwrap(),
                    before_root: None,
                    after_root: Some(policy_bundle_root.clone()),
                    object_kind: "policy_bundle".into(),
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
                intent_digest: migration_intent,
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
                        .sign(&dsse_pae(AUTHORITY_PAYLOAD_TYPE_V1, &first_payload))
                        .to_bytes(),
                ),
            }],
        };
        fs::create_dir_all(root_path.join(".vela/authority/records")).unwrap();
        fs::write(
            root_path.join(authority_record_path(&first_record.record_id)),
            to_canonical_bytes(&first_envelope).unwrap(),
        )
        .unwrap();
        let keyset_path = authority_keyset_path(&keyset_root).unwrap();
        let policy_path = authority_policy_path(&policy_bundle_root).unwrap();
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
            to_canonical_bytes(&policy_bundle).unwrap(),
        )
        .unwrap();

        let fixture_input = InputBinding::existing_file(
            root_path,
            RepoPath::parse(".vela/input.json".to_string()).unwrap(),
        )
        .unwrap();
        let request = AuthorityTransactionRequest {
            history: AuthorityHistorySnapshot {
                frontier_id: FRONTIER_ID.into(),
                legacy_events,
                legacy_actor_registry_bytes: actor_registry_bytes.clone(),
                legacy_active_policy_head_root: root('3'),
                legacy_policy_store_manifest_root: root('4'),
                authority_keyset: keyset.clone(),
                policy_bundle: policy_bundle.clone(),
                retained_authority_keysets: vec![keyset],
                retained_policy_bundles: vec![policy_bundle],
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
            object_drafts: Vec::new(),
            derived_drafts: Vec::new(),
            next_authority_keyset: None,
            next_policy_bundle: None,
            next_policy_material: None,
            read_set: vec![fixture_input],
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
            authority_keysets: &fixture.request.history.retained_authority_keysets,
            policy_bundles: &fixture.request.history.retained_policy_bundles,
            authority_events: events,
            authority_envelopes: &envelopes,
        })
        .unwrap();
        assert_eq!(result.authority_event_count, events.len());
        assert_eq!(result.authority_record_count, 2);
    }

    fn verified_fixture_history(fixture: &Fixture) -> AuthorityHistoryVerification {
        verify_authority_history(AuthorityHistoryInput {
            frontier_id: FRONTIER_ID,
            legacy_events: &fixture.request.history.legacy_events,
            legacy_actor_registry_bytes: &fixture.actor_registry_bytes,
            legacy_active_policy_head_root: &fixture.request.history.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &fixture
                .request
                .history
                .legacy_policy_store_manifest_root,
            authority_keysets: &fixture.request.history.retained_authority_keysets,
            policy_bundles: &fixture.request.history.retained_policy_bundles,
            authority_events: &fixture.request.history.authority_events,
            authority_envelopes: &fixture.request.history.authority_envelopes,
        })
        .unwrap()
    }

    fn authority_transaction_postimages_absent(fixture: &Fixture) -> bool {
        let events_absent = !fixture
            .temporary
            .path()
            .join(".vela/authority/events")
            .exists();
        let record_count = fs::read_dir(fixture.temporary.path().join(".vela/authority/records"))
            .unwrap()
            .count();
        let keyset_path =
            authority_keyset_path(&fixture.request.history.authority_keyset.root().unwrap())
                .unwrap();
        let policy_path =
            authority_policy_path(&fixture.request.history.policy_bundle.root().unwrap()).unwrap();
        let keyset_unchanged =
            fs::read(fixture.temporary.path().join(keyset_path)).is_ok_and(|bytes| {
                bytes == to_canonical_bytes(&fixture.request.history.authority_keyset).unwrap()
            });
        let policy_unchanged =
            fs::read(fixture.temporary.path().join(policy_path)).is_ok_and(|bytes| {
                bytes == to_canonical_bytes(&fixture.request.history.policy_bundle).unwrap()
            });
        events_absent && record_count == 1 && keyset_unchanged && policy_unchanged
    }

    fn prepared_journal_absent(fixture: &Fixture) -> bool {
        !fixture.journal_dir().join("frontier").exists()
    }

    #[test]
    fn disposable_writer_installs_one_exact_transaction_and_replays_offline() {
        let fixture = self::fixture();
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
        let keyset_path =
            authority_keyset_path(&fixture.request.history.authority_keyset.root().unwrap())
                .unwrap();
        let policy_path =
            authority_policy_path(&fixture.request.history.policy_bundle.root().unwrap()).unwrap();
        assert_eq!(
            fs::read(fixture.temporary.path().join(keyset_path)).unwrap(),
            to_canonical_bytes(&fixture.request.history.authority_keyset).unwrap()
        );
        assert_eq!(
            fs::read(fixture.temporary.path().join(policy_path)).unwrap(),
            to_canonical_bytes(&fixture.request.history.policy_bundle).unwrap()
        );
        let material = CedarPolicyMaterial::from_evaluation(&fixture.request.authorization_input);
        for (path, _, expected) in
            authority_policy_material_snapshots(&fixture.request.history.policy_bundle, &material)
                .unwrap()
        {
            assert_eq!(
                fs::read(fixture.temporary.path().join(path)).unwrap(),
                expected
            );
        }

        verify_installed_history(&fixture, &[event], &envelope);
    }

    fn acceptance_request(fixture: &Fixture) -> (AuthorityTransactionRequest, StateEvent, Vec<u8>) {
        let proposal_id = "vpr_0123456789abcdef";
        let reason = "Accept the exact bounded scientific transition.";
        let mut semantic_domain = StateEvent {
            schema: EVENT_SCHEMA.into(),
            id: String::new(),
            kind: EventKind::FindingNoted,
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
        request.authorization_input.action = "review_accept".into();
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
                    "proposal_kind": "finding.note",
                    "verdict": "accepted",
                    "applied_event_id": semantic_domain.id.clone(),
                }),
                caveats: Vec::new(),
            },
        ];
        request.object_drafts = vec![AuthorityObjectDraft {
            path: format!(".vela/proposals/{proposal_id}.json"),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(proposal_postimage.clone()),
        }];
        (request, semantic_domain, proposal_postimage)
    }

    #[test]
    fn acceptance_transaction_covers_domain_review_and_proposal_postimage() {
        let fixture = self::fixture();
        let proposal_id = "vpr_0123456789abcdef";
        let (request, semantic_domain, proposal_postimage) = acceptance_request(&fixture);
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = execute_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
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
            .find(|event| event.content.kind == EventKind::FindingNoted)
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
                    .join(format!(".vela/proposals/{proposal_id}.json"))
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
            let error = prepare_authority_transaction(
                fixture.barrier(),
                fixture.temporary.path(),
                request,
                &mut adapter,
                &mut signer,
            )
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
    fn dual_log_rejects_duplicate_semantic_event_identity() {
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
                .contains("occurs more than once across the dual log"),
            "{error}"
        );
    }

    #[test]
    fn retained_policy_material_cannot_disappear_or_change_after_backfill() {
        for mutation in ["delete", "alter"] {
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
            let material = CedarPolicyMaterial::from_evaluation(&request.authorization_input);
            let material_path =
                authority_policy_material_snapshots(&request.history.policy_bundle, &material)
                    .unwrap()
                    .remove(0)
                    .0;
            let absolute = fixture.temporary.path().join(&material_path);
            if mutation == "delete" {
                fs::remove_file(&absolute).unwrap();
            } else {
                fs::write(&absolute, b"tampered Cedar schema").unwrap();
            }

            let mut adapter = fixture.adapter();
            let mut signer = fixture.signer();
            let error = prepare_authority_transaction(
                fixture.barrier(),
                fixture.temporary.path(),
                request,
                &mut adapter,
                &mut signer,
            )
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
                        AuthorityTransactionError::Transaction(FrontierTxnError::StaleInput { .. })
                    ),
                    "{error:?}"
                );
            }
            assert_eq!(signer.calls, 0);
        }
    }

    #[test]
    fn keyset_rotation_writer_installs_one_exact_snapshot_and_replays_offline() {
        let fixture = fixture();
        let current = verified_fixture_history(&fixture);
        let next_key = SigningKey::from_bytes(&[13; 32]);
        let next_keyset = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            generation: 2,
            threshold: 1,
            keys: vec![AuthorityKeyV1 {
                key_id: "repository-key-2".into(),
                algorithm: AUTHORITY_KEY_ALGORITHM.into(),
                public_key: hex::encode(next_key.verifying_key().to_bytes()),
                valid_from_sequence: 3,
                valid_through_sequence: None,
                purpose: AUTHORITY_KEY_PURPOSE.into(),
            }],
            previous_keyset_root: Some(fixture.request.history.authority_keyset.root().unwrap()),
            activation_record_root: current.final_authority_record_root.clone(),
            closed: false,
        };
        let intent = root('a');
        let reason = "Rotate the disposable repository authority.";
        let mut request = fixture.request.clone();
        request.intent_digest = intent.clone();
        request.authorization_input.action = AUTHORITY_ROTATE_ACTION.into();
        request.authorization_input.resource = format!(r#"Frontier::"{FRONTIER_ID}""#);
        request.semantic_approvals = vec![SemanticApprovalV1 {
            principal_id: REPOSITORY_PRINCIPAL.into(),
            role: "frontier_administrator".into(),
            action: AUTHORITY_ROTATE_ACTION.into(),
            reason: reason.into(),
            approved_at: RECORDED_AT.into(),
            intent_digest: intent,
        }];
        request.event_drafts = vec![AuthorityEventDraft {
            kind: EventKind::Other("authority.rotated".into()),
            target: StateTarget {
                r#type: "frontier".into(),
                id: FRONTIER_ID.into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: REPOSITORY_PRINCIPAL.into(),
            },
            timestamp: RECORDED_AT.into(),
            reason: reason.into(),
            before_hash: root('f'),
            after_hash: root('f'),
            payload: json!({"authority_keyset_root": next_keyset.root().unwrap()}),
            caveats: Vec::new(),
        }];
        request.next_authority_keyset = Some(next_keyset.clone());

        let mut missing_approval = request.clone();
        missing_approval.semantic_approvals.clear();
        let mut followup_request = request.clone();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            missing_approval,
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("lacks authority_rotate"),
            "{error}"
        );
        assert_eq!(signer.calls, 0);
        assert!(prepared_journal_absent(&fixture));

        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = execute_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        assert_eq!(signer.calls, 1);

        let next_path = authority_keyset_path(&next_keyset.root().unwrap()).unwrap();
        assert_eq!(
            fs::read(fixture.temporary.path().join(next_path)).unwrap(),
            to_canonical_bytes(&next_keyset).unwrap()
        );
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
        let mut keysets = fixture.request.history.retained_authority_keysets.clone();
        keysets.push(next_keyset.clone());
        let mut events = fixture.request.history.authority_events.clone();
        events.push(event);
        let mut envelopes = fixture.request.history.authority_envelopes.clone();
        envelopes.push(envelope);
        let replay = verify_authority_history(AuthorityHistoryInput {
            frontier_id: FRONTIER_ID,
            legacy_events: &fixture.request.history.legacy_events,
            legacy_actor_registry_bytes: &fixture.actor_registry_bytes,
            legacy_active_policy_head_root: &fixture.request.history.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &fixture
                .request
                .history
                .legacy_policy_store_manifest_root,
            authority_keysets: &keysets,
            policy_bundles: &fixture.request.history.retained_policy_bundles,
            authority_events: &events,
            authority_envelopes: &envelopes,
        })
        .unwrap();
        assert_eq!(
            replay.final_authority_keyset_root,
            Some(next_keyset.root().unwrap())
        );
        assert_eq!(
            replay.final_policy_bundle_root,
            Some(fixture.request.history.policy_bundle.root().unwrap())
        );

        followup_request.history.authority_keyset = next_keyset.clone();
        followup_request
            .history
            .retained_authority_keysets
            .push(next_keyset.clone());
        followup_request.history.authority_events = events.clone();
        followup_request.history.authority_envelopes = envelopes.clone();
        followup_request.intent_digest = root('c');
        followup_request.authentication_request.transaction_at = "2026-07-24T12:06:00Z".into();
        followup_request.authorization_input = fixture_authorization_input();
        followup_request.semantic_approvals = vec![SemanticApprovalV1 {
            principal_id: REPOSITORY_PRINCIPAL.into(),
            role: "frontier_administrator".into(),
            action: "review_reject".into(),
            reason: "Reject a later proposal under the rotated repository key.".into(),
            approved_at: "2026-07-24T12:06:00Z".into(),
            intent_digest: root('c'),
        }];
        followup_request.event_drafts = vec![AuthorityEventDraft {
            kind: EventKind::ReviewRejected,
            target: StateTarget {
                r#type: "proposal".into(),
                id: "vpr_1123456789abcdef".into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: REPOSITORY_PRINCIPAL.into(),
            },
            timestamp: "2026-07-24T12:06:00Z".into(),
            reason: "Reject a later proposal under the rotated repository key.".into(),
            before_hash: root('f'),
            after_hash: root('f'),
            payload: json!({"proposal_id": "vpr_1123456789abcdef"}),
            caveats: Vec::new(),
        }];
        followup_request.next_authority_keyset = None;
        followup_request.recorded_at = "2026-07-24T12:06:00Z".into();

        let mut adapter = fixture.adapter();
        let mut next_signer = TestSigner {
            key: next_key,
            key_id: "repository-key-2".into(),
            calls: 0,
            fail: false,
        };
        let followup = execute_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            followup_request,
            &mut adapter,
            &mut next_signer,
        )
        .unwrap();
        assert_eq!(next_signer.calls, 1);

        let followup_event: AuthorityEventV1 = serde_json::from_slice(
            &fs::read(
                fixture
                    .temporary
                    .path()
                    .join(authority_event_path(&followup.event_ids[0])),
            )
            .unwrap(),
        )
        .unwrap();
        let followup_envelope: AuthorityEnvelopeV1 = serde_json::from_slice(
            &fs::read(
                fixture
                    .temporary
                    .path()
                    .join(authority_record_path(&followup.authority_record_id)),
            )
            .unwrap(),
        )
        .unwrap();
        let mut final_events = events;
        final_events.push(followup_event);
        let mut final_envelopes = envelopes;
        final_envelopes.push(followup_envelope);
        let final_replay = verify_authority_history(AuthorityHistoryInput {
            frontier_id: FRONTIER_ID,
            legacy_events: &fixture.request.history.legacy_events,
            legacy_actor_registry_bytes: &fixture.actor_registry_bytes,
            legacy_active_policy_head_root: &fixture.request.history.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &fixture
                .request
                .history
                .legacy_policy_store_manifest_root,
            authority_keysets: &keysets,
            policy_bundles: &fixture.request.history.retained_policy_bundles,
            authority_events: &final_events,
            authority_envelopes: &final_envelopes,
        })
        .unwrap();
        assert_eq!(final_replay.authority_record_count, 3);
        assert_eq!(final_replay.authority_event_count, 2);
        assert_eq!(
            final_replay.final_authority_keyset_root,
            Some(next_keyset.root().unwrap())
        );
    }

    #[test]
    fn policy_rotation_writer_installs_one_exact_snapshot_and_replays_offline() {
        let fixture = fixture();
        let next_policy = PolicyBundleV1 {
            tests_root: root('e'),
            previous_bundle_root: Some(fixture.request.history.policy_bundle.root().unwrap()),
            authority_summary: "Rotated disposable repository policy.".into(),
            ..fixture.request.history.policy_bundle.clone()
        };
        let intent = root('b');
        let reason = "Rotate the disposable repository policy.";
        let mut request = fixture.request.clone();
        request.intent_digest = intent.clone();
        request.authorization_input.action = POLICY_ROTATE_ACTION.into();
        request.authorization_input.resource = format!(r#"Frontier::"{FRONTIER_ID}""#);
        request.semantic_approvals = vec![SemanticApprovalV1 {
            principal_id: REPOSITORY_PRINCIPAL.into(),
            role: "frontier_administrator".into(),
            action: POLICY_ROTATE_ACTION.into(),
            reason: reason.into(),
            approved_at: RECORDED_AT.into(),
            intent_digest: intent,
        }];
        request.event_drafts = vec![AuthorityEventDraft {
            kind: EventKind::Other("policy.rotated".into()),
            target: StateTarget {
                r#type: "frontier".into(),
                id: FRONTIER_ID.into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: REPOSITORY_PRINCIPAL.into(),
            },
            timestamp: RECORDED_AT.into(),
            reason: reason.into(),
            before_hash: root('f'),
            after_hash: root('f'),
            payload: json!({"policy_bundle_root": next_policy.root().unwrap()}),
            caveats: Vec::new(),
        }];
        request.next_policy_bundle = Some(next_policy.clone());
        request.next_policy_material = Some(CedarPolicyMaterial::from_evaluation(
            &request.authorization_input,
        ));

        let mut both = request.clone();
        let current = verified_fixture_history(&fixture);
        let next_key = SigningKey::from_bytes(&[14; 32]);
        both.next_authority_keyset = Some(AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            generation: 2,
            threshold: 1,
            keys: vec![AuthorityKeyV1 {
                key_id: "repository-key-2".into(),
                algorithm: AUTHORITY_KEY_ALGORITHM.into(),
                public_key: hex::encode(next_key.verifying_key().to_bytes()),
                valid_from_sequence: 3,
                valid_through_sequence: None,
                purpose: AUTHORITY_KEY_PURPOSE.into(),
            }],
            previous_keyset_root: Some(fixture.request.history.authority_keyset.root().unwrap()),
            activation_record_root: current.final_authority_record_root,
            closed: false,
        });
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            both,
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("either the keyset or the policy")
        );
        assert_eq!(signer.calls, 0);

        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = execute_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        assert_eq!(signer.calls, 1);

        let next_path = authority_policy_path(&next_policy.root().unwrap()).unwrap();
        assert_eq!(
            fs::read(fixture.temporary.path().join(next_path)).unwrap(),
            to_canonical_bytes(&next_policy).unwrap()
        );
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
        let mut policies = fixture.request.history.retained_policy_bundles.clone();
        policies.push(next_policy.clone());
        let mut events = fixture.request.history.authority_events.clone();
        events.push(event);
        let mut envelopes = fixture.request.history.authority_envelopes.clone();
        envelopes.push(envelope);
        let replay = verify_authority_history(AuthorityHistoryInput {
            frontier_id: FRONTIER_ID,
            legacy_events: &fixture.request.history.legacy_events,
            legacy_actor_registry_bytes: &fixture.actor_registry_bytes,
            legacy_active_policy_head_root: &fixture.request.history.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &fixture
                .request
                .history
                .legacy_policy_store_manifest_root,
            authority_keysets: &fixture.request.history.retained_authority_keysets,
            policy_bundles: &policies,
            authority_events: &events,
            authority_envelopes: &envelopes,
        })
        .unwrap();
        assert_eq!(
            replay.final_authority_keyset_root,
            Some(fixture.request.history.authority_keyset.root().unwrap())
        );
        assert_eq!(
            replay.final_policy_bundle_root,
            Some(next_policy.root().unwrap())
        );
    }

    #[test]
    fn emergency_close_installs_one_terminal_keyset_and_refuses_continuation() {
        let fixture = fixture();
        let current = verified_fixture_history(&fixture);
        let current_record_root = current.final_authority_record_root.clone().unwrap();
        let current_keyset_root = fixture.request.history.authority_keyset.root().unwrap();
        let current_policy_root = fixture.request.history.policy_bundle.root().unwrap();
        let closed_keyset = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            generation: 2,
            threshold: 0,
            keys: Vec::new(),
            previous_keyset_root: Some(current_keyset_root.clone()),
            activation_record_root: Some(current_record_root.clone()),
            closed: true,
        };
        let reason = "Close future authority after the disposable compromise drill.";
        let payload = AuthorityCloseV1 {
            schema: vela_protocol::authority_history::AUTHORITY_CLOSE_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            last_trusted_sequence: 1,
            last_trusted_authority_record_root: current_record_root,
            previous_authority_keyset_root: current_keyset_root,
            closed_authority_keyset_root: closed_keyset.root().unwrap(),
            policy_bundle_root: current_policy_root.clone(),
            incident_id: "incident:disposable-close-1".into(),
            reason: reason.into(),
        };
        let intent = root('d');
        let mut request = fixture.request.clone();
        request.intent_digest = intent.clone();
        request.authorization_input.action = AUTHORITY_CLOSE_ACTION.into();
        request.authorization_input.resource = format!(r#"Frontier::"{FRONTIER_ID}""#);
        request.semantic_approvals = vec![SemanticApprovalV1 {
            principal_id: REPOSITORY_PRINCIPAL.into(),
            role: "frontier_administrator".into(),
            action: AUTHORITY_CLOSE_ACTION.into(),
            reason: reason.into(),
            approved_at: RECORDED_AT.into(),
            intent_digest: intent,
        }];
        request.event_drafts = vec![AuthorityEventDraft {
            kind: EventKind::Other(AUTHORITY_CLOSED_EVENT_KIND.into()),
            target: StateTarget {
                r#type: "frontier".into(),
                id: FRONTIER_ID.into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: REPOSITORY_PRINCIPAL.into(),
            },
            timestamp: RECORDED_AT.into(),
            reason: reason.into(),
            before_hash: root('0'),
            after_hash: root('0'),
            payload: serde_json::to_value(payload).unwrap(),
            caveats: Vec::new(),
        }];
        request.next_authority_keyset = Some(closed_keyset.clone());

        let mut missing_approval = request.clone();
        missing_approval.semantic_approvals.clear();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            missing_approval,
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("lacks authority_close"),
            "{error}"
        );
        assert_eq!(signer.calls, 0);
        assert!(prepared_journal_absent(&fixture));

        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = execute_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        assert_eq!(signer.calls, 1);

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
        let mut keysets = fixture.request.history.retained_authority_keysets.clone();
        keysets.push(closed_keyset.clone());
        let mut envelopes = fixture.request.history.authority_envelopes.clone();
        envelopes.push(envelope.clone());
        let replay = verify_authority_history(AuthorityHistoryInput {
            frontier_id: FRONTIER_ID,
            legacy_events: &fixture.request.history.legacy_events,
            legacy_actor_registry_bytes: &fixture.actor_registry_bytes,
            legacy_active_policy_head_root: &fixture.request.history.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &fixture
                .request
                .history
                .legacy_policy_store_manifest_root,
            authority_keysets: &keysets,
            policy_bundles: &fixture.request.history.retained_policy_bundles,
            authority_events: std::slice::from_ref(&event),
            authority_envelopes: &envelopes,
        })
        .unwrap();
        assert!(replay.closed);
        assert_eq!(replay.closure_event_id, Some(event.id.clone()));
        assert_eq!(
            replay.final_authority_keyset_root,
            Some(closed_keyset.root().unwrap())
        );
        assert_eq!(replay.final_policy_bundle_root, Some(current_policy_root));

        envelopes.push(envelope);
        let error = verify_authority_history(AuthorityHistoryInput {
            frontier_id: FRONTIER_ID,
            legacy_events: &fixture.request.history.legacy_events,
            legacy_actor_registry_bytes: &fixture.actor_registry_bytes,
            legacy_active_policy_head_root: &fixture.request.history.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &fixture
                .request
                .history
                .legacy_policy_store_manifest_root,
            authority_keysets: &keysets,
            policy_bundles: &fixture.request.history.retained_policy_bundles,
            authority_events: std::slice::from_ref(&event),
            authority_envelopes: &envelopes,
        })
        .unwrap_err();
        assert!(
            error.contains("continues after its terminal close"),
            "{error}"
        );
    }

    #[test]
    fn completed_retry_returns_only_the_exact_durable_result_without_another_signature() {
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

        let barrier = fixture.barrier();
        let retried = retry_completed_authority_transaction(&barrier, &result).unwrap();
        assert_eq!(retried, result);
        assert_eq!(signer.calls, 1);

        let mut substituted = result.clone();
        substituted.transaction_id.push('0');
        let error = retry_completed_authority_transaction(&barrier, &substituted).unwrap_err();
        assert!(matches!(error, AuthorityTransactionError::Invalid(_)));
        assert_eq!(signer.calls, 1);
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
        let error = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
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
        let error = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            fixture.request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                AuthorityTransactionError::Transaction(FrontierTxnError::StaleInput { .. })
            ),
            "{error:?}"
        );
        assert_eq!(signer.calls, 0);

        let fixture = self::fixture();
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
                AuthorityTransactionError::Transaction(FrontierTxnError::StaleSnapshot { .. })
            ),
            "{error:?}"
        );
        assert_eq!(signer.calls, 1);
        assert!(
            !fixture
                .temporary
                .path()
                .join(".vela/authority/events")
                .exists()
        );
        assert_eq!(
            fs::read_dir(fixture.temporary.path().join(".vela/authority/records"))
                .unwrap()
                .count(),
            1
        );
        let policy_path =
            authority_policy_path(&fixture.request.history.policy_bundle.root().unwrap()).unwrap();
        assert_eq!(
            fs::read(fixture.temporary.path().join(policy_path)).unwrap(),
            to_canonical_bytes(&fixture.request.history.policy_bundle).unwrap()
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
        assert!(authority_transaction_postimages_absent(&fixture_one));
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
        assert!(matches!(error, AuthorityTransactionError::Invalid(_)));
        assert_eq!(signer.calls, 0);
        assert!(authority_transaction_postimages_absent(&fixture_two));
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
        assert!(authority_transaction_postimages_absent(&fixture));
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
        assert!(authority_transaction_postimages_absent(&fixture));
        assert!(!prepared.transaction.plan().operation_id.as_str().is_empty());
        assert!(matches!(
            prepared.transaction.recovery_state(),
            crate::frontier_txn::RecoveryState::Aborted
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
        let mut prepared = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        fs::write(&path, to_canonical_bytes(&json!({"version": 3})).unwrap()).unwrap();
        let error = prepared.mark_committed().unwrap_err();
        assert!(matches!(
            error,
            AuthorityTransactionError::Transaction(FrontierTxnError::StaleInput { .. })
        ));
        assert_eq!(signer.calls, 1);
        assert!(authority_transaction_postimages_absent(&fixture));
    }

    #[test]
    fn derived_postimages_share_recovery_without_entering_authority_delta() {
        assert!(authority_derived_path("frontier.json"));
        assert!(authority_derived_path("vela.lock"));
        assert!(authority_derived_path(".vela/proof-state.json"));
        assert!(authority_derived_path("proof/latest.json"));
        assert!(
            !authority_derived_path("frontier.yaml"),
            "repository configuration is not a derived materialized view"
        );
        let fixture = fixture();
        fs::write(fixture.temporary.path().join("frontier.json"), b"before\n").unwrap();
        let mut request = fixture.request.clone();
        request.derived_drafts = vec![AuthorityDerivedDraft {
            path: "frontier.json".into(),
            postimage: Some(b"after\n".to_vec()),
        }];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        let payload = BASE64_STANDARD.decode(&prepared.envelope.payload).unwrap();
        let record: AuthorityRecordV1 = serde_json::from_slice(&payload).unwrap();
        assert!(
            record
                .content
                .object_delta
                .iter()
                .all(|object| object.path != "frontier.json"),
            "derived materialization must not become a signed scientific object"
        );
        assert!(
            prepared
                .transaction
                .plan()
                .canonical_delta
                .writes()
                .iter()
                .any(|write| {
                    write.path.as_str() == "frontier.json" && write.class == WriteClass::Derived
                }),
            "the recoverable journal must retain the exact derived postimage"
        );
        prepared.mark_committed().unwrap();
        prepared.install().unwrap();
        prepared.complete().unwrap();
        assert_eq!(
            fs::read(fixture.temporary.path().join("frontier.json")).unwrap(),
            b"after\n"
        );
    }

    #[test]
    fn derived_drift_and_derived_only_authority_attempts_fail_closed() {
        let drift_fixture = fixture();
        fs::write(
            drift_fixture.temporary.path().join("vela.lock"),
            b"before\n",
        )
        .unwrap();
        let mut request = drift_fixture.request.clone();
        request.derived_drafts = vec![AuthorityDerivedDraft {
            path: "vela.lock".into(),
            postimage: Some(b"after\n".to_vec()),
        }];
        let mut adapter = drift_fixture.adapter();
        let mut signer = drift_fixture.signer();
        let mut prepared = prepare_authority_transaction(
            drift_fixture.barrier(),
            drift_fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        fs::write(
            drift_fixture.temporary.path().join("vela.lock"),
            b"changed after signing\n",
        )
        .unwrap();
        let error = prepared.mark_committed().unwrap_err();
        assert!(matches!(
            error,
            AuthorityTransactionError::Transaction(FrontierTxnError::StaleInput { .. })
        ));
        assert_eq!(signer.calls, 1);

        let derived_only_fixture = fixture();
        let mut request = derived_only_fixture.request.clone();
        request.event_drafts.clear();
        request.object_drafts.clear();
        request.derived_drafts = vec![AuthorityDerivedDraft {
            path: "frontier.json".into(),
            postimage: Some(b"derived only\n".to_vec()),
        }];
        let mut adapter = derived_only_fixture.adapter();
        let mut signer = derived_only_fixture.signer();
        let error = prepare_authority_transaction(
            derived_only_fixture.barrier(),
            derived_only_fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(
            matches!(error, AuthorityTransactionError::Invalid(message) if message.contains("changes no event, object, keyset, or policy bundle"))
        );
        assert_eq!(signer.calls, 0);

        let invalid_path_fixture = fixture();
        let mut request = invalid_path_fixture.request.clone();
        request.derived_drafts = vec![AuthorityDerivedDraft {
            path: ".vela/proposals/forbidden.json".into(),
            postimage: Some(b"{}\n".to_vec()),
        }];
        let mut adapter = invalid_path_fixture.adapter();
        let mut signer = invalid_path_fixture.signer();
        let error = prepare_authority_transaction(
            invalid_path_fixture.barrier(),
            invalid_path_fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(
            matches!(error, AuthorityTransactionError::Invalid(message) if message.contains("not a Vela-owned materialized view"))
        );
        assert_eq!(signer.calls, 0);
    }

    #[test]
    fn repository_history_membership_refuses_stale_forks_and_marker_time_additions() {
        let fixture = fixture();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        execute_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            fixture.request.clone(),
            &mut adapter,
            &mut signer,
        )
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
            AuthorityTransactionError::Transaction(FrontierTxnError::CorruptPlan(message))
                if message.contains("membership differs")
        ));
        assert_eq!(stale_signer.calls, 0);

        let fresh = self::fixture();
        let mut adapter = fresh.adapter();
        let mut signer = fresh.signer();
        let mut prepared = prepare_authority_transaction(
            fresh.barrier(),
            fresh.temporary.path(),
            fresh.request.clone(),
            &mut adapter,
            &mut signer,
        )
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
            AuthorityTransactionError::Transaction(FrontierTxnError::StaleSnapshot { .. })
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
        let legacy_path = fixture.temporary.path().join(format!(
            ".vela/events/{}.json",
            fixture.request.history.legacy_events[0].id
        ));
        fs::write(legacy_path, b"tampered").unwrap();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            fixture.request.clone(),
            &mut adapter,
            &mut signer,
        )
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
        let error = prepare_authority_transaction(
            missing.barrier(),
            missing.temporary.path(),
            missing.request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(
            matches!(error, AuthorityTransactionError::History(_)),
            "{error}"
        );
        assert_eq!(signer.calls, 0);
        assert!(prepared_journal_absent(&missing));
    }

    #[test]
    fn immutable_legacy_event_formatting_is_bound_without_rewriting_history() {
        let fixture = fixture();
        let event = &fixture.request.history.legacy_events[0];
        let path = fixture
            .temporary
            .path()
            .join(format!(".vela/events/{}.json", event.id));
        let mut historical_bytes = serde_json::to_vec_pretty(event).unwrap();
        historical_bytes.push(b'\n');
        fs::write(&path, &historical_bytes).unwrap();

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
        assert_eq!(fs::read(path).unwrap(), historical_bytes);
        assert_eq!(result.event_ids.len(), 1);
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
                path: ".vela/proposals/vpr_fixture.json".into(),
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
        let mut prepared = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        let payload = BASE64_STANDARD.decode(&prepared.envelope.payload).unwrap();
        let record: AuthorityRecordV1 = serde_json::from_slice(&payload).unwrap();
        assert_eq!(record.content.object_delta.len(), 8);
        assert!(!record.content.object_delta.iter().any(|delta| {
            delta.object_kind == "authority_keyset" || delta.object_kind == "policy_bundle"
        }));
        assert_eq!(
            record
                .content
                .object_delta
                .iter()
                .filter(|delta| {
                    matches!(
                        delta.object_kind.as_str(),
                        "cedar_schema" | "cedar_policies" | "cedar_entities"
                    )
                })
                .count(),
            3
        );
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
                    .join(".vela/proposals/vpr_fixture.json")
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
        request.object_drafts = vec![AuthorityObjectDraft {
            path: ".vela/proposals/vpr_pending.json".into(),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(to_canonical_bytes(&json!({"standing": "pending"})).unwrap()),
        }];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = execute_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();

        assert!(result.event_ids.is_empty());
        assert_eq!(result.before_event_log_root, result.after_event_log_root);
        assert_eq!(
            fs::read(
                fixture
                    .temporary
                    .path()
                    .join(".vela/proposals/vpr_pending.json")
            )
            .unwrap(),
            to_canonical_bytes(&json!({"standing": "pending"})).unwrap()
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
            frontier_id: FRONTIER_ID,
            legacy_events: &fixture.request.history.legacy_events,
            legacy_actor_registry_bytes: &fixture.actor_registry_bytes,
            legacy_active_policy_head_root: &fixture.request.history.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &fixture
                .request
                .history
                .legacy_policy_store_manifest_root,
            authority_keysets: &fixture.request.history.retained_authority_keysets,
            policy_bundles: &fixture.request.history.retained_policy_bundles,
            authority_events: &[],
            authority_envelopes: &envelopes,
        })
        .unwrap();
        assert_eq!(verification.authority_event_count, 0);
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
        let error = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(matches!(error, AuthorityTransactionError::Invalid(_)));
        assert_eq!(signer.calls, 0);
        assert!(prepared_journal_absent(&fixture));

        let fixture = self::fixture();
        let mut request = fixture.request.clone();
        request.object_drafts = vec![AuthorityObjectDraft {
            path: ".vela/proposals/bad.json".into(),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(b"{ \"not\": \"canonical\" }".to_vec()),
        }];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let error = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
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
        let error = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
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
        let mut request = fixture.request.clone();
        let proposal_bytes = to_canonical_bytes(&json!({"proposal": "recoverable"})).unwrap();
        request.object_drafts = vec![AuthorityObjectDraft {
            path: ".vela/proposals/vpr_recovery.json".into(),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(proposal_bytes.clone()),
        }];
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
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
        assert_eq!(
            fs::read(
                fixture
                    .temporary
                    .path()
                    .join(".vela/proposals/vpr_recovery.json")
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
            path: ".vela/proposals/vpr_object_recovery.json".into(),
            object_kind: "proposal".into(),
            class: WriteClass::PublicReview,
            postimage: Some(proposal_bytes.clone()),
        }];
        let before_event_root = verified_fixture_history(&fixture).final_event_log_root;
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let mut prepared = prepare_authority_transaction(
            fixture.barrier(),
            fixture.temporary.path(),
            request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        assert!(prepared.result.event_ids.is_empty());
        assert_eq!(prepared.result.before_event_log_root, before_event_root);
        assert_eq!(prepared.result.after_event_log_root, before_event_root);
        let result = prepared.result.clone();
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
        assert_eq!(
            fs::read(
                fixture
                    .temporary
                    .path()
                    .join(".vela/proposals/vpr_object_recovery.json")
            )
            .unwrap(),
            proposal_bytes
        );
        verify_installed_history(&fixture, &[], &envelope);
    }
}
