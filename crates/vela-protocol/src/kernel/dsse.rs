//! The one DSSE envelope implementation every signed Vela object uses.
//!
//! DSSE owns transport authentication and nothing else. An envelope that
//! verifies proves that some key signed exactly these payload bytes under
//! exactly this payload type. It does not grant repository authority, select a
//! scientific outcome, or change Standing — those are the Decision path's, and
//! they read the decoded payload after this module has finished.
//!
//! Vela had two signing conventions before this module existed: repository
//! authority records used DSSE, while Submission, Verification Record and
//! producer Withdrawal each signed a bespoke preimage built by cloning the
//! object and clearing its id and signature fields. The zeroed-field
//! convention was three near-copies of the same code, it had no vectors
//! outside this repository, and it made the signed bytes something a reader
//! had to reconstruct rather than something it was handed. ADR 0035 §2 chose
//! DSSE for all of them.
//!
//! ## What the envelope contract requires
//!
//! DSSE 1.0.2 is deliberately tolerant at the envelope and strict underneath,
//! and this module implements that split:
//!
//! - the PAE is exact, and is the only thing signatures cover;
//! - both base64 alphabets are accepted, padded or not, because the spec
//!   requires a consumer to accept what any conforming producer emits;
//! - `keyid` is an unauthenticated selection hint — a signature with the wrong
//!   `keyid` and the right key is still a valid signature, and this module
//!   treats a present `keyid` only as a filter that may narrow the search;
//! - unknown envelope and signature fields are tolerated, which is why the
//!   types here do not close their field sets;
//! - a signature that does not verify is skipped rather than fatal, so one
//!   unknown or corrupt entry cannot deny a met threshold.
//!
//! Everything inside the payload is closed. [`verify`] hands back the exact
//! payload bytes it authenticated so the caller parses those and never a
//! re-encoding of them.

use std::collections::BTreeSet;

use base64::Engine as _;
use base64::engine::general_purpose::{
    STANDARD as BASE64_STANDARD, STANDARD_NO_PAD as BASE64_STANDARD_NO_PAD,
    URL_SAFE as BASE64_URL_SAFE, URL_SAFE_NO_PAD as BASE64_URL_SAFE_NO_PAD,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One DSSE signature entry.
///
/// Neither this type nor an envelope carrying it closes its field set. DSSE
/// requires the transport to tolerate entries it does not understand, so the
/// absent `deny_unknown_fields` is the rule, not an oversight, and the wire
/// schemas generated from these types stay open for the same reason. The
/// decoded Vela payload underneath remains closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("additionalProperties" = true))]
pub struct SignatureV1 {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub keyid: String,
    #[schemars(schema_with = "crate::wire_schema::base64_body")]
    pub sig: String,
}

/// One Ed25519 key a verifier is willing to accept, already decoded.
///
/// Callers select their own candidates: the authority path filters its keyset
/// by the sequence window a record claims, and a producer object offers the
/// single key its payload declares. Selection is policy and stays with the
/// caller; counting valid signatures is transport and lives here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateKey {
    pub key_id: String,
    pub public_key: [u8; 32],
}

impl CandidateKey {
    /// Build a candidate from lowercase or uppercase 32-byte hex.
    ///
    /// Returns `None` rather than an error: an unusable key in a keyset is one
    /// fewer candidate, not a reason to fail an envelope that other keys may
    /// still satisfy.
    pub fn from_hex(key_id: impl Into<String>, public_key_hex: &str) -> Option<Self> {
        let bytes = hex::decode(public_key_hex).ok()?;
        let public_key: [u8; 32] = bytes.try_into().ok()?;
        VerifyingKey::from_bytes(&public_key).ok()?;
        Some(Self {
            key_id: key_id.into(),
            public_key,
        })
    }
}

/// What a verified envelope establishes: these exact bytes, under these keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPayload {
    /// The exact authenticated bytes. Parse these; do not re-encode them.
    pub payload: Vec<u8>,
    /// The `key_id`s of the distinct candidates that verified, sorted.
    pub verified_key_ids: Vec<String>,
}

/// The one envelope every signed Vela object is stored and transported in.
///
/// The field set is open because DSSE requires it. What the envelope carries
/// is closed: each object's `open` refuses a payload type other than its own
/// before it looks at a signature, so an envelope is never read as a kind of
/// object it does not claim to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("additionalProperties" = true))]
pub struct EnvelopeV1 {
    #[serde(rename = "payloadType")]
    #[schemars(schema_with = "crate::wire_schema::vela_payload_type")]
    pub payload_type: String,
    #[schemars(schema_with = "crate::wire_schema::base64_body")]
    pub payload: String,
    // Every `open` refuses an empty list before it reads a key.
    #[schemars(length(min = 1))]
    pub signatures: Vec<SignatureV1>,
}

impl EnvelopeV1 {
    /// Wrap already-canonical payload bytes with signatures made over them.
    pub fn seal(payload_type: &str, payload: &[u8], signatures: Vec<SignatureV1>) -> Self {
        Self {
            payload_type: payload_type.to_string(),
            payload: encode_base64(payload),
            signatures,
        }
    }

