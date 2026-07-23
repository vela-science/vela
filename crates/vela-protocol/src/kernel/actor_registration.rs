//! Temporal actor-registration boundary.
//!
//! The canonical event records a key holder's decision that exact events
//! already present at a Git/Vela anchor may retain their historical signature
//! state. It does not authenticate unsigned history and it grants no scientific
//! or accepted-state authority.

use serde::{Deserialize, Serialize};

use crate::events::{NULL_HASH, StateEvent};

pub const ACTOR_REGISTRATION_BOUNDARY_SCHEMA: &str = "vela.actor-registration-boundary.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorRegistrationMode {
    Bootstrap,
    TemporalizeExisting,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActorRegistrationAnchor {
    pub git_object_format: String,
    pub git_commit: String,
    pub git_tree: String,
    pub event_log_root: String,
    pub event_count: u64,
    pub actor_registry_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActorRegistrationBoundaryPayload {
    pub schema: String,
    pub mode: ActorRegistrationMode,
    pub frontier_id: String,
    pub actor_id: String,
    pub public_key: String,
    pub algorithm: String,
    pub anchor: ActorRegistrationAnchor,
}

pub(crate) fn require_lower_hex(field: &str, value: &str, len: usize) -> Result<(), String> {
    if value.len() != len
        || !value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
    {
        return Err(format!("{field} must be {len} lowercase hex characters"));
    }
    Ok(())
}

pub(crate) fn require_sha256_root(field: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(format!("{field} must use the sha256:<64hex> form"));
    };
    require_lower_hex(field, hex, 64)
}

impl ActorRegistrationBoundaryPayload {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != ACTOR_REGISTRATION_BOUNDARY_SCHEMA {
            return Err(format!(
                "payload.schema must be {ACTOR_REGISTRATION_BOUNDARY_SCHEMA}"
            ));
        }
        if !self.frontier_id.starts_with("vfr_") {
            return Err("payload.frontier_id must start with vfr_".to_string());
        }
        if !(self.actor_id.starts_with("reviewer:") || self.actor_id.starts_with("steward:")) {
            return Err(
                "payload.actor_id must identify a reviewer: or steward: human actor".to_string(),
            );
        }
        require_lower_hex("payload.public_key", &self.public_key, 64)?;
        if self.algorithm != "ed25519" {
            return Err("payload.algorithm must be ed25519".to_string());
        }
        match self.anchor.git_object_format.as_str() {
            "sha1" => {
                require_lower_hex("payload.anchor.git_commit", &self.anchor.git_commit, 40)?;
                require_lower_hex("payload.anchor.git_tree", &self.anchor.git_tree, 40)?;
            }
            "sha256" => {
                require_lower_hex("payload.anchor.git_commit", &self.anchor.git_commit, 64)?;
                require_lower_hex("payload.anchor.git_tree", &self.anchor.git_tree, 64)?;
            }
            other => {
                return Err(format!(
                    "payload.anchor.git_object_format must be sha1 or sha256, got {other:?}"
                ));
            }
        }
        require_sha256_root("payload.anchor.event_log_root", &self.anchor.event_log_root)?;
        require_sha256_root(
            "payload.anchor.actor_registry_root",
            &self.anchor.actor_registry_root,
        )?;
        if self.mode == ActorRegistrationMode::TemporalizeExisting && self.anchor.event_count == 0 {
            return Err("temporalize_existing requires a non-empty anchored event log".to_string());
        }
        Ok(())
    }
}

