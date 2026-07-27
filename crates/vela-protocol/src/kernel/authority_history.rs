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
    VerifiedAuthorityRecord, verify_authority_envelope, verify_authority_keyset_transition,
    verify_policy_bundle_transition,
};
use crate::canonical::{sha256_canonical, to_canonical_bytes};
use crate::events::{
    EVENT_KIND_AUTHORITY_MODEL_MIGRATED, NULL_HASH, StateEvent, compute_event_id, event_log_hash,
};
use crate::sign::{ActorRecord, verify_event_signature};

pub const AUTHORITY_MODEL_MIGRATION_SCHEMA_V1: &str = "vela.authority-model-migration.v1";
pub const AUTHORITY_INITIALIZATION_SCHEMA_V1: &str = "vela.authority-initialization.v1";
pub const AUTHORITY_EVENT_LOG_SCHEMA_V1: &str = "vela.authority-event-log.v1";
pub const AUTHORITY_MIGRATION_ACTION: &str = "authority_model_migrate";
pub const AUTHORITY_INITIALIZE_ACTION: &str = "authority_initialize";
pub const AUTHORITY_INITIALIZED_EVENT_KIND: &str = "authority.initialized";
pub const AUTHORITY_ROTATE_ACTION: &str = "authority_rotate";
pub const AUTHORITY_CLOSE_ACTION: &str = "authority_close";
pub const POLICY_ROTATE_ACTION: &str = "policy_rotate";
pub const AUTHORITY_CLOSE_SCHEMA_V1: &str = "vela.authority-close.v1";
pub const AUTHORITY_CLOSED_EVENT_KIND: &str = "authority.closed";

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityCloseV1 {
    pub schema: String,
    pub frontier_id: String,
    pub last_trusted_sequence: u64,
    pub last_trusted_authority_record_root: String,
    pub previous_authority_keyset_root: String,
    pub closed_authority_keyset_root: String,
    pub policy_bundle_root: String,
    pub incident_id: String,
    pub reason: String,
}

impl AuthorityCloseV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORITY_CLOSE_SCHEMA_V1 {
            return Err(format!(
                "authority close schema must be {AUTHORITY_CLOSE_SCHEMA_V1}"
            ));
        }
        require_frontier(&self.frontier_id)?;
        for (name, root) in [
            (
                "last_trusted_authority_record_root",
                self.last_trusted_authority_record_root.as_str(),
            ),
            (
                "previous_authority_keyset_root",
                self.previous_authority_keyset_root.as_str(),
            ),
            (
                "closed_authority_keyset_root",
                self.closed_authority_keyset_root.as_str(),
            ),
            ("policy_bundle_root", self.policy_bundle_root.as_str()),
        ] {
            require_sha256_root(name, root)?;
        }
        if self.incident_id.trim().is_empty() || self.reason.trim().is_empty() {
            return Err("authority close incident and reason must be non-empty".into());
        }
        Ok(())
    }
}

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

/// Fresh-repository bootstrap for Era-1 authority.
///
/// Unlike [`AuthorityModelMigrationV1`], this object has no legacy signer and
/// grants no exemption to historical events. It is valid only over the exact
/// one-event Profile v1 skeleton produced by `vela init`, with an empty actor
/// registry. The covering sequence-1 authority record proves possession of the
/// selected repository key; consumers still pin the resulting full authority
/// root through their ordinary distribution trust path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityInitializationV1 {
    pub schema: String,
    pub frontier_id: String,
    pub initial_event_log_root: String,
    pub initial_actor_registry_root: String,
    pub new_authority_keyset_root: String,
    pub new_policy_bundle_root: String,
    pub new_principal_id: String,
    pub minimum_writer_version: String,
    pub reason: String,
}