    /// Seal under exactly one key. This is the producer and verifier shape:
    /// one actor, one signature, no threshold to configure.
    pub fn seal_single(key: &SigningKey, payload_type: &str, payload: &[u8]) -> Self {
        Self::seal(
            payload_type,
            payload,
            vec![sign(key, payload_type, payload)],
        )
    }

    /// The exact retained bytes of the envelope itself.
    ///
    /// This is what a repository writes and what its content address is taken
    /// over — the payload root would name the same scientific content signed
    /// by anyone, which is not what a retained object is.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        crate::canonical::to_canonical_bytes(self)
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        Ok(crate::canonical::sha256_root(&self.canonical_bytes()?))
    }

    /// Parse an envelope from retained bytes, requiring them to be canonical.
    pub fn parse(name: &str, bytes: &[u8], max_bytes: usize) -> Result<Self, String> {
        if bytes.len() > max_bytes {
            return Err(format!("{name} exceeds the {max_bytes}-byte encoded limit"));
        }
        // The envelope is open, so this is an ordinary tolerant parse; the
        // payload underneath is read strictly by the object that owns it.
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse {name} envelope: {error}"))?;
        if value.canonical_bytes()? != bytes {
            return Err(format!("{name} envelope bytes are not canonical JSON"));
        }
        Ok(value)
    }

    /// Verify against candidate keys and return the exact payload bytes.
    pub fn open(
        &self,
        name: &str,
        expected_payload_type: &str,
        candidates: &[CandidateKey],
        threshold: usize,
    ) -> Result<VerifiedPayload, String> {
        if self.payload_type != expected_payload_type {
            return Err(format!(
                "{name} payload type is `{}`, expected `{expected_payload_type}`",
                self.payload_type
            ));
        }
        verify(
            name,
            &self.payload_type,
            &self.payload,
            &self.signatures,
            candidates,
            threshold,
        )
    }

    /// Verify against the single key the payload itself declares.
    pub fn open_single(
        &self,
        name: &str,
        expected_payload_type: &str,
        public_key_hex: &str,
    ) -> Result<Vec<u8>, String> {
        let candidate = CandidateKey::from_hex(public_key_hex, public_key_hex)
            .ok_or_else(|| format!("{name} declares an unusable public key"))?;
        Ok(self
            .open(name, expected_payload_type, &[candidate], 1)?
            .payload)
    }
}

/// DSSE Pre-Authentication Encoding:
/// `DSSEv1 SP LEN(payloadType) SP payloadType SP LEN(payload) SP payload`.
pub fn pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(payload_type.len() + payload.len() + 32);
    output.extend_from_slice(b"DSSEv1 ");
    output.extend_from_slice(payload_type.len().to_string().as_bytes());
    output.push(b' ');
    output.extend_from_slice(payload_type.as_bytes());
    output.push(b' ');
    output.extend_from_slice(payload.len().to_string().as_bytes());
    output.push(b' ');
    output.extend_from_slice(payload);
    output
}

/// Decode a DSSE base64 field, accepting both alphabets, padded or not.
pub fn decode_base64(name: &str, value: &str) -> Result<Vec<u8>, String> {
    BASE64_STANDARD
        .decode(value)
        .or_else(|_| BASE64_URL_SAFE.decode(value))
        .or_else(|_| BASE64_STANDARD_NO_PAD.decode(value))
        .or_else(|_| BASE64_URL_SAFE_NO_PAD.decode(value))
        .map_err(|error| format!("{name} is not base64: {error}"))
}

/// Encode payload or signature bytes in the alphabet Vela emits.
///
/// Vela writes standard padded base64. Readers accept all four forms; a writer
/// that chose between them would make canonical bytes ambiguous.
pub fn encode_base64(bytes: &[u8]) -> String {
    BASE64_STANDARD.encode(bytes)
}

/// Sign exact payload bytes under one payload type.
pub fn sign(key: &SigningKey, payload_type: &str, payload: &[u8]) -> SignatureV1 {
    SignatureV1 {
        keyid: hex::encode(key.verifying_key().to_bytes()),
        sig: encode_base64(&key.sign(&pae(payload_type, payload)).to_bytes()),
    }
}