pub fn payload_from_event(event: &StateEvent) -> Result<ActorRegistrationBoundaryPayload, String> {
    if event.kind.as_str() != crate::events::EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED {
        return Err(format!(
            "expected {}, got {}",
            crate::events::EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED,
            event.kind
        ));
    }
    if event.target.r#type != "actor" {
        return Err("activation target.type must be actor".to_string());
    }
    if event.actor.r#type != "human" {
        return Err("activation actor.type must be human".to_string());
    }
    if event.before_hash != NULL_HASH || event.after_hash != NULL_HASH {
        return Err("activation must use null before_hash and after_hash".to_string());
    }
    if event.signature.is_none() {
        return Err("activation event must carry an ordinary event signature".to_string());
    }
    let payload: ActorRegistrationBoundaryPayload =
        serde_json::from_value(event.payload.clone())
            .map_err(|error| format!("invalid actor-registration payload: {error}"))?;
    payload.validate()?;
    if event.target.id != payload.actor_id {
        return Err("activation target.id must equal payload.actor_id".to_string());
    }
    if event.actor.id != payload.actor_id {
        return Err("activation actor.id must equal payload.actor_id".to_string());
    }
    Ok(payload)
}

pub fn verify_activation_signature(
    event: &StateEvent,
    expected_public_key: &str,
) -> Result<(), String> {
    let payload = payload_from_event(event)?;
    if payload.public_key != expected_public_key {
        return Err(
            "activation payload public key does not match the selected actor key".to_string(),
        );
    }
    match crate::sign::verify_event_signature(event, expected_public_key)? {
        true => Ok(()),
        false => Err("activation event signature does not verify".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use serde_json::json;

    use super::*;
    use crate::events::{EVENT_SCHEMA, StateActor, StateTarget, compute_event_id};

    fn activation() -> (StateEvent, SigningKey) {
        let key = SigningKey::generate(&mut OsRng);
        let public_key = hex::encode(key.verifying_key().to_bytes());
        let mut event = StateEvent {
            schema: EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: crate::events::EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED.into(),
            target: StateTarget {
                r#type: "actor".to_string(),
                id: "reviewer:test".to_string(),
            },
            actor: StateActor {
                r#type: "human".to_string(),
                id: "reviewer:test".to_string(),
            },
            timestamp: "2026-07-16T00:00:00Z".to_string(),
            reason: "Activate signature enforcement after the anchored history.".to_string(),
            before_hash: NULL_HASH.to_string(),
            after_hash: NULL_HASH.to_string(),
            payload: json!({
                "schema": ACTOR_REGISTRATION_BOUNDARY_SCHEMA,
                "mode": "temporalize_existing",
                "frontier_id": "vfr_1234567890abcdef",
                "actor_id": "reviewer:test",
                "public_key": public_key,
                "algorithm": "ed25519",
                "anchor": {
                    "git_object_format": "sha1",
                    "git_commit": "1".repeat(40),
                    "git_tree": "2".repeat(40),
                    "event_log_root": format!("sha256:{}", "3".repeat(64)),
                    "event_count": 1,
                    "actor_registry_root": format!("sha256:{}", "4".repeat(64))
                }
            }),
            caveats: vec![
                "The anchor preserves legacy bytes but does not authenticate unsigned history."
                    .to_string(),
            ],
            signature: None,
        };
        event.id = compute_event_id(&event);
        event.signature = Some(crate::sign::sign_event(&event, &key).unwrap());
        (event, key)
    }

    #[test]
    fn actor_registration_boundary_accepts_exact_signed_shape() {
        let (event, key) = activation();
        let payload = payload_from_event(&event).unwrap();
        assert_eq!(payload.mode, ActorRegistrationMode::TemporalizeExisting);
        verify_activation_signature(&event, &hex::encode(key.verifying_key().to_bytes())).unwrap();
    }

    #[test]
    fn actor_registration_boundary_rejects_timestamp_cutoff_fields() {
        let (mut event, _) = activation();
        event.payload["created_at_cutoff"] = json!("2026-07-16T00:00:00Z");
        assert!(payload_from_event(&event).is_err());
    }

    #[test]
    fn actor_registration_boundary_rejects_mismatched_actor() {
        let (mut event, _) = activation();
        event.actor.id = "reviewer:other".to_string();
        assert!(payload_from_event(&event).is_err());
    }

    #[test]
    fn actor_registration_boundary_rejects_unsigned_event() {
        let (mut event, _) = activation();
        event.signature = None;
        assert!(payload_from_event(&event).is_err());
    }
}
