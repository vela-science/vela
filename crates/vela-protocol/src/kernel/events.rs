//! Canonical semantic events used by current repository authority.
//!
//! Current Decisions are authenticated by `AuthorityEventV1`. Its semantic
//! payload retains the established `vela.event.v0.1` content-addressing shape
//! so existing event IDs and roots remain stable. Historical reducer logic and
//! Retired-era constructors do not live in this module.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::canonical;

pub const EVENT_SCHEMA: &str = "vela.event.v0.1";
pub const NULL_HASH: &str = "sha256:null";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateTarget {
    pub r#type: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateActor {
    pub id: String,
    pub r#type: String,
}

macro_rules! event_kinds {
    ($($variant:ident => $wire:literal),+ $(,)?) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum EventKind {
            $($variant,)+
            /// An authority or historical semantic kind not enumerated here.
            Other(String),
        }

        impl EventKind {
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $wire,)+
                    Self::Other(value) => value,
                }
            }
        }

        impl From<&str> for EventKind {
            fn from(value: &str) -> Self {
                match value {
                    $($wire => Self::$variant,)+
                    other => Self::Other(other.to_string()),
                }
            }
        }
    };
}

// Only kinds with current semantic behavior receive typed variants. All other
// retained strings round-trip through `Other` without reintroducing historical
// writers, reducers, or product concepts.
event_kinds! {
    ClaimAsserted => "claim.asserted",
    ClaimNoted => "claim.noted",
    ClaimRetracted => "claim.retracted",
    TargetClaimed => "target.claimed",
    ClaimSuperseded => "claim.superseded",
    ReviewAccepted => "review.accepted",
    ReviewRejected => "review.rejected",
    ReviewRevisionRequested => "review.revision_requested",
}

impl From<String> for EventKind {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<str> for EventKind {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for EventKind {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl Serialize for EventKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EventKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from(value))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEvent {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub id: String,
    pub kind: EventKind,
    pub target: StateTarget,
    pub actor: StateActor,
    pub timestamp: String,
    pub reason: String,
    pub before_hash: String,
    pub after_hash: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub caveats: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

fn default_schema() -> String {
    EVENT_SCHEMA.to_string()
}

/// Content-only commitment over a canonically ID-sorted event set.
pub fn event_log_hash(events: &[StateEvent]) -> String {
    let mut sorted = events.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.id.cmp(&right.id));
    let stripped = sorted
        .into_iter()
        .map(|event| {
            let mut value = serde_json::to_value(event).unwrap_or(Value::Null);
            if let Value::Object(object) = &mut value {
                object.remove("signature");
            }
            value
        })
        .collect::<Vec<_>>();
    let bytes = canonical::to_canonical_bytes(&stripped).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

pub fn compute_event_id(event: &StateEvent) -> String {
    event_id(event)
}

/// Exact established content preimage, excluding `id` and `signature`.
pub fn event_content_preimage_bytes(event: &StateEvent) -> Vec<u8> {
    canonical::to_canonical_bytes(&json!({
        "schema": event.schema,
        "kind": event.kind,
        "target": event.target,
        "actor": event.actor,
        "timestamp": event.timestamp,
        "reason": event.reason,
        "before_hash": event.before_hash,
        "after_hash": event.after_hash,
        "payload": event.payload,
        "caveats": event.caveats,
    }))
    .unwrap_or_default()
}

pub fn event_id(event: &StateEvent) -> String {
    format!(
        "vev_{}",
        &hex::encode(Sha256::digest(event_content_preimage_bytes(event)))[..16]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: EventKind, label: &str) -> StateEvent {
        let mut value = StateEvent {
            schema: EVENT_SCHEMA.into(),
            id: String::new(),
            kind,
            target: StateTarget {
                r#type: "claim".into(),
                id: format!("vcl_{label}"),
            },
            actor: StateActor {
                id: "agent:test".into(),
                r#type: "agent".into(),
            },
            timestamp: "2026-07-28T00:00:00Z".into(),
            reason: "Exact bounded transition.".into(),
            before_hash: NULL_HASH.into(),
            after_hash: format!("sha256:{label}"),
            payload: json!({"label": label}),
            caveats: Vec::new(),
            signature: None,
        };
        value.id = compute_event_id(&value);
        value
    }

    #[test]
    fn signatures_do_not_change_event_identity_or_log_root() {
        let mut value = event(EventKind::ClaimAsserted, "a");
        let id = value.id.clone();
        let root = event_log_hash(&[value.clone()]);
        value.signature = Some("detached".into());
        assert_eq!(compute_event_id(&value), id);
        assert_eq!(event_log_hash(&[value]), root);
    }

    #[test]
    fn event_log_order_is_canonical() {
        let left = event(EventKind::ClaimAsserted, "a");
        let right = event(EventKind::ReviewAccepted, "b");
        assert_eq!(
            event_log_hash(&[left.clone(), right.clone()]),
            event_log_hash(&[right, left])
        );
    }

    /// Every scientific kind keeps a typed variant, and `Other` keeps every
    /// string it is given.
    ///
    /// The two halves are one contract.
    /// `authority_transaction.rs::validate_semantic_event_links` *skips*
    /// `Other(_)` when it checks that no two events share a semantic identity,
    /// because the authority vocabulary — `authority.initialized`,
    /// `authority.rotated`, `policy.rotated`, `authority.closed` — is emitted
    /// through `Other` and is not scientific. So a scientific kind that lost its
    /// typed variant would not fail to parse; it would quietly stop being
    /// checked for uniqueness. That is the failure this asserts against, and it
    /// is why the inert `FrontierCreated => "repository.created"` variant could
    /// be deleted safely: nothing emits that string, and `claim.*`,
    /// `target.claimed` and `review.*` all still land on their own variants.
    #[test]
    fn every_scientific_kind_is_typed_and_other_round_trips() {
        for wire in [
            "claim.asserted",
            "claim.noted",
            "claim.retracted",
            "claim.superseded",
            "target.claimed",
            "review.accepted",
            "review.rejected",
            "review.revision_requested",
        ] {
            let kind = EventKind::from(wire);
            assert!(
                !matches!(kind, EventKind::Other(_)),
                "{wire} falls through to Other, which the semantic-identity \
                 check skips"
            );
            assert_eq!(kind.as_str(), wire);
        }

        for wire in [
            "authority.initialized",
            "authority.rotated",
            "policy.rotated",
            "authority.closed",
            "frontier.created",
        ] {
            let kind = EventKind::from(wire);
            assert!(matches!(kind, EventKind::Other(_)), "{wire} is not typed");
            assert_eq!(kind.as_str(), wire, "{wire} does not round-trip");
        }
    }
}