/// Verify an envelope against candidate keys and a signature threshold.
///
/// Returns the exact authenticated payload bytes. A signature is counted only
/// once per candidate key, so repeating one signer cannot reach a threshold of
/// two.
pub fn verify(
    name: &str,
    payload_type: &str,
    encoded_payload: &str,
    signatures: &[SignatureV1],
    candidates: &[CandidateKey],
    threshold: usize,
) -> Result<VerifiedPayload, String> {
    if threshold == 0 {
        return Err(format!("{name} signature threshold must be at least one"));
    }
    if signatures.is_empty() {
        return Err(format!("{name} has no signatures"));
    }
    let payload = decode_base64(&format!("{name} payload"), encoded_payload)?;
    let pae = pae(payload_type, &payload);

    let mut verified: BTreeSet<String> = BTreeSet::new();
    'signatures: for signed in signatures {
        let Ok(signature_bytes) = decode_base64(&format!("{name} signature"), &signed.sig) else {
            continue;
        };
        let Ok(signature_bytes) = <[u8; 64]>::try_from(signature_bytes.as_slice()) else {
            continue;
        };
        let signature = Signature::from_bytes(&signature_bytes);

        for candidate in candidates {
            if (!signed.keyid.is_empty() && signed.keyid != candidate.key_id)
                || verified.contains(&candidate.key_id)
            {
                continue;
            }
            let Ok(public_key) = VerifyingKey::from_bytes(&candidate.public_key) else {
                continue;
            };
            if public_key.verify(&pae, &signature).is_ok() {
                verified.insert(candidate.key_id.clone());
                if verified.len() >= threshold {
                    break 'signatures;
                }
                break;
            }
        }
    }
    if verified.len() < threshold {
        return Err(format!("{name} signature threshold was not met"));
    }

    Ok(VerifiedPayload {
        payload,
        verified_key_ids: verified.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::OsRng;

    fn key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    fn candidate(key: &SigningKey) -> CandidateKey {
        CandidateKey::from_hex(
            hex::encode(key.verifying_key().to_bytes()),
            &hex::encode(key.verifying_key().to_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn pae_matches_the_spec_shape() {
        assert_eq!(pae("text/plain", b"hello"), b"DSSEv1 10 text/plain 5 hello");
    }

    #[test]
    fn signed_payload_verifies_and_is_returned_exactly() {
        let key = key();
        let payload = br#"{"a":1}"#;
        let signature = sign(&key, "application/vnd.vela.test+json", payload);
        let verified = verify(
            "test envelope",
            "application/vnd.vela.test+json",
            &encode_base64(payload),
            std::slice::from_ref(&signature),
            &[candidate(&key)],
            1,
        )
        .unwrap();
        assert_eq!(verified.payload, payload);
        assert_eq!(verified.verified_key_ids.len(), 1);
    }

    #[test]
    fn payload_type_substitution_fails() {
        let key = key();
        let payload = br#"{"a":1}"#;
        let signature = sign(&key, "application/vnd.vela.test+json", payload);
        assert!(
            verify(
                "test envelope",
                "application/vnd.vela.other+json",
                &encode_base64(payload),
                &[signature],
                &[candidate(&key)],
                1,
            )
            .is_err()
        );
    }

    #[test]
    fn every_base64_alphabet_decodes() {
        let payload = [0xfb_u8, 0xff, 0xbf, 0x00];
        for encoded in [
            BASE64_STANDARD.encode(payload),
            BASE64_URL_SAFE.encode(payload),
            BASE64_STANDARD_NO_PAD.encode(payload),
            BASE64_URL_SAFE_NO_PAD.encode(payload),
        ] {
            assert_eq!(decode_base64("payload", &encoded).unwrap(), payload);
        }
    }

    #[test]
    fn an_invalid_extra_signature_does_not_deny_a_met_threshold() {
        let key = key();
        let payload = br#"{"a":1}"#;
        let good = sign(&key, "application/vnd.vela.test+json", payload);
        let junk = SignatureV1 {
            keyid: String::new(),
            sig: encode_base64(&[0_u8; 64]),
        };
        let verified = verify(
            "test envelope",
            "application/vnd.vela.test+json",
            &encode_base64(payload),
            &[junk, good],
            &[candidate(&key)],
            1,
        )
        .unwrap();
        assert_eq!(verified.payload, payload);
    }

    #[test]
    fn a_wrong_keyid_hint_does_not_verify_against_the_named_candidate() {
        let key = key();
        let payload = br#"{"a":1}"#;
        let mut signature = sign(&key, "application/vnd.vela.test+json", payload);
        signature.keyid = "not-the-key".into();
        assert!(
            verify(
                "test envelope",
                "application/vnd.vela.test+json",
                &encode_base64(payload),
                &[signature],
                &[candidate(&key)],
                1,
            )
            .is_err()
        );
    }

    #[test]
    fn one_signer_cannot_meet_a_threshold_of_two() {
        let signer = key();
        let other = key();
        let payload = br#"{"a":1}"#;
        let signature = sign(&signer, "application/vnd.vela.test+json", payload);
        assert!(
            verify(
                "test envelope",
                "application/vnd.vela.test+json",
                &encode_base64(payload),
                &[signature.clone(), signature],
                &[candidate(&signer), candidate(&other)],
                2,
            )
            .is_err()
        );
    }

    #[test]
    fn an_empty_signature_list_fails_before_a_key_is_read() {
        assert!(
            verify(
                "test envelope",
                "application/vnd.vela.test+json",
                &encode_base64(b"{}"),
                &[],
                &[],
                1,
            )
            .is_err()
        );
    }

    #[test]
    fn a_malformed_public_key_is_not_a_candidate() {
        assert!(CandidateKey::from_hex("k", "00").is_none());
        assert!(CandidateKey::from_hex("k", "zz".repeat(32).as_str()).is_none());
    }
}
