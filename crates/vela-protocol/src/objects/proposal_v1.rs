//! Candidate transition: `vela.proposal.v1`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROPOSAL_V1_SCHEMA: &str = "vela.proposal.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalSubject {
    pub kind: String,
    pub id: String,
    pub root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalProducerPackage {
    pub kind: String,
    pub id: String,
    pub root: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalV1 {
    pub schema: String,
    pub proposal_id: String,
    pub action: String,
    pub subject: ProposalSubject,
    pub actor: String,
    pub created_at: String,
    pub reason: String,
    pub producer_package: ProposalProducerPackage,
    pub caveats: Vec<String>,
}

impl ProposalV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        action: String,
        subject: ProposalSubject,
        actor: String,
        created_at: String,
        reason: String,
        producer_package: ProposalProducerPackage,
        caveats: Vec<String>,
    ) -> Result<Self, String> {
        let mut value = Self {
            schema: PROPOSAL_V1_SCHEMA.to_string(),
            proposal_id: String::new(),
            action,
            subject,
            actor,
            created_at,
            reason,
            producer_package,
            caveats,
        };
        value.validate_semantics()?;
        value.proposal_id = value.derive_id()?;
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("Proposal exceeds the 2 MiB encoded limit".into());
        }
        let value: Self = crate::canonical::from_json_slice_strict(bytes)
            .map_err(|error| format!("parse Proposal v1: {error}"))?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()?;
        let expected = self.derive_id()?;
        if self.proposal_id != expected {
            return Err(format!(
                "Proposal id mismatch: declared {}, rebuilt {expected}",
                self.proposal_id
            ));
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        crate::canonical::to_canonical_bytes(self)
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        Ok(crate::canonical::sha256_root(&self.canonical_bytes()?))
    }

    fn derive_id(&self) -> Result<String, String> {
        let mut body = self.clone();
        body.proposal_id.clear();
        let bytes = crate::canonical::to_canonical_bytes(&body)?;
        Ok(format!("vpr_{}", &hex::encode(Sha256::digest(bytes))[..16]))
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != PROPOSAL_V1_SCHEMA {
            return Err(format!("Proposal schema must be `{PROPOSAL_V1_SCHEMA}`"));
        }
        if !["claim.add", "claim.revise", "claim.withdraw"].contains(&self.action.as_str()) {
            return Err("Proposal action is not a current transition".into());
        }
        if self.subject.kind != "claim" {
            return Err("Proposal subject.kind must be `claim`".into());
        }
        require_prefixed("subject.id", &self.subject.id, "vcl_")?;
        require_sha256("subject.root", &self.subject.root)?;
        require_text("actor", &self.actor)?;
        chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|_| "Proposal created_at must be RFC 3339".to_string())?;
        require_text("reason", &self.reason)?;
        if self.producer_package.kind != "submission_v1" {
            return Err("Proposal producer_package.kind must be `submission_v1`".into());
        }
        require_prefixed("producer_package.id", &self.producer_package.id, "vsb_")?;
        require_sha256("producer_package.root", &self.producer_package.root)?;
        require_relative_path("producer_package.path", &self.producer_package.path)?;
        for caveat in &self.caveats {
            require_text("caveats", caveat)?;
        }
        Ok(())
    }
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!("Proposal {field} must be non-empty, trimmed text"));
    }
    Ok(())
}

fn require_prefixed(field: &str, value: &str, prefix: &str) -> Result<(), String> {
    require_text(field, value)?;
    if !value.starts_with(prefix) {
        return Err(format!("Proposal {field} must start with {prefix}"));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    if crate::shape::is_full_sha256_root(value) {
        Ok(())
    } else {
        Err(format!("Proposal {field} must be a full sha256: digest"))
    }
}

fn require_relative_path(field: &str, value: &str) -> Result<(), String> {
    require_text(field, value)?;
    if value.starts_with('/') || value.split('/').any(|segment| segment == "..") {
        return Err(format!("Proposal {field} must be a safe relative path"));
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
    fn proposal_v1_requires_a_current_submission_and_claim() {
        let value = ProposalV1::build(
            "claim.add".into(),
            ProposalSubject {
                kind: "claim".into(),
                id: "vcl_fixture".into(),
                root: root('a'),
            },
            "agent:producer".into(),
            "2026-07-27T00:00:00Z".into(),
            "Submit bounded evidence for review.".into(),
            ProposalProducerPackage {
                kind: "submission_v1".into(),
                id: "vsb_fixture".into(),
                root: root('b'),
                path: format!("records/submissions/sha256/{}.json", "b".repeat(64)),
            },
            vec!["Bounded result only.".into()],
        )
        .unwrap();
        assert!(value.proposal_id.starts_with("vpr_"));
        ProposalV1::parse(&value.canonical_bytes().unwrap()).unwrap();
    }

    #[test]
    fn receipt_backed_proposal_is_not_current() {
        let error = ProposalV1::build(
            "claim.add".into(),
            ProposalSubject {
                kind: "claim".into(),
                id: "vcl_fixture".into(),
                root: root('a'),
            },
            "agent:producer".into(),
            "2026-07-27T00:00:00Z".into(),
            "Request".into(),
            ProposalProducerPackage {
                kind: "receipt_v1".into(),
                id: "vrc_fixture".into(),
                root: root('b'),
                path: "records/receipts/sha256/fixture.json".into(),
            },
            vec![],
        )
        .unwrap_err();
        assert!(error.contains("submission_v1"));
    }
}
