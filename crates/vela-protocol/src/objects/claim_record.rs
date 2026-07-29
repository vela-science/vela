//! Current scientific assertion object: `vela.claim-record.v1`.
//!
//! Claim Records contain the assertion and its exact support identity. Standing
//! is derived from the current repository and its covered Decisions; it is
//! never stored in this object.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::artifact_reference::require_artifact_reference_id;

pub const CLAIM_RECORD_V1_SCHEMA: &str = "vela.claim-record.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimAssertion {
    pub text: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimEvidenceRef {
    pub relation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
    pub artifact_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimSource {
    pub kind: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRelation {
    pub kind: String,
    pub target_claim_id: String,
}

/// A versioned Claim body. The full canonical root is security identity; the
/// readable `vcl_` prefix is routing only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRecordV1 {
    pub schema: String,
    pub claim_id: String,
    pub revision: u32,
    pub assertion: ClaimAssertion,
    pub conditions: Vec<String>,
    pub evidence: Vec<ClaimEvidenceRef>,
    pub provenance: Vec<ClaimSource>,
    pub relations: Vec<ClaimRelation>,
    pub created_at: String,
    /// Closed protocol fields remain small. Domain detail is
    /// namespaced and canonical but cannot carry Standing or authority.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, Value>,
}

impl ClaimRecordV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        revision: u32,
        assertion: ClaimAssertion,
        conditions: Vec<String>,
        evidence: Vec<ClaimEvidenceRef>,
        provenance: Vec<ClaimSource>,
        relations: Vec<ClaimRelation>,
        created_at: String,
        extensions: BTreeMap<String, Value>,
    ) -> Result<Self, String> {
        let mut value = Self {
            schema: CLAIM_RECORD_V1_SCHEMA.to_string(),
            claim_id: String::new(),
            revision,
            assertion,
            conditions,
            evidence,
            provenance,
            relations,
            created_at,
            extensions,
        };
        value.validate_semantics()?;
        value.claim_id = value.derive_id()?;
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("Claim Record exceeds the 8 MiB encoded limit".into());
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse Claim Record v1: {error}"))?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()?;
        let expected = self.derive_id()?;
        if self.claim_id != expected {
            return Err(format!(
                "Claim Record id mismatch: declared {}, rebuilt {expected}",
                self.claim_id
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
        #[derive(Serialize)]
        struct ClaimIdentity<'a> {
            schema: &'static str,
            revision: u32,
            assertion: &'a ClaimAssertion,
            conditions: &'a [String],
            evidence: &'a [ClaimEvidenceRef],
            provenance: &'a [ClaimSource],
        }
        let bytes = crate::canonical::to_canonical_bytes(&ClaimIdentity {
            schema: "vela.claim-identity.v1",
            revision: self.revision,
            assertion: &self.assertion,
            conditions: &self.conditions,
            evidence: &self.evidence,
            provenance: &self.provenance,
        })?;
        Ok(format!("vcl_{}", hex::encode(Sha256::digest(bytes))))
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != CLAIM_RECORD_V1_SCHEMA {
            return Err(format!(
                "Claim Record schema must be `{CLAIM_RECORD_V1_SCHEMA}`"
            ));
        }
        if self.revision == 0 {
            return Err("Claim Record revision must be positive".into());
        }
        require_scientific_text("assertion.text", &self.assertion.text)?;
        require_text("assertion.kind", &self.assertion.kind)?;
        for condition in &self.conditions {
            require_scientific_text("conditions", condition)?;
        }
        for evidence in &self.evidence {
            require_text("evidence.relation", &evidence.relation)?;
            if let Some(artifact_id) = &evidence.artifact_id {
                require_artifact_reference_id("Claim Record", "evidence.artifact_id", artifact_id)?;
            }
            require_sha256("evidence.artifact_root", &evidence.artifact_root)?;
            if let Some(path) = &evidence.artifact_path {
                require_relative_path("evidence.artifact_path", path)?;
            }
        }
        for source in &self.provenance {
            require_text("provenance.kind", &source.kind)?;
            require_text("provenance.title", &source.title)?;
            if let Some(locator) = &source.locator {
                require_text("provenance.locator", locator)?;
            }
            for author in &source.authors {
                require_text("provenance.authors", author)?;
            }
        }
        for relation in &self.relations {
            require_text("relations.kind", &relation.kind)?;
            require_full_claim_id("relations.target_claim_id", &relation.target_claim_id)?;
        }
        chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|_| "Claim Record created_at must be RFC 3339".to_string())?;
        for (namespace, value) in &self.extensions {
            if !namespace.contains('.') || namespace.starts_with('.') || namespace.ends_with('.') {
                return Err(format!(
                    "Claim Record extension `{namespace}` must use a dotted namespace"
                ));
            }
            if !value.is_object() {
                return Err(format!(
                    "Claim Record extension `{namespace}` must be a JSON object"
                ));
            }
            reject_authority_extension(namespace, value)?;
        }
        Ok(())
    }
}