impl AuthorityInitializationV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORITY_INITIALIZATION_SCHEMA_V1 {
            return Err(format!(
                "authority initialization schema must be {AUTHORITY_INITIALIZATION_SCHEMA_V1}"
            ));
        }
        require_frontier(&self.frontier_id)?;
        for (name, root) in [
            (
                "initial_event_log_root",
                self.initial_event_log_root.as_str(),
            ),
            (
                "initial_actor_registry_root",
                self.initial_actor_registry_root.as_str(),
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
                "authority initialization principal, minimum writer version, and reason must be non-empty"
                    .into(),
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization_event_id: Option<String>,
    pub final_event_log_root: String,
    pub final_authority_record_root: Option<String>,
    pub final_authority_keyset_root: Option<String>,
    pub final_policy_bundle_root: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub closed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure_event_id: Option<String>,
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
    pub authority_keysets: &'a [AuthorityKeysetV1],
    pub policy_bundles: &'a [PolicyBundleV1],
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

    if migrations.is_empty()
        && input.authority_events.is_empty()
        && input.authority_envelopes.is_empty()
    {
        return Ok(AuthorityHistoryVerification {
            era: AuthorityHistoryEra::LegacyOnly,
            frontier_id: input.frontier_id.into(),
            legacy_event_count: input.legacy_events.len(),
            authority_event_count: 0,
            authority_record_count: 0,
            migration_event_id: None,
            initialization_event_id: None,
            final_event_log_root: prefixed_legacy_root(input.legacy_events),
            final_authority_record_root: None,
            final_authority_keyset_root: None,
            final_policy_bundle_root: None,
            closed: false,
            closure_event_id: None,
        });
    }
    if migrations.len() > 1 {
        return Err("authority history must contain exactly one migration bridge".into());
    }
    if input.authority_envelopes.is_empty() {
        return Err("repository-authority history has no covering authority record".into());
    }

    let authority_keysets = index_authority_keysets(input.frontier_id, input.authority_keysets)?;
    let policy_bundles = index_policy_bundles(input.frontier_id, input.policy_bundles)?;

    enum Boundary<'a> {
        Migration {
            event: &'a StateEvent,
            payload: AuthorityModelMigrationV1,
            legacy_prefix: Vec<StateEvent>,
        },
        Initialization {
            event: &'a AuthorityEventV1,
            payload: AuthorityInitializationV1,
        },
    }

    let boundary = if let Some(migration_event) = migrations.first().copied() {
        let legacy_prefix = input
            .legacy_events
            .iter()
            .filter(|event| event.id != migration_event.id)
            .cloned()
            .collect::<Vec<_>>();
        Boundary::Migration {
            event: migration_event,
            payload: migration_payload_from_event(migration_event)?,
            legacy_prefix,
        }
    } else {
        let initializations = input
            .authority_events
            .iter()
            .filter(|event| event.content.kind.as_str() == AUTHORITY_INITIALIZED_EVENT_KIND)
            .collect::<Vec<_>>();
        let [initialization_event] = initializations.as_slice() else {
            return Err(
                "Era-1 history without a migration bridge must contain exactly one fresh authority initialization"
                    .into(),
            );
        };
        Boundary::Initialization {
            event: initialization_event,
            payload: initialization_payload_from_event(initialization_event)?,
        }
    };

    let (mut active_keyset_root, mut active_policy_root) = match &boundary {
        Boundary::Migration { payload, .. } => (
            payload.new_authority_keyset_root.clone(),
            payload.new_policy_bundle_root.clone(),
        ),
        Boundary::Initialization { payload, .. } => (
            payload.new_authority_keyset_root.clone(),
            payload.new_policy_bundle_root.clone(),
        ),
    };
    let mut active_keyset = authority_keysets
        .get(&active_keyset_root)
        .copied()
        .ok_or_else(|| "initial authority keyset is not retained".to_string())?;
    let mut active_policy = policy_bundles
        .get(&active_policy_root)
        .copied()
        .ok_or_else(|| "initial policy bundle is not retained".to_string())?;
    match &boundary {
        Boundary::Migration {
            event,
            legacy_prefix,
            ..
        } => {
            verify_authority_migration_bridge(
                input.frontier_id,
                legacy_prefix,
                input.legacy_actor_registry_bytes,
                input.legacy_active_policy_head_root,
                input.legacy_policy_store_manifest_root,
                active_keyset,
                active_policy,
                event,
            )?;
        }
        Boundary::Initialization { event, .. } => {
            verify_authority_initialization(
                input.frontier_id,
                input.legacy_events,
                input.legacy_actor_registry_bytes,
                active_keyset,
                active_policy,
                event,
            )?;
        }
    }
    if active_keyset.generation != 1
        || active_keyset.previous_keyset_root.is_some()
        || active_keyset.activation_record_root.is_some()
        || active_policy.previous_bundle_root.is_some()
    {
        return Err(
            "authority boundary must activate initial keyset and policy generations".into(),
        );
    }
    let mut activated_keysets = BTreeSet::from([active_keyset_root.clone()]);
    let mut activated_policies = BTreeSet::from([active_policy_root.clone()]);

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

    let legacy_event_root = prefixed_legacy_root(input.legacy_events);
    let base_event_root = match &boundary {
        Boundary::Migration { payload, .. } => payload.legacy_event_log_root.clone(),
        Boundary::Initialization { payload, .. } => payload.initial_event_log_root.clone(),
    };
    let mut current_event_root = base_event_root;
    let mut previous_record_root: Option<String> = None;
    let mut covered_era_one: BTreeSet<String> = BTreeSet::new();
    let mut cumulative_era_one = Vec::new();
    let mut verified_records = Vec::new();
    let mut closed = false;
    let mut closure_event_id = None;

    for (offset, envelope) in input.authority_envelopes.iter().enumerate() {
        if closed {
            return Err("authority history continues after its terminal close".into());
        }
        let sequence = u64::try_from(offset + 1)
            .map_err(|_| "authority record sequence exceeds u64".to_string())?;
        let verified = verify_authority_envelope(
            envelope,
            active_keyset,
            input.frontier_id,
            sequence,
            previous_record_root.as_deref(),
        )?;
        verify_record_authorization(&verified, active_policy)?;
        if verified.record.content.before_event_log_root != current_event_root {
            return Err(format!(
                "authority record {sequence} has the wrong before-event root"
            ));
        }

        if sequence == 1 {
            match &boundary {
                Boundary::Migration { event, payload, .. } => {
                    verify_first_record(&verified, event, payload, &legacy_event_root)?;
                    current_event_root = legacy_event_root.clone();
                }
                Boundary::Initialization { event, payload } => {
                    verify_first_initialization_record(
                        &verified,
                        event,
                        payload,
                        &legacy_event_root,
                    )?;
                    covered_era_one.insert(event.id.clone());
                    cumulative_era_one.push(*event);
                    current_event_root =
                        authority_event_log_root(&legacy_event_root, &cumulative_era_one)?;
                }
            }
        } else {
            let transaction_id = verified.record.content.transaction_id.as_str();
            let actual_ids: BTreeSet<&str> = verified
                .record
                .content
                .event_ids
                .iter()
                .map(String::as_str)
                .collect();
            match era_one_by_transaction.get(transaction_id) {
                Some(expected_ids) if &actual_ids == expected_ids => {}
                None if actual_ids.is_empty() => {}
                Some(_) => {
                    return Err(format!(
                        "authority record {sequence} does not exactly cover its transaction events"
                    ));
                }
                None => {
                    return Err(format!(
                        "authority record {sequence} references an unknown transaction"
                    ));
                }
            }
            let mut transaction_events = Vec::new();
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
                transaction_events.push(event);
            }
            let expected_after = if transaction_events.is_empty() {
                current_event_root.clone()
            } else {
                authority_event_log_root(&legacy_event_root, &cumulative_era_one)?
            };
            if verified.record.content.after_event_log_root != expected_after {
                return Err(format!(
                    "authority record {sequence} has the wrong after-event root"
                ));
            }
            current_event_root = expected_after;

            let previous_keyset_root = active_keyset_root.clone();
            let previous_policy_root = active_policy_root.clone();
            let next_keyset = keyset_transition_for_record(
                &verified,
                active_keyset,
                &authority_keysets,
                sequence
                    .checked_add(1)
                    .ok_or_else(|| "authority keyset activation sequence overflows".to_string())?,
            )?;
            let next_policy =
                policy_transition_for_record(&verified, active_policy, &policy_bundles)?;
            if next_keyset.is_some_and(|next| next.closed) && next_policy.is_some() {
                return Err("terminal authority close cannot also activate a policy bundle".into());
            }
            if let Some(next) = next_keyset {
                active_keyset_root = next.root()?;
                if !activated_keysets.insert(active_keyset_root.clone()) {
                    return Err("authority keyset generation was activated more than once".into());
                }
                active_keyset = next;
            }
            if let Some(next) = next_policy {
                active_policy_root = next.root()?;
                if !activated_policies.insert(active_policy_root.clone()) {
                    return Err("policy bundle generation was activated more than once".into());
                }
                active_policy = next;
            }
            if active_keyset.closed {
                let event_id = verify_authority_close_record(
                    &verified,
                    &transaction_events,
                    sequence,
                    &previous_keyset_root,
                    &active_keyset_root,
                    &previous_policy_root,
                )?;
                closed = true;
                closure_event_id = Some(event_id);
            }
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
    if activated_keysets
        != authority_keysets
            .keys()
            .cloned()
            .collect::<BTreeSet<String>>()
    {
        return Err("retained authority keyset store contains an unactivated generation".into());
    }
    if activated_policies != policy_bundles.keys().cloned().collect::<BTreeSet<String>>() {
        return Err("retained policy store contains an unactivated generation".into());
    }

    Ok(AuthorityHistoryVerification {
        era: AuthorityHistoryEra::RepositoryAuthority,
        frontier_id: input.frontier_id.into(),
        legacy_event_count: input.legacy_events.len(),
        authority_event_count: input.authority_events.len(),
        authority_record_count: verified_records.len(),
        migration_event_id: match &boundary {
            Boundary::Migration { event, .. } => Some(event.id.clone()),
            Boundary::Initialization { .. } => None,
        },
        initialization_event_id: match &boundary {
            Boundary::Initialization { event, .. } => Some(event.id.clone()),
            Boundary::Migration { .. } => None,
        },
        final_event_log_root: current_event_root,
        final_authority_record_root: previous_record_root,
        final_authority_keyset_root: Some(active_keyset_root),
        final_policy_bundle_root: Some(active_policy_root),
        closed,
        closure_event_id,
    })
}

fn index_authority_keysets<'a>(
    frontier_id: &str,
    keysets: &'a [AuthorityKeysetV1],
) -> Result<BTreeMap<String, &'a AuthorityKeysetV1>, String> {
    let mut indexed = BTreeMap::new();
    for keyset in keysets {
        keyset.validate()?;
        if keyset.frontier_id != frontier_id {
            return Err("retained authority keyset names a different Frontier".into());
        }
        let root = keyset.root()?;
        if indexed.insert(root, keyset).is_some() {
            return Err("retained authority keyset store contains a duplicate root".into());
        }
    }
    Ok(indexed)
}

