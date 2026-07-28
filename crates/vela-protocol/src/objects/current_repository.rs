//! Closed current-only repository manifest for Frontier Profile v2.
//!
//! The manifest indexes active content-addressed objects and the authority
//! material that can change their standing. It carries no decision or
//! acceptance power by itself; its exact root follows the signed
//! repository-manifest delta chain.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::bundle::Artifact;
use crate::frontier_profile::{
    FrontierProfileLicenseV1, FrontierProfileScopeV1, reject_yaml_indirection,
    validate_profile_metadata, validate_yaml_structure,
};

pub const CURRENT_REPOSITORY_SCHEMA_V2: &str = "vela.repository.v2";
pub const CURRENT_FRONTIER_PROFILE_SCHEMA_V2: &str = "vela.frontier-profile.v2";
pub const CURRENT_ARTIFACT_RECORD_SCHEMA_V1: &str = "vela.artifact-record.v1";

/// Exact current wrapper for an Artifact descriptor imported from Era 0.
///
/// The nested descriptor is preserved field-for-field. The wrapper supplies a
/// schema and predecessor root without pretending that the old descriptor was
/// originally signed or emitted as a current record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentArtifactRecordV1 {
    pub schema: String,
    pub artifact_id: String,
    pub artifact: Artifact,
    pub imported_object_root: String,
    pub predecessor_commit: String,
}

impl CurrentArtifactRecordV1 {
    pub fn build(
        artifact: Artifact,
        imported_object_root: String,
        predecessor_commit: String,
    ) -> Result<Self, String> {
        let value = Self {
            schema: CURRENT_ARTIFACT_RECORD_SCHEMA_V1.into(),
            artifact_id: artifact.id.clone(),
            artifact,
            imported_object_root,
            predecessor_commit,
        };
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse current Artifact Record v1: {error}"))?;
        value.verify()?;
        if value.canonical_bytes()? != bytes {
            return Err("current Artifact Record bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        if self.schema != CURRENT_ARTIFACT_RECORD_SCHEMA_V1 {
            return Err(format!(
                "current Artifact Record schema must be `{CURRENT_ARTIFACT_RECORD_SCHEMA_V1}`"
            ));
        }
        self.artifact.validate_reference_axes()?;
        if self.artifact_id != self.artifact.id {
            return Err("current Artifact Record ID does not match its descriptor".into());
        }
        require_sha256("artifact.imported_object_root", &self.imported_object_root)?;
        require_git_oid("artifact.predecessor_commit", &self.predecessor_commit)?;
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.verify()?;
        crate::canonical::to_canonical_bytes(self)
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        Ok(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(self.canonical_bytes()?))
        ))
    }
}

/// Current-only repository metadata.
///
/// Profile v2 deliberately keeps the small, human-editable field set from
/// Profile v1. The schema boundary says that repository identity and standing
/// now come from `.vela/epoch.json` and `.vela/repository.json`, never from an
/// Era-0 event log or generated lock file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentFrontierProfileV2 {
    pub schema: String,
    pub frontier_id: String,
    pub name: String,
    pub summary: String,
    pub scope: FrontierProfileScopeV1,
    pub maintainers: Vec<String>,
    pub license: FrontierProfileLicenseV1,
}

impl CurrentFrontierProfileV2 {
    pub fn from_yaml_str(source: &str) -> Result<Self, String> {
        reject_yaml_indirection(source)?;
        let value: serde_yaml::Value = serde_yaml::from_str(source).map_err(|error| {
            format!("invalid {CURRENT_FRONTIER_PROFILE_SCHEMA_V2} YAML: {error}")
        })?;
        validate_yaml_structure(&value)?;
        let profile: Self = serde_yaml::from_value(value).map_err(|error| {
            format!("invalid {CURRENT_FRONTIER_PROFILE_SCHEMA_V2} YAML: {error}")
        })?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != CURRENT_FRONTIER_PROFILE_SCHEMA_V2 {
            return Err(format!(
                "profile.schema must be `{CURRENT_FRONTIER_PROFILE_SCHEMA_V2}`"
            ));
        }
        validate_profile_metadata(
            &self.frontier_id,
            &self.name,
            &self.summary,
            &self.scope,
            &self.maintainers,
            &self.license,
        )
    }

