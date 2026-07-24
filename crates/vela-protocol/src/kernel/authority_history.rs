//! Read-only verification across the Era-0/Era-1 authority boundary.
//!
//! The migration bridge is one ordinary legacy-signed event. Authority record
//! sequence 1 covers that event and starts the repository-authority chain.
//! This module deliberately exposes no writer or key-custody surface.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::actor_registration::require_sha256_root;
use crate::authority::{
    AuthorityEnvelopeV1, AuthorityEventV1, AuthorityKeysetV1, CedarDecision, PolicyBundleV1,
    VerifiedAuthorityRecord, verify_authority_envelope,
};
use crate::canonical::{sha256_canonical, to_canonical_bytes};
use crate::events::{
    EVENT_KIND_AUTHORITY_MODEL_MIGRATED, NULL_HASH, StateEvent, compute_event_id, event_log_hash,
};
use crate::sign::{ActorRecord, verify_event_signature};

pub const AUTHORITY_MODEL_MIGRATION_SCHEMA_V1: &str = "vela.authority-model-migration.v1";
pub const AUTHORITY_EVENT_LOG_SCHEMA_V1: &str = "vela.authority-event-log.v1";
pub const AUTHORITY_MIGRATION_ACTION: &str = "authority_model_migrate";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityModelMigrationV1 {
    pub schema: String,
    pub frontier_id: String,
    pub legacy_event_log_root: String,
    pub legacy_actor_registry_root: String,
    pub legacy_active_policy_head_root: String,
    pub legacy_policy_store_manifest_root: String,
    pub new_authority_keyset_root: String,
    pub new_policy_bundle_root: String,
    pub new_principal_id: String,
    pub minimum_writer_version: String,
    pub reason: String,
}

