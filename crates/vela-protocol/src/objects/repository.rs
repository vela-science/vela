//! Closed current-only repository manifest for Repository Profile v1.
//!
//! The manifest indexes active content-addressed objects and the authority
//! material that can change their standing. It carries no decision or
//! acceptance power by itself; its exact root follows the signed
//! repository-manifest delta chain.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

pub const REPOSITORY_SCHEMA_V4: &str = "vela.repository.v4";
pub const REPOSITORY_PROFILE_SCHEMA_V1: &str = "vela.repository-profile.v1";
const PROFILE_NAME_MAX_BYTES: usize = 256;
const PROFILE_SUMMARY_MAX_BYTES: usize = 2 * 1024;
const PROFILE_QUESTION_MAX_BYTES: usize = 4 * 1024;
const PROFILE_SCOPE_ITEM_MAX_BYTES: usize = 2 * 1024;
const PROFILE_MAINTAINER_MAX_BYTES: usize = 256;
const PROFILE_LICENSE_MAX_BYTES: usize = 256;
const PROFILE_ENCODED_MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepositoryProfileScopeV1 {
    pub question: String,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepositoryProfileLicenseV1 {
    pub content: String,
    pub code: String,
    pub data: String,
}

/// Repository metadata.
///
/// Profile v2 deliberately keeps a small, human-editable field set. Repository
/// identity and standing come from `.vela/origin.json` and
/// `.vela/repository.json`, never from a generated compatibility view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryProfileV1 {
    pub schema: String,
    pub repository_id: String,
    pub name: String,
    pub summary: String,
    pub scope: RepositoryProfileScopeV1,
    pub maintainers: Vec<String>,
    pub license: RepositoryProfileLicenseV1,
}

