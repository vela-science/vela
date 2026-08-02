//! Scoped verifier output: `vela.verification-record.v1`.
//!
//! Verification is an authenticated observation over exact inputs. Even a
//! passing record changes no Claim Standing without a separate authorized
//! Decision and canonical Event.

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::artifact_reference::require_artifact_reference_id;
use crate::identity::IdentityBinding;

pub const VERIFICATION_RECORD_V1_SCHEMA: &str = "vela.verification-record.v1";
pub const VERIFICATION_RECORD_AUTH_ALGORITHM: &str = "ed25519";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationSubject {
    pub claim_id: String,
    pub artifact_ids: Vec<String>,
    pub submission_id: String,
    pub submission_root: String,
    pub proposal_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationMethod {
    pub profile: String,
    pub implementation: String,
    pub environment_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationScope {
    pub property: String,
    pub does_not_establish: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependenceDisclosure {
    pub declared_independent_of: Vec<String>,
    pub shared_dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationAuthentication {
    pub algorithm: String,
    pub identity_binding: IdentityBinding,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRecordV1 {
    pub schema: String,
    pub verification_record_id: String,
    pub subject: VerificationSubject,
    pub method: VerificationMethod,
    pub scope: VerificationScope,
    pub outcome: String,
    pub verifier: String,
    pub independence: IndependenceDisclosure,
    pub output_artifact_ids: Vec<String>,
    pub started_at: String,
    pub completed_at: String,
    pub authentication: VerificationAuthentication,
}

#[derive(Debug, Clone)]
pub struct VerificationRecordDraft {
    pub subject: VerificationSubject,
    pub method: VerificationMethod,
    pub scope: VerificationScope,
    pub outcome: String,
    pub verifier: String,
    pub independence: IndependenceDisclosure,
    pub output_artifact_ids: Vec<String>,
    pub started_at: String,
    pub completed_at: String,
}

impl VerificationRecordV1 {
    pub fn build(
        draft: VerificationRecordDraft,
        identity_binding: IdentityBinding,
        key: &SigningKey,
    ) -> Result<Self, String> {
        identity_binding.verify()?;
        if identity_binding.actor_id != draft.verifier {
            return Err("Verification Record verifier must match its identity binding".into());
        }
        if identity_binding.public_key_hex != hex::encode(key.verifying_key().to_bytes()) {
            return Err(
                "Verification Record signing key does not match its identity binding".into(),
            );
        }
        let mut value = Self {
            schema: VERIFICATION_RECORD_V1_SCHEMA.to_string(),
            verification_record_id: String::new(),
            subject: draft.subject,
            method: draft.method,
            scope: draft.scope,
            outcome: draft.outcome,
            verifier: draft.verifier,
            independence: draft.independence,
            output_artifact_ids: draft.output_artifact_ids,
            started_at: draft.started_at,
            completed_at: draft.completed_at,
            authentication: VerificationAuthentication {
                algorithm: VERIFICATION_RECORD_AUTH_ALGORITHM.to_string(),
                identity_binding,
                signature: String::new(),
            },
        };
        value.validate_semantics()?;
        let preimage = value.signed_preimage()?;
        value.authentication.signature = hex::encode(crate::sign::sign_bytes(key, &preimage));
        value.verification_record_id = value.derive_id()?;
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("Verification Record exceeds the 4 MiB encoded limit".into());
        }
        let value: Self = crate::canonical::from_json_slice_strict(bytes)
            .map_err(|error| format!("parse Verification Record v1: {error}"))?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()?;
        self.authentication.identity_binding.verify()?;
        if self.authentication.identity_binding.actor_id != self.verifier {
            return Err("Verification Record authentication does not bind its verifier".into());
        }
        let preimage = self.signed_preimage()?;
        if !crate::sign::verify_action_signature(
            &preimage,
            &self.authentication.signature,
            &self.authentication.identity_binding.public_key_hex,
        )? {
            return Err("Verification Record whole-body signature does not verify".into());
        }
        let expected = self.derive_id()?;
        if expected != self.verification_record_id {
            return Err(format!(
                "Verification Record id mismatch: declared {}, rebuilt {expected}",
                self.verification_record_id
            ));
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        crate::canonical::to_canonical_bytes(self)
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        Ok(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(self.canonical_bytes()?))
        ))
    }

    fn signed_preimage(&self) -> Result<Vec<u8>, String> {
        let mut unsigned = self.clone();
        unsigned.verification_record_id.clear();
        unsigned.authentication.signature.clear();
        crate::canonical::to_canonical_bytes(&unsigned)
    }

    fn derive_id(&self) -> Result<String, String> {
        Ok(format!(
            "vvr_{}",
            &hex::encode(Sha256::digest(self.signed_preimage()?))[..16]
        ))
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != VERIFICATION_RECORD_V1_SCHEMA {
            return Err(format!(
                "Verification Record schema must be `{VERIFICATION_RECORD_V1_SCHEMA}`"
            ));
        }
        if self.authentication.algorithm != VERIFICATION_RECORD_AUTH_ALGORITHM {
            return Err("Verification Record authentication.algorithm must be `ed25519`".into());
        }
        if !(self.subject.claim_id.starts_with("vcl_") || self.subject.claim_id.starts_with("vf_"))
        {
            return Err(
                "Verification Record subject.claim_id must be vcl_ or historical vf_".into(),
            );
        }
        require_prefixed("subject.submission_id", &self.subject.submission_id, "vsb_")?;
        require_sha256("subject.submission_root", &self.subject.submission_root)?;
        require_prefixed("subject.proposal_id", &self.subject.proposal_id, "vpr_")?;
        for artifact_id in self
            .subject
            .artifact_ids
            .iter()
            .chain(self.output_artifact_ids.iter())
        {
            require_artifact_reference_id("Verification Record", "artifact id", artifact_id)?;
        }
        require_text("method.profile", &self.method.profile)?;
        require_text("method.implementation", &self.method.implementation)?;
        require_sha256("method.environment_root", &self.method.environment_root)?;
        require_text("scope.property", &self.scope.property)?;
        if self.scope.does_not_establish.is_empty() {
            return Err(
                "Verification Record scope must state at least one limitation or explicit nonclaim"
                    .into(),
            );
        }
        for limitation in &self.scope.does_not_establish {
            require_text("scope.does_not_establish", limitation)?;
        }
        if !["pass", "fail", "error", "inconclusive"].contains(&self.outcome.as_str()) {
            return Err(
                "Verification Record outcome must be pass, fail, error, or inconclusive".into(),
            );
        }
        require_text("verifier", &self.verifier)?;
        for actor in &self.independence.declared_independent_of {
            require_text("independence.declared_independent_of", actor)?;
            if actor == &self.verifier {
                return Err("Verification Record cannot claim independence from itself".into());
            }
        }
        for dependency in &self.independence.shared_dependencies {
            require_text("independence.shared_dependencies", dependency)?;
        }
        let started = chrono::DateTime::parse_from_rfc3339(&self.started_at)
            .map_err(|_| "Verification Record started_at must be RFC 3339".to_string())?;
        let completed = chrono::DateTime::parse_from_rfc3339(&self.completed_at)
            .map_err(|_| "Verification Record completed_at must be RFC 3339".to_string())?;
        if completed < started {
            return Err("Verification Record completed_at precedes started_at".into());
        }
        Ok(())
    }
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "Verification Record {field} must be non-empty, trimmed text"
        ));
    }
    if value.len() > 16 * 1024 {
        return Err(format!("Verification Record {field} exceeds 16 KiB"));
    }
    Ok(())
}