fn index_policy_bundles<'a>(
    frontier_id: &str,
    bundles: &'a [PolicyBundleV1],
) -> Result<BTreeMap<String, &'a PolicyBundleV1>, String> {
    let mut indexed = BTreeMap::new();
    for bundle in bundles {
        bundle.validate()?;
        if bundle.frontier_id != frontier_id {
            return Err("retained policy bundle names a different Frontier".into());
        }
        let root = bundle.root()?;
        if indexed.insert(root, bundle).is_some() {
            return Err("retained policy store contains a duplicate root".into());
        }
    }
    Ok(indexed)
}

fn keyset_transition_for_record<'a>(
    verified: &VerifiedAuthorityRecord,
    current: &AuthorityKeysetV1,
    retained: &BTreeMap<String, &'a AuthorityKeysetV1>,
    activation_sequence: u64,
) -> Result<Option<&'a AuthorityKeysetV1>, String> {
    let deltas = verified
        .record
        .content
        .object_delta
        .iter()
        .filter(|delta| delta.object_kind == "authority_keyset")
        .collect::<Vec<_>>();
    if deltas.is_empty() {
        return Ok(None);
    }
    if deltas.len() != 1 {
        return Err("one authority record cannot activate multiple keysets".into());
    }
    let delta = deltas[0];
    if delta.before_root.is_some() {
        return Err("authority keyset snapshots are immutable and content addressed".into());
    }
    let root = delta
        .after_root
        .as_ref()
        .ok_or_else(|| "authority keyset transition cannot delete its snapshot".to_string())?;
    let stem = root
        .strip_prefix("sha256:")
        .ok_or_else(|| "authority keyset transition root lacks sha256 tag".to_string())?;
    if delta.path != format!(".vela/authority/keysets/{stem}.json") {
        return Err("authority keyset transition path does not match its full root".into());
    }
    let next = retained
        .get(root)
        .copied()
        .ok_or_else(|| "authority keyset transition snapshot is not retained".to_string())?;
    let required_action = if next.closed {
        AUTHORITY_CLOSE_ACTION
    } else {
        AUTHORITY_ROTATE_ACTION
    };
    if !verified
        .record
        .content
        .semantic_approvals
        .iter()
        .any(|approval| approval.action == required_action)
    {
        return Err(format!(
            "authority keyset transition lacks {required_action} approval"
        ));
    }
    let previous_record_root = verified
        .record
        .content
        .previous_authority_record_root
        .as_deref()
        .ok_or_else(|| "authority keyset transition lacks its prior chain head".to_string())?;
    verify_authority_keyset_transition(current, next, activation_sequence, previous_record_root)?;
    Ok(Some(next))
}

fn policy_transition_for_record<'a>(
    verified: &VerifiedAuthorityRecord,
    current: &PolicyBundleV1,
    retained: &BTreeMap<String, &'a PolicyBundleV1>,
) -> Result<Option<&'a PolicyBundleV1>, String> {
    let deltas = verified
        .record
        .content
        .object_delta
        .iter()
        .filter(|delta| delta.object_kind == "policy_bundle")
        .collect::<Vec<_>>();
    if deltas.is_empty() {
        return Ok(None);
    }
    if deltas.len() != 1 {
        return Err("one authority record cannot activate multiple policy bundles".into());
    }
    if !verified
        .record
        .content
        .semantic_approvals
        .iter()
        .any(|approval| approval.action == POLICY_ROTATE_ACTION)
    {
        return Err("policy bundle transition lacks policy_rotate approval".into());
    }
    let delta = deltas[0];
    if delta.before_root.is_some() {
        return Err("policy bundle snapshots are immutable and content addressed".into());
    }
    let root = delta
        .after_root
        .as_ref()
        .ok_or_else(|| "policy bundle transition cannot delete its snapshot".to_string())?;
    let stem = root
        .strip_prefix("sha256:")
        .ok_or_else(|| "policy bundle transition root lacks sha256 tag".to_string())?;
    if delta.path != format!(".vela/authority/policies/{stem}.json") {
        return Err("policy bundle transition path does not match its full root".into());
    }
    let next = retained
        .get(root)
        .copied()
        .ok_or_else(|| "policy bundle transition snapshot is not retained".to_string())?;
    verify_policy_bundle_transition(current, next)?;
    Ok(Some(next))
}

