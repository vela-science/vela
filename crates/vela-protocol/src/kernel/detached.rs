//! Detached artifact signatures: one schema for "a human signed these
//! exact bytes" (`vela.detached-signature.v0.1`), superseding the
//! per-script shapes that grew around the fixtures manifest and the
//! policy ceremony. The subject is signed as raw bytes on disk — no
//! canonicalization step to drift — and the record is self-describing
//! (it carries the subject's sha256 so a verifier can name WHICH bytes
//! it expected when they've changed).

use ed25519_dalek::{Signer, Verifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DETACHED_SIGNATURE_SCHEMA: &str = "vela.detached-signature.v0.1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DetachedSignature {
    pub schema: String,
    /// The subject's file name (informational; the bytes are the truth).
    pub subject: String,
    /// sha256 of the exact signed bytes.
    pub subject_sha256: String,
    pub signer_pubkey_hex: String,
    pub signature_hex: String,
    pub signed_at: String,
}

/// Sign exact bytes under an Ed25519 key. `signed_at` is caller-supplied
/// so ceremonies stay reproducible in tests.
pub fn sign_detached(
    subject: &str,
    bytes: &[u8],
    key: &ed25519_dalek::SigningKey,
    signed_at: &str,
) -> DetachedSignature {
    let sig = key.sign(bytes);
    DetachedSignature {
        schema: DETACHED_SIGNATURE_SCHEMA.to_string(),
        subject: subject.to_string(),
        subject_sha256: hex::encode(Sha256::digest(bytes)),
        signer_pubkey_hex: hex::encode(key.verifying_key().to_bytes()),
        signature_hex: hex::encode(sig.to_bytes()),
        signed_at: signed_at.to_string(),
    }
}

/// Verify a detached signature over exact bytes. Errors name what
/// diverged: content drift (digest mismatch) is reported distinctly
/// from a bad signature, because the operator's next step differs
/// (re-sign deliberately changed bytes vs investigate a forgery).
pub fn verify_detached(bytes: &[u8], record: &DetachedSignature) -> Result<(), String> {
    if record.schema != DETACHED_SIGNATURE_SCHEMA {
        return Err(format!("unknown schema `{}`", record.schema));
    }
    let digest = hex::encode(Sha256::digest(bytes));
    if digest != record.subject_sha256 {
        return Err(format!(
            "subject bytes changed since signing: sha256 {} now, {} signed",
            &digest[..16],
            &record.subject_sha256[..16.min(record.subject_sha256.len())]
        ));
    }
    let pk: [u8; 32] = hex::decode(&record.signer_pubkey_hex)
        .map_err(|e| format!("pubkey hex: {e}"))?
        .try_into()
        .map_err(|_| "pubkey must be 32 bytes".to_string())?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&pk).map_err(|e| e.to_string())?;
    let sig: [u8; 64] = hex::decode(&record.signature_hex)
        .map_err(|e| format!("signature hex: {e}"))?
        .try_into()
        .map_err(|_| "signature must be 64 bytes".to_string())?;
    vk.verify(bytes, &ed25519_dalek::Signature::from_bytes(&sig))
        .map_err(|_| "signature does not verify over the subject bytes".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_tamper() {
        let key = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
        let bytes = b"the exact manifest bytes";
        let rec = sign_detached("manifest.json", bytes, &key, "2026-07-03T00:00:00Z");
        verify_detached(bytes, &rec).expect("round trip");
        let err = verify_detached(b"tampered bytes", &rec).unwrap_err();
        assert!(err.contains("changed since signing"), "{err}");
        let mut forged = rec.clone();
        forged.signature_hex = "00".repeat(64);
        let err = verify_detached(bytes, &forged).unwrap_err();
        assert!(err.contains("does not verify"), "{err}");
    }
}