impl AuthorityModelMigrationV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORITY_MODEL_MIGRATION_SCHEMA_V1 {
            return Err(format!(
                "migration schema must be {AUTHORITY_MODEL_MIGRATION_SCHEMA_V1}"
            ));
        }
        if !self.frontier_id.starts_with("vfr_") {
            return Err("migration frontier_id must start with vfr_".into());
        }
        for (name, root) in [
            ("legacy_event_log_root", self.legacy_event_log_root.as_str()),
            (
                "legacy_actor_registry_root",
                self.legacy_actor_registry_root.as_str(),
            ),
            (
                "legacy_active_policy_head_root",
                self.legacy_active_policy_head_root.as_str(),
            ),
            (
                "legacy_policy_store_manifest_root",
                self.legacy_policy_store_manifest_root.as_str(),
            ),
            (
                "new_authority_keyset_root",
                self.new_authority_keyset_root.as_str(),
            ),
            (
                "new_policy_bundle_root",
                self.new_policy_bundle_root.as_str(),
            ),
        ] {
            require_sha256_root(name, root)?;
        }
        if self.new_principal_id.trim().is_empty()
            || self.minimum_writer_version.trim().is_empty()
            || self.reason.trim().is_empty()
        {
            return Err(
                "migration principal, minimum writer version, and reason must be non-empty".into(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityHistoryEra {
    LegacyOnly,
    RepositoryAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityHistoryVerification {
    pub era: AuthorityHistoryEra,
    pub frontier_id: String,
    pub legacy_event_count: usize,
    pub authority_event_count: usize,
    pub authority_record_count: usize,
    pub migration_event_id: Option<String>,
    pub final_event_log_root: String,
    pub final_authority_record_root: Option<String>,
}

/// Complete read-side inputs. Registry bytes are the exact retained
/// `.vela/actors.json` bytes so the bridge binds the immutable Era-0 object,
/// including its formatting, rather than a lossy reconstruction.
pub struct AuthorityHistoryInput<'a> {
    pub frontier_id: &'a str,
    pub legacy_events: &'a [StateEvent],
    pub legacy_actor_registry_bytes: &'a [u8],
    pub legacy_active_policy_head_root: &'a str,
    pub legacy_policy_store_manifest_root: &'a str,
    pub authority_keyset: &'a AuthorityKeysetV1,
    pub policy_bundle: &'a PolicyBundleV1,
    pub authority_events: &'a [AuthorityEventV1],
    pub authority_envelopes: &'a [AuthorityEnvelopeV1],
}

/// Verify an unmigrated Era-0 history or the complete one-time bridge into
/// Era-1. Existing Era-0 signature/policy checks remain separate and unchanged;
/// this verifier proves the boundary and all post-boundary record coverage.
pub fn verify_authority_history(
    input: AuthorityHistoryInput<'_>,
) -> Result<AuthorityHistoryVerification, String> {
    require_frontier(input.frontier_id)?;
    let migrations: Vec<&StateEvent> = input
        .legacy_events
        .iter()
        .filter(|event| event.kind.as_str() == EVENT_KIND_AUTHORITY_MODEL_MIGRATED)
        .collect();

    if migrations.is_empty() {
        if !input.authority_events.is_empty() || !input.authority_envelopes.is_empty() {
            return Err("Era-1 history exists without an authority-model migration bridge".into());
        }
        return Ok(AuthorityHistoryVerification {
            era: AuthorityHistoryEra::LegacyOnly,
            frontier_id: input.frontier_id.into(),
            legacy_event_count: input.legacy_events.len(),
            authority_event_count: 0,
            authority_record_count: 0,
            migration_event_id: None,
            final_event_log_root: prefixed_legacy_root(input.legacy_events),
            final_authority_record_root: None,
        });
    }
    if migrations.len() != 1 {
        return Err("authority history must contain exactly one migration bridge".into());
    }
    if input.authority_envelopes.is_empty() {
        return Err("migration bridge has no covering authority record".into());
    }

    let migration_event = migrations[0];
    let migration = migration_payload_from_event(migration_event)?;
    if migration.frontier_id != input.frontier_id {
        return Err("migration bridge targets a different frontier".into());
    }

    let legacy_prefix: Vec<StateEvent> = input
        .legacy_events
        .iter()
        .filter(|event| event.id != migration_event.id)
        .cloned()
        .collect();
    if migration.legacy_event_log_root != prefixed_legacy_root(&legacy_prefix) {
        return Err(
            "legacy event-log root does not match the exact pre-migration history; a legacy write may have occurred after migration"
                .into(),
        );
    }
    if migration.legacy_active_policy_head_root != input.legacy_active_policy_head_root
        || migration.legacy_policy_store_manifest_root != input.legacy_policy_store_manifest_root
    {
        return Err("migration bridge does not bind the supplied legacy policy state".into());
    }

    verify_legacy_migration_signature(
        migration_event,
        input.legacy_actor_registry_bytes,
        &migration,
    )?;

    input.authority_keyset.validate()?;
    input.policy_bundle.validate()?;
    if input.authority_keyset.frontier_id != input.frontier_id
        || input.policy_bundle.frontier_id != input.frontier_id
        || migration.new_authority_keyset_root != input.authority_keyset.root()?
        || migration.new_policy_bundle_root != input.policy_bundle.root()?
    {
        return Err("migration bridge does not bind the supplied Era-1 authority inputs".into());
    }

    let mut legacy_ids = BTreeSet::new();
    for event in input.legacy_events {
        if event.id != compute_event_id(event) || !legacy_ids.insert(event.id.as_str()) {
            return Err(format!(
                "legacy event {} has an invalid or duplicate content address",
                event.id
            ));
        }
    }

    let mut era_one_by_id = BTreeMap::new();
    let mut era_one_by_transaction: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for event in input.authority_events {
        event.validate()?;
        if legacy_ids.contains(event.id.as_str())
            || era_one_by_id.insert(event.id.as_str(), event).is_some()
        {
            return Err(format!("duplicate event coverage identity {}", event.id));
        }
        era_one_by_transaction
            .entry(event.content.transaction_id.as_str())
            .or_default()
            .insert(event.id.as_str());
    }

    let legacy_root_with_bridge = prefixed_legacy_root(input.legacy_events);
    let mut current_event_root = migration.legacy_event_log_root.clone();
    let mut previous_record_root: Option<String> = None;
    let mut covered_era_one: BTreeSet<String> = BTreeSet::new();
    let mut cumulative_era_one = Vec::new();
    let mut verified_records = Vec::new();

    for (offset, envelope) in input.authority_envelopes.iter().enumerate() {
        let sequence = u64::try_from(offset + 1)
            .map_err(|_| "authority record sequence exceeds u64".to_string())?;
        let verified = verify_authority_envelope(
            envelope,
            input.authority_keyset,
            input.frontier_id,
            sequence,
            previous_record_root.as_deref(),
        )?;
        verify_record_authorization(&verified, input.policy_bundle)?;
        if verified.record.content.before_event_log_root != current_event_root {
            return Err(format!(
                "authority record {sequence} has the wrong before-event root"
            ));
        }

        if sequence == 1 {
            verify_first_record(
                &verified,
                migration_event,
                &migration,
                &legacy_root_with_bridge,
            )?;
            current_event_root = legacy_root_with_bridge.clone();
        } else {
            let transaction_id = verified.record.content.transaction_id.as_str();
            let expected_ids = era_one_by_transaction.get(transaction_id).ok_or_else(|| {
                format!("authority record {sequence} references an unknown or empty transaction")
            })?;
            let actual_ids: BTreeSet<&str> = verified
                .record
                .content
                .event_ids
                .iter()
                .map(String::as_str)
                .collect();
            if &actual_ids != expected_ids {
                return Err(format!(
                    "authority record {sequence} does not exactly cover its transaction events"
                ));
            }
            for event_id in actual_ids {
                if !covered_era_one.insert(event_id.to_string()) {
                    return Err(format!("Era-1 event {event_id} is covered more than once"));
                }
                let event = era_one_by_id[event_id];
                if event.content.principal_id != verified.record.content.principal.principal_id
                    || event.content.actor.id != event.content.principal_id
                {
                    return Err(format!(
                        "Era-1 event {event_id} attribution does not match its authority record"
                    ));
                }
                verify_event_object_delta(&verified, event_id, &event.root()?)?;
                cumulative_era_one.push(event);
            }
            let expected_after =
                authority_event_log_root(&legacy_root_with_bridge, &cumulative_era_one)?;
            if verified.record.content.after_event_log_root != expected_after {
                return Err(format!(
                    "authority record {sequence} has the wrong after-event root"
                ));
            }
            current_event_root = expected_after;
        }
        previous_record_root = Some(verified.record_root.clone());
        verified_records.push(verified);
    }

    if covered_era_one.len() != input.authority_events.len() {
        let missing: Vec<&str> = era_one_by_id
            .keys()
            .copied()
            .filter(|event_id| !covered_era_one.contains(*event_id))
            .collect();
        return Err(format!(
            "Era-1 history has events without unique authority-record coverage: {}",
            missing.join(", ")
        ));
    }

    Ok(AuthorityHistoryVerification {
        era: AuthorityHistoryEra::RepositoryAuthority,
        frontier_id: input.frontier_id.into(),
        legacy_event_count: input.legacy_events.len(),
        authority_event_count: input.authority_events.len(),
        authority_record_count: verified_records.len(),
        migration_event_id: Some(migration_event.id.clone()),
        final_event_log_root: current_event_root,
        final_authority_record_root: previous_record_root,
    })
}

pub fn migration_payload_from_event(
    event: &StateEvent,
) -> Result<AuthorityModelMigrationV1, String> {
    if event.kind.as_str() != EVENT_KIND_AUTHORITY_MODEL_MIGRATED
        || event.target.r#type != "frontier"
        || event.actor.r#type != "human"
        || event.before_hash != NULL_HASH
        || event.after_hash != NULL_HASH
        || event.signature.is_none()
        || event.id != compute_event_id(event)
    {
        return Err("authority-model migration event shape is invalid".into());
    }
    let payload: AuthorityModelMigrationV1 = serde_json::from_value(event.payload.clone())
        .map_err(|error| format!("authority-model migration payload is invalid: {error}"))?;
    payload.validate()?;
    if event.target.id != payload.frontier_id
        || event.reason != payload.reason
        || event.actor.id.trim().is_empty()
    {
        return Err("authority-model migration event does not match its payload".into());
    }
    Ok(payload)
}

pub fn authority_event_log_root(
    legacy_root_with_bridge: &str,
    authority_events: &[&AuthorityEventV1],
) -> Result<String, String> {
    require_sha256_root("legacy_root_with_bridge", legacy_root_with_bridge)?;
    let mut roots = authority_events
        .iter()
        .map(|event| event.root().map(|root| (event.id.as_str(), root)))
        .collect::<Result<Vec<_>, _>>()?;
    roots.sort_by(|left, right| left.0.cmp(right.0));
    let commitment = serde_json::json!({
        "schema": AUTHORITY_EVENT_LOG_SCHEMA_V1,
        "legacy_event_log_root": legacy_root_with_bridge,
        "authority_event_roots": roots.into_iter().map(|(_, root)| root).collect::<Vec<_>>(),
    });
    Ok(format!("sha256:{}", sha256_canonical(&commitment)?))
}

fn verify_legacy_migration_signature(
    event: &StateEvent,
    actor_registry_bytes: &[u8],
    migration: &AuthorityModelMigrationV1,
) -> Result<(), String> {
    let registry_root = sha256_bytes(actor_registry_bytes);
    if migration.legacy_actor_registry_root != registry_root {
        return Err("migration bridge actor-registry root does not match retained bytes".into());
    }
    let actors: Vec<ActorRecord> = serde_json::from_slice(actor_registry_bytes)
        .map_err(|error| format!("legacy actor registry is invalid: {error}"))?;
    let actor = actors
        .iter()
        .find(|actor| actor.id == event.actor.id)
        .ok_or_else(|| "migration signer is absent from the legacy actor registry".to_string())?;
    if actor.algorithm != "ed25519" || actor.is_revoked_at(&event.timestamp) {
        return Err("migration signer key is invalid or revoked at the bridge time".into());
    }
    if !verify_event_signature(event, &actor.public_key)? {
        return Err("migration bridge legacy signature does not verify".into());
    }
    Ok(())
}

fn verify_first_record(
    verified: &VerifiedAuthorityRecord,
    migration_event: &StateEvent,
    migration: &AuthorityModelMigrationV1,
    legacy_root_with_bridge: &str,
) -> Result<(), String> {
    let record = &verified.record;
    if record.content.event_ids != [migration_event.id.clone()]
        || record.content.after_event_log_root != legacy_root_with_bridge
        || record.content.principal.principal_id != migration.new_principal_id
    {
        return Err("authority record 1 does not exactly cover the migration bridge".into());
    }
    let approval = record.content.semantic_approvals.iter().find(|approval| {
        approval.principal_id == migration_event.actor.id
            && approval.action == AUTHORITY_MIGRATION_ACTION
            && approval.reason == migration.reason
            && approval.intent_digest == record.content.intent_digest
    });
    if approval.is_none() {
        return Err("authority record 1 lacks the exact legacy semantic approval".into());
    }
    verify_event_object_delta(
        verified,
        &migration_event.id,
        &canonical_object_root(migration_event)?,
    )
}

fn verify_record_authorization(
    verified: &VerifiedAuthorityRecord,
    policy_bundle: &PolicyBundleV1,
) -> Result<(), String> {
    let authorization = &verified.record.content.authorization;
    let evaluation = &authorization.evaluation;
    if authorization.policy_bundle_root != policy_bundle.root()?
        || !evaluation.valid
        || evaluation.decision != CedarDecision::Allow
        || evaluation.engine != crate::authority::CEDAR_ENGINE
        || evaluation.engine_version != crate::authority::CEDAR_ENGINE_VERSION
        || evaluation.profile != crate::authority::CEDAR_PROFILE_V1
    {
        return Err(format!(
            "authority record {} lacks a valid pinned Cedar authorization",
            verified.record.content.sequence
        ));
    }
    Ok(())
}

fn verify_event_object_delta(
    verified: &VerifiedAuthorityRecord,
    event_id: &str,
    event_root: &str,
) -> Result<(), String> {
    let expected_path = format!(".vela/events/{event_id}.json");
    let matches = verified
        .record
        .content
        .object_delta
        .iter()
        .filter(|delta| {
            delta.path == expected_path
                && delta.before_root.is_none()
                && delta.after_root.as_deref() == Some(event_root)
                && delta.object_kind == "event"
        })
        .count();
    if matches != 1 {
        return Err(format!(
            "authority record {} lacks one exact object delta for {event_id}",
            verified.record.content.sequence
        ));
    }
    Ok(())
}

fn prefixed_legacy_root(events: &[StateEvent]) -> String {
    format!("sha256:{}", event_log_hash(events))
}

fn canonical_object_root<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(to_canonical_bytes(value)?))
    ))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn require_frontier(frontier_id: &str) -> Result<(), String> {
    if frontier_id.starts_with("vfr_") {
        Ok(())
    } else {
        Err("authority history frontier_id must start with vfr_".into())
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;

    use super::*;
    use crate::authority::{
        AUTHORITY_KEY_ALGORITHM, AUTHORITY_KEY_PURPOSE, AUTHORITY_KEYSET_SCHEMA_V1,
        AUTHORITY_PAYLOAD_TYPE_V1, AuthenticationClaimV1, AuthorityEventContentV1, AuthorityKeyV1,
        AuthorityRecordContentV1, AuthorityRecordV1, AuthorizationClaimV1, CedarEvaluation,
        DelegationClaimV1, DsseSignatureV1, ExecutionClaimV1, ObjectDeltaV1,
        POLICY_BUNDLE_SCHEMA_V1, PrincipalClass, PrincipalSnapshotV1, SemanticApprovalV1, dsse_pae,
    };
    use crate::events::{EVENT_SCHEMA, EventKind, StateActor, StateTarget, compute_event_id};

    const FRONTIER_ID: &str = "vfr_0123456789abcdef";
    const LEGACY_ACTOR: &str = "reviewer:legacy";
    const REPOSITORY_PRINCIPAL: &str = "principal:repository-admin";

    fn root(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    struct Fixture {
        legacy_events: Vec<StateEvent>,
        actor_registry_bytes: Vec<u8>,
        legacy_active_policy_head_root: String,
        legacy_policy_store_manifest_root: String,
        keyset: AuthorityKeysetV1,
        bundle: PolicyBundleV1,
        authority_events: Vec<AuthorityEventV1>,
        envelopes: Vec<AuthorityEnvelopeV1>,
        repository_key: SigningKey,
    }

    impl Fixture {
        fn input(&self) -> AuthorityHistoryInput<'_> {
            AuthorityHistoryInput {
                frontier_id: FRONTIER_ID,
                legacy_events: &self.legacy_events,
                legacy_actor_registry_bytes: &self.actor_registry_bytes,
                legacy_active_policy_head_root: &self.legacy_active_policy_head_root,
                legacy_policy_store_manifest_root: &self.legacy_policy_store_manifest_root,
                authority_keyset: &self.keyset,
                policy_bundle: &self.bundle,
                authority_events: &self.authority_events,
                authority_envelopes: &self.envelopes,
            }
        }

        fn resign_record(&mut self, index: usize, record: AuthorityRecordV1) {
            self.envelopes[index] = signed_envelope(&record, &self.repository_key);
        }
    }

    fn fixture() -> Fixture {
        let legacy_key = SigningKey::from_bytes(&[11; 32]);
        let repository_key = SigningKey::from_bytes(&[12; 32]);
        let actor = ActorRecord {
            id: LEGACY_ACTOR.into(),
            public_key: hex::encode(legacy_key.verifying_key().to_bytes()),
            algorithm: "ed25519".into(),
            created_at: "2026-07-01T00:00:00Z".into(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        let actor_registry_bytes = serde_json::to_vec_pretty(&vec![actor]).unwrap();

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
        let bundle = PolicyBundleV1 {
            schema: POLICY_BUNDLE_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            cedar_schema_root: root('a'),
            policies_root: root('b'),
            entities_root: root('c'),
            tests_root: root('d'),
            engine: crate::authority::CEDAR_ENGINE.into(),
            engine_version: crate::authority::CEDAR_ENGINE_VERSION.into(),
            restricted_profile: crate::authority::CEDAR_PROFILE_V1.into(),
            previous_bundle_root: None,
            authority_summary: "Repository authority may record this exact migration.".into(),
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
            reason: "Create the fixture frontier.".into(),
            before_hash: NULL_HASH.into(),
            after_hash: NULL_HASH.into(),
            payload: json!({}),
            caveats: Vec::new(),
            signature: None,
        };
        genesis.id = compute_event_id(&genesis);
        let legacy_root = prefixed_legacy_root(&[genesis.clone()]);

        let migration_payload = AuthorityModelMigrationV1 {
            schema: AUTHORITY_MODEL_MIGRATION_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            legacy_event_log_root: legacy_root.clone(),
            legacy_actor_registry_root: sha256_bytes(&actor_registry_bytes),
            legacy_active_policy_head_root: root('3'),
            legacy_policy_store_manifest_root: root('4'),
            new_authority_keyset_root: keyset.root().unwrap(),
            new_policy_bundle_root: bundle.root().unwrap(),
            new_principal_id: REPOSITORY_PRINCIPAL.into(),
            minimum_writer_version: "0.930.0".into(),
            reason: "Move this fixture to attributed repository authority.".into(),
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
        migration.signature = Some(crate::sign::sign_event(&migration, &legacy_key).unwrap());
        let legacy_events = vec![genesis, migration.clone()];
        let legacy_root_with_bridge = prefixed_legacy_root(&legacy_events);

        let first = record(
            1,
            None,
            "txn_migration",
            &legacy_root,
            &legacy_root_with_bridge,
            vec![migration.id.clone()],
            vec![event_delta(
                &migration.id,
                &canonical_object_root(&migration).unwrap(),
            )],
            &keyset,
            &bundle,
            vec![SemanticApprovalV1 {
                principal_id: LEGACY_ACTOR.into(),
                role: "frontier_administrator".into(),
                action: AUTHORITY_MIGRATION_ACTION.into(),
                reason: migration_payload.reason,
                approved_at: "2026-07-24T12:00:00Z".into(),
                intent_digest: root('e'),
            }],
        );
        let first_root = first.root().unwrap();

        let era_one = AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "txn_era_one".into(),
            principal_id: REPOSITORY_PRINCIPAL.into(),
            authority_mode: crate::authority::AUTHORITY_MODE.into(),
            kind: EventKind::ReviewRejected,
            target: StateTarget {
                r#type: "proposal".into(),
                id: "vpr_0123456789abcdef".into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: REPOSITORY_PRINCIPAL.into(),
            },
            timestamp: "2026-07-24T12:01:00Z".into(),
            reason: "Reject the fixture proposal.".into(),
            before_hash: root('f'),
            after_hash: root('f'),
            payload: json!({"proposal_id": "vpr_0123456789abcdef"}),
            caveats: Vec::new(),
        })
        .unwrap();
        let final_root = authority_event_log_root(&legacy_root_with_bridge, &[&era_one]).unwrap();
        let second = record(
            2,
            Some(first_root),
            "txn_era_one",
            &legacy_root_with_bridge,
            &final_root,
            vec![era_one.id.clone()],
            vec![event_delta(&era_one.id, &era_one.root().unwrap())],
            &keyset,
            &bundle,
            Vec::new(),
        );
        let envelopes = vec![
            signed_envelope(&first, &repository_key),
            signed_envelope(&second, &repository_key),
        ];

        Fixture {
            legacy_events,
            actor_registry_bytes,
            legacy_active_policy_head_root: root('3'),
            legacy_policy_store_manifest_root: root('4'),
            keyset,
            bundle,
            authority_events: vec![era_one],
            envelopes,
            repository_key,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        sequence: u64,
        previous: Option<String>,
        transaction_id: &str,
        before_root: &str,
        after_root: &str,
        event_ids: Vec<String>,
        object_delta: Vec<ObjectDeltaV1>,
        keyset: &AuthorityKeysetV1,
        bundle: &PolicyBundleV1,
        semantic_approvals: Vec<SemanticApprovalV1>,
    ) -> AuthorityRecordV1 {
        AuthorityRecordV1::new(AuthorityRecordContentV1 {
            frontier_id: FRONTIER_ID.into(),
            sequence,
            previous_authority_record_root: previous,
            operation_id: format!("vop_{sequence}"),
            transaction_id: transaction_id.into(),
            intent_digest: root('e'),
            before_event_log_root: before_root.into(),
            after_event_log_root: after_root.into(),
            event_ids,
            object_delta,
            principal: PrincipalSnapshotV1 {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                principal_class: PrincipalClass::Human,
                display_name: Some("Repository administrator".into()),
                affiliation: None,
                account_links: vec!["github:fixture".into()],
            },
            authentication: AuthenticationClaimV1 {
                method: "ssh_agent".into(),
                session_id: format!("session-{sequence}"),
                authenticated_at: "2026-07-24T12:00:00Z".into(),
                assurance: "proof_of_possession".into(),
                provider: "openssh".into(),
            },
            delegation: None::<DelegationClaimV1>,
            authorization: AuthorizationClaimV1 {
                policy_bundle_root: bundle.root().unwrap(),
                request_root: root('6'),
                entity_snapshot_root: root('7'),
                evaluation: CedarEvaluation {
                    engine: crate::authority::CEDAR_ENGINE.into(),
                    engine_version: crate::authority::CEDAR_ENGINE_VERSION.into(),
                    profile: crate::authority::CEDAR_PROFILE_V1.into(),
                    valid: true,
                    decision: CedarDecision::Allow,
                    automatic_permit: false,
                    determining_policies: vec!["permit_repository_admin".into()],
                    diagnostics: Vec::new(),
                },
            },
            semantic_approvals,
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
        .unwrap()
    }

    fn event_delta(event_id: &str, event_root: &str) -> ObjectDeltaV1 {
        ObjectDeltaV1 {
            path: format!(".vela/events/{event_id}.json"),
            before_root: None,
            after_root: Some(event_root.into()),
            object_kind: "event".into(),
        }
    }

    fn signed_envelope(record: &AuthorityRecordV1, key: &SigningKey) -> AuthorityEnvelopeV1 {
        let payload = to_canonical_bytes(record).unwrap();
        let signature = key.sign(&dsse_pae(AUTHORITY_PAYLOAD_TYPE_V1, &payload));
        AuthorityEnvelopeV1 {
            payload_type: AUTHORITY_PAYLOAD_TYPE_V1.into(),
            payload: BASE64_STANDARD.encode(payload),
            signatures: vec![DsseSignatureV1 {
                keyid: "repository-key-1".into(),
                sig: BASE64_STANDARD.encode(signature.to_bytes()),
            }],
        }
    }

    #[test]
    fn legacy_history_replays_without_an_era_one_writer() {
        let mut fixture = fixture();
        fixture.legacy_events.pop();
        fixture.authority_events.clear();
        fixture.envelopes.clear();
        let result = verify_authority_history(fixture.input()).unwrap();
        assert_eq!(result.era, AuthorityHistoryEra::LegacyOnly);
        assert_eq!(
            result.final_event_log_root,
            prefixed_legacy_root(&fixture.legacy_events)
        );
    }

    #[test]
    fn exact_bridge_and_unique_era_one_coverage_verify() {
        let fixture = fixture();
        let result = verify_authority_history(fixture.input()).unwrap();
        assert_eq!(result.era, AuthorityHistoryEra::RepositoryAuthority);
        assert_eq!(result.legacy_event_count, 2);
        assert_eq!(result.authority_event_count, 1);
        assert_eq!(result.authority_record_count, 2);
        assert_eq!(
            result.final_authority_record_root,
            Some(
                verify_authority_envelope(
                    &fixture.envelopes[1],
                    &fixture.keyset,
                    FRONTIER_ID,
                    2,
                    Some(
                        &verify_authority_envelope(
                            &fixture.envelopes[0],
                            &fixture.keyset,
                            FRONTIER_ID,
                            1,
                            None,
                        )
                        .unwrap()
                        .record_root
                    )
                )
                .unwrap()
                .record_root
            )
        );
    }

    #[test]
    fn post_migration_legacy_write_fails_closed() {
        let mut fixture = fixture();
        let mut later = fixture.legacy_events[0].clone();
        later.timestamp = "2026-07-24T12:02:00Z".into();
        later.reason = "Illegitimate legacy write after migration.".into();
        later.id = compute_event_id(&later);
        fixture.legacy_events.push(later);
        let error = verify_authority_history(fixture.input()).unwrap_err();
        assert!(error.contains("legacy write"), "{error}");
    }

    #[test]
    fn registry_tampering_and_bridge_signature_tampering_fail_closed() {
        let mut registry_tampered = fixture();
        registry_tampered.actor_registry_bytes.push(b'\n');
        assert!(verify_authority_history(registry_tampered.input()).is_err());

        let mut signature_tampered = fixture();
        signature_tampered.legacy_events[1].signature = Some(format!("v1:{}", "0".repeat(128)));
        assert!(verify_authority_history(signature_tampered.input()).is_err());
    }

    #[test]
    fn missing_duplicate_and_wrong_transaction_coverage_fail_closed() {
        let mut missing = fixture();
        missing.envelopes.pop();
        assert!(
            verify_authority_history(missing.input())
                .unwrap_err()
                .contains("without unique")
        );

        let duplicate = fixture();
        let mut second = verify_authority_envelope(
            &duplicate.envelopes[1],
            &duplicate.keyset,
            FRONTIER_ID,
            2,
            Some(
                &verify_authority_envelope(
                    &duplicate.envelopes[0],
                    &duplicate.keyset,
                    FRONTIER_ID,
                    1,
                    None,
                )
                .unwrap()
                .record_root,
            ),
        )
        .unwrap()
        .record;
        second
            .content
            .event_ids
            .push(second.content.event_ids[0].clone());
        second.record_id = second.derive_id().unwrap();
        assert!(second.validate().is_err());

        let mut wrong_transaction = fixture();
        let mut second = verify_authority_envelope(
            &wrong_transaction.envelopes[1],
            &wrong_transaction.keyset,
            FRONTIER_ID,
            2,
            Some(
                &verify_authority_envelope(
                    &wrong_transaction.envelopes[0],
                    &wrong_transaction.keyset,
                    FRONTIER_ID,
                    1,
                    None,
                )
                .unwrap()
                .record_root,
            ),
        )
        .unwrap()
        .record;
        second.content.transaction_id = "txn_substituted".into();
        second.record_id = second.derive_id().unwrap();
        wrong_transaction.resign_record(1, second);
        assert!(verify_authority_history(wrong_transaction.input()).is_err());
    }

    #[test]
    fn wrong_root_fork_and_policy_substitution_fail_closed() {
        let mut wrong_root = fixture();
        let mut second = verify_authority_envelope(
            &wrong_root.envelopes[1],
            &wrong_root.keyset,
            FRONTIER_ID,
            2,
            Some(
                &verify_authority_envelope(
                    &wrong_root.envelopes[0],
                    &wrong_root.keyset,
                    FRONTIER_ID,
                    1,
                    None,
                )
                .unwrap()
                .record_root,
            ),
        )
        .unwrap()
        .record;
        second.content.after_event_log_root = root('2');
        second.record_id = second.derive_id().unwrap();
        wrong_root.resign_record(1, second);
        assert!(verify_authority_history(wrong_root.input()).is_err());

        let mut fork = fixture();
        let mut second = verify_authority_envelope(
            &fork.envelopes[1],
            &fork.keyset,
            FRONTIER_ID,
            2,
            Some(
                &verify_authority_envelope(&fork.envelopes[0], &fork.keyset, FRONTIER_ID, 1, None)
                    .unwrap()
                    .record_root,
            ),
        )
        .unwrap()
        .record;
        second.content.previous_authority_record_root = Some(root('5'));
        second.record_id = second.derive_id().unwrap();
        fork.resign_record(1, second);
        assert!(verify_authority_history(fork.input()).is_err());

        let mut substituted = fixture();
        substituted.bundle.policies_root = root('1');
        assert!(verify_authority_history(substituted.input()).is_err());
    }

    #[test]
    fn migration_payload_rejects_unknown_or_incomplete_fields() {
        let fixture = fixture();
        let mut event = fixture.legacy_events[1].clone();
        event.payload["timestamp_cutoff"] = json!("2026-07-24T12:00:00Z");
        assert!(migration_payload_from_event(&event).is_err());

        let mut event = fixture.legacy_events[1].clone();
        event.payload["minimum_writer_version"] = json!("");
        event.id = compute_event_id(&event);
        assert!(migration_payload_from_event(&event).is_err());
    }
}