fn verify_authority_close_record(
    verified: &VerifiedAuthorityRecord,
    transaction_events: &[&AuthorityEventV1],
    sequence: u64,
    previous_keyset_root: &str,
    closed_keyset_root: &str,
    policy_bundle_root: &str,
) -> Result<String, String> {
    if transaction_events.len() != 1 || verified.record.content.object_delta.len() != 2 {
        return Err(
            "terminal authority close must cover exactly one event and one closed keyset".into(),
        );
    }
    let event = transaction_events[0];
    if event.content.kind.as_str() != AUTHORITY_CLOSED_EVENT_KIND
        || event.content.target.r#type != "frontier"
        || event.content.target.id != verified.record.content.frontier_id
        || event.content.actor.r#type != "human"
        || event.content.before_hash != event.content.after_hash
    {
        return Err("authority close event shape is invalid".into());
    }
    let payload: AuthorityCloseV1 = serde_json::from_value(event.content.payload.clone())
        .map_err(|error| format!("authority close payload is invalid: {error}"))?;
    payload.validate()?;
    let previous_record_root = verified
        .record
        .content
        .previous_authority_record_root
        .as_deref()
        .ok_or_else(|| "authority close lacks its prior chain head".to_string())?;
    if payload.frontier_id != verified.record.content.frontier_id
        || payload.last_trusted_sequence != sequence.saturating_sub(1)
        || payload.last_trusted_authority_record_root != previous_record_root
        || payload.previous_authority_keyset_root != previous_keyset_root
        || payload.closed_authority_keyset_root != closed_keyset_root
        || payload.policy_bundle_root != policy_bundle_root
        || payload.reason != event.content.reason
    {
        return Err("authority close payload does not match its exact terminal transition".into());
    }
    Ok(event.id.clone())
}

