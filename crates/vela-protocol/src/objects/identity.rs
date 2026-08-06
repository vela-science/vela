//! Producer identity binding (`vib_`): a self-signed proof that a key controls
//! an actor id.
//!
//! An [`IdentityBinding`] is signed by the key it binds, so it proves
//! possession rather than trusting a mutable actor registry. It also records
//! `actor_class` (human / agent / org), which must never be interpreted as
//! repository or review authority.
//!
//! ## The honest limit
//!
//! This proves key possession and binds a class. It does NOT prove the actor is
//! a distinct human (one person can self-sign a hundred bindings), so it is the
//! foundation for, not the solution to, the sybil-resistance named open in
//! `docs/SIGNIFICANCE_SLOT.md`. Distinct-personhood is a layer above this one.

use ed25519_dalek::SigningKey;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const IDENTITY_BINDING_SCHEMA: &str = "vela.identity_binding.v0.1";

/// What kind of actor controls the id. Inferred-by-prefix before; bound here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorClass {
    Human,
    Agent,
    Org,
}

/// A self-signed proof that `public_key_hex` controls `actor_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentityBinding {
    #[schemars(schema_with = "crate::wire_schema::identity_binding_schema_tag")]
    pub schema: String,
    /// `vib_<16hex>`, content-addressed over the body with id/signature zeroed.
    #[schemars(schema_with = "crate::wire_schema::identity_binding_id")]
    pub binding_id: String,
    /// Stable namespaced id, e.g. "reviewer:human-reviewer".
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub actor_id: String,
    pub actor_class: ActorClass,
    /// The Ed25519 public key being bound. MUST equal the signer (proof of
    /// possession): the binding is signed by the key it binds.
    #[schemars(schema_with = "crate::wire_schema::public_key_hex")]
    pub public_key_hex: String,
    #[schemars(schema_with = "crate::wire_schema::timestamp")]
    pub created_at: String,
    #[schemars(schema_with = "crate::wire_schema::ed25519_signature")]
    pub signature: String,
}

/// Draft for [`IdentityBinding::build`].
pub struct IdentityBindingDraft {
    pub actor_id: String,
    pub actor_class: ActorClass,
    pub created_at: String,
}

impl IdentityBinding {
    /// Self-sign a binding: the key proves it controls `actor_id`.
    pub fn build(draft: IdentityBindingDraft, key: &SigningKey) -> Result<Self, String> {
        if draft.actor_id.trim().is_empty() {
            return Err("identity_binding.actor_id cannot be empty".to_string());
        }
        let mut b = IdentityBinding {
            schema: IDENTITY_BINDING_SCHEMA.to_string(),
            binding_id: String::new(),
            actor_id: draft.actor_id,
            actor_class: draft.actor_class,
            public_key_hex: hex::encode(key.verifying_key().to_bytes()),
            created_at: draft.created_at,
            signature: String::new(),
        };
        let preimage = b.id_preimage_bytes()?;
        b.signature = hex::encode(crate::sign::sign_bytes(key, &preimage));
        b.binding_id = b.derive_id()?;
        Ok(b)
    }

    fn id_preimage_bytes(&self) -> Result<Vec<u8>, String> {
        let mut p = self.clone();
        p.binding_id = String::new();
        p.signature = String::new();
        crate::canonical::to_canonical_bytes(&p)
            .map_err(|e| format!("canonicalize identity_binding preimage: {e}"))
    }

    pub fn derive_id(&self) -> Result<String, String> {
        let bytes = self.id_preimage_bytes()?;
        Ok(format!(
            "vib_{}",
            &hex::encode(Sha256::digest(&bytes))[..16]
        ))
    }

    /// Full typed credential identity over the same canonical preimage used by
    /// the readable `vib_` handle. The short handle is routing only; authority
    /// comparisons use this complete root.
    pub fn credential_root(&self) -> Result<String, String> {
        let bytes = self.id_preimage_bytes()?;
        Ok(format!("sha256:{}", hex::encode(Sha256::digest(&bytes))))
    }

    /// Verify: id re-derives, and the signature is valid under
    /// `public_key_hex` — i.e. the key being bound actually signed this. That
    /// equality (signer == bound key) is the proof of possession.
    pub fn verify(&self) -> Result<(), String> {
        if self.schema != IDENTITY_BINDING_SCHEMA {
            return Err(format!(
                "identity_binding.schema must be `{IDENTITY_BINDING_SCHEMA}`"
            ));
        }
        if !self.binding_id.starts_with("vib_") {
            return Err(format!(
                "binding id must start with `vib_`, got `{}`",
                self.binding_id
            ));
        }
        let preimage = self.id_preimage_bytes()?;
        if !crate::sign::verify_action_signature(&preimage, &self.signature, &self.public_key_hex)?
        {
            return Err(
                "identity_binding signature does not verify under the bound key \
                        (no proof of possession)"
                    .to_string(),
            );
        }
        let rederived = self.derive_id()?;
        if rederived != self.binding_id {
            return Err(format!(
                "binding_id mismatch: declared {}, rebuilt {}",
                self.binding_id, rederived
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::OsRng;

    fn key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    fn draft() -> IdentityBindingDraft {
        IdentityBindingDraft {
            actor_id: "reviewer:will-blair".into(),
            actor_class: ActorClass::Human,
            created_at: "2026-06-09T00:00:00Z".into(),
        }
    }

    #[test]
    fn self_signed_binding_verifies() {
        let b = IdentityBinding::build(draft(), &key()).unwrap();
        assert!(b.binding_id.starts_with("vib_"));
        let full = b.credential_root().unwrap();
        assert!(full.starts_with("sha256:"));
        assert_eq!(
            &full["sha256:".len()..][..16],
            &b.binding_id["vib_".len()..]
        );
        b.verify().unwrap();
    }

    #[test]
    fn identity_binding_rejects_unknown_fields_before_signature_verification() {
        let binding = IdentityBinding::build(draft(), &key()).unwrap();
        let mut value = serde_json::to_value(binding).unwrap();
        value["unexpected"] = serde_json::json!("must not be discarded");
        assert!(serde_json::from_value::<IdentityBinding>(value).is_err());
    }

    #[test]
    fn binding_signed_by_other_key_fails_possession() {
        let mut b = IdentityBinding::build(draft(), &key()).unwrap();
        // Swap in a different key's pubkey: the signature no longer matches the
        // bound key, so proof-of-possession fails.
        b.public_key_hex = hex::encode(key().verifying_key().to_bytes());
        assert!(b.verify().is_err());
    }
}