fn require_prefixed(field: &str, value: &str, prefix: &str) -> Result<(), String> {
    require_text(field, value)?;
    if !value.starts_with(prefix) {
        return Err(format!(
            "Verification Record {field} must start with {prefix}"
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("Verification Record {field} must be a full sha256: digest"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "Verification Record {field} must be a full sha256: digest"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{ActorClass, IdentityBindingDraft};
    use rand::rngs::OsRng;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn fixture() -> (VerificationRecordDraft, IdentityBinding, SigningKey) {
        let key = SigningKey::generate(&mut OsRng);
        let identity = IdentityBinding::build(
            IdentityBindingDraft {
                actor_id: "service:fixture-verifier".into(),
                actor_class: ActorClass::Org,
                created_at: "2026-07-26T00:00:00Z".into(),
            },
            &key,
        )
        .unwrap();
        let draft = VerificationRecordDraft {
            subject: VerificationSubject {
                claim_id: "vf_fixture".into(),
                artifact_ids: vec!["a".repeat(64)],
                submission_id: "vsb_fixture".into(),
                submission_root: root('a'),
                proposal_id: "vpr_fixture".into(),
            },
            method: VerificationMethod {
                profile: "fixture-v1".into(),
                implementation: "oci://fixture@sha256:abc".into(),
                environment_root: root('b'),
            },
            scope: VerificationScope {
                property: "The witness satisfies the bounded condition.".into(),
                does_not_establish: vec!["Scientific acceptance.".into()],
            },
            outcome: "pass".into(),
            verifier: "service:fixture-verifier".into(),
            independence: IndependenceDisclosure {
                declared_independent_of: vec!["agent:fixture".into()],
                shared_dependencies: vec!["problem specification v1".into()],
            },
            output_artifact_ids: vec!["b".repeat(64)],
            started_at: "2026-07-26T00:00:00Z".into(),
            completed_at: "2026-07-26T00:00:01Z".into(),
        };
        (draft, identity, key)
    }

    #[test]
    fn verification_record_is_signed_and_changes_no_standing() {
        let (draft, identity, key) = fixture();
        let record = VerificationRecordV1::build(draft, identity, &key).unwrap();
        assert!(record.verification_record_id.starts_with("vvr_"));
        VerificationRecordV1::parse(&record.canonical_bytes().unwrap()).unwrap();
        let value = serde_json::to_value(&record).unwrap();
        assert!(value.get("standing").is_none());
        assert!(value.get("accepted").is_none());
    }

    #[test]
    fn subject_drift_breaks_whole_body_signature() {
        let (draft, identity, key) = fixture();
        let mut record = VerificationRecordV1::build(draft, identity, &key).unwrap();
        record.subject.submission_root = root('c');
        assert!(record.verify().is_err());
    }

    #[test]
    fn current_content_hash_artifact_ids_are_valid() {
        let (mut draft, identity, key) = fixture();
        draft.subject.artifact_ids = vec!["a".repeat(64)];
        draft.output_artifact_ids = vec!["f".repeat(64)];
        VerificationRecordV1::build(draft, identity, &key).unwrap();
    }

    #[test]
    fn malformed_artifact_ids_fail_closed() {
        for artifact_id in [
            "sha256:aaaaaaaa".to_string(),
            "A".repeat(64),
            "artifact".to_string(),
        ] {
            let (mut draft, identity, key) = fixture();
            draft.subject.artifact_ids = vec![artifact_id];
            let error = VerificationRecordV1::build(draft, identity, &key).unwrap_err();
            assert!(error.contains("full lowercase content hash"), "{error}");
        }
    }
}
