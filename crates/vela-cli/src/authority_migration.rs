//! Sequence-1 repository-authority migration writer.
//!
//! The writer accepts one already legacy-signed continuity event, verifies it
//! against the exact held Era-0 history, authenticates and authorizes the new
//! repository administrator, signs one covering authority record, and installs
//! the bridge plus initial keyset and policy manifests through the existing
//! recoverable frontier transaction. It has no CLI route and never reads a
//! legacy human key.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use serde_json::Value;
use vela_authority::CedarEvaluationInput;
use vela_authority::runtime_authentication::{
    AuthenticationAdapter, AuthenticationRequest, RuntimeSessionState, preflight_authority_action,
};
use vela_protocol::authority::{
    AUTHORITY_PAYLOAD_TYPE_V1, AuthorityEnvelopeV1, AuthorityKeysetV1, AuthorityRecordContentV1,
    AuthorityRecordV1, AuthorizationClaimV1, ExecutionClaimV1, ObjectDeltaV1, PolicyBundleV1,
    PrincipalSnapshotV1, SemanticApprovalV1, verify_authority_envelope,
};
use vela_protocol::authority_history::{
    AUTHORITY_MIGRATION_ACTION, AuthorityHistoryInput, verify_authority_history,
    verify_authority_migration_bridge,
};
use vela_protocol::canonical::to_canonical_bytes;
use vela_protocol::events::{StateEvent, event_log_hash};

use crate::authority_transaction::{
    AuthorityTransactionError, AuthorityTransactionResult, RepositoryAuthoritySigner,
    authority_keyset_path, authority_policy_path, authority_record_path,
};
use crate::frontier_txn::{
    CanonicalWriteBarrier, ContentDigest, DeltaDraft, FileState, FrontierBinding, FrontierTxn,
    FrontierTxnError, FrontierTxnPlan, FrontierTxnPlanSpec, InputBinding, OperationId,
    OperationKind, PlannedWrite, RepoPath, WriteClass,
};

const MIGRATION_TRANSACTION_SCHEMA: &str = "vela.authority-migration-transaction.internal.v1";
const MIGRATION_READ_SET_SCHEMA: &str = "vela.authority-migration-read-set.internal.v1";
const MIGRATION_WRITE_SET_SCHEMA: &str = "vela.authority-migration-write-set.internal.v1";
const MIGRATION_LAYOUT_SCHEMA: &str = "vela.authority-migration-layout.internal.v1";
const RESULT_SCHEMA: &str = "vela.authority-transaction-result.internal.v1";
const OPERATION_DOMAIN: &str = "authority_migration";