impl RepositoryProfileV1 {
    pub fn from_toml_str(source: &str) -> Result<Self, String> {
        if source.len() > PROFILE_ENCODED_MAX_BYTES {
            return Err(format!(
                "{REPOSITORY_PROFILE_SCHEMA_V1} exceeds the 64 KiB encoded limit"
            ));
        }
        let profile: Self = toml::from_str(source)
            .map_err(|error| format!("invalid {REPOSITORY_PROFILE_SCHEMA_V1} TOML: {error}"))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != REPOSITORY_PROFILE_SCHEMA_V1 {
            return Err(format!(
                "profile.schema must be `{REPOSITORY_PROFILE_SCHEMA_V1}`"
            ));
        }
        validate_repository_id(&self.repository_id)?;
        validate_profile_text("profile.name", &self.name, PROFILE_NAME_MAX_BYTES)?;
        validate_profile_text("profile.summary", &self.summary, PROFILE_SUMMARY_MAX_BYTES)?;
        validate_profile_text(
            "profile.scope.question",
            &self.scope.question,
            PROFILE_QUESTION_MAX_BYTES,
        )?;
        let includes = validate_unique_profile_text(
            "profile.scope.includes",
            &self.scope.includes,
            PROFILE_SCOPE_ITEM_MAX_BYTES,
        )?;
        let excludes = validate_unique_profile_text(
            "profile.scope.excludes",
            &self.scope.excludes,
            PROFILE_SCOPE_ITEM_MAX_BYTES,
        )?;
        if includes.intersection(&excludes).next().is_some() {
            return Err(
                "profile.scope cannot contain the same statement in includes and excludes".into(),
            );
        }
        validate_unique_profile_text(
            "profile.maintainers",
            &self.maintainers,
            PROFILE_MAINTAINER_MAX_BYTES,
        )?;
        validate_profile_text(
            "profile.license.content",
            &self.license.content,
            PROFILE_LICENSE_MAX_BYTES,
        )?;
        validate_profile_text(
            "profile.license.code",
            &self.license.code,
            PROFILE_LICENSE_MAX_BYTES,
        )?;
        validate_profile_text(
            "profile.license.data",
            &self.license.data,
            PROFILE_LICENSE_MAX_BYTES,
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

/// Current repository manifest. Scientific and operational object sets remain
/// explicit and content addressed behind one immutable repository origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryV4 {
    pub schema: String,
    pub repository_id: String,
    pub profile_root: String,
    pub origin_id: String,
    pub origin_root: String,
    pub accepted_claims: Vec<ClaimStandingRefV1>,
    pub pending_claims: Vec<ClaimStandingRefV1>,
    pub proposals: Vec<RepositoryObjectRefV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proposal_withdrawals: Vec<RepositoryObjectRefV1>,
    pub submissions: Vec<RepositoryObjectRefV1>,
    pub verifications: Vec<RepositoryObjectRefV1>,
    pub artifacts: Vec<RepositoryObjectRefV1>,
    pub authority_keyset_root: String,
    pub authority_policy_root: String,
}

impl RepositoryV4 {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("current repository exceeds the 8 MiB encoded limit".into());
        }
        let value: Self = crate::canonical::from_json_slice_strict(bytes)
            .map_err(|error| format!("parse repository v4: {error}"))?;
        value.verify()?;
        if value.canonical_bytes()? != bytes {
            return Err("current repository bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        if self.schema != REPOSITORY_SCHEMA_V4 {
            return Err(format!(
                "repository schema must be `{REPOSITORY_SCHEMA_V4}`"
            ));
        }
        require_prefixed("repository_id", &self.repository_id, "vrepo_")?;
        require_sha256("profile_root", &self.profile_root)?;
        require_prefixed("origin_id", &self.origin_id, "vro_")?;
        require_sha256("origin_root", &self.origin_root)?;
        require_sha256("authority_keyset_root", &self.authority_keyset_root)?;
        require_sha256("authority_policy_root", &self.authority_policy_root)?;

        verify_claim_refs("accepted_claims", &self.accepted_claims, "accepted")?;
        verify_claim_refs("pending_claims", &self.pending_claims, "pending_review")?;
        verify_object_refs("proposals", &self.proposals)?;
        verify_object_refs("proposal_withdrawals", &self.proposal_withdrawals)?;
        verify_object_refs("submissions", &self.submissions)?;
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
                    .chain(&self.proposal_withdrawals)
                    .chain(&self.submissions)
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
        Ok(crate::canonical::sha256_root(&self.canonical_bytes()?))
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

fn validate_repository_id(value: &str) -> Result<(), String> {
    if !crate::shape::is_prefixed_lower_hex(value, "vrepo_", crate::shape::REPOSITORY_ID_HEX_LEN) {
        return Err("profile.repository_id must be vrepo_<32 lowercase hex>".into());
    }
    Ok(())
}

fn validate_profile_text(field: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if value.len() > max_bytes {
        return Err(format!("{field} must be at most {max_bytes} UTF-8 bytes"));
    }
    if value.nfc().collect::<String>() != value {
        return Err(format!("{field} must use Unicode NFC normalization"));
    }
    if value
        .chars()
        .any(|character| character.is_control() && character != '\n')
    {
        return Err(format!("{field} contains a forbidden control character"));
    }
    Ok(())
}

fn validate_unique_profile_text(
    field: &str,
    values: &[String],
    max_item_bytes: usize,
) -> Result<BTreeSet<String>, String> {
    let mut observed = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        validate_profile_text(&format!("{field}[{index}]"), value, max_item_bytes)?;
        if !observed.insert(value.nfc().collect()) {
            return Err(format!("{field} must not contain duplicate values"));
        }
    }
    Ok(observed)
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
    if !crate::shape::is_prefixed_lower_hex(value, "vcl_", 64) {
        return Err(format!(
            "current repository {field} must be vcl_<64 lowercase hex>"
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    if crate::shape::is_full_sha256_root(value) {
        Ok(())
    } else {
        Err(format!(
            "current repository {field} must be a full sha256: digest"
        ))
    }
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

    fn fixture() -> RepositoryV4 {
        RepositoryV4 {
            schema: REPOSITORY_SCHEMA_V4.into(),
            repository_id: "vrepo_0123456789abcdef0123456789abcdef".into(),
            profile_root: root('a'),
            origin_id: "vro_0123456789abcdef".into(),
            origin_root: root('b'),
            accepted_claims: vec![claim('c', "accepted")],
            pending_claims: vec![claim('d', "pending_review")],
            proposals: vec![],
            proposal_withdrawals: vec![],
            submissions: vec![],
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
    fn current_repository_v4_binds_one_origin() {
        let repository = fixture();
        repository.verify().unwrap();
        let bytes = repository.canonical_bytes().unwrap();
        assert_eq!(RepositoryV4::parse(&bytes).unwrap(), repository);
        let mut tampered = repository;
        tampered.origin_id = "vre_0123456789abcdef".into();
        assert!(tampered.verify().is_err());
    }

    #[test]
    fn current_profile_is_native_closed_metadata() {
        let current = RepositoryProfileV1 {
            schema: REPOSITORY_PROFILE_SCHEMA_V1.into(),
            repository_id: "vrepo_0123456789abcdef0123456789abcdef".into(),
            name: "Example".into(),
            summary: "A bounded example repository.".into(),
            scope: RepositoryProfileScopeV1 {
                question: "What is true?".into(),
                includes: vec!["Exact claims.".into()],
                excludes: vec!["Unbounded claims.".into()],
            },
            maintainers: vec!["Example Maintainer".into()],
            license: RepositoryProfileLicenseV1 {
                content: "CC-BY-4.0".into(),
                code: "Apache-2.0 OR MIT".into(),
                data: "CC0-1.0".into(),
            },
        };
        let toml = toml::to_string_pretty(&current).unwrap();
        let parsed = RepositoryProfileV1::from_toml_str(&toml).unwrap();
        assert_eq!(parsed, current);
        assert_eq!(
            current.profile_root().unwrap(),
            "sha256:b85e57f820a78e509b1577faff333862b9983d340a3a28132fc24856a848157e"
        );
    }

    #[test]
    fn current_profile_rejects_unknown_duplicate_and_oversized_toml() {
        let valid = r#"
schema = "vela.repository-profile.v1"
repository_id = "vrepo_0123456789abcdef0123456789abcdef"
name = "Example"
summary = "A bounded example repository."
maintainers = []

[scope]
question = "What is true?"
includes = []
excludes = []

[license]
content = "CC-BY-4.0"
code = "Apache-2.0 OR MIT"
data = "CC0-1.0"
"#;
        RepositoryProfileV1::from_toml_str(valid).unwrap();
        assert!(RepositoryProfileV1::from_toml_str(&format!("{valid}\nunknown = 1\n")).is_err());
        let duplicate = valid.replacen(
            "name = \"Example\"",
            "name = \"Example\"\nname = \"Again\"",
            1,
        );
        assert!(RepositoryProfileV1::from_toml_str(&duplicate).is_err());
        assert!(
            RepositoryProfileV1::from_toml_str(&"x".repeat(PROFILE_ENCODED_MAX_BYTES + 1)).is_err()
        );
    }
}
