//! Candidate transition: `vela.proposal.v1`.
//!
//! A Proposal is minted by the repository rather than signed by a producer, so
//! it has no envelope. It does share the identity rule: `vpr_` is derived from
//! the Proposal's canonical root by [`ProposalV1::id`] and is not a
//! stored field. It used to be one, over a preimage built by clearing it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PROPOSAL_V1_SCHEMA: &str = "vela.proposal.v1";
pub const PROPOSAL_HANDLE_PREFIX: &str = "vpr_";

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
        let value = Self {
            schema: PROPOSAL_V1_SCHEMA.to_string(),
            action,
            subject,
            actor,
            created_at,
            reason,
            producer_package,
            caveats,
        };
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
        self.validate_semantics()
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        crate::canonical::to_canonical_bytes(self)
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        Ok(crate::canonical::sha256_root(&self.canonical_bytes()?))
    }

    /// The readable `vpr_` handle for this Proposal's canonical root.
    ///
    /// Infallible, unlike the roots of the objects that carry evidence.
    /// Canonicalization refuses only non-finite floats and integers outside
    /// the interoperable range, and every field of a Proposal is a bounded
    /// string — so there is nothing here for it to refuse, and the read
    /// surfaces that render a Proposal do not have to carry a `Result`
    /// through every closure to print its name.
    pub fn id(&self) -> String {
        let root = self
            .canonical_root()
            .expect("a Proposal carries no numeric field that canonicalization can refuse");
        crate::shape::derive_handle(PROPOSAL_HANDLE_PREFIX, &root)
            .expect("a full sha256 root always derives a handle")
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
        if self.producer_package.kind != "submission_v2" {
            return Err("Proposal producer_package.kind must be `submission_v2`".into());
        }
        require_sha256("producer_package.root", &self.producer_package.root)?;
        crate::shape::require_derived_handle(
            "Proposal producer_package.id",
            &self.producer_package.id,
            "vsb_",
            &self.producer_package.root,
        )?;
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

    fn producer_package() -> ProposalProducerPackage {
        ProposalProducerPackage {
            kind: "submission_v2".into(),
            id: crate::shape::derive_handle("vsb_", &root('b')).unwrap(),
            root: root('b'),
            path: format!("records/submissions/sha256/{}.json", "b".repeat(64)),
        }
    }

    fn build(package: ProposalProducerPackage) -> Result<ProposalV1, String> {
        ProposalV1::build(
            "claim.add".into(),
            ProposalSubject {
                kind: "claim".into(),
                id: format!("vcl_{}", "a".repeat(64)),
                root: root('a'),
            },
            "agent:producer".into(),
            "2026-07-27T00:00:00Z".into(),
            "Submit bounded evidence for review.".into(),
            package,
            vec!["Bounded result only.".into()],
        )
    }

    #[test]
    fn a_proposal_derives_its_handle_from_its_own_root() {
        let value = build(producer_package()).unwrap();
        assert_eq!(
            value.id(),
            crate::shape::derive_handle("vpr_", &value.canonical_root().unwrap()).unwrap()
        );
        ProposalV1::parse(&value.canonical_bytes().unwrap()).unwrap();

        // The handle is not a stored field, so it cannot disagree with the
        // bytes and cannot be carried across an edit.
        let mut value = serde_json::to_value(&value).unwrap();
        assert!(value.get("proposal_id").is_none());
        value["proposal_id"] = serde_json::json!("vpr_0000000000000000");
        assert!(ProposalV1::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn the_producer_package_handle_must_derive_from_its_root() {
        let mut package = producer_package();
        package.id = crate::shape::derive_handle("vsb_", &root('c')).unwrap();
        let error = build(package).unwrap_err();
        assert!(error.contains("producer_package.id"), "{error}");
        assert!(error.contains("the handle its root derives"), "{error}");
    }

    #[test]
    fn receipt_backed_proposal_is_not_current() {
        let error = build(ProposalProducerPackage {
            kind: "receipt_v1".into(),
            id: "vrc_fixture".into(),
            root: root('b'),
            path: "records/receipts/sha256/fixture.json".into(),
        })
        .unwrap_err();
        assert!(error.contains("submission_v2"));
    }
}
