//! Read-only verification of current repository-authority history.
//!
//! Sequence 1 covers one exact authority initialization. For current
//! repositories with a predecessor origin, that initialization binds the roots
//! retained by the signed origin. This module exposes no writer or
//! key-custody surface.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::authority::{
    AuthorityEnvelopeV1, AuthorityEventV1, AuthorityKeysetV1, VerifiedAuthorityRecord,
    verify_authority_envelope, verify_authority_keyset_transition,
};
use crate::authorization::{
    AuthorizationDecisionV1, AuthorizationModelV1, evaluate_authorization_v1,
};
use crate::canonical::sha256_canonical;
use crate::events::NULL_HASH;

pub const AUTHORITY_INITIALIZATION_SCHEMA_V1: &str = "vela.authority-initialization.v1";
pub const AUTHORITY_EVENT_LOG_SCHEMA_V1: &str = "vela.authority-event-log.v1";
pub const AUTHORITY_INITIALIZE_ACTION: &str = "authority_initialize";
pub const AUTHORITY_INITIALIZED_EVENT_KIND: &str = "authority.initialized";
pub const AUTHORITY_ROTATE_ACTION: &str = "authority_rotate";
pub const AUTHORITY_CLOSE_ACTION: &str = "authority_close";
pub const AUTHORITY_MODEL_UPDATE_ACTION: &str = "authority_model_update";
pub const AUTHORITY_CLOSE_SCHEMA_V1: &str = "vela.authority-close.v1";
pub const AUTHORITY_CLOSED_EVENT_KIND: &str = "authority.closed";

fn is_false(value: &bool) -> bool {
    !*value
}

fn require_sha256_root(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("{field} must use the sha256:<64hex> form"))?;
    if !crate::shape::is_lower_hex_64(digest) {
        return Err(format!("{field} must be 64 lowercase hex characters"));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityCloseV1 {
    pub schema: String,
    pub repository_id: String,
    pub last_trusted_sequence: u64,
    pub last_trusted_authority_record_root: String,
    pub previous_authority_keyset_root: String,
    pub closed_authority_keyset_root: String,
    pub authorization_model_root: String,
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
        require_repository(&self.repository_id)?;
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
            (
                "authorization_model_root",
                self.authorization_model_root.as_str(),
            ),
        ] {
            require_sha256_root(name, root)?;
        }
        if self.incident_id.trim().is_empty() || self.reason.trim().is_empty() {
            return Err("authority close incident and reason must be non-empty".into());
        }
        Ok(())
    }
}

/// Fresh-repository bootstrap for current repository authority.
///
/// This object has no predecessor signer and grants no exemption to historical
/// events. It is valid only over the exact
/// one-event Profile v1 skeleton produced by `vela init`, with an empty actor
/// registry. The covering sequence-1 authority record proves possession of the
/// selected repository key; consumers still pin the resulting full authority
/// root through their ordinary distribution trust path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityInitializationV1 {
    pub schema: String,
    pub repository_id: String,
    pub initial_event_log_root: String,
    pub initial_actor_registry_root: String,
    pub new_authority_keyset_root: String,
    pub new_authorization_model_root: String,
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
        require_repository(&self.repository_id)?;
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
                "new_authorization_model_root",
                self.new_authorization_model_root.as_str(),
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
    Uninitialized,
    RepositoryAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityHistoryVerification {
    pub era: AuthorityHistoryEra,
    pub repository_id: String,
    pub authority_event_count: usize,
    pub authority_record_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization_event_id: Option<String>,
    pub final_event_log_root: String,
    pub first_authority_record_root: Option<String>,
    pub final_authority_record_root: Option<String>,
    pub final_authority_keyset_root: Option<String>,
    pub final_authorization_model_root: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub closed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure_event_id: Option<String>,
}

/// Complete current read-side inputs.
pub struct AuthorityHistoryInput<'a> {
    pub repository_id: &'a str,
    pub initial_event_log_root: &'a str,
    pub initial_actor_registry_root: &'a str,
    /// Compacted origins retained initial keyset and policy snapshots as
    /// canonical evidence rather than sequence-one transition deltas.
    pub authority_keysets: &'a [AuthorityKeysetV1],
    pub authorization_models: &'a [AuthorizationModelV1],
    pub authority_events: &'a [AuthorityEventV1],
    pub authority_envelopes: &'a [AuthorityEnvelopeV1],
}

