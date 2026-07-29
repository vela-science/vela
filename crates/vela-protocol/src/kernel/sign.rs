//! Ed25519 primitives for authority-free producer and verifier records.
//!
//! This module does not implement repository authority or scientific
//! Decisions. Current repository authority is authenticated by the dedicated
//! runtime boundary; producer identities, Submissions, and Verification
//! Records use only the small byte-signing surface below.

use std::path::Path;

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};

/// Generate an Ed25519 keypair for an authority-free producer or verifier.
///
/// The private seed is written as lowercase hex to `private.key`; the public
/// key is written to `public.key`. Repository-authority credentials are never
/// loaded through this path.
pub fn generate_keypair(output_dir: &Path) -> Result<String, String> {
    use rand::rngs::OsRng;

    std::fs::create_dir_all(output_dir)
        .map_err(|error| format!("failed to create output directory: {error}"))?;
    let signing_key = SigningKey::generate(&mut OsRng);
    let public_hex = pubkey_hex(&signing_key);
    let private_path = output_dir.join("private.key");
    std::fs::write(&private_path, hex::encode(signing_key.to_bytes()))
        .map_err(|error| format!("failed to write private key: {error}"))?;
    harden_key_permissions(&private_path);
    std::fs::write(output_dir.join("public.key"), &public_hex)
        .map_err(|error| format!("failed to write public key: {error}"))?;
    Ok(public_hex)
}

/// Set owner-only permissions on a seed file where supported.
pub fn harden_key_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600);
            let _ = std::fs::set_permissions(path, permissions);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// Parse a lowercase or uppercase hex-encoded 32-byte Ed25519 seed.
pub fn signing_key_from_hex(hex_str: &str) -> Result<SigningKey, String> {
    let bytes =
        hex::decode(hex_str.trim()).map_err(|error| format!("invalid private-key hex: {error}"))?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "private key must be exactly 32 bytes".to_string())?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Load the producer/verifier seed format emitted by [`generate_keypair`].
pub fn load_signing_key_from_path(path: &Path) -> Result<SigningKey, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read private key: {error}"))?;
    signing_key_from_hex(&source)
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
        use rand::rngs::OsRng;

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