#[derive(Debug, Clone)]
pub(crate) struct AuthorityMigrationHistorySnapshot {
    pub(crate) frontier_id: String,
    pub(crate) legacy_events: Vec<StateEvent>,
    pub(crate) legacy_actor_registry_bytes: Vec<u8>,
    pub(crate) legacy_active_policy_head_root: String,
    pub(crate) legacy_policy_store_manifest_root: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthorityMigrationRequest {
    pub(crate) history: AuthorityMigrationHistorySnapshot,
    pub(crate) migration_event: StateEvent,
    pub(crate) authority_keyset: AuthorityKeysetV1,
    pub(crate) policy_bundle: PolicyBundleV1,
    pub(crate) intent_digest: String,
    pub(crate) principal: PrincipalSnapshotV1,
    pub(crate) authentication_request: AuthenticationRequest,
    pub(crate) runtime_session_state: RuntimeSessionState,
    pub(crate) authorization_input: CedarEvaluationInput,
    pub(crate) read_set: Vec<InputBinding>,
    pub(crate) vela_version: String,
    pub(crate) binary_sha256: String,
    pub(crate) recorded_at: String,
}

#[derive(Debug)]
pub(crate) struct PreparedAuthorityMigration {
    transaction: FrontierTxn,
    pub(crate) result: AuthorityTransactionResult,
    pub(crate) envelope: AuthorityEnvelopeV1,
}

impl PreparedAuthorityMigration {
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

pub(crate) fn prepare_authority_migration<A, S>(
    barrier: CanonicalWriteBarrier,
    frontier_root: &Path,
    mut request: AuthorityMigrationRequest,
    authentication_adapter: &mut A,
    signer: &mut S,
) -> Result<PreparedAuthorityMigration, AuthorityTransactionError>
where
    A: AuthenticationAdapter,
    S: RepositoryAuthoritySigner,
{
    validate_request_shape(&request)?;
    let migration = verify_authority_migration_bridge(
        &request.history.frontier_id,
        &request.history.legacy_events,
        &request.history.legacy_actor_registry_bytes,
        &request.history.legacy_active_policy_head_root,
        &request.history.legacy_policy_store_manifest_root,
        &request.authority_keyset,
        &request.policy_bundle,
        &request.migration_event,
    )
    .map_err(AuthorityTransactionError::History)?;
    let migration_intent = ContentDigest::hash(
        to_canonical_bytes(&request.migration_event).map_err(AuthorityTransactionError::Invalid)?,
    )
    .as_str()
    .to_string();
    if request.intent_digest != migration_intent {
        return Err(AuthorityTransactionError::Invalid(
            "sequence-1 intent must equal the full canonical migration-event root".into(),
        ));
    }
    bind_pre_migration_repository(frontier_root, &mut request)?;

    let preflight = preflight_authority_action(
        authentication_adapter,
        &request.authentication_request,
        &request.runtime_session_state,
        &request.authorization_input,
    )
    .map_err(AuthorityTransactionError::Authentication)?;
    if request.principal.principal_id != migration.new_principal_id
        || request.principal.principal_id != preflight.authentication.principal_id
        || request.principal.principal_class != preflight.authentication.principal_class
        || !request
            .principal
            .account_links
            .contains(&preflight.authentication.principal_id)
    {
        return Err(AuthorityTransactionError::Invalid(
            "migration principal differs from the signed bridge or verified authentication".into(),
        ));
    }

    normalize_read_set(&mut request.read_set)?;
    let before_event_log_root =
        format!("sha256:{}", event_log_hash(&request.history.legacy_events));
    let mut migrated_events = request.history.legacy_events.clone();
    migrated_events.push(request.migration_event.clone());
    let after_event_log_root = format!("sha256:{}", event_log_hash(&migrated_events));
    let authority_keyset_root = request
        .authority_keyset
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    let policy_bundle_root = request
        .policy_bundle
        .root()
        .map_err(AuthorityTransactionError::Invalid)?;
    let request_root = authorization_request_root(
        &request.authorization_input,
        &preflight.authorization_context,
    )?;
    let entity_snapshot_root = domain_root(
        b"vela.authority-entity-snapshot.internal.v1\0",
        &request.authorization_input.entities,
    )?;
    let approval = SemanticApprovalV1 {
        principal_id: request.migration_event.actor.id.clone(),
        role: "frontier_administrator".into(),
        action: AUTHORITY_MIGRATION_ACTION.into(),
        reason: migration.reason.clone(),
        approved_at: request.migration_event.timestamp.clone(),
        intent_digest: request.intent_digest.clone(),
    };
    let transaction_id = transaction_id(
        &request,
        &before_event_log_root,
        &request_root,
        &entity_snapshot_root,
        &authority_keyset_root,
        &policy_bundle_root,
        &approval,
    )?;
    let operation_id = OperationId::derive(OPERATION_DOMAIN, transaction_id.as_bytes());

    let keyset_path = authority_keyset_path(&authority_keyset_root)?;
    let policy_path = authority_policy_path(&policy_bundle_root)?;
    let migration_path = format!(".vela/events/{}.json", request.migration_event.id);
    let content_writes = vec![
        PlannedWrite::write(
            RepoPath::parse(migration_path.clone())
                .map_err(AuthorityTransactionError::Transaction)?,
            WriteClass::Authority,
            to_canonical_bytes(&request.migration_event)
                .map_err(AuthorityTransactionError::Invalid)?,
        ),
        PlannedWrite::write(
            RepoPath::parse(keyset_path.clone()).map_err(AuthorityTransactionError::Transaction)?,
            WriteClass::Authority,
            to_canonical_bytes(&request.authority_keyset)
                .map_err(AuthorityTransactionError::Invalid)?,
        ),
        PlannedWrite::write(
            RepoPath::parse(policy_path.clone()).map_err(AuthorityTransactionError::Transaction)?,
            WriteClass::Authority,
            to_canonical_bytes(&request.policy_bundle)
                .map_err(AuthorityTransactionError::Invalid)?,
        ),
    ];
    let unsigned_draft = DeltaDraft::prepare(frontier_root, content_writes.clone())
        .map_err(AuthorityTransactionError::Transaction)?;
    let object_delta = exact_initial_object_delta(
        &unsigned_draft,
        &[
            (migration_path.clone(), "event"),
            (keyset_path.clone(), "authority_keyset"),
            (policy_path.clone(), "policy_bundle"),
        ],
    )?;
    let read_set_root = read_set_root(
        &request,
        &before_event_log_root,
        &authority_keyset_root,
        &policy_bundle_root,
    )?;
    let write_set_root = write_set_root(
        &transaction_id,
        &before_event_log_root,
        &after_event_log_root,
        &request.migration_event.id,
        &object_delta,
    )?;

    let record = AuthorityRecordV1::new(AuthorityRecordContentV1 {
        frontier_id: request.history.frontier_id.clone(),
        sequence: 1,
        previous_authority_record_root: None,
        operation_id: operation_id.as_str().into(),
        transaction_id: transaction_id.clone(),
        intent_digest: request.intent_digest.clone(),
        before_event_log_root: before_event_log_root.clone(),
        after_event_log_root: after_event_log_root.clone(),
        event_ids: vec![request.migration_event.id.clone()],
        object_delta,
        principal: request.principal.clone(),
        authentication: preflight.authentication,
        delegation: None,
        authorization: AuthorizationClaimV1 {
            policy_bundle_root,
            request_root,
            entity_snapshot_root,
            evaluation: preflight.authorization,
        },
        semantic_approvals: vec![approval],
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
        &request.authority_keyset,
        &request.history.frontier_id,
        1,
        None,
    )
    .map_err(AuthorityTransactionError::Signing)?;

    verify_authority_history(AuthorityHistoryInput {
        frontier_id: &request.history.frontier_id,
        legacy_events: &migrated_events,
        legacy_actor_registry_bytes: &request.history.legacy_actor_registry_bytes,
        legacy_active_policy_head_root: &request.history.legacy_active_policy_head_root,
        legacy_policy_store_manifest_root: &request.history.legacy_policy_store_manifest_root,
        authority_keysets: std::slice::from_ref(&request.authority_keyset),
        policy_bundles: std::slice::from_ref(&request.policy_bundle),
        authority_events: &[],
        authority_envelopes: std::slice::from_ref(&envelope),
    })
    .map_err(AuthorityTransactionError::History)?;

    let result = AuthorityTransactionResult {
        operation_id: record.content.operation_id.clone(),
        transaction_id: record.content.transaction_id.clone(),
        event_ids: record.content.event_ids.clone(),
        authority_record_id: record.record_id.clone(),
        authority_record_root: verified.record_root,
        before_event_log_root,
        after_event_log_root,
        read_set_root,
        write_set_root,
    };
    let mut writes = content_writes;
    writes.push(PlannedWrite::write(
        RepoPath::parse(authority_record_path(&record.record_id))
            .map_err(AuthorityTransactionError::Transaction)?,
        WriteClass::Authority,
        to_canonical_bytes(&envelope).map_err(AuthorityTransactionError::Invalid)?,
    ));
    let draft = DeltaDraft::prepare(frontier_root, writes)
        .map_err(AuthorityTransactionError::Transaction)?;
    if draft.delta.writes().len() != unsigned_draft.delta.writes().len() + 1 {
        return Err(AuthorityTransactionError::Invalid(
            "sequence-1 covering record must be a new exact postimage".into(),
        ));
    }

    let mut resulting_event_ids = migrated_events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    resulting_event_ids.sort();
    if resulting_event_ids
        .windows(2)
        .any(|pair| pair[0] == pair[1])
    {
        return Err(AuthorityTransactionError::Invalid(
            "migration event set contains duplicate IDs".into(),
        ));
    }
    let layout_identity = to_canonical_bytes(&MigrationLayoutCommitment {
        schema: MIGRATION_LAYOUT_SCHEMA,
        frontier_id: &request.history.frontier_id,
        migration_event_path: &migration_path,
        authority_keyset_path: &keyset_path,
        policy_bundle_path: &policy_path,
        authority_record_path: &authority_record_path(&record.record_id),
    })
    .map_err(AuthorityTransactionError::Invalid)?;
    let plan = FrontierTxnPlan::new(
        FrontierTxnPlanSpec {
            kind: OperationKind::Decision,
            operation_id,
            request_root: ContentDigest::parse(request.intent_digest)
                .map_err(AuthorityTransactionError::Transaction)?,
            frontier: FrontierBinding::new(
                frontier_root,
                &request.history.frontier_id,
                &layout_identity,
            )
            .map_err(AuthorityTransactionError::Transaction)?,
            fixed_time: request.recorded_at,
            expected_event_log_root: ContentDigest::parse(result.before_event_log_root.clone())
                .map_err(AuthorityTransactionError::Transaction)?,
            resulting_event_log_root: ContentDigest::parse(result.after_event_log_root.clone())
                .map_err(AuthorityTransactionError::Transaction)?,
            resulting_event_ids,
            read_set: request.read_set,
            result: serde_json::json!({
                "schema": RESULT_SCHEMA,
                "result": result.clone(),
            }),
        },
        draft.delta.clone(),
    )
    .map_err(AuthorityTransactionError::Transaction)?;
    let transaction = FrontierTxn::prepare_with_barrier(barrier, plan, draft)
        .map_err(AuthorityTransactionError::Transaction)?;
    Ok(PreparedAuthorityMigration {
        transaction,
        result,
        envelope,
    })
}

pub(crate) fn execute_authority_migration<A, S>(
    barrier: CanonicalWriteBarrier,
    frontier_root: &Path,
    request: AuthorityMigrationRequest,
    authentication_adapter: &mut A,
    signer: &mut S,
) -> Result<AuthorityTransactionResult, AuthorityTransactionError>
where
    A: AuthenticationAdapter,
    S: RepositoryAuthoritySigner,
{
    let mut prepared = prepare_authority_migration(
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
    request: &AuthorityMigrationRequest,
) -> Result<(), AuthorityTransactionError> {
    request
        .authority_keyset
        .validate()
        .map_err(AuthorityTransactionError::Invalid)?;
    request
        .policy_bundle
        .validate()
        .map_err(AuthorityTransactionError::Invalid)?;
    if request.authority_keyset.frontier_id != request.history.frontier_id
        || request.policy_bundle.frontier_id != request.history.frontier_id
        || request.authority_keyset.generation != 1
        || request.authority_keyset.previous_keyset_root.is_some()
        || request.authority_keyset.activation_record_root.is_some()
    {
        return Err(AuthorityTransactionError::Invalid(
            "sequence-1 migration requires one initial Frontier keyset and policy bundle".into(),
        ));
    }
    if request.authorization_input.action != AUTHORITY_MIGRATION_ACTION
        || request.authentication_request.transaction_at != request.recorded_at
        || request.intent_digest.trim().is_empty()
        || request.vela_version.trim().is_empty()
        || request.recorded_at.trim().is_empty()
    {
        return Err(AuthorityTransactionError::Invalid(
            "migration action, intent, version, and exact transaction time are required".into(),
        ));
    }
    ContentDigest::parse(request.intent_digest.clone())
        .map_err(AuthorityTransactionError::Transaction)?;
    ContentDigest::parse(request.binary_sha256.clone())
        .map_err(AuthorityTransactionError::Transaction)?;
    let migration_time = DateTime::<FixedOffset>::parse_from_rfc3339(
        &request.migration_event.timestamp,
    )
    .map_err(|error| {
        AuthorityTransactionError::Invalid(format!("migration event timestamp is invalid: {error}"))
    })?;
    let recorded_at =
        DateTime::<FixedOffset>::parse_from_rfc3339(&request.recorded_at).map_err(|error| {
            AuthorityTransactionError::Invalid(format!(
                "migration record timestamp is invalid: {error}"
            ))
        })?;
    if recorded_at < migration_time {
        return Err(AuthorityTransactionError::Invalid(
            "authority record precedes the signed migration approval".into(),
        ));
    }
    validate_policy_input_binding(&request.policy_bundle, &request.authorization_input)
}

fn validate_policy_input_binding(
    bundle: &PolicyBundleV1,
    input: &CedarEvaluationInput,
) -> Result<(), AuthorityTransactionError> {
    let entities =
        to_canonical_bytes(&input.entities).map_err(AuthorityTransactionError::Invalid)?;
    if bundle.cedar_schema_root != ContentDigest::hash(input.schema.as_bytes()).as_str()
        || bundle.policies_root != ContentDigest::hash(input.policies.as_bytes()).as_str()
        || bundle.entities_root != ContentDigest::hash(entities).as_str()
    {
        return Err(AuthorityTransactionError::Invalid(
            "migration Cedar bytes differ from the retained policy bundle".into(),
        ));
    }
    Ok(())
}

fn bind_pre_migration_repository(
    frontier_root: &Path,
    request: &mut AuthorityMigrationRequest,
) -> Result<(), AuthorityTransactionError> {
    let mut event_paths = Vec::new();
    for event in &request.history.legacy_events {
        let path = RepoPath::parse(format!(".vela/events/{}.json", event.id))
            .map_err(AuthorityTransactionError::Transaction)?;
        let absolute = frontier_root.join(path.as_str());
        let bytes = std::fs::read(&absolute).map_err(|error| {
            AuthorityTransactionError::Transaction(FrontierTxnError::Io(format!(
                "read retained legacy event {}: {error}",
                absolute.display()
            )))
        })?;
        let held: StateEvent = serde_json::from_slice(&bytes).map_err(|error| {
            AuthorityTransactionError::Invalid(format!(
                "retained legacy event {} is invalid: {error}",
                path.as_str()
            ))
        })?;
        let held_canonical =
            to_canonical_bytes(&held).map_err(AuthorityTransactionError::Invalid)?;
        let expected_canonical =
            to_canonical_bytes(event).map_err(AuthorityTransactionError::Invalid)?;
        if held_canonical != expected_canonical {
            return Err(AuthorityTransactionError::Invalid(format!(
                "retained legacy event {} differs from the held history",
                event.id
            )));
        }
        merge_input(
            &mut request.read_set,
            InputBinding::exact_file(frontier_root, path.clone(), &bytes)
                .map_err(AuthorityTransactionError::Transaction)?,
        )?;
        event_paths.push(path);
    }
    let migration_path =
        RepoPath::parse(format!(".vela/events/{}.json", request.migration_event.id))
            .map_err(AuthorityTransactionError::Transaction)?;
    merge_input(
        &mut request.read_set,
        InputBinding::absent_file(frontier_root, migration_path)
            .map_err(AuthorityTransactionError::Transaction)?,
    )?;
    merge_input(
        &mut request.read_set,
        InputBinding::exact_file(
            frontier_root,
            RepoPath::parse(".vela/actors.json").map_err(AuthorityTransactionError::Transaction)?,
            &request.history.legacy_actor_registry_bytes,
        )
        .map_err(AuthorityTransactionError::Transaction)?,
    )?;
    merge_input(
        &mut request.read_set,
        InputBinding::exact_directory(
            frontier_root,
            RepoPath::parse(".vela/events").map_err(AuthorityTransactionError::Transaction)?,
            &event_paths,
        )
        .map_err(AuthorityTransactionError::Transaction)?,
    )?;
    for directory in [
        ".vela/authority/events",
        ".vela/authority/records",
        ".vela/authority/keysets",
        ".vela/authority/policies",
    ] {
        merge_input(
            &mut request.read_set,
            InputBinding::exact_directory(
                frontier_root,
                RepoPath::parse(directory).map_err(AuthorityTransactionError::Transaction)?,
                &[],
            )
            .map_err(AuthorityTransactionError::Transaction)?,
        )?;
    }
    Ok(())
}

fn merge_input(
    read_set: &mut Vec<InputBinding>,
    binding: InputBinding,
) -> Result<(), AuthorityTransactionError> {
    if let Some(existing) = read_set.iter().find(|input| input.name == binding.name) {
        if existing.digest == binding.digest {
            return Ok(());
        }
        return Err(AuthorityTransactionError::Invalid(format!(
            "migration input {} conflicts with the held repository",
            binding.name
        )));
    }
    read_set.push(binding);
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
            "migration read-set names must be unique".into(),
        ));
    }
    Ok(())
}

fn exact_initial_object_delta(
    draft: &DeltaDraft,
    kinds: &[(String, &str)],
) -> Result<Vec<ObjectDeltaV1>, AuthorityTransactionError> {
    let kinds = kinds
        .iter()
        .map(|(path, kind)| (path.as_str(), *kind))
        .collect::<BTreeMap<_, _>>();
    if draft.delta.writes().len() != kinds.len() {
        return Err(AuthorityTransactionError::Invalid(
            "sequence-1 migration requires three new exact canonical objects".into(),
        ));
    }
    draft
        .delta
        .writes()
        .iter()
        .map(|write| {
            if !matches!(write.preimage, FileState::Absent) {
                return Err(AuthorityTransactionError::Invalid(format!(
                    "sequence-1 migration refuses existing path {}",
                    write.path.as_str()
                )));
            }
            let after_root = match &write.postimage {
                FileState::File { digest, .. } => Some(digest.as_str().to_string()),
                FileState::Absent => None,
            };
            let object_kind = kinds.get(write.path.as_str()).ok_or_else(|| {
                AuthorityTransactionError::Invalid(format!(
                    "sequence-1 object {} is not covered",
                    write.path.as_str()
                ))
            })?;
            Ok(ObjectDeltaV1 {
                path: write.path.as_str().into(),
                before_root: None,
                after_root,
                object_kind: (*object_kind).into(),
            })
        })
        .collect()
}

fn transaction_id(
    request: &AuthorityMigrationRequest,
    before_event_log_root: &str,
    authorization_request_root: &str,
    entity_snapshot_root: &str,
    authority_keyset_root: &str,
    policy_bundle_root: &str,
    approval: &SemanticApprovalV1,
) -> Result<String, AuthorityTransactionError> {
    let root = domain_root(
        b"vela.authority-migration-transaction.internal.v1\0",
        &MigrationTransactionCommitment {
            schema: MIGRATION_TRANSACTION_SCHEMA,
            frontier_id: &request.history.frontier_id,
            intent_digest: &request.intent_digest,
            before_event_log_root,
            migration_event: &request.migration_event,
            principal: &request.principal,
            authorization_request_root,
            entity_snapshot_root,
            authority_keyset_root,
            policy_bundle_root,
            approval,
            read_set: &request.read_set,
            vela_version: &request.vela_version,
            binary_sha256: &request.binary_sha256,
            recorded_at: &request.recorded_at,
        },
    )?;
    Ok(format!(
        "vtx_{}",
        root.strip_prefix("sha256:")
            .expect("domain root always has a sha256 prefix")
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
            action: &input.action,
            resource: &input.resource,
            context: verified_context,
        },
    )
}

fn read_set_root(
    request: &AuthorityMigrationRequest,
    current_event_log_root: &str,
    authority_keyset_root: &str,
    policy_bundle_root: &str,
) -> Result<String, AuthorityTransactionError> {
    domain_root(
        b"vela.authority-migration-read-set.internal.v1\0",
        &MigrationReadSetCommitment {
            schema: MIGRATION_READ_SET_SCHEMA,
            frontier_id: &request.history.frontier_id,
            current_event_log_root,
            legacy_active_policy_head_root: &request.history.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &request.history.legacy_policy_store_manifest_root,
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
    migration_event_id: &str,
    object_delta: &[ObjectDeltaV1],
) -> Result<String, AuthorityTransactionError> {
    domain_root(
        b"vela.authority-migration-write-set.internal.v1\0",
        &MigrationWriteSetCommitment {
            schema: MIGRATION_WRITE_SET_SCHEMA,
            transaction_id,
            before_event_log_root,
            after_event_log_root,
            migration_event_id,
            object_delta,
        },
    )
}

fn domain_root(domain: &[u8], value: &impl Serialize) -> Result<String, AuthorityTransactionError> {
    let canonical = to_canonical_bytes(value).map_err(AuthorityTransactionError::Invalid)?;
    let mut preimage = Vec::with_capacity(domain.len() + canonical.len());
    preimage.extend_from_slice(domain);
    preimage.extend_from_slice(&canonical);
    Ok(ContentDigest::hash(preimage).as_str().into())
}

#[derive(Serialize)]
struct MigrationTransactionCommitment<'a> {
    schema: &'static str,
    frontier_id: &'a str,
    intent_digest: &'a str,
    before_event_log_root: &'a str,
    migration_event: &'a StateEvent,
    principal: &'a PrincipalSnapshotV1,
    authorization_request_root: &'a str,
    entity_snapshot_root: &'a str,
    authority_keyset_root: &'a str,
    policy_bundle_root: &'a str,
    approval: &'a SemanticApprovalV1,
    read_set: &'a [InputBinding],
    vela_version: &'a str,
    binary_sha256: &'a str,
    recorded_at: &'a str,
}

#[derive(Serialize)]
struct AuthorizationRequestCommitment<'a> {
    schema: &'static str,
    principal: &'a str,
    action: &'a str,
    resource: &'a str,
    context: &'a Value,
}

#[derive(Serialize)]
struct MigrationReadSetCommitment<'a> {
    schema: &'static str,
    frontier_id: &'a str,
    current_event_log_root: &'a str,
    legacy_active_policy_head_root: &'a str,
    legacy_policy_store_manifest_root: &'a str,
    authority_keyset_root: &'a str,
    policy_bundle_root: &'a str,
    inputs: &'a [InputBinding],
}