/// Verify one current repository-origin boundary and authority chain.
pub fn verify_authority_history(
    input: AuthorityHistoryInput<'_>,
) -> Result<AuthorityHistoryVerification, String> {
    require_repository(input.repository_id)?;
    require_sha256_root("initial event_log_root", input.initial_event_log_root)?;
    require_sha256_root(
        "initial actor_registry_root",
        input.initial_actor_registry_root,
    )?;
    if input.authority_events.is_empty() && input.authority_envelopes.is_empty() {
        return Ok(AuthorityHistoryVerification {
            era: AuthorityHistoryEra::Uninitialized,
            repository_id: input.repository_id.into(),
            authority_event_count: 0,
            authority_record_count: 0,
            initialization_event_id: None,
            final_event_log_root: input.initial_event_log_root.into(),
            first_authority_record_root: None,
            final_authority_record_root: None,
            final_authority_keyset_root: None,
            final_authorization_model_root: None,
            closed: false,
            closure_event_id: None,
        });
    }
    if input.authority_envelopes.is_empty() {
        return Err("repository-authority history has no covering authority record".into());
    }

    let authority_keysets = index_authority_keysets(input.repository_id, input.authority_keysets)?;
    let authorization_models =
        index_authorization_models(input.repository_id, input.authorization_models)?;
    let initializations = input
        .authority_events
        .iter()
        .filter(|event| event.content.kind.as_str() == AUTHORITY_INITIALIZED_EVENT_KIND)
        .collect::<Vec<_>>();
    let [initialization_event] = initializations.as_slice() else {
        return Err(
            "repository-authority history must contain exactly one authority initialization".into(),
        );
    };
    let initialization = initialization_payload_from_event(initialization_event)?;
    let mut active_keyset_root = initialization.new_authority_keyset_root.clone();
    let mut active_model_root = initialization.new_authorization_model_root.clone();
    let mut active_keyset = authority_keysets
        .get(&active_keyset_root)
        .copied()
        .ok_or_else(|| "initial authority keyset is not retained".to_string())?;
    let mut active_model = authorization_models
        .get(&active_model_root)
        .copied()
        .ok_or_else(|| "initial policy bundle is not retained".to_string())?;
    verify_origin_authority_initialization(
        input.repository_id,
        input.initial_event_log_root,
        input.initial_actor_registry_root,
        active_keyset,
        active_model,
        initialization_event,
    )?;
    if active_keyset.generation != 1
        || active_keyset.previous_keyset_root.is_some()
        || active_keyset.activation_record_root.is_some()
        || active_model.previous_model_root.is_some()
    {
        return Err(
            "authority boundary must activate initial keyset and policy generations".into(),
        );
    }
    let mut activated_keysets = BTreeSet::from([active_keyset_root.clone()]);
    let mut activated_models = BTreeSet::from([active_model_root.clone()]);

    let mut era_one_by_id = BTreeMap::new();
    let mut era_one_by_transaction: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for event in input.authority_events {
        event.validate()?;
        if era_one_by_id.insert(event.id.as_str(), event).is_some() {
            return Err(format!("duplicate event coverage identity {}", event.id));
        }
        era_one_by_transaction
            .entry(event.content.transaction_id.as_str())
            .or_default()
            .insert(event.id.as_str());
    }

    let initial_event_log_root = input.initial_event_log_root.to_string();
    let mut current_event_root = initialization.initial_event_log_root.clone();
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
            input.repository_id,
            sequence,
            previous_record_root.as_deref(),
        )?;
        verify_record_authorization(&verified, active_model)?;
        if verified.record.content.before_event_log_root != current_event_root {
            return Err(format!(
                "authority record {sequence} has the wrong before-event root"
            ));
        }

        if sequence == 1 {
            verify_first_initialization_record(
                &verified,
                initialization_event,
                &initialization,
                &initial_event_log_root,
            )?;
            covered_era_one.insert(initialization_event.id.clone());
            cumulative_era_one.push(*initialization_event);
            current_event_root =
                authority_event_log_root(&initial_event_log_root, &cumulative_era_one)?;
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
                    return Err(format!(
                        "authority event {event_id} is covered more than once"
                    ));
                }
                let event = era_one_by_id[event_id];
                if event.content.principal_id != verified.record.content.principal.principal_id
                    || event.content.actor.id != event.content.principal_id
                {
                    return Err(format!(
                        "authority event {event_id} attribution does not match its authority record"
                    ));
                }
                verify_event_object_delta(&verified, event_id, &event.root()?)?;
                cumulative_era_one.push(event);
                transaction_events.push(event);
            }
            let expected_after = if transaction_events.is_empty() {
                current_event_root.clone()
            } else {
                authority_event_log_root(&initial_event_log_root, &cumulative_era_one)?
            };
            if verified.record.content.after_event_log_root != expected_after {
                return Err(format!(
                    "authority record {sequence} has the wrong after-event root"
                ));
            }
            current_event_root = expected_after;

            let previous_keyset_root = active_keyset_root.clone();
            let previous_policy_root = active_model_root.clone();
            let next_keyset = keyset_transition_for_record(
                &verified,
                active_keyset,
                &authority_keysets,
                sequence
                    .checked_add(1)
                    .ok_or_else(|| "authority keyset activation sequence overflows".to_string())?,
            )?;
            let next_policy =
                model_transition_for_record(&verified, active_model, &authorization_models)?;
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
                active_model_root = next.root()?;
                if !activated_models.insert(active_model_root.clone()) {
                    return Err("policy bundle generation was activated more than once".into());
                }
                active_model = next;
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
            "authority history has events without unique authority-record coverage: {}",
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
    if activated_models
        != authorization_models
            .keys()
            .cloned()
            .collect::<BTreeSet<String>>()
    {
        return Err("retained policy store contains an unactivated generation".into());
    }

    Ok(AuthorityHistoryVerification {
        era: AuthorityHistoryEra::RepositoryAuthority,
        repository_id: input.repository_id.into(),
        authority_event_count: input.authority_events.len(),
        authority_record_count: verified_records.len(),
        initialization_event_id: Some(initialization_event.id.clone()),
        final_event_log_root: current_event_root,
        first_authority_record_root: verified_records
            .first()
            .map(|record| record.record_root.clone()),
        final_authority_record_root: previous_record_root,
        final_authority_keyset_root: Some(active_keyset_root),
        final_authorization_model_root: Some(active_model_root),
        closed,
        closure_event_id,
    })
}

