//! Vela-issued intake record: `vela.registration-record.v1`.
//!
//! This object proves which exact Submission crossed Vela's canonical intake
//! transaction and which records resulted. It is not a truth, verification, or
//! acceptance receipt.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::artifact_reference::require_artifact_reference_id;

pub const REGISTRATION_RECORD_V1_SCHEMA: &str = "vela.registration-record.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationRoots {
    pub event_log_before: String,
    pub event_log_after: String,
    pub proposal_after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationRecordV1 {
    pub schema: String,
    pub registration_record_id: String,
    pub frontier_id: String,
    pub submission_id: String,
    pub submission_root: String,
    pub submission_path: String,
    pub registered_at: String,
    pub registered_by: String,
    pub producer_identity_binding_id: String,
    pub artifact_ids: Vec<String>,
    /// Claim Record ids use `vcl_` after that separately gated era ships.
    /// Submission v1 may temporarily reference a historical `vf_` claim.
    pub claim_id: String,
    pub proposal_id: String,
    pub route: String,
    pub transaction_root: String,
    pub roots: RegistrationRoots,
    pub accepted_state_changed: bool,
}

impl RegistrationRecordV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        frontier_id: String,
        submission_id: String,
        submission_root: String,
        submission_path: String,
        registered_at: String,
        registered_by: String,
        producer_identity_binding_id: String,
        artifact_ids: Vec<String>,
        claim_id: String,
        proposal_id: String,
        route: String,
        transaction_root: String,
        roots: RegistrationRoots,
        accepted_state_changed: bool,
    ) -> Result<Self, String> {
        let mut value = Self {
            schema: REGISTRATION_RECORD_V1_SCHEMA.to_string(),
            registration_record_id: String::new(),
            frontier_id,
            submission_id,
            submission_root,
            submission_path,
            registered_at,
            registered_by,
            producer_identity_binding_id,
            artifact_ids,
            claim_id,
            proposal_id,
            route,
            transaction_root,
            roots,
            accepted_state_changed,
        };
        value.validate_semantics()?;
        value.registration_record_id = value.derive_id()?;
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 1024 * 1024 {
            return Err("Registration Record exceeds the 1 MiB encoded limit".into());
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse Registration Record v1: {error}"))?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()?;
        let expected = self.derive_id()?;
        if expected != self.registration_record_id {
            return Err(format!(
                "Registration Record id mismatch: declared {}, rebuilt {expected}",
                self.registration_record_id
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

    fn derive_id(&self) -> Result<String, String> {
        let mut body = self.clone();
        body.registration_record_id.clear();
        let bytes = crate::canonical::to_canonical_bytes(&body)?;
        Ok(format!("vrr_{}", &hex::encode(Sha256::digest(bytes))[..16]))
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != REGISTRATION_RECORD_V1_SCHEMA {
            return Err(format!(
                "Registration Record schema must be `{REGISTRATION_RECORD_V1_SCHEMA}`"
            ));
        }
        require_prefixed("frontier_id", &self.frontier_id, "vfr_")?;
        require_prefixed("submission_id", &self.submission_id, "vsb_")?;
        require_sha256("submission_root", &self.submission_root)?;
        let expected_path = format!(
            "records/submissions/sha256/{}.json",
            self.submission_root
                .strip_prefix("sha256:")
                .expect("validated directly above")
        );
        if self.submission_path != expected_path {
            return Err(format!(
                "Registration Record submission_path must be `{expected_path}`"
            ));
        }
        chrono::DateTime::parse_from_rfc3339(&self.registered_at)
            .map_err(|_| "Registration Record registered_at must be RFC 3339".to_string())?;
        require_text("registered_by", &self.registered_by)?;
        require_prefixed(
            "producer_identity_binding_id",
            &self.producer_identity_binding_id,
            "vib_",
        )?;
        for artifact_id in &self.artifact_ids {
            require_artifact_reference_id("Registration Record", "artifact_ids", artifact_id)?;
        }
        if !(self.claim_id.starts_with("vcl_") || self.claim_id.starts_with("vf_")) {
            return Err("Registration Record claim_id must be vcl_ or historical vf_".into());
        }
        require_prefixed("proposal_id", &self.proposal_id, "vpr_")?;
        if !["pending_review", "accepted_by_policy"].contains(&self.route.as_str()) {
            return Err(
                "Registration Record route must be pending_review or accepted_by_policy".into(),
            );
        }
        require_sha256("transaction_root", &self.transaction_root)?;
        require_sha256("roots.event_log_before", &self.roots.event_log_before)?;
        require_sha256("roots.event_log_after", &self.roots.event_log_after)?;
        require_sha256("roots.proposal_after", &self.roots.proposal_after)?;
        if self.route == "pending_review" && self.accepted_state_changed {
            return Err(
                "pending_review Registration Record cannot report accepted_state_changed".into(),
            );
        }
        Ok(())
    }
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "Registration Record {field} must be non-empty, trimmed text"
        ));
    }
    Ok(())
}

fn require_prefixed(field: &str, value: &str, prefix: &str) -> Result<(), String> {
    require_text(field, value)?;
    if !value.starts_with(prefix) {
        return Err(format!(
            "Registration Record {field} must start with {prefix}"
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("Registration Record {field} must be a full sha256: digest"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "Registration Record {field} must be a full sha256: digest"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    #[test]
    fn registration_record_is_content_addressed_and_non_authoritative() {
        let record = RegistrationRecordV1::build(
            "vfr_fixture".into(),
            "vsb_fixture".into(),
            root('a'),
            format!("records/submissions/sha256/{}.json", "a".repeat(64)),
            "2026-07-26T00:00:00Z".into(),
            "vela-cli@0.940.0".into(),
            "vib_fixture".into(),
            vec!["va_fixture".into()],
            "vf_fixture".into(),
            "vpr_fixture".into(),
            "pending_review".into(),
            root('b'),
            RegistrationRoots {
                event_log_before: root('c'),
                event_log_after: root('c'),
                proposal_after: root('d'),
            },
            false,
        )
        .unwrap();
        assert!(record.registration_record_id.starts_with("vrr_"));
        RegistrationRecordV1::parse(&record.canonical_bytes().unwrap()).unwrap();
    }

    #[test]
    fn pending_registration_cannot_claim_accepted_change() {
        let error = RegistrationRecordV1::build(
            "vfr_fixture".into(),
            "vsb_fixture".into(),
            root('a'),
            format!("records/submissions/sha256/{}.json", "a".repeat(64)),
            "2026-07-26T00:00:00Z".into(),
            "vela-cli@0.940.0".into(),
            "vib_fixture".into(),
            vec![],
            "vf_fixture".into(),
            "vpr_fixture".into(),
            "pending_review".into(),
            root('b'),
            RegistrationRoots {
                event_log_before: root('c'),
                event_log_after: root('c'),
                proposal_after: root('d'),
            },
            true,
        )
        .unwrap_err();
        assert!(
            error.contains("cannot report accepted_state_changed"),
            "{error}"
        );
    }

    #[test]
    fn current_content_hash_artifact_ids_are_valid() {
        RegistrationRecordV1::build(
            "vfr_fixture".into(),
            "vsb_fixture".into(),
            root('a'),
            format!("records/submissions/sha256/{}.json", "a".repeat(64)),
            "2026-07-29T00:00:00Z".into(),
            "vela-cli@0.940.9".into(),
            "vib_fixture".into(),
            vec!["f".repeat(64)],
            "vcl_fixture".into(),
            "vpr_fixture".into(),
            "pending_review".into(),
            root('b'),
            RegistrationRoots {
                event_log_before: root('c'),
                event_log_after: root('c'),
                proposal_after: root('d'),
            },
            false,
        )
        .unwrap();
    }
}