#[derive(Serialize)]
struct MigrationWriteSetCommitment<'a> {
    schema: &'static str,
    transaction_id: &'a str,
    before_event_log_root: &'a str,
    after_event_log_root: &'a str,
    migration_event_id: &'a str,
    object_delta: &'a [ObjectDeltaV1],
}

#[derive(Serialize)]
struct MigrationLayoutCommitment<'a> {
    schema: &'static str,
    frontier_id: &'a str,
    migration_event_path: &'a str,
    authority_keyset_path: &'a str,
    policy_bundle_path: &'a str,
    authority_record_path: &'a str,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;
    use tempfile::TempDir;
    use vela_authority::runtime_authentication::{AuthenticationFailure, LocalOsSession};
    use vela_protocol::authentication::AuthenticationObservationV1;
    use vela_protocol::authority::{
        AUTHORITY_KEY_ALGORITHM, AUTHORITY_KEY_PURPOSE, AUTHORITY_KEYSET_SCHEMA_V1,
        AuthorityEventV1, AuthorityKeyV1, CEDAR_ENGINE, CEDAR_ENGINE_VERSION, CEDAR_PROFILE_V1,
        DsseSignatureV1, POLICY_BUNDLE_SCHEMA_V1, SemanticApprovalV1, dsse_pae,
    };
    use vela_protocol::authority_history::{
        AUTHORITY_CLOSE_ACTION, AUTHORITY_CLOSE_SCHEMA_V1, AUTHORITY_CLOSED_EVENT_KIND,
        AUTHORITY_MODEL_MIGRATION_SCHEMA_V1, AUTHORITY_ROTATE_ACTION, AuthorityCloseV1,
        AuthorityHistoryEra, AuthorityModelMigrationV1,
    };
    use vela_protocol::events::{
        EVENT_SCHEMA, EventKind, NULL_HASH, StateActor, StateTarget, compute_event_id,
    };
    use vela_protocol::principal_capability::PrincipalClass;
    use vela_protocol::sign::{ActorRecord, sign_event};

    use super::*;
    use crate::authority_transaction::{
        AuthorityEventDraft, AuthorityHistorySnapshot, AuthorityTransactionRequest,
        authority_event_path, execute_authority_transaction, retry_completed_authority_transaction,
    };
    use crate::frontier_txn::{FrontierTxnStep, RecoveryOutcome};

    const FRONTIER_ID: &str = "vfr_0123456789abcdef";
    const LEGACY_ACTOR: &str = "reviewer:legacy";
    const REPOSITORY_PRINCIPAL: &str = "local:device-1|uid:501";
    const RECORDED_AT: &str = "2026-07-24T12:05:00Z";

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn run_git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn authorization_input() -> CedarEvaluationInput {
        CedarEvaluationInput {
            schema: r#"
                entity Human;
                entity Frontier;
                entity Proposal;
                action "authority_model_migrate" appliesTo {
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
                    "uid": {"type": "Frontier", "id": FRONTIER_ID},
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
            action: AUTHORITY_MIGRATION_ACTION.into(),
            resource: format!(r#"Frontier::"{FRONTIER_ID}""#),
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
            Ok(vec![DsseSignatureV1 {
                keyid: self.key_id.clone(),
                sig: BASE64_STANDARD.encode(
                    self.key
                        .sign(&dsse_pae(payload_type, canonical_payload))
                        .to_bytes(),
                ),
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
        request: AuthorityMigrationRequest,
        repository_key: SigningKey,
    }

    impl Fixture {
        fn root(&self) -> &Path {
            self.temporary.path()
        }

        fn journals(&self) -> std::path::PathBuf {
            self.root().join(".vela/operation-journals")
        }

        fn barrier(&self) -> CanonicalWriteBarrier {
            FrontierTxn::acquire_write_barrier_for_test(self.root(), &self.journals()).unwrap()
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

        fn migration_path(&self) -> std::path::PathBuf {
            self.root()
                .join(".vela/events")
                .join(format!("{}.json", self.request.migration_event.id))
        }

        fn journal_count(&self) -> usize {
            fs::read_dir(self.journals().join("frontier"))
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                        .count()
                })
                .unwrap_or(0)
        }
    }

    fn fixture() -> Fixture {
        let temporary = TempDir::new().unwrap();
        let frontier_root = temporary.path();
        for directory in [
            ".vela/events",
            ".vela/authority/events",
            ".vela/authority/records",
            ".vela/authority/keysets",
            ".vela/authority/policies",
        ] {
            fs::create_dir_all(frontier_root.join(directory)).unwrap();
        }

        let legacy_key = SigningKey::from_bytes(&[21; 32]);
        let repository_key = SigningKey::from_bytes(&[22; 32]);
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
        fs::write(
            frontier_root.join(".vela/actors.json"),
            &actor_registry_bytes,
        )
        .unwrap();
        fs::write(
            frontier_root.join(".vela/input.json"),
            b"{\"fixture\":true}\n",
        )
        .unwrap();

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
            reason: "Create the disposable migration fixture.".into(),
            before_hash: NULL_HASH.into(),
            after_hash: NULL_HASH.into(),
            payload: json!({}),
            caveats: Vec::new(),
            signature: None,
        };
        genesis.id = compute_event_id(&genesis);
        fs::write(
            frontier_root
                .join(".vela/events")
                .join(format!("{}.json", genesis.id)),
            to_canonical_bytes(&genesis).unwrap(),
        )
        .unwrap();

        let authorization_input = authorization_input();
        let authority_keyset = AuthorityKeysetV1 {
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
            authority_summary: "One exact administrator may install Era-1 authority.".into(),
        };
        let legacy_active_policy_head_root = root('3');
        let legacy_policy_store_manifest_root = root('4');
        let migration_payload = AuthorityModelMigrationV1 {
            schema: AUTHORITY_MODEL_MIGRATION_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            legacy_event_log_root: format!("sha256:{}", event_log_hash(&[genesis.clone()])),
            legacy_actor_registry_root: ContentDigest::hash(&actor_registry_bytes).as_str().into(),
            legacy_active_policy_head_root: legacy_active_policy_head_root.clone(),
            legacy_policy_store_manifest_root: legacy_policy_store_manifest_root.clone(),
            new_authority_keyset_root: authority_keyset.root().unwrap(),
            new_policy_bundle_root: policy_bundle.root().unwrap(),
            new_principal_id: REPOSITORY_PRINCIPAL.into(),
            minimum_writer_version: "0.930.0".into(),
            reason: "Move this disposable fixture to repository authority.".into(),
        };
        let mut migration_event = StateEvent {
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
            payload: serde_json::to_value(migration_payload).unwrap(),
            caveats: vec!["Historical events remain byte-identical.".into()],
            signature: None,
        };
        migration_event.id = compute_event_id(&migration_event);
        migration_event.signature = Some(sign_event(&migration_event, &legacy_key).unwrap());
        let migration_intent = ContentDigest::hash(to_canonical_bytes(&migration_event).unwrap())
            .as_str()
            .to_string();

        let fixture_input = InputBinding::existing_file(
            frontier_root,
            RepoPath::parse(".vela/input.json").unwrap(),
        )
        .unwrap();
        let request = AuthorityMigrationRequest {
            history: AuthorityMigrationHistorySnapshot {
                frontier_id: FRONTIER_ID.into(),
                legacy_events: vec![genesis],
                legacy_actor_registry_bytes: actor_registry_bytes,
                legacy_active_policy_head_root,
                legacy_policy_store_manifest_root,
            },
            migration_event,
            authority_keyset,
            policy_bundle,
            intent_digest: migration_intent,
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
            read_set: vec![fixture_input],
            vela_version: "0.930.0-rc.1".into(),
            binary_sha256: root('8'),
            recorded_at: RECORDED_AT.into(),
        };
        Fixture {
            temporary,
            request,
            repository_key,
        }
    }

    fn transaction_request(
        fixture: &Fixture,
        history: AuthorityHistorySnapshot,
        action: &str,
        resource: String,
        intent_digest: String,
        recorded_at: &str,
        event: AuthorityEventDraft,
        next_authority_keyset: Option<AuthorityKeysetV1>,
    ) -> AuthorityTransactionRequest {
        let mut authorization = authorization_input();
        authorization.action = action.into();
        authorization.resource = resource;
        let reason = event.reason.clone();
        AuthorityTransactionRequest {
            history,
            intent_digest: intent_digest.clone(),
            principal: fixture.request.principal.clone(),
            authentication_request: AuthenticationRequest {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                principal_class: PrincipalClass::Human,
                transaction_at: recorded_at.into(),
            },
            runtime_session_state: RuntimeSessionState::default(),
            authorization_input: authorization,
            delegation: None,
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                role: "frontier_administrator".into(),
                action: action.into(),
                reason,
                approved_at: recorded_at.into(),
                intent_digest,
            }],
            event_drafts: vec![event],
            object_drafts: Vec::new(),
            next_authority_keyset,
            next_policy_bundle: None,
            next_policy_material: None,
            read_set: fixture.request.read_set.clone(),
            vela_version: fixture.request.vela_version.clone(),
            binary_sha256: fixture.request.binary_sha256.clone(),
            recorded_at: recorded_at.into(),
        }
    }

    fn append_installed_transaction(
        fixture: &Fixture,
        history: &mut AuthorityHistorySnapshot,
        result: &AuthorityTransactionResult,
    ) {
        assert_eq!(result.event_ids.len(), 1);
        let event: AuthorityEventV1 = serde_json::from_slice(
            &fs::read(
                fixture
                    .root()
                    .join(authority_event_path(&result.event_ids[0])),
            )
            .unwrap(),
        )
        .unwrap();
        let envelope: AuthorityEnvelopeV1 = serde_json::from_slice(
            &fs::read(
                fixture
                    .root()
                    .join(authority_record_path(&result.authority_record_id)),
            )
            .unwrap(),
        )
        .unwrap();
        history.authority_events.push(event);
        history.authority_envelopes.push(envelope);
    }

    #[test]
    fn sequence_one_installs_exact_bridge_snapshots_and_covering_record() {
        let fixture = fixture();
        let request = fixture.request.clone();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let result = execute_authority_migration(
            fixture.barrier(),
            fixture.root(),
            request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap();

        assert_eq!(signer.calls, 1);
        assert!(fixture.migration_path().is_file());
        assert!(
            fixture
                .root()
                .join(authority_keyset_path(&request.authority_keyset.root().unwrap()).unwrap())
                .is_file()
        );
        assert!(
            fixture
                .root()
                .join(authority_policy_path(&request.policy_bundle.root().unwrap()).unwrap())
                .is_file()
        );
        let envelope: AuthorityEnvelopeV1 = serde_json::from_slice(
            &fs::read(
                fixture
                    .root()
                    .join(authority_record_path(&result.authority_record_id)),
            )
            .unwrap(),
        )
        .unwrap();
        let mut migrated = request.history.legacy_events.clone();
        migrated.push(request.migration_event);
        let verified = verify_authority_history(AuthorityHistoryInput {
            frontier_id: FRONTIER_ID,
            legacy_events: &migrated,
            legacy_actor_registry_bytes: &request.history.legacy_actor_registry_bytes,
            legacy_active_policy_head_root: &request.history.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &request.history.legacy_policy_store_manifest_root,
            authority_keysets: std::slice::from_ref(&request.authority_keyset),
            policy_bundles: std::slice::from_ref(&request.policy_bundle),
            authority_events: &[],
            authority_envelopes: &[envelope],
        })
        .unwrap();
        assert_eq!(verified.era, AuthorityHistoryEra::RepositoryAuthority);
        assert_eq!(verified.authority_record_count, 1);
        assert_eq!(
            verified.final_authority_record_root,
            Some(result.authority_record_root)
        );
    }

    #[test]
    fn disposable_frontier_migrates_rotates_writes_closes_and_replays_from_bytes() {
        let fixture = fixture();
        let migration_request = fixture.request.clone();
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let migration = execute_authority_migration(
            fixture.barrier(),
            fixture.root(),
            migration_request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        assert_eq!(signer.calls, 1);

        let migration_envelope: AuthorityEnvelopeV1 = serde_json::from_slice(
            &fs::read(
                fixture
                    .root()
                    .join(authority_record_path(&migration.authority_record_id)),
            )
            .unwrap(),
        )
        .unwrap();
        let mut legacy_events = migration_request.history.legacy_events.clone();
        legacy_events.push(migration_request.migration_event.clone());
        let mut history = AuthorityHistorySnapshot {
            frontier_id: FRONTIER_ID.into(),
            legacy_events,
            legacy_actor_registry_bytes: migration_request
                .history
                .legacy_actor_registry_bytes
                .clone(),
            legacy_active_policy_head_root: migration_request
                .history
                .legacy_active_policy_head_root
                .clone(),
            legacy_policy_store_manifest_root: migration_request
                .history
                .legacy_policy_store_manifest_root
                .clone(),
            authority_keyset: migration_request.authority_keyset.clone(),
            policy_bundle: migration_request.policy_bundle.clone(),
            retained_authority_keysets: vec![migration_request.authority_keyset.clone()],
            retained_policy_bundles: vec![migration_request.policy_bundle.clone()],
            authority_events: Vec::new(),
            authority_envelopes: vec![migration_envelope],
        };

        let ordinary_reason = "Reject the first disposable Era-1 proposal.";
        let ordinary_request = transaction_request(
            &fixture,
            history.clone(),
            "review_reject",
            r#"Proposal::"vpr_0123456789abcdef""#.into(),
            root('a'),
            "2026-07-24T12:06:00Z",
            AuthorityEventDraft {
                kind: EventKind::ReviewRejected,
                target: StateTarget {
                    r#type: "proposal".into(),
                    id: "vpr_0123456789abcdef".into(),
                },
                actor: StateActor {
                    r#type: "human".into(),
                    id: REPOSITORY_PRINCIPAL.into(),
                },
                timestamp: "2026-07-24T12:06:00Z".into(),
                reason: ordinary_reason.into(),
                before_hash: root('f'),
                after_hash: root('f'),
                payload: json!({"proposal_id": "vpr_0123456789abcdef"}),
                caveats: Vec::new(),
            },
            None,
        );
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let ordinary = execute_authority_transaction(
            fixture.barrier(),
            fixture.root(),
            ordinary_request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        append_installed_transaction(&fixture, &mut history, &ordinary);

        let next_key = SigningKey::from_bytes(&[24; 32]);
        let next_keyset = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            generation: 2,
            threshold: 1,
            keys: vec![AuthorityKeyV1 {
                key_id: "repository-key-2".into(),
                algorithm: AUTHORITY_KEY_ALGORITHM.into(),
                public_key: hex::encode(next_key.verifying_key().to_bytes()),
                valid_from_sequence: 4,
                valid_through_sequence: None,
                purpose: AUTHORITY_KEY_PURPOSE.into(),
            }],
            previous_keyset_root: Some(history.authority_keyset.root().unwrap()),
            activation_record_root: Some(ordinary.authority_record_root.clone()),
            closed: false,
        };
        let rotate_reason = "Rotate the disposable Frontier repository key.";
        let rotation_request = transaction_request(
            &fixture,
            history.clone(),
            AUTHORITY_ROTATE_ACTION,
            format!(r#"Frontier::"{FRONTIER_ID}""#),
            root('b'),
            "2026-07-24T12:07:00Z",
            AuthorityEventDraft {
                kind: EventKind::Other("authority.rotated".into()),
                target: StateTarget {
                    r#type: "frontier".into(),
                    id: FRONTIER_ID.into(),
                },
                actor: StateActor {
                    r#type: "human".into(),
                    id: REPOSITORY_PRINCIPAL.into(),
                },
                timestamp: "2026-07-24T12:07:00Z".into(),
                reason: rotate_reason.into(),
                before_hash: root('f'),
                after_hash: root('f'),
                payload: json!({"authority_keyset_root": next_keyset.root().unwrap()}),
                caveats: Vec::new(),
            },
            Some(next_keyset.clone()),
        );
        let mut adapter = fixture.adapter();
        let mut signer = fixture.signer();
        let rotation = execute_authority_transaction(
            fixture.barrier(),
            fixture.root(),
            rotation_request,
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        append_installed_transaction(&fixture, &mut history, &rotation);
        history.authority_keyset = next_keyset.clone();
        history.retained_authority_keysets.push(next_keyset.clone());

        let later_reason = "Reject a later proposal under the rotated repository key.";
        let later_request = transaction_request(
            &fixture,
            history.clone(),
            "review_reject",
            r#"Proposal::"vpr_0123456789abcdef""#.into(),
            root('c'),
            "2026-07-24T12:08:00Z",
            AuthorityEventDraft {
                kind: EventKind::ReviewRejected,
                target: StateTarget {
                    r#type: "proposal".into(),
                    id: "vpr_0123456789abcdef".into(),
                },
                actor: StateActor {
                    r#type: "human".into(),
                    id: REPOSITORY_PRINCIPAL.into(),
                },
                timestamp: "2026-07-24T12:08:00Z".into(),
                reason: later_reason.into(),
                before_hash: root('f'),
                after_hash: root('f'),
                payload: json!({"proposal_id": "vpr_0123456789abcdef"}),
                caveats: Vec::new(),
            },
            None,
        );
        let mut adapter = fixture.adapter();
        let mut next_signer = TestSigner {
            key: next_key.clone(),
            key_id: "repository-key-2".into(),
            calls: 0,
            fail: false,
        };
        let later = execute_authority_transaction(
            fixture.barrier(),
            fixture.root(),
            later_request,
            &mut adapter,
            &mut next_signer,
        )
        .unwrap();
        assert_eq!(next_signer.calls, 1);
        append_installed_transaction(&fixture, &mut history, &later);

        let closed_keyset = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            generation: 3,
            threshold: 0,
            keys: Vec::new(),
            previous_keyset_root: Some(next_keyset.root().unwrap()),
            activation_record_root: Some(later.authority_record_root.clone()),
            closed: true,
        };
        let close_reason = "Close future authority after the disposable lifecycle drill.";
        let close_payload = AuthorityCloseV1 {
            schema: AUTHORITY_CLOSE_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            last_trusted_sequence: 4,
            last_trusted_authority_record_root: later.authority_record_root.clone(),
            previous_authority_keyset_root: next_keyset.root().unwrap(),
            closed_authority_keyset_root: closed_keyset.root().unwrap(),
            policy_bundle_root: history.policy_bundle.root().unwrap(),
            incident_id: "incident:disposable-lifecycle-close".into(),
            reason: close_reason.into(),
        };
        let close_request = transaction_request(
            &fixture,
            history.clone(),
            AUTHORITY_CLOSE_ACTION,
            format!(r#"Frontier::"{FRONTIER_ID}""#),
            root('e'),
            "2026-07-24T12:09:00Z",
            AuthorityEventDraft {
                kind: EventKind::Other(AUTHORITY_CLOSED_EVENT_KIND.into()),
                target: StateTarget {
                    r#type: "frontier".into(),
                    id: FRONTIER_ID.into(),
                },
                actor: StateActor {
                    r#type: "human".into(),
                    id: REPOSITORY_PRINCIPAL.into(),
                },
                timestamp: "2026-07-24T12:09:00Z".into(),
                reason: close_reason.into(),
                before_hash: root('0'),
                after_hash: root('0'),
                payload: serde_json::to_value(close_payload).unwrap(),
                caveats: Vec::new(),
            },
            Some(closed_keyset.clone()),
        );
        let mut adapter = fixture.adapter();
        let mut next_signer = TestSigner {
            key: next_key,
            key_id: "repository-key-2".into(),
            calls: 0,
            fail: false,
        };
        let close = execute_authority_transaction(
            fixture.barrier(),
            fixture.root(),
            close_request,
            &mut adapter,
            &mut next_signer,
        )
        .unwrap();
        append_installed_transaction(&fixture, &mut history, &close);
        history.authority_keyset = closed_keyset.clone();
        history
            .retained_authority_keysets
            .push(closed_keyset.clone());

        run_git(fixture.root(), &["init", "-q", "-b", "main"]);
        run_git(
            fixture.root(),
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "add",
                ".vela/events",
                ".vela/authority",
                ".vela/actors.json",
            ],
        );
        run_git(
            fixture.root(),
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=Vela fixture",
                "-c",
                "user.email=fixture@invalid",
                "commit",
                "-q",
                "-m",
                "disposable authority lifecycle",
            ],
        );
        let clone_parent = TempDir::new().unwrap();
        let clone_root = clone_parent.path().join("frontier");
        let clone_output = Command::new("git")
            .args(["clone", "-q", "--no-local", "--no-hardlinks"])
            .arg(fixture.root())
            .arg(&clone_root)
            .output()
            .unwrap();
        assert!(
            clone_output.status.success(),
            "clean clone failed: {}",
            String::from_utf8_lossy(&clone_output.stderr)
        );
        let status = Command::new("git")
            .current_dir(&clone_root)
            .args(["status", "--porcelain"])
            .output()
            .unwrap();
        assert!(status.status.success());
        assert!(status.stdout.is_empty());

        let installed_events = history
            .authority_events
            .iter()
            .map(|event| {
                serde_json::from_slice::<AuthorityEventV1>(
                    &fs::read(clone_root.join(authority_event_path(&event.id))).unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let installed_envelopes = [
            migration.authority_record_id.as_str(),
            ordinary.authority_record_id.as_str(),
            rotation.authority_record_id.as_str(),
            later.authority_record_id.as_str(),
            close.authority_record_id.as_str(),
        ]
        .into_iter()
        .map(|record_id| {
            serde_json::from_slice::<AuthorityEnvelopeV1>(
                &fs::read(clone_root.join(authority_record_path(record_id))).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
        let installed_keysets = [
            migration_request.authority_keyset.root().unwrap(),
            next_keyset.root().unwrap(),
            closed_keyset.root().unwrap(),
        ]
        .into_iter()
        .map(|keyset_root| {
            serde_json::from_slice::<AuthorityKeysetV1>(
                &fs::read(clone_root.join(authority_keyset_path(&keyset_root).unwrap())).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
        let installed_policy: PolicyBundleV1 = serde_json::from_slice(
            &fs::read(clone_root.join(
                authority_policy_path(&migration_request.policy_bundle.root().unwrap()).unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();
        let installed_legacy_events = history
            .legacy_events
            .iter()
            .map(|event| {
                serde_json::from_slice::<StateEvent>(
                    &fs::read(
                        clone_root
                            .join(".vela/events")
                            .join(format!("{}.json", event.id)),
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let replay = verify_authority_history(AuthorityHistoryInput {
            frontier_id: FRONTIER_ID,
            legacy_events: &installed_legacy_events,
            legacy_actor_registry_bytes: &fs::read(clone_root.join(".vela/actors.json")).unwrap(),
            legacy_active_policy_head_root: &migration_request
                .history
                .legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: &migration_request
                .history
                .legacy_policy_store_manifest_root,
            authority_keysets: &installed_keysets,
            policy_bundles: std::slice::from_ref(&installed_policy),
            authority_events: &installed_events,
            authority_envelopes: &installed_envelopes,
        })
        .unwrap();
        assert_eq!(replay.authority_record_count, 5);
        assert_eq!(replay.authority_event_count, 4);
        assert!(replay.closed);
        assert_eq!(
            replay.final_authority_record_root,
            Some(close.authority_record_root)
        );
        assert_eq!(
            replay.final_authority_keyset_root,
            Some(closed_keyset.root().unwrap())
        );
        assert_eq!(
            replay.final_policy_bundle_root,
            Some(installed_policy.root().unwrap())
        );
    }

    #[test]
    fn invalid_bridge_and_policy_substitution_fail_before_repository_signing() {
        let first = fixture();
        let mut bad_signature = first.request.clone();
        let wrong_key = SigningKey::from_bytes(&[23; 32]);
        bad_signature.migration_event.signature =
            Some(sign_event(&bad_signature.migration_event, &wrong_key).unwrap());
        let mut adapter = first.adapter();
        let mut signer = first.signer();
        let error = prepare_authority_migration(
            first.barrier(),
            first.root(),
            bad_signature,
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(error.to_string().contains("legacy signature"));
        assert_eq!(signer.calls, 0);
        assert_eq!(first.journal_count(), 0);

        let second = fixture();
        let mut substituted = second.request.clone();
        substituted
            .authorization_input
            .policies
            .push_str("\n// byte substitution\n");
        let mut adapter = second.adapter();
        let mut signer = second.signer();
        let error = prepare_authority_migration(
            second.barrier(),
            second.root(),
            substituted,
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(error.to_string().contains("Cedar bytes"));
        assert_eq!(signer.calls, 0);
        assert_eq!(second.journal_count(), 0);
    }

    #[test]
    fn cancellation_and_signer_refusal_leave_zero_canonical_delta() {
        let first = fixture();
        let mut cancelled = CancelledAdapter;
        let mut signer = first.signer();
        let error = prepare_authority_migration(
            first.barrier(),
            first.root(),
            first.request.clone(),
            &mut cancelled,
            &mut signer,
        )
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(signer.calls, 0);
        assert!(!first.migration_path().exists());
        assert_eq!(first.journal_count(), 0);

        let second = fixture();
        let mut adapter = second.adapter();
        let mut signer = second.signer();
        signer.fail = true;
        let error = prepare_authority_migration(
            second.barrier(),
            second.root(),
            second.request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap_err();
        assert!(error.to_string().contains("signer refusal"));
        assert_eq!(signer.calls, 1);
        assert!(!second.migration_path().exists());
        assert_eq!(second.journal_count(), 0);
    }

    #[test]
    fn stale_membership_blocks_marker_and_post_marker_recovery_needs_no_signer() {
        let first = fixture();
        let mut adapter = first.adapter();
        let mut signer = first.signer();
        let mut prepared = prepare_authority_migration(
            first.barrier(),
            first.root(),
            first.request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        fs::write(
            first.root().join(".vela/events/external-drift.json"),
            b"{}\n",
        )
        .unwrap();
        let error = prepared.mark_committed().unwrap_err();
        assert!(error.to_string().contains("changed"));
        assert!(!first.migration_path().exists());
        drop(prepared);

        let second = fixture();
        let mut adapter = second.adapter();
        let mut signer = second.signer();
        let mut prepared = prepare_authority_migration(
            second.barrier(),
            second.root(),
            second.request.clone(),
            &mut adapter,
            &mut signer,
        )
        .unwrap();
        let expected = prepared.result.clone();
        let operation_id = OperationId::parse(&expected.operation_id).unwrap();
        prepared.mark_committed().unwrap();
        let error = prepared
            .transaction_mut()
            .install_at_failpoint(FrontierTxnStep::AfterInstallingJournalWrite { index: 0 })
            .unwrap_err();
        assert!(error.to_string().contains("injected"));
        drop(prepared);
        assert_eq!(
            FrontierTxn::recover(second.root(), &second.journals(), &operation_id).unwrap(),
            RecoveryOutcome::Completed
        );
        assert_eq!(signer.calls, 1);
        let barrier =
            FrontierTxn::acquire_write_barrier_for_test(second.root(), &second.journals()).unwrap();
        assert_eq!(
            retry_completed_authority_transaction(&barrier, &expected).unwrap(),
            expected
        );
        assert_eq!(signer.calls, 1);
    }
}