fn index_authority_keysets<'a>(
    repository_id: &str,
    keysets: &'a [AuthorityKeysetV1],
) -> Result<BTreeMap<String, &'a AuthorityKeysetV1>, String> {
    let mut indexed = BTreeMap::new();
    for keyset in keysets {
        keyset.validate()?;
        if keyset.repository_id != repository_id {
            return Err("retained authority keyset names a different repository".into());
        }
        let root = keyset.root()?;
        if indexed.insert(root, keyset).is_some() {
            return Err("retained authority keyset store contains a duplicate root".into());
        }
    }
    Ok(indexed)
}

fn index_authorization_models<'a>(
    repository_id: &str,
    bundles: &'a [AuthorizationModelV1],
) -> Result<BTreeMap<String, &'a AuthorizationModelV1>, String> {
    let mut indexed = BTreeMap::new();
    for bundle in bundles {
        bundle.validate()?;
        if bundle.repository_id != repository_id {
            return Err("retained policy bundle names a different repository".into());
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

fn model_transition_for_record<'a>(
    verified: &VerifiedAuthorityRecord,
    current: &AuthorizationModelV1,
    retained: &BTreeMap<String, &'a AuthorizationModelV1>,
) -> Result<Option<&'a AuthorizationModelV1>, String> {
    let deltas = verified
        .record
        .content
        .object_delta
        .iter()
        .filter(|delta| delta.object_kind == "authorization_model")
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
        .any(|approval| approval.action == AUTHORITY_MODEL_UPDATE_ACTION)
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
    if delta.path != format!(".vela/authority/models/{stem}.json") {
        return Err("policy bundle transition path does not match its full root".into());
    }
    let next = retained
        .get(root)
        .copied()
        .ok_or_else(|| "policy bundle transition snapshot is not retained".to_string())?;
    crate::authorization::verify_authorization_model_transition(current, next)?;
    Ok(Some(next))
}

fn verify_authority_close_record(
    verified: &VerifiedAuthorityRecord,
    transaction_events: &[&AuthorityEventV1],
    sequence: u64,
    previous_keyset_root: &str,
    closed_keyset_root: &str,
    authorization_model_root: &str,
) -> Result<String, String> {
    if transaction_events.len() != 1 || verified.record.content.object_delta.len() != 2 {
        return Err(
            "terminal authority close must cover exactly one event and one closed keyset".into(),
        );
    }
    let event = transaction_events[0];
    /* `target.id` is the test that means something: it binds the close to the
    repository the record is for. `target.type` was tested beside it against a
    literal this binary also writes, which is a check of a value against itself. */
    if event.content.kind.as_str() != AUTHORITY_CLOSED_EVENT_KIND
        || event.content.target.id != verified.record.content.repository_id
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
    if payload.repository_id != verified.record.content.repository_id
        || payload.last_trusted_sequence != sequence.saturating_sub(1)
        || payload.last_trusted_authority_record_root != previous_record_root
        || payload.previous_authority_keyset_root != previous_keyset_root
        || payload.closed_authority_keyset_root != closed_keyset_root
        || payload.authorization_model_root != authorization_model_root
        || payload.reason != event.content.reason
    {
        return Err("authority close payload does not match its exact terminal transition".into());
    }
    Ok(event.id.clone())
}

