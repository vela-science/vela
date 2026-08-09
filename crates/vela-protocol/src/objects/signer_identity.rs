//! Who signed a portable object, and under which key: `vela.signer-identity.v1`.
//!
//! This declares three things about the actor behind a Submission or
//! Verification Record: a stable namespaced id, the class of actor it is, and
//! the Ed25519 key that must have signed the envelope carrying it.
//!
//! ## Why it no longer signs itself
//!
//! Its predecessor, `vela.identity_binding.v0.1`, carried its own signature
//! over its own zeroed preimage, plus a `vib_` handle derived from that same
//! preimage. The signature proved possession: the key being declared had
//! signed the declaration, so nobody could name a key they did not hold.
//!
//! Under DSSE the enclosing envelope proves exactly that and proves more of
//! it. The envelope signature covers the whole payload — this identity
//! included — and it must verify under the key named here, so a payload
//! naming a key its signer does not hold fails before any field is read. The
//! nested signature restated one half of a fact the outer one already
//! established, which is the duplication ADR 0035 §2 rules out: "The outer
//! signature proves possession; a nested second raw signature must not repeat
//! the same fact."
//!
//! ## The honest limit, unchanged
//!
//! This proves key possession and declares a class. It does not prove the
//! actor is a distinct person — one key holder can mint a hundred identities —
//! and `actor_class` must never be read as repository or review authority.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SIGNER_IDENTITY_V1_SCHEMA: &str = "vela.signer-identity.v1";

/// What kind of actor controls the id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorClass {
    Human,
    Agent,
    Org,
}

/// The actor and key behind one signed payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignerIdentityV1 {
    #[schemars(schema_with = "crate::wire_schema::signer_identity_schema_tag")]
    pub schema: String,
    /// Stable namespaced id, e.g. `agent:erdos-search` or `reviewer:a-person`.
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub actor_id: String,
    pub actor_class: ActorClass,
    /// The Ed25519 key the enclosing envelope must verify under.
    #[schemars(schema_with = "crate::wire_schema::public_key_hex")]
    pub public_key_hex: String,
    #[schemars(schema_with = "crate::wire_schema::timestamp")]
    pub declared_at: String,
}

impl SignerIdentityV1 {
    /// Declare the identity of a key the caller holds.
    pub fn new(
        actor_id: impl Into<String>,
        actor_class: ActorClass,
        key: &ed25519_dalek::SigningKey,
        declared_at: impl Into<String>,
    ) -> Result<Self, String> {
        let value = Self {
            schema: SIGNER_IDENTITY_V1_SCHEMA.to_string(),
            actor_id: actor_id.into(),
            actor_class,
            public_key_hex: hex::encode(key.verifying_key().to_bytes()),
            declared_at: declared_at.into(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SIGNER_IDENTITY_V1_SCHEMA {
            return Err(format!(
                "signer identity schema must be `{SIGNER_IDENTITY_V1_SCHEMA}`"
            ));
        }
        if self.actor_id.trim().is_empty() || self.actor_id != self.actor_id.trim() {
            return Err("signer identity actor_id must be non-empty, trimmed text".into());
        }
        crate::shape::require_bounded_text(
            "signer identity actor_id",
            &self.actor_id,
            crate::wire_schema::TEXT_MAX_BYTES,
        )?;
        if !crate::shape::is_lower_hex_64(&self.public_key_hex) {
            return Err(
                "signer identity public_key_hex must be 64 lowercase hexadecimal characters".into(),
            );
        }
        crate::shape::parse_canonical_time("signer identity declared_at", &self.declared_at)?;
        Ok(())
    }
}

/// Read the public key a payload declares, before anything is verified.
///
/// A self-declared payload has to be read once ahead of its own signature
/// check, to learn which key that check should use. This reads exactly one
/// field and returns it, so nothing else unauthenticated can reach a decision;
/// the closed, strict parse of the payload happens afterwards, over the same
/// bytes the signature covered.
pub(crate) fn declared_public_key(name: &str, payload: &[u8]) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Declared {
        identity: DeclaredKey,
    }
    #[derive(Deserialize)]
    struct DeclaredKey {
        public_key_hex: String,
    }

    let declared: Declared = serde_json::from_slice(payload)
        .map_err(|error| format!("{name} payload declares no signer identity: {error}"))?;
    Ok(declared.identity.public_key_hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;

    fn key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn a_declared_identity_names_the_key_it_was_built_from() {
        let key = key();
        let identity = SignerIdentityV1::new(
            "agent:fixture",
            ActorClass::Agent,
            &key,
            "2026-08-09T00:00:00Z",
        )
        .unwrap();
        assert_eq!(
            identity.public_key_hex,
            hex::encode(key.verifying_key().to_bytes())
        );
        identity.validate().unwrap();
    }

    #[test]
    fn unknown_fields_are_rejected_before_anything_else() {
        let identity = SignerIdentityV1::new(
            "agent:fixture",
            ActorClass::Agent,
            &key(),
            "2026-08-09T00:00:00Z",
        )
        .unwrap();
        let mut value = serde_json::to_value(identity).unwrap();
        value["signature"] = serde_json::json!("a signature this type no longer carries");
        assert!(serde_json::from_value::<SignerIdentityV1>(value).is_err());
    }

    #[test]
    fn a_malformed_key_or_time_fails_closed() {
        let mut identity = SignerIdentityV1::new(
            "agent:fixture",
            ActorClass::Agent,
            &key(),
            "2026-08-09T00:00:00Z",
        )
        .unwrap();
        identity.public_key_hex = "00".into();
        assert!(identity.validate().is_err());

        let mut identity = SignerIdentityV1::new(
            "agent:fixture",
            ActorClass::Agent,
            &key(),
            "2026-08-09T00:00:00Z",
        )
        .unwrap();
        identity.declared_at = "2026-08-09T00:00:00+00:00".into();
        assert!(identity.validate().is_err());
    }
}
