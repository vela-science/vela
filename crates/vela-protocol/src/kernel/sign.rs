//! Ed25519 primitives for authority-free producer and verifier records.
//!
//! This module does not implement repository authority or scientific
//! Decisions. Current repository authority is authenticated by the dedicated
//! runtime boundary; producer identities, Submissions, and Verification
//! Records use only the small byte-signing surface below.

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};

/// Parse a lowercase or uppercase hex-encoded 32-byte Ed25519 seed.
pub fn signing_key_from_hex(hex_str: &str) -> Result<SigningKey, String> {
    let bytes =
        hex::decode(hex_str.trim()).map_err(|error| format!("invalid private-key hex: {error}"))?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "private key must be exactly 32 bytes".to_string())?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Sign exact canonical bytes.
pub fn sign_bytes(signing_key: &SigningKey, bytes: &[u8]) -> [u8; 64] {
    signing_key.sign(bytes).to_bytes()
}

/// Return the lowercase hex-encoded 32-byte public key.
pub fn pubkey_hex(signing_key: &SigningKey) -> String {
    hex::encode(signing_key.verifying_key().to_bytes())
}

/// Verify an Ed25519 signature over exact canonical bytes.
pub fn verify_action_signature(
    signing_bytes: &[u8],
    signature_hex: &str,
    expected_pubkey_hex: &str,
) -> Result<bool, String> {
    let verifying_key = parse_verifying_key(expected_pubkey_hex)?;
    let bytes =
        hex::decode(signature_hex).map_err(|error| format!("invalid signature hex: {error}"))?;
    let signature = ed25519_dalek::Signature::from_bytes(
        &bytes
            .try_into()
            .map_err(|_| "signature must be exactly 64 bytes")?,
    );
    Ok(verifying_key.verify(signing_bytes, &signature).is_ok())
}

fn parse_verifying_key(hex_str: &str) -> Result<VerifyingKey, String> {
    let bytes = hex::decode(hex_str).map_err(|error| format!("invalid public-key hex: {error}"))?;
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "public key must be exactly 32 bytes".to_string())?;
    VerifyingKey::from_bytes(&key).map_err(|error| format!("invalid public key: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_bytes_verify_and_drift_fails() {
        use rand_core::OsRng;

        let key = SigningKey::generate(&mut OsRng);
        let signature = hex::encode(sign_bytes(&key, b"bounded claim"));
        assert!(verify_action_signature(b"bounded claim", &signature, &pubkey_hex(&key)).unwrap());
        assert!(!verify_action_signature(b"changed claim", &signature, &pubkey_hex(&key)).unwrap());
    }

    #[test]
    fn malformed_keys_and_signatures_fail_closed() {
        assert!(signing_key_from_hex("00").is_err());
        assert!(verify_action_signature(b"x", "00", "00").is_err());
    }
}