/// Verify sequence-one initialization against the immutable repository origin.
pub fn verify_origin_authority_initialization(
    repository_id: &str,
    initial_event_log_root: &str,
    initial_actor_registry_root: &str,
    authority_keyset: &AuthorityKeysetV1,
    authorization_model: &AuthorizationModelV1,
    initialization_event: &AuthorityEventV1,
) -> Result<AuthorityInitializationV1, String> {
    require_repository(repository_id)?;
    require_sha256_root("initial event_log_root", initial_event_log_root)?;
    require_sha256_root("initial actor_registry_root", initial_actor_registry_root)?;
    let payload = initialization_payload_from_event(initialization_event)?;
    if payload.repository_id != repository_id
        || payload.initial_event_log_root != initial_event_log_root
        || payload.initial_actor_registry_root != initial_actor_registry_root
    {
        return Err("current authority initialization does not bind its exact origin roots".into());
    }
    authority_keyset.validate()?;
    authorization_model.validate()?;
    if authority_keyset.repository_id != repository_id
        || authorization_model.repository_id != repository_id
        || payload.new_authority_keyset_root != authority_keyset.root()?
        || payload.new_authorization_model_root != authorization_model.root()?
    {
        return Err(
            "current authority initialization does not bind its initial authority inputs".into(),
        );
    }
    Ok(payload)
}

pub fn initialization_payload_from_event(
    event: &AuthorityEventV1,
) -> Result<AuthorityInitializationV1, String> {
    event.validate()?;
    if event.content.kind.as_str() != AUTHORITY_INITIALIZED_EVENT_KIND
        || event.content.actor.r#type != "human"
        || event.content.before_hash != NULL_HASH
        || event.content.after_hash != NULL_HASH
    {
        return Err("authority initialization event shape is invalid".into());
    }
    let payload: AuthorityInitializationV1 = serde_json::from_value(event.content.payload.clone())
        .map_err(|error| format!("authority initialization payload is invalid: {error}"))?;
    /* The target names the repository this event initializes. The shape test
    above used to compare `target.type` against a literal instead, which checked
    that the writer had written what the writer chose to write, and left the id
    unchecked — so an initialization event could name one repository in its
    target and another in its payload and pass. */
    if event.content.target.id != payload.repository_id {
        return Err(
            "authority initialization event targets a different repository than its payload".into(),
        );
    }
    payload.validate()?;
    if event.content.target.id != payload.repository_id
        || event.content.reason != payload.reason
        || event.content.principal_id != payload.new_principal_id
        || event.content.actor.id != payload.new_principal_id
    {
        return Err("authority initialization event does not match its payload".into());
    }
    Ok(payload)
}

pub fn authority_event_log_root(
    initial_event_log_root: &str,
    authority_events: &[&AuthorityEventV1],
) -> Result<String, String> {
    require_sha256_root("initial_event_log_root", initial_event_log_root)?;
    let mut roots = authority_events
        .iter()
        .map(|event| event.root().map(|root| (event.id.as_str(), root)))
        .collect::<Result<Vec<_>, _>>()?;
    roots.sort_by(|left, right| left.0.cmp(right.0));
    let commitment = serde_json::json!({
        "schema": AUTHORITY_EVENT_LOG_SCHEMA_V1,
        // Retained canonical field name: changing it would rewrite signed
        // current authority records.
        "legacy_event_log_root": initial_event_log_root,
        "authority_event_roots": roots.into_iter().map(|(_, root)| root).collect::<Vec<_>>(),
    });
    Ok(format!("sha256:{}", sha256_canonical(&commitment)?))
}

fn verify_first_initialization_record(
    verified: &VerifiedAuthorityRecord,
    initialization_event: &AuthorityEventV1,
    initialization: &AuthorityInitializationV1,
    initial_event_log_root: &str,
) -> Result<(), String> {
    let record = &verified.record;
    let event_root = initialization_event.root()?;
    let expected_after = authority_event_log_root(initial_event_log_root, &[initialization_event])?;
    if record.content.event_ids != [initialization_event.id.clone()]
        || record.content.before_event_log_root != initial_event_log_root
        || record.content.after_event_log_root != expected_after
        || record.content.principal.principal_id != initialization.new_principal_id
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
    /* A compaction origin carried its predecessor's keyset and policy
    snapshots forward, so record 1 bound them rather than installing them.
    Genesis is the only origin now: record 1 always installs both. */
    verify_initial_snapshot_delta(
        verified,
        &initialization.new_authority_keyset_root,
        &initialization.new_authorization_model_root,
    )
}

