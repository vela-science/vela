//! v0.39: Hub federation — peer registry + conflict detection.
//!
//! Pre-v0.39, every Vela frontier had exactly one source of truth: the
//! single hub it was published to (`vela-hub.fly.dev`). The substrate
//! claimed the kernel was content-addressed and signed, but the
//! distribution layer was centralized — there was no way for a second
//! hub to mirror a frontier and detect when its view diverged from
//! the original.
//!
//! v0.39.0 lands the *schema layer* of federation. A frontier can now
//! register peer hubs (id + URL + public key) in `Project.peers`, and
//! the kernel knows two new event kinds:
//!
//! - `frontier.synced_with_peer` — append-only record of a sync pass:
//!   what we exchanged, what hash we ended up agreeing on, how many
//!   findings differed.
//! - `frontier.conflict_detected` — emitted per finding when our view
//!   and the peer's view disagree on a substantive field (review
//!   verdict, confidence, retraction, presence).
//!
//! The actual sync runtime (HTTP fetch, manifest verification,
//! conflict-resolution proposal emission) ships in v0.39.1+. Same
//! staging discipline as v0.32 (Replication object) → v0.36.1
//! (Project.replications becomes authoritative) and v0.38.0 (causal
//! schema) → v0.38.1 (causal math).
//!
//! Doctrine for v0.39.0:
//! - The peer registry is a frontier-local declaration. Adding a peer
//!   does not yet trust their state; it just establishes who we know
//!   about.
//! - Peer signatures still verify under the same Ed25519 discipline
//!   as `actors`. A peer's `frontier.merged` event signed by their
//!   key can be replayed locally only when their pubkey is in our
//!   `peers` registry.
//! - Conflicts are recorded, not auto-resolved. v0.39.1+ will surface
//!   them through proposals so a human reviewer chooses which side
//!   to accept.

use serde::{Deserialize, Serialize};

/// v0.39: A registered peer hub the local frontier knows about.
/// Content-addressed by `(id, public_key)` so two registry entries
/// for the same peer with different keys can be detected as a
/// material change rather than silent overwrite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerHub {
    /// Stable, namespaced identifier — the equivalent of an
    /// `actor.id` for hub-scale identities. Recommended form
    /// `hub:<short-name>` (e.g. `hub:vela-mirror-eu`).
    pub id: String,
    /// HTTPS URL where the peer publishes signed manifests. The
    /// expected shape is `<base>/manifest/<vfr_id>.json` with a
    /// detached signature at `<base>/manifest/<vfr_id>.sig`.
    pub url: String,
    /// Hex-encoded Ed25519 public key (64 hex chars) the peer signs
    /// their manifests with. Used to verify any
    /// `frontier.merged` event coming from them.
    pub public_key: String,
    /// ISO 8601 timestamp of when the peer was added to this
    /// frontier's registry.
    pub added_at: String,
    /// Optional human-readable note: "EU mirror, run by lab Z."
    /// Doesn't enter any content address; stored verbatim.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl PeerHub {
    /// Validate the structural shape of a `PeerHub` before insertion.
    /// Specifically: id must be non-empty, url must be HTTPS, and
    /// public_key must be 64 hex chars.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("peer id must be non-empty".into());
        }
        if !self.url.starts_with("https://") {
            return Err(format!(
                "peer url must start with `https://` (got `{}`)",
                self.url
            ));
        }
        let trimmed = self.public_key.trim();
        if trimmed.len() != 64 {
            return Err(format!(
                "peer public_key must be 64 hex chars (got {})",
                trimmed.len()
            ));
        }
        if hex::decode(trimmed).is_err() {
            return Err("peer public_key must be valid hex".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> PeerHub {
        PeerHub {
            id: "hub:test".into(),
            url: "https://example.invalid/".into(),
            public_key: "00".repeat(32),
            added_at: "2026-04-27T00:00:00Z".into(),
            note: String::new(),
        }
    }

    #[test]
    fn validates_correct_shape() {
        assert!(good().validate().is_ok());
    }

    #[test]
    fn rejects_empty_id() {
        let mut p = good();
        p.id = "  ".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_http_url() {
        let mut p = good();
        p.url = "http://insecure.example/".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_short_pubkey() {
        let mut p = good();
        p.public_key = "abcd".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_non_hex_pubkey() {
        let mut p = good();
        p.public_key = "z".repeat(64);
        assert!(p.validate().is_err());
    }
}