    pub fn profile_root(&self) -> Result<String, String> {
        self.validate()?;
        crate::canonical::sha256_canonical(self).map(|digest| format!("sha256:{digest}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryObjectRefV1 {
    pub schema: String,
    pub id: String,
    pub root: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimStandingRefV1 {
    pub claim_id: String,
    pub claim_root: String,
    pub standing: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentRepositoryV2 {
    pub schema: String,
    pub frontier_id: String,
    pub profile_root: String,
    pub epoch_id: String,
    pub epoch_root: String,
    pub accepted_claims: Vec<ClaimStandingRefV1>,
    pub pending_claims: Vec<ClaimStandingRefV1>,
    pub proposals: Vec<RepositoryObjectRefV1>,
    pub submissions: Vec<RepositoryObjectRefV1>,
    pub registrations: Vec<RepositoryObjectRefV1>,
    pub verifications: Vec<RepositoryObjectRefV1>,
    pub artifacts: Vec<RepositoryObjectRefV1>,
    pub authority_keyset_root: String,
    pub authority_policy_root: String,
}

impl CurrentRepositoryV2 {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("current repository exceeds the 8 MiB encoded limit".into());
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse current repository v2: {error}"))?;
        value.verify()?;
        if value.canonical_bytes()? != bytes {
            return Err("current repository bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        if self.schema != CURRENT_REPOSITORY_SCHEMA_V2 {
            return Err(format!(
                "current repository schema must be `{CURRENT_REPOSITORY_SCHEMA_V2}`"
            ));
        }
        require_prefixed("frontier_id", &self.frontier_id, "vfr_")?;
        require_sha256("profile_root", &self.profile_root)?;
        require_prefixed("epoch_id", &self.epoch_id, "vre_")?;
        require_sha256("epoch_root", &self.epoch_root)?;
        require_sha256("authority_keyset_root", &self.authority_keyset_root)?;
        require_sha256("authority_policy_root", &self.authority_policy_root)?;

        verify_claim_refs("accepted_claims", &self.accepted_claims, "accepted")?;
        verify_claim_refs("pending_claims", &self.pending_claims, "pending_review")?;
        verify_object_refs("proposals", &self.proposals)?;
        verify_object_refs("submissions", &self.submissions)?;
        verify_object_refs("registrations", &self.registrations)?;
        verify_object_refs("verifications", &self.verifications)?;
        verify_object_refs("artifacts", &self.artifacts)?;

        let mut paths = BTreeSet::new();
        for path in self
            .accepted_claims
            .iter()
            .chain(&self.pending_claims)
            .map(|reference| reference.path.as_str())
            .chain(
                self.proposals
                    .iter()
                    .chain(&self.submissions)
                    .chain(&self.registrations)
                    .chain(&self.verifications)
                    .chain(&self.artifacts)
                    .map(|reference| reference.path.as_str()),
            )
        {
            if !paths.insert(path) {
                return Err(format!("current repository repeats object path `{path}`"));
            }
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.verify()?;
        crate::canonical::to_canonical_bytes(self)
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        Ok(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(self.canonical_bytes()?))
        ))
    }
}

fn verify_claim_refs(
    field: &str,
    references: &[ClaimStandingRefV1],
    expected_standing: &str,
) -> Result<(), String> {
    let mut prior = None;
    let mut roots = BTreeSet::new();
    for reference in references {
        require_full_claim_id(&format!("{field}.claim_id"), &reference.claim_id)?;
        require_sha256(&format!("{field}.claim_root"), &reference.claim_root)?;
        require_path(&format!("{field}.path"), &reference.path)?;
        if reference.standing != expected_standing {
            return Err(format!(
                "current repository {field} standing must be `{expected_standing}`"
            ));
        }
        if prior.is_some_and(|prior: &str| prior >= reference.claim_id.as_str()) {
            return Err(format!(
                "current repository {field} must be strictly sorted by Claim ID"
            ));
        }
        if !roots.insert(reference.claim_root.as_str()) {
            return Err(format!(
                "current repository {field} repeats Claim root {}",
                reference.claim_root
            ));
        }
        prior = Some(reference.claim_id.as_str());
    }
    Ok(())
}

fn verify_object_refs(field: &str, references: &[RepositoryObjectRefV1]) -> Result<(), String> {
    let mut prior = None;
    let mut roots = BTreeSet::new();
    for reference in references {
        require_text(&format!("{field}.schema"), &reference.schema)?;
        require_text(&format!("{field}.id"), &reference.id)?;
        require_sha256(&format!("{field}.root"), &reference.root)?;
        require_path(&format!("{field}.path"), &reference.path)?;
        if prior.is_some_and(|prior: &str| prior >= reference.id.as_str()) {
            return Err(format!(
                "current repository {field} must be strictly sorted by object ID"
            ));
        }
        if !roots.insert(reference.root.as_str()) {
            return Err(format!(
                "current repository {field} repeats object root {}",
                reference.root
            ));
        }
        prior = Some(reference.id.as_str());
    }
    Ok(())
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "current repository {field} must be non-empty, trimmed text"
        ));
    }
    Ok(())
}

fn require_prefixed(field: &str, value: &str, prefix: &str) -> Result<(), String> {
    require_text(field, value)?;
    if !value.starts_with(prefix) {
        return Err(format!(
            "current repository {field} must start with `{prefix}`"
        ));
    }
    Ok(())
}

fn require_full_claim_id(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("vcl_")
        .ok_or_else(|| format!("current repository {field} must be vcl_<64 lowercase hex>"))?;
    if digest.len() != 64 || !digest.bytes().all(is_lower_hex) {
        return Err(format!(
            "current repository {field} must be vcl_<64 lowercase hex>"
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("current repository {field} must be a full sha256: digest"))?;
    if digest.len() != 64 || !digest.bytes().all(is_lower_hex) {
        return Err(format!(
            "current repository {field} must be a full sha256: digest"
        ));
    }
    Ok(())
}

fn require_git_oid(field: &str, value: &str) -> Result<(), String> {
    if !matches!(value.len(), 40 | 64) || !value.bytes().all(is_lower_hex) {
        return Err(format!(
            "current repository {field} must be a full lowercase Git object ID"
        ));
    }
    Ok(())
}

fn require_path(field: &str, value: &str) -> Result<(), String> {
    require_text(field, value)?;
    if value.starts_with('/')
        || value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "..")
    {
        return Err(format!(
            "current repository {field} must be a safe relative path"
        ));
    }
    Ok(())
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn claim(byte: char, standing: &str) -> ClaimStandingRefV1 {
        ClaimStandingRefV1 {
            claim_id: format!("vcl_{}", byte.to_string().repeat(64)),
            claim_root: root(byte),
            standing: standing.into(),
            path: format!("records/claims/sha256/{}.json", byte.to_string().repeat(64)),
        }
    }

    fn fixture() -> CurrentRepositoryV2 {
        CurrentRepositoryV2 {
            schema: CURRENT_REPOSITORY_SCHEMA_V2.into(),
            frontier_id: "vfr_0123456789abcdef".into(),
            profile_root: root('a'),
            epoch_id: "vre_0123456789abcdef".into(),
            epoch_root: root('b'),
            accepted_claims: vec![claim('c', "accepted")],
            pending_claims: vec![claim('d', "pending_review")],
            proposals: vec![],
            submissions: vec![],
            registrations: vec![],
            verifications: vec![],
            artifacts: vec![],
            authority_keyset_root: root('e'),
            authority_policy_root: root('f'),
        }
    }

    #[test]
    fn current_repository_is_closed_and_rooted() {
        let repository = fixture();
        repository.verify().unwrap();
        assert!(repository.canonical_root().unwrap().starts_with("sha256:"));
        let mut tampered = repository;
        tampered.accepted_claims[0].standing = "verified".into();
        assert!(tampered.verify().is_err());
    }

    #[test]
    fn current_profile_is_native_closed_metadata() {
        let current = CurrentFrontierProfileV2 {
            schema: CURRENT_FRONTIER_PROFILE_SCHEMA_V2.into(),
            frontier_id: "vfr_0123456789abcdef".into(),
            name: "Example".into(),
            summary: "A bounded example Frontier.".into(),
            scope: FrontierProfileScopeV1 {
                question: "What is true?".into(),
                includes: vec!["Exact claims.".into()],
                excludes: vec!["Unbounded claims.".into()],
            },
            maintainers: vec!["Example Maintainer".into()],
            license: FrontierProfileLicenseV1 {
                content: "CC-BY-4.0".into(),
                code: "Apache-2.0 OR MIT".into(),
                data: "CC0-1.0".into(),
            },
        };
        let yaml = serde_yaml::to_string(&current).unwrap();
        let parsed = CurrentFrontierProfileV2::from_yaml_str(&yaml).unwrap();
        assert_eq!(parsed, current);
        assert!(current.profile_root().unwrap().starts_with("sha256:"));
    }
}