fn reject_authority_extension(namespace: &str, value: &Value) -> Result<(), String> {
    const FORBIDDEN: &[&str] = &[
        "standing",
        "accepted",
        "accepted_state",
        "decision",
        "authority",
        "signature",
    ];
    if let Some(object) = value.as_object() {
        for key in object.keys() {
            if FORBIDDEN.contains(&key.as_str()) {
                return Err(format!(
                    "Claim Record extension `{namespace}` cannot carry authority field `{key}`"
                ));
            }
        }
    }
    Ok(())
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "Claim Record {field} must be non-empty, trimmed text"
        ));
    }
    Ok(())
}

fn require_scientific_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value != value.trim()
        || value
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
    {
        return Err(format!(
            "Claim Record {field} must be non-empty, trimmed scientific text"
        ));
    }
    Ok(())
}

fn require_full_claim_id(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("vcl_")
        .ok_or_else(|| format!("Claim Record {field} must be a full vcl_ digest"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("Claim Record {field} must be a full vcl_ digest"));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("Claim Record {field} must be a full sha256: digest"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "Claim Record {field} must be a full sha256: digest"
        ));
    }
    Ok(())
}

fn require_relative_path(field: &str, value: &str) -> Result<(), String> {
    require_text(field, value)?;
    if value.starts_with('/') || value.split('/').any(|segment| segment == "..") {
        return Err(format!("Claim Record {field} must be a safe relative path"));
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
    fn claim_record_is_content_addressed_and_carries_no_standing() {
        let record = ClaimRecordV1::build(
            1,
            ClaimAssertion {
                text: "A bounded computation returned no witness.".into(),
                kind: "computational".into(),
            },
            vec!["Range 1..100 inclusive.".into()],
            vec![ClaimEvidenceRef {
                relation: "supports".into(),
                artifact_id: Some("a".repeat(64)),
                artifact_root: root('a'),
                artifact_path: Some(format!("records/artifacts/sha256/{}", "a".repeat(64))),
            }],
            vec![ClaimSource {
                kind: "repository_import".into(),
                title: "Historical Finding vf_fixture".into(),
                locator: None,
                authors: vec![],
                year: None,
            }],
            vec![],
            "2026-07-27T00:00:00Z".into(),
            BTreeMap::new(),
        )
        .unwrap();
        assert!(record.claim_id.starts_with("vcl_"));
        ClaimRecordV1::parse(&record.canonical_bytes().unwrap()).unwrap();

        let mut raw = serde_json::to_value(&record).unwrap();
        raw["claim_id"] = Value::String("vcl_tampered".into());
        assert!(ClaimRecordV1::parse(&serde_json::to_vec(&raw).unwrap()).is_err());
    }

    #[test]
    fn extension_cannot_smuggle_standing_or_authority() {
        let mut extensions = BTreeMap::new();
        extensions.insert(
            "example.extension.v1".into(),
            serde_json::json!({"standing": "accepted"}),
        );
        let error = ClaimRecordV1::build(
            1,
            ClaimAssertion {
                text: "Claim".into(),
                kind: "theoretical".into(),
            },
            vec![],
            vec![],
            vec![],
            vec![],
            "2026-07-27T00:00:00Z".into(),
            extensions,
        )
        .unwrap_err();
        assert!(error.contains("cannot carry authority field"));
    }

    #[test]
    fn scientific_assertions_preserve_bounded_multiline_text() {
        let record = ClaimRecordV1::build(
            1,
            ClaimAssertion {
                text: "Let n be positive.\n\nThen the bounded conclusion follows.".into(),
                kind: "theoretical".into(),
            },
            vec!["Assume the stated hypotheses.\nUse the exact pinned source.".into()],
            vec![],
            vec![],
            vec![],
            "2026-07-27T00:00:00Z".into(),
            BTreeMap::new(),
        )
        .unwrap();
        assert!(record.assertion.text.contains("\n\n"));
        ClaimRecordV1::parse(&record.canonical_bytes().unwrap()).unwrap();
    }

    #[test]
    fn current_content_hash_evidence_is_valid() {
        let record = ClaimRecordV1::build(
            1,
            ClaimAssertion {
                text: "A retained verifier reproduced the bounded result.".into(),
                kind: "computational".into(),
            },
            vec!["Exact retained inputs only.".into()],
            vec![ClaimEvidenceRef {
                relation: "supports".into(),
                artifact_id: Some("a".repeat(64)),
                artifact_root: root('a'),
                artifact_path: Some(format!("records/artifacts/sha256/{}", "a".repeat(64))),
            }],
            vec![ClaimSource {
                kind: "repository".into(),
                title: "Current content-addressed evidence".into(),
                locator: None,
                authors: vec![],
                year: None,
            }],
            vec![],
            "2026-07-29T00:00:00Z".into(),
            BTreeMap::new(),
        )
        .unwrap();
        assert!(record.claim_id.starts_with("vcl_"));
    }

    #[test]
    fn malformed_evidence_artifact_id_fails_closed() {
        let error = ClaimRecordV1::build(
            1,
            ClaimAssertion {
                text: "A retained verifier reproduced the bounded result.".into(),
                kind: "computational".into(),
            },
            vec![],
            vec![ClaimEvidenceRef {
                relation: "supports".into(),
                artifact_id: Some("sha256:short".into()),
                artifact_root: root('a'),
                artifact_path: None,
            }],
            vec![],
            vec![],
            "2026-07-29T00:00:00Z".into(),
            BTreeMap::new(),
        )
        .unwrap_err();
        assert!(error.contains("full lowercase content hash"));
    }
}