fn verify_initial_snapshot_delta(
    verified: &VerifiedAuthorityRecord,
    authority_keyset_root: &str,
    authorization_model_root: &str,
) -> Result<(), String> {
    let keyset_stem = authority_keyset_root
        .strip_prefix("sha256:")
        .ok_or_else(|| "initial authority keyset root lacks sha256 tag".to_string())?;
    let policy_stem = authorization_model_root
        .strip_prefix("sha256:")
        .ok_or_else(|| "initial policy bundle root lacks sha256 tag".to_string())?;
    let expected = [
        (
            format!(".vela/authority/keysets/{keyset_stem}.json"),
            authority_keyset_root,
            "authority_keyset",
        ),
        (
            format!(".vela/authority/models/{policy_stem}.json"),
            authorization_model_root,
            "authorization_model",
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

/// Re-decide the authorization this record was written under.
///
/// This used to check that a retained attestation said `Allow` and that it
/// named the pinned engine, version and profile. It could do no more: the
/// request and the entity snapshot were named by root only, so nothing in the
/// record could be evaluated again. Replay verified that someone had recorded
/// an `Allow`, not that `Allow` was the right answer — ADR 0035 §4's history
/// gap, in one function.
///
/// The record now retains the exact request, and the model it was decided
/// under is the one this repository retains at that root, so the decision is
/// recomputed here and compared. An authority record whose evaluation does not
/// follow from its own inputs fails replay.
fn verify_record_authorization(
    verified: &VerifiedAuthorityRecord,
    authorization_model: &AuthorizationModelV1,
) -> Result<(), String> {
    let authorization = &verified.record.content.authorization;
    let sequence = verified.record.content.sequence;
    if authorization.model_root != authorization_model.root()? {
        return Err(format!(
            "authority record {sequence} does not bind the active authorization model"
        ));
    }
    let recomputed = evaluate_authorization_v1(authorization_model, &authorization.request)
        .map_err(|error| {
            format!("authority record {sequence} authorization is invalid: {error}")
        })?;
    if recomputed != authorization.evaluation {
        return Err(format!(
            "authority record {sequence} retains an authorization its own inputs do not produce"
        ));
    }
    if recomputed.decision != AuthorizationDecisionV1::Allow {
        return Err(format!(
            "authority record {sequence} was written under a denied authorization"
        ));
    }
    Ok(())
}

fn verify_event_object_delta(
    verified: &VerifiedAuthorityRecord,
    event_id: &str,
    event_root: &str,
) -> Result<(), String> {
    let expected_path = format!(".vela/authority/events/{event_id}.json");
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

fn require_repository(repository_id: &str) -> Result<(), String> {
    if repository_id.starts_with("vrepo_") {
        Ok(())
    } else {
        Err("authority history repository_id must start with vrepo_".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    #[test]
    fn initialization_is_closed_and_root_bound() {
        let value = AuthorityInitializationV1 {
            schema: AUTHORITY_INITIALIZATION_SCHEMA_V1.into(),
            repository_id: "vrepo_fixture".into(),
            initial_event_log_root: root('1'),
            initial_actor_registry_root: root('2'),
            new_authority_keyset_root: root('3'),
            new_authorization_model_root: root('4'),
            new_principal_id: "local:fixture|uid:501".into(),
            minimum_writer_version: "0.930.0".into(),
            reason: "Initialize the exact current repository authority.".into(),
        };
        value.validate().unwrap();

        let mut invalid = value.clone();
        invalid.initial_event_log_root = "sha256:short".into();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn uninitialized_origin_preserves_its_event_root() {
        let initial_event_root = root('a');
        let initial_actor_root = root('b');
        let verification = verify_authority_history(AuthorityHistoryInput {
            repository_id: "vrepo_fixture",
            initial_event_log_root: &initial_event_root,
            initial_actor_registry_root: &initial_actor_root,
            authority_keysets: &[],
            authorization_models: &[],
            authority_events: &[],
            authority_envelopes: &[],
        })
        .unwrap();

        assert_eq!(verification.era, AuthorityHistoryEra::Uninitialized);
        assert_eq!(verification.final_event_log_root, initial_event_root);
        assert_eq!(verification.authority_event_count, 0);
        assert_eq!(verification.authority_record_count, 0);
    }
}