/// Verify the one legacy-signed bridge before a sequence-1 writer may access
/// the repository-authority signer.
///
/// The candidate is checked against the exact retained Era-0 prefix, actor
/// registry, legacy policy state, and proposed Era-1 keyset and policy bundle.
/// This function performs no write and requires no live identity provider.
pub fn verify_authority_migration_bridge(
    frontier_id: &str,
    legacy_prefix: &[StateEvent],
    legacy_actor_registry_bytes: &[u8],
    legacy_active_policy_head_root: &str,
    legacy_policy_store_manifest_root: &str,
    authority_keyset: &AuthorityKeysetV1,
    policy_bundle: &PolicyBundleV1,
    migration_event: &StateEvent,
) -> Result<AuthorityModelMigrationV1, String> {
    require_frontier(frontier_id)?;
    if legacy_prefix
        .iter()
        .any(|event| event.kind.as_str() == EVENT_KIND_AUTHORITY_MODEL_MIGRATED)
    {
        return Err("pre-migration history already contains a migration bridge".into());
    }
    let mut legacy_ids = BTreeSet::new();
    for event in legacy_prefix {
        if event.id != compute_event_id(event) || !legacy_ids.insert(event.id.as_str()) {
            return Err(format!(
                "legacy event {} has an invalid or duplicate content address",
                event.id
            ));
        }
    }

    let migration = migration_payload_from_event(migration_event)?;
    if legacy_ids.contains(migration_event.id.as_str()) {
        return Err("migration bridge duplicates an Era-0 event identity".into());
    }
    if migration.frontier_id != frontier_id {
        return Err("migration bridge targets a different frontier".into());
    }
    if migration.legacy_event_log_root != prefixed_legacy_root(legacy_prefix) {
        return Err(
            "legacy event-log root does not match the exact pre-migration history; a legacy write may have occurred after migration"
                .into(),
        );
    }
    if migration.legacy_active_policy_head_root != legacy_active_policy_head_root
        || migration.legacy_policy_store_manifest_root != legacy_policy_store_manifest_root
    {
        return Err("migration bridge does not bind the supplied legacy policy state".into());
    }

    verify_legacy_migration_signature(migration_event, legacy_actor_registry_bytes, &migration)?;

    authority_keyset.validate()?;
    policy_bundle.validate()?;
    if authority_keyset.frontier_id != frontier_id
        || policy_bundle.frontier_id != frontier_id
        || migration.new_authority_keyset_root != authority_keyset.root()?
        || migration.new_policy_bundle_root != policy_bundle.root()?
    {
        return Err("migration bridge does not bind the supplied Era-1 authority inputs".into());
    }
    Ok(migration)
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

/// Verify a fresh Profile v1 authority boundary.
///
/// Fresh initialization is deliberately narrower than migration: the retained
/// legacy side must be exactly the unsigned structural `frontier.created`
/// event and an empty actor registry. No historical scientific event can gain
/// authority through this path.
pub fn verify_authority_initialization(
    frontier_id: &str,
    initial_events: &[StateEvent],
    initial_actor_registry_bytes: &[u8],
    authority_keyset: &AuthorityKeysetV1,
    policy_bundle: &PolicyBundleV1,
    initialization_event: &AuthorityEventV1,
) -> Result<AuthorityInitializationV1, String> {
    require_frontier(frontier_id)?;
    let [created] = initial_events else {
        return Err(
            "fresh authority initialization requires exactly one structural frontier.created event"
                .into(),
        );
    };
    if created.kind.as_str() != "frontier.created"
        || created.id != compute_event_id(created)
        || created.signature.is_some()
    {
        return Err(
            "fresh authority initialization requires the exact unsigned frontier.created event"
                .into(),
        );
    }
    let actors: Vec<ActorRecord> = serde_json::from_slice(initial_actor_registry_bytes)
        .map_err(|error| format!("fresh actor registry is invalid: {error}"))?;
    if !actors.is_empty() {
        return Err("fresh authority initialization requires an empty actor registry".into());
    }
    let payload = initialization_payload_from_event(initialization_event)?;
    if payload.frontier_id != frontier_id
        || payload.initial_event_log_root != prefixed_legacy_root(initial_events)
        || payload.initial_actor_registry_root != sha256_bytes(initial_actor_registry_bytes)
    {
        return Err(
            "fresh authority initialization does not bind the exact structural state".into(),
        );
    }
    authority_keyset.validate()?;
    policy_bundle.validate()?;
    if authority_keyset.frontier_id != frontier_id
        || policy_bundle.frontier_id != frontier_id
        || payload.new_authority_keyset_root != authority_keyset.root()?
        || payload.new_policy_bundle_root != policy_bundle.root()?
    {
        return Err(
            "fresh authority initialization does not bind its initial authority inputs".into(),
        );
    }
    Ok(payload)
}

pub fn initialization_payload_from_event(
    event: &AuthorityEventV1,
) -> Result<AuthorityInitializationV1, String> {
    event.validate()?;
    if event.content.kind.as_str() != AUTHORITY_INITIALIZED_EVENT_KIND
        || event.content.target.r#type != "frontier"
        || event.content.actor.r#type != "human"
        || event.content.before_hash != NULL_HASH
        || event.content.after_hash != NULL_HASH
    {
        return Err("authority initialization event shape is invalid".into());
    }
    let payload: AuthorityInitializationV1 = serde_json::from_value(event.content.payload.clone())
        .map_err(|error| format!("authority initialization payload is invalid: {error}"))?;
    payload.validate()?;
    if event.content.target.id != payload.frontier_id
        || event.content.reason != payload.reason
        || event.content.principal_id != payload.new_principal_id
        || event.content.actor.id != payload.new_principal_id
    {
        return Err("authority initialization event does not match its payload".into());
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
    let migration_event_root = canonical_object_root(migration_event)?;
    if record.content.event_ids != [migration_event.id.clone()]
        || record.content.after_event_log_root != legacy_root_with_bridge
        || record.content.principal.principal_id != migration.new_principal_id
        || record.content.intent_digest != migration_event_root
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
    verify_initial_migration_object_delta(
        verified,
        migration_event,
        &migration_event_root,
        &migration.new_authority_keyset_root,
        &migration.new_policy_bundle_root,
    )
}

fn verify_initial_migration_object_delta(
    verified: &VerifiedAuthorityRecord,
    migration_event: &StateEvent,
    migration_event_root: &str,
    authority_keyset_root: &str,
    policy_bundle_root: &str,
) -> Result<(), String> {
    if verified.record.content.object_delta.len() != 3 {
        return Err("authority record 1 must contain exactly three initial object deltas".into());
    }
    let keyset_stem = authority_keyset_root
        .strip_prefix("sha256:")
        .ok_or_else(|| "initial authority keyset root lacks sha256 tag".to_string())?;
    let policy_stem = policy_bundle_root
        .strip_prefix("sha256:")
        .ok_or_else(|| "initial policy bundle root lacks sha256 tag".to_string())?;
    let expected = [
        (
            format!(".vela/events/{}.json", migration_event.id),
            migration_event_root,
            "event",
        ),
        (
            format!(".vela/authority/keysets/{keyset_stem}.json"),
            authority_keyset_root,
            "authority_keyset",
        ),
        (
            format!(".vela/authority/policies/{policy_stem}.json"),
            policy_bundle_root,
            "policy_bundle",
        ),
    ];
    for (path, root, kind) in expected {
        let matches = verified
            .record
            .content
            .object_delta
            .iter()
            .filter(|delta| {
                delta.path == path
                    && delta.before_root.is_none()
                    && delta.after_root.as_deref() == Some(root)
                    && delta.object_kind == kind
            })
            .count();
        if matches != 1 {
            return Err(format!(
                "authority record 1 lacks one exact initial object delta for {path}"
            ));
        }
    }
    Ok(())
}

fn verify_first_initialization_record(
    verified: &VerifiedAuthorityRecord,
    initialization_event: &AuthorityEventV1,
    initialization: &AuthorityInitializationV1,
    initial_event_log_root: &str,
) -> Result<(), String> {
    let record = &verified.record;
    let event_root = initialization_event.root()?;
    let initialization_intent = format!("sha256:{}", sha256_canonical(initialization)?);
    let expected_after = authority_event_log_root(initial_event_log_root, &[initialization_event])?;
    if record.content.event_ids != [initialization_event.id.clone()]
        || record.content.before_event_log_root != initial_event_log_root
        || record.content.after_event_log_root != expected_after
        || record.content.principal.principal_id != initialization.new_principal_id
        || record.content.intent_digest != initialization_intent
        || initialization_event.content.transaction_id != record.content.transaction_id
    {
        return Err("authority record 1 does not exactly cover fresh initialization".into());
    }
    let approval = record.content.semantic_approvals.iter().find(|approval| {
        approval.principal_id == initialization.new_principal_id
            && approval.action == AUTHORITY_INITIALIZE_ACTION
            && approval.reason == initialization.reason
            && approval.intent_digest == record.content.intent_digest
    });
    if approval.is_none() {
        return Err("authority record 1 lacks the exact initialization approval".into());
    }
    let event_path = format!(".vela/authority/events/{}.json", initialization_event.id);
    let event_matches = record
        .content
        .object_delta
        .iter()
        .filter(|delta| {
            delta.path == event_path
                && delta.before_root.is_none()
                && delta.after_root.as_deref() == Some(event_root.as_str())
                && delta.object_kind == "event"
        })
        .count();
    if event_matches != 1 {
        return Err("authority record 1 lacks one exact fresh initialization event delta".into());
    }
    verify_initial_snapshot_delta(
        verified,
        &initialization.new_authority_keyset_root,
        &initialization.new_policy_bundle_root,
    )
}

fn verify_initial_snapshot_delta(
    verified: &VerifiedAuthorityRecord,
    authority_keyset_root: &str,
    policy_bundle_root: &str,
) -> Result<(), String> {
    let keyset_stem = authority_keyset_root
        .strip_prefix("sha256:")
        .ok_or_else(|| "initial authority keyset root lacks sha256 tag".to_string())?;
    let policy_stem = policy_bundle_root
        .strip_prefix("sha256:")
        .ok_or_else(|| "initial policy bundle root lacks sha256 tag".to_string())?;
    let expected = [
        (
            format!(".vela/authority/keysets/{keyset_stem}.json"),
            authority_keyset_root,
            "authority_keyset",
        ),
        (
            format!(".vela/authority/policies/{policy_stem}.json"),
            policy_bundle_root,
            "policy_bundle",
        ),
    ];
    for (path, root, kind) in expected {
        let matches = verified
            .record
            .content
            .object_delta
            .iter()
            .filter(|delta| {
                delta.path == path
                    && delta.before_root.is_none()
                    && delta.after_root.as_deref() == Some(root)
                    && delta.object_kind == kind
            })
            .count();
        if matches != 1 {
            return Err(format!(
                "authority record 1 lacks one exact initial object delta for {path}"
            ));
        }
    }
    Ok(())
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
        || !evaluation.diagnostics.is_empty()
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
    let expected_path = if verified.record.content.sequence == 1 {
        format!(".vela/events/{event_id}.json")
    } else {
        format!(".vela/authority/events/{event_id}.json")
    };
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
        AUTHORITY_PAYLOAD_TYPE_V1, AuthenticationAssurance, AuthenticationClaimV1,
        AuthenticationMethod, AuthorityEventContentV1, AuthorityKeyV1, AuthorityRecordContentV1,
        AuthorityRecordV1, AuthorizationClaimV1, CedarEvaluation, DelegationClaimV1,
        DsseSignatureV1, ExecutionClaimV1, ObjectDeltaV1, POLICY_BUNDLE_SCHEMA_V1, PrincipalClass,
        PrincipalSnapshotV1, SemanticApprovalV1, dsse_pae,
    };
    use crate::events::{EVENT_SCHEMA, EventKind, StateActor, StateTarget, compute_event_id};

    const FRONTIER_ID: &str = "vfr_0123456789abcdef";
    const LEGACY_ACTOR: &str = "reviewer:legacy";
    const REPOSITORY_PRINCIPAL: &str = "oidc:https://github.com|1234567";

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
            self.input_with_stores(
                std::slice::from_ref(&self.keyset),
                std::slice::from_ref(&self.bundle),
            )
        }

        fn input_with_stores<'a>(
            &'a self,
            authority_keysets: &'a [AuthorityKeysetV1],
            policy_bundles: &'a [PolicyBundleV1],
        ) -> AuthorityHistoryInput<'a> {
            AuthorityHistoryInput {
                frontier_id: FRONTIER_ID,
                legacy_events: &self.legacy_events,
                legacy_actor_registry_bytes: &self.actor_registry_bytes,
                legacy_active_policy_head_root: &self.legacy_active_policy_head_root,
                legacy_policy_store_manifest_root: &self.legacy_policy_store_manifest_root,
                authority_keysets,
                policy_bundles,
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
            closed: false,
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
        let migration_event_root = canonical_object_root(&migration).unwrap();
        let keyset_root = keyset.root().unwrap();
        let bundle_root = bundle.root().unwrap();

        let first = record(
            1,
            None,
            "txn_migration",
            &migration_event_root,
            &legacy_root,
            &legacy_root_with_bridge,
            vec![migration.id.clone()],
            vec![
                event_delta(1, &migration.id, &migration_event_root),
                snapshot_delta(".vela/authority/keysets", &keyset_root, "authority_keyset"),
                snapshot_delta(".vela/authority/policies", &bundle_root, "policy_bundle"),
            ],
            &keyset,
            &bundle,
            vec![SemanticApprovalV1 {
                principal_id: LEGACY_ACTOR.into(),
                role: "frontier_administrator".into(),
                action: AUTHORITY_MIGRATION_ACTION.into(),
                reason: migration_payload.reason,
                approved_at: "2026-07-24T12:00:00Z".into(),
                intent_digest: migration_event_root.clone(),
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
            &root('e'),
            &legacy_root_with_bridge,
            &final_root,
            vec![era_one.id.clone()],
            vec![event_delta(2, &era_one.id, &era_one.root().unwrap())],
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
        intent_digest: &str,
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
            intent_digest: intent_digest.into(),
            before_event_log_root: before_root.into(),
            after_event_log_root: after_root.into(),
            event_ids,
            object_delta,
            principal: PrincipalSnapshotV1 {
                principal_id: REPOSITORY_PRINCIPAL.into(),
                principal_class: PrincipalClass::Human,
                display_name: Some("Repository administrator".into()),
                affiliation: None,
                account_links: vec![REPOSITORY_PRINCIPAL.into()],
            },
            authentication: AuthenticationClaimV1 {
                schema: crate::authentication::AUTHENTICATION_OBSERVATION_SCHEMA_V1.into(),
                principal_id: REPOSITORY_PRINCIPAL.into(),
                principal_class: PrincipalClass::Human,
                issuer: "https://github.com".into(),
                subject: "1234567".into(),
                method: AuthenticationMethod::Passkey,
                assurance: AuthenticationAssurance::PhishingResistant,
                session_root: root(if sequence == 1 { '8' } else { '9' }),
                authenticated_at: "2026-07-24T12:00:00Z".into(),
                observed_at: "2026-07-24T12:00:00Z".into(),
                expires_at: "2026-07-24T13:00:00Z".into(),
                user_presence: true,
                user_verification: true,
                recovery_recent: false,
                revocation_ref: None,
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

    fn event_delta(sequence: u64, event_id: &str, event_root: &str) -> ObjectDeltaV1 {
        ObjectDeltaV1 {
            path: if sequence == 1 {
                format!(".vela/events/{event_id}.json")
            } else {
                format!(".vela/authority/events/{event_id}.json")
            },
            before_root: None,
            after_root: Some(event_root.into()),
            object_kind: "event".into(),
        }
    }

    fn snapshot_delta(directory: &str, root: &str, object_kind: &str) -> ObjectDeltaV1 {
        ObjectDeltaV1 {
            path: format!("{directory}/{}.json", root.strip_prefix("sha256:").unwrap()),
            before_root: None,
            after_root: Some(root.into()),
            object_kind: object_kind.into(),
        }
    }

    fn signed_envelope(record: &AuthorityRecordV1, key: &SigningKey) -> AuthorityEnvelopeV1 {
        signed_envelope_with_key_id(record, key, "repository-key-1")
    }

    fn signed_envelope_with_key_id(
        record: &AuthorityRecordV1,
        key: &SigningKey,
        key_id: &str,
    ) -> AuthorityEnvelopeV1 {
        let payload = to_canonical_bytes(record).unwrap();
        let signature = key.sign(&dsse_pae(AUTHORITY_PAYLOAD_TYPE_V1, &payload));
        AuthorityEnvelopeV1 {
            payload_type: AUTHORITY_PAYLOAD_TYPE_V1.into(),
            payload: BASE64_STANDARD.encode(payload),
            signatures: vec![DsseSignatureV1 {
                keyid: key_id.into(),
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
    fn object_only_record_advances_authority_without_advancing_event_history() {
        let mut fixture = fixture();
        let baseline = verify_authority_history(fixture.input()).unwrap();
        let second = verify_authority_envelope(
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
                .record_root,
            ),
        )
        .unwrap();
        let proposal_root = root('d');
        let third = record(
            3,
            Some(second.record_root),
            "txn_pending_submission",
            &root('c'),
            &baseline.final_event_log_root,
            &baseline.final_event_log_root,
            Vec::new(),
            vec![ObjectDeltaV1 {
                path: ".vela/proposals/vpr_object_only.json".into(),
                before_root: None,
                after_root: Some(proposal_root),
                object_kind: "proposal".into(),
            }],
            &fixture.keyset,
            &fixture.bundle,
            Vec::new(),
        );
        fixture
            .envelopes
            .push(signed_envelope(&third, &fixture.repository_key));

        let verified = verify_authority_history(fixture.input()).unwrap();
        assert_eq!(verified.authority_event_count, 1);
        assert_eq!(verified.authority_record_count, 3);
        assert_eq!(verified.final_event_log_root, baseline.final_event_log_root);

        let mut wrong_after_root = third;
        wrong_after_root.content.after_event_log_root = root('1');
        wrong_after_root.record_id = wrong_after_root.derive_id().unwrap();
        fixture.envelopes[2] = signed_envelope(&wrong_after_root, &fixture.repository_key);
        let error = verify_authority_history(fixture.input()).unwrap_err();
        assert!(error.contains("wrong after-event root"), "{error}");
    }

    #[test]
    fn keyset_and_policy_rotation_activate_on_the_following_record() {
        let mut fixture = fixture();
        let initial_keyset = fixture.keyset.clone();
        let initial_bundle = fixture.bundle.clone();
        let first =
            verify_authority_envelope(&fixture.envelopes[0], &initial_keyset, FRONTIER_ID, 1, None)
                .unwrap();
        let second = verify_authority_envelope(
            &fixture.envelopes[1],
            &initial_keyset,
            FRONTIER_ID,
            2,
            Some(&first.record_root),
        )
        .unwrap();
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
                valid_from_sequence: 4,
                valid_through_sequence: None,
                purpose: AUTHORITY_KEY_PURPOSE.into(),
            }],
            previous_keyset_root: Some(initial_keyset.root().unwrap()),
            activation_record_root: Some(second.record_root.clone()),
            closed: false,
        };
        let next_bundle = PolicyBundleV1 {
            policies_root: root('5'),
            tests_root: root('6'),
            previous_bundle_root: Some(initial_bundle.root().unwrap()),
            authority_summary: "Rotated repository authority.".into(),
            ..initial_bundle.clone()
        };
        let rotation_event = AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "txn_rotate".into(),
            principal_id: REPOSITORY_PRINCIPAL.into(),
            authority_mode: crate::authority::AUTHORITY_MODE.into(),
            kind: EventKind::Other("authority.rotated".into()),
            target: StateTarget {
                r#type: "frontier".into(),
                id: FRONTIER_ID.into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: REPOSITORY_PRINCIPAL.into(),
            },
            timestamp: "2026-07-24T12:02:00Z".into(),
            reason: "Rotate repository authority and policy.".into(),
            before_hash: root('f'),
            after_hash: root('f'),
            payload: json!({
                "authority_keyset_root": next_keyset.root().unwrap(),
                "policy_bundle_root": next_bundle.root().unwrap()
            }),
            caveats: Vec::new(),
        })
        .unwrap();
        let legacy_root_with_bridge = prefixed_legacy_root(&fixture.legacy_events);
        let current_root = verify_authority_history(fixture.input())
            .unwrap()
            .final_event_log_root;
        let rotation_root = authority_event_log_root(
            &legacy_root_with_bridge,
            &[&fixture.authority_events[0], &rotation_event],
        )
        .unwrap();
        let rotation_intent = root('5');
        let third = record(
            3,
            Some(second.record_root),
            "txn_rotate",
            &rotation_intent,
            &current_root,
            &rotation_root,
            vec![rotation_event.id.clone()],
            vec![
                event_delta(3, &rotation_event.id, &rotation_event.root().unwrap()),
                snapshot_delta(
                    ".vela/authority/keysets",
                    &next_keyset.root().unwrap(),
                    "authority_keyset",
                ),
                snapshot_delta(
                    ".vela/authority/policies",
                    &next_bundle.root().unwrap(),
                    "policy_bundle",
                ),
            ],
            &initial_keyset,
            &initial_bundle,
            vec![
                SemanticApprovalV1 {
                    principal_id: REPOSITORY_PRINCIPAL.into(),
                    role: "frontier_administrator".into(),
                    action: AUTHORITY_ROTATE_ACTION.into(),
                    reason: "Rotate the repository keyset.".into(),
                    approved_at: "2026-07-24T12:02:00Z".into(),
                    intent_digest: rotation_intent.clone(),
                },
                SemanticApprovalV1 {
                    principal_id: REPOSITORY_PRINCIPAL.into(),
                    role: "frontier_administrator".into(),
                    action: POLICY_ROTATE_ACTION.into(),
                    reason: "Rotate the repository policy.".into(),
                    approved_at: "2026-07-24T12:02:00Z".into(),
                    intent_digest: rotation_intent.clone(),
                },
            ],
        );
        let third_root = third.root().unwrap();
        let post_rotation_event = AuthorityEventV1::new(AuthorityEventContentV1 {
            transaction_id: "txn_after_rotate".into(),
            principal_id: REPOSITORY_PRINCIPAL.into(),
            authority_mode: crate::authority::AUTHORITY_MODE.into(),
            kind: EventKind::ReviewRejected,
            target: StateTarget {
                r#type: "proposal".into(),
                id: "vpr_after_rotation".into(),
            },
            actor: StateActor {
                r#type: "human".into(),
                id: REPOSITORY_PRINCIPAL.into(),
            },
            timestamp: "2026-07-24T12:03:00Z".into(),
            reason: "Use the rotated repository authority.".into(),
            before_hash: root('f'),
            after_hash: root('f'),
            payload: json!({"proposal_id": "vpr_after_rotation"}),
            caveats: Vec::new(),
        })
        .unwrap();
        let final_root = authority_event_log_root(
            &legacy_root_with_bridge,
            &[
                &fixture.authority_events[0],
                &rotation_event,
                &post_rotation_event,
            ],
        )
        .unwrap();
        let fourth = record(
            4,
            Some(third_root),
            "txn_after_rotate",
            &root('6'),
            &rotation_root,
            &final_root,
            vec![post_rotation_event.id.clone()],
            vec![event_delta(
                4,
                &post_rotation_event.id,
                &post_rotation_event.root().unwrap(),
            )],
            &next_keyset,
            &next_bundle,
            Vec::new(),
        );

        fixture.authority_events.push(rotation_event);
        fixture.authority_events.push(post_rotation_event);
        fixture
            .envelopes
            .push(signed_envelope(&third, &fixture.repository_key));
        fixture.envelopes.push(signed_envelope_with_key_id(
            &fourth,
            &next_key,
            "repository-key-2",
        ));
        let keysets = [initial_keyset, next_keyset.clone()];
        let bundles = [initial_bundle, next_bundle.clone()];
        let mut missing_approval = third.clone();
        missing_approval
            .content
            .semantic_approvals
            .retain(|approval| approval.action != AUTHORITY_ROTATE_ACTION);
        missing_approval.record_id = missing_approval.derive_id().unwrap();
        let mut hostile_envelopes = fixture.envelopes.clone();
        hostile_envelopes[2] = signed_envelope(&missing_approval, &fixture.repository_key);
        let original_envelopes = std::mem::replace(&mut fixture.envelopes, hostile_envelopes);
        let error =
            verify_authority_history(fixture.input_with_stores(&keysets, &bundles)).unwrap_err();
        assert!(error.contains("lacks authority_rotate approval"), "{error}");
        fixture.envelopes = original_envelopes;

        let mut wrong_path = third.clone();
        wrong_path
            .content
            .object_delta
            .iter_mut()
            .find(|delta| delta.object_kind == "authority_keyset")
            .unwrap()
            .path = ".vela/authority/keysets/substituted.json".into();
        wrong_path.record_id = wrong_path.derive_id().unwrap();
        let mut hostile_envelopes = fixture.envelopes.clone();
        hostile_envelopes[2] = signed_envelope(&wrong_path, &fixture.repository_key);
        let original_envelopes = std::mem::replace(&mut fixture.envelopes, hostile_envelopes);
        let error =
            verify_authority_history(fixture.input_with_stores(&keysets, &bundles)).unwrap_err();
        assert!(
            error.contains("path does not match its full root"),
            "{error}"
        );
        fixture.envelopes = original_envelopes;

        let mut hostile_envelopes = fixture.envelopes.clone();
        hostile_envelopes[3] = signed_envelope(&fourth, &fixture.repository_key);
        let original_envelopes = std::mem::replace(&mut fixture.envelopes, hostile_envelopes);
        assert!(verify_authority_history(fixture.input_with_stores(&keysets, &bundles)).is_err());
        fixture.envelopes = original_envelopes;

        let result =
            verify_authority_history(fixture.input_with_stores(&keysets, &bundles)).unwrap();
        assert_eq!(result.authority_record_count, 4);
        assert_eq!(
            result.final_authority_keyset_root,
            Some(next_keyset.root().unwrap())
        );
        assert_eq!(
            result.final_policy_bundle_root,
            Some(next_bundle.root().unwrap())
        );
    }

    #[test]
    fn unactivated_retained_keyset_and_policy_generations_fail_closed() {
        let fixture = fixture();
        let next_key = SigningKey::from_bytes(&[14; 32]);
        let extra_keyset = AuthorityKeysetV1 {
            schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
            frontier_id: FRONTIER_ID.into(),
            generation: 2,
            threshold: 1,
            keys: vec![AuthorityKeyV1 {
                key_id: "unactivated-repository-key".into(),
                algorithm: AUTHORITY_KEY_ALGORITHM.into(),
                public_key: hex::encode(next_key.verifying_key().to_bytes()),
                valid_from_sequence: 3,
                valid_through_sequence: None,
                purpose: AUTHORITY_KEY_PURPOSE.into(),
            }],
            previous_keyset_root: Some(fixture.keyset.root().unwrap()),
            activation_record_root: Some(root('a')),
            closed: false,
        };
        let keysets = [fixture.keyset.clone(), extra_keyset];
        let error = verify_authority_history(
            fixture.input_with_stores(&keysets, std::slice::from_ref(&fixture.bundle)),
        )
        .unwrap_err();
        assert!(error.contains("unactivated generation"), "{error}");

        let extra_bundle = PolicyBundleV1 {
            policies_root: root('7'),
            previous_bundle_root: Some(fixture.bundle.root().unwrap()),
            authority_summary: "Unactivated retained policy.".into(),
            ..fixture.bundle.clone()
        };
        let bundles = [fixture.bundle.clone(), extra_bundle];
        let error = verify_authority_history(
            fixture.input_with_stores(std::slice::from_ref(&fixture.keyset), &bundles),
        )
        .unwrap_err();
        assert!(error.contains("unactivated generation"), "{error}");
    }

    #[test]
    fn sequence_one_intent_and_initial_snapshot_delta_are_exact() {
        let mut wrong_intent = fixture();
        let mut first = verify_authority_envelope(
            &wrong_intent.envelopes[0],
            &wrong_intent.keyset,
            FRONTIER_ID,
            1,
            None,
        )
        .unwrap()
        .record;
        first.content.intent_digest = root('e');
        first.content.semantic_approvals[0].intent_digest = root('e');
        first.record_id = first.derive_id().unwrap();
        wrong_intent.resign_record(0, first);
        assert!(
            verify_authority_history(wrong_intent.input())
                .unwrap_err()
                .contains("does not exactly cover")
        );

        let mut missing_snapshot = fixture();
        let mut first = verify_authority_envelope(
            &missing_snapshot.envelopes[0],
            &missing_snapshot.keyset,
            FRONTIER_ID,
            1,
            None,
        )
        .unwrap()
        .record;
        first
            .content
            .object_delta
            .retain(|delta| delta.object_kind != "policy_bundle");
        first.record_id = first.derive_id().unwrap();
        missing_snapshot.resign_record(0, first);
        assert!(
            verify_authority_history(missing_snapshot.input())
                .unwrap_err()
                .contains("exactly three")
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

        let mut diagnostics = fixture();
        let mut second = verify_authority_envelope(
            &diagnostics.envelopes[1],
            &diagnostics.keyset,
            FRONTIER_ID,
            2,
            Some(
                &verify_authority_envelope(
                    &diagnostics.envelopes[0],
                    &diagnostics.keyset,
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
            .authorization
            .evaluation
            .diagnostics
            .push("fixture diagnostic".into());
        second.record_id = second.derive_id().unwrap();
        diagnostics.resign_record(1, second);
        assert!(verify_authority_history(diagnostics.input()).is_err());
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

    fn conformance_fixture_value(fixture: &Fixture) -> serde_json::Value {
        let expected = verify_authority_history(fixture.input()).unwrap();
        let mut value = json!({
            "schema": "vela.authority-history-conformance.v1",
            "frontier_id": FRONTIER_ID,
            "legacy_events": fixture.legacy_events,
            "legacy_actor_registry_base64": BASE64_STANDARD.encode(&fixture.actor_registry_bytes),
            "legacy_active_policy_head_root": fixture.legacy_active_policy_head_root,
            "legacy_policy_store_manifest_root": fixture.legacy_policy_store_manifest_root,
            "authority_keyset": fixture.keyset,
            "policy_bundle": fixture.bundle,
            "authority_events": fixture.authority_events,
            "authority_envelopes": fixture.envelopes,
            "expected": expected,
        });
        let fixture_root = format!("sha256:{}", sha256_canonical(&value).unwrap());
        value
            .as_object_mut()
            .unwrap()
            .insert("fixture_root".into(), json!(fixture_root));
        value
    }

    #[test]
    fn authority_history_cross_implementation_fixture_is_exact() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/fixtures/authority-history-migration-v1.json");
        let mut generated =
            serde_json::to_string_pretty(&conformance_fixture_value(&fixture())).unwrap();
        generated.push('\n');
        if std::env::var_os("VELA_UPDATE_AUTHORITY_HISTORY_FIXTURE").is_some() {
            std::fs::write(&path, generated.as_bytes()).unwrap();
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "read committed authority-history fixture {}: {error}",
                path.display()
            )
        });
        assert_eq!(
            committed, generated,
            "authority-history fixture drifted; inspect the protocol change, then rerun with VELA_UPDATE_AUTHORITY_HISTORY_FIXTURE=1"
        );
    }
}
