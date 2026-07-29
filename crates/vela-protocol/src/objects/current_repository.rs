//! Closed current-only repository manifest for Frontier Profile v2.
//!
//! The manifest indexes active content-addressed objects and the authority
//! material that can change their standing. It carries no decision or
//! acceptance power by itself; its exact root follows the signed
//! repository-manifest delta chain.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

pub const CURRENT_REPOSITORY_SCHEMA_V2: &str = "vela.repository.v2";
pub const CURRENT_REPOSITORY_SCHEMA_V3: &str = "vela.repository.v3";
pub const CURRENT_FRONTIER_PROFILE_SCHEMA_V2: &str = "vela.frontier-profile.v2";
pub const CURRENT_ARTIFACT_RECORD_SCHEMA_V1: &str = "vela.artifact-record.v1";
const PROFILE_NAME_MAX_BYTES: usize = 256;
const PROFILE_SUMMARY_MAX_BYTES: usize = 2 * 1024;
const PROFILE_QUESTION_MAX_BYTES: usize = 4 * 1024;
const PROFILE_SCOPE_ITEM_MAX_BYTES: usize = 2 * 1024;
const PROFILE_MAINTAINER_MAX_BYTES: usize = 256;
const PROFILE_LICENSE_MAX_BYTES: usize = 256;

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
    /// Exact descriptor imported at the repository epoch boundary.
    ///
    /// Imported descriptors are immutable evidence, not a live protocol
    /// object model. Keeping their closed record wrapper while treating the
    /// nested predecessor descriptor as canonical JSON avoids retaining the
    /// entire historical Finding bundle runtime.
    pub artifact: Value,
    pub imported_object_root: String,
    pub predecessor_commit: String,
}

impl CurrentArtifactRecordV1 {
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
        let descriptor = self
            .artifact
            .as_object()
            .ok_or_else(|| "current Artifact Record descriptor must be an object".to_string())?;
        let descriptor_id = descriptor
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| "current Artifact Record descriptor lacks its ID".to_string())?;
        if self.artifact_id != descriptor_id {
            return Err("current Artifact Record ID does not match its descriptor".into());
        }
        require_prefixed("artifact.artifact_id", &self.artifact_id, "va_")?;
        validate_imported_artifact_axes(descriptor)?;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierProfileScopeV2 {
    pub question: String,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierProfileLicenseV2 {
    pub content: String,
    pub code: String,
    pub data: String,
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
    pub scope: FrontierProfileScopeV2,
    pub maintainers: Vec<String>,
    pub license: FrontierProfileLicenseV2,
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
        validate_frontier_id(&self.frontier_id)?;
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

/// Final pre-release repository manifest. The origin replaces the temporary
/// predecessor epoch vocabulary; scientific and operational object sets stay
/// explicit and content addressed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentRepositoryV3 {
    pub schema: String,
    pub frontier_id: String,
    pub profile_root: String,
    pub origin_id: String,
    pub origin_root: String,
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

impl CurrentRepositoryV3 {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("current repository exceeds the 8 MiB encoded limit".into());
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse current repository v3: {error}"))?;
        value.verify()?;
        if value.canonical_bytes()? != bytes {
            return Err("current repository bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        if self.schema != CURRENT_REPOSITORY_SCHEMA_V3 {
            return Err(format!(
                "current repository schema must be `{CURRENT_REPOSITORY_SCHEMA_V3}`"
            ));
        }
        require_prefixed("frontier_id", &self.frontier_id, "vfr_")?;
        require_sha256("profile_root", &self.profile_root)?;
        require_prefixed("origin_id", &self.origin_id, "vro_")?;
        require_sha256("origin_root", &self.origin_root)?;
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

fn validate_imported_artifact_axes(
    descriptor: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let disclosure = descriptor
        .get("disclosure")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let content_hash = descriptor
        .get("content_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| "current Artifact descriptor lacks content_hash".to_string())?;
    let locator = descriptor.get("locator").and_then(Value::as_str);
    match disclosure {
        "public" => require_sha256("artifact.content_hash", content_hash)?,
        "restricted" => {
            if !content_hash.is_empty() {
                return Err(
                    "restricted artifact must use an opaque custodian reference; public digest disclosure requires a separately reviewed commitment scheme"
                        .into(),
                );
            }
            if !locator.is_some_and(|value| {
                value.starts_with("custodian:") || value.starts_with("opaque:")
            }) {
                return Err(
                    "restricted artifact requires an opaque custodian: or opaque: locator".into(),
                );
            }
        }
        "unknown" => {}
        _ => return Err("current Artifact descriptor has an invalid disclosure".into()),
    }
    let storage_mode = descriptor
        .get("storage_mode")
        .and_then(Value::as_str)
        .ok_or_else(|| "current Artifact descriptor lacks storage_mode".to_string())?;
    let availability = descriptor
        .get("availability")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if storage_mode == "local_blob" && availability == "unavailable" {
        return Err("local_blob artifact cannot be declared unavailable".into());
    }
    Ok(())
}

fn validate_frontier_id(value: &str) -> Result<(), String> {
    let suffix = value
        .strip_prefix("vfr_")
        .ok_or_else(|| "profile.frontier_id must be vfr_<16 lowercase hex>".to_string())?;
    if suffix.len() != 16 || !suffix.bytes().all(is_lower_hex) {
        return Err("profile.frontier_id must be vfr_<16 lowercase hex>".into());
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

/// Reject YAML anchors, aliases, and explicit tags before Serde resolves them.
fn reject_yaml_indirection(source: &str) -> Result<(), String> {
    let characters = source.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut single_quoted = false;
    let mut double_quoted = false;
    let mut double_escape = false;
    let mut comment = false;
    while index < characters.len() {
        let character = characters[index];
        if comment {
            comment = character != '\n';
            index += 1;
            continue;
        }
        if single_quoted {
            if character == '\'' {
                if characters.get(index + 1) == Some(&'\'') {
                    index += 2;
                    continue;
                }
                single_quoted = false;
            }
            index += 1;
            continue;
        }
        if double_quoted {
            if double_escape {
                double_escape = false;
            } else if character == '\\' {
                double_escape = true;
            } else if character == '"' {
                double_quoted = false;
            }
            index += 1;
            continue;
        }
        let previous = index
            .checked_sub(1)
            .and_then(|offset| characters.get(offset));
        match character {
            '\'' => single_quoted = true,
            '"' => double_quoted = true,
            '#' if previous.is_none_or(|value| value.is_whitespace()) => comment = true,
            '&' | '*'
                if previous.is_none_or(|value| {
                    value.is_whitespace() || matches!(value, '-' | '[' | '{' | ',' | ':')
                }) && characters.get(index + 1).is_some_and(|value| {
                    !value.is_whitespace() && !matches!(value, ']' | '}' | ',' | '#')
                }) =>
            {
                return Err(format!(
                    "{CURRENT_FRONTIER_PROFILE_SCHEMA_V2} forbids YAML anchors and aliases"
                ));
            }
            '!' if previous.is_none_or(|value| {
                value.is_whitespace() || matches!(value, '-' | '[' | '{' | ',' | ':')
            }) && characters.get(index + 1).is_some_and(|value| {
                !value.is_whitespace() && !matches!(value, ']' | '}' | ',' | '#')
            }) =>
            {
                return Err(format!(
                    "{CURRENT_FRONTIER_PROFILE_SCHEMA_V2} forbids explicit YAML tags"
                ));
            }
            _ => {}
        }
        index += 1;
    }
    Ok(())
}

fn validate_yaml_structure(value: &serde_yaml::Value) -> Result<(), String> {
    match value {
        serde_yaml::Value::Mapping(mapping) => {
            for (key, value) in mapping {
                if key.as_str() == Some("<<") {
                    return Err(format!(
                        "{CURRENT_FRONTIER_PROFILE_SCHEMA_V2} forbids YAML merge keys"
                    ));
                }
                validate_yaml_structure(key)?;
                validate_yaml_structure(value)?;
            }
        }
        serde_yaml::Value::Sequence(sequence) => {
            for value in sequence {
                validate_yaml_structure(value)?;
            }
        }
        serde_yaml::Value::Tagged(_) => {
            return Err(format!(
                "{CURRENT_FRONTIER_PROFILE_SCHEMA_V2} forbids explicit YAML tags"
            ));
        }
        _ => {}
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
    fn current_repository_v3_binds_one_origin() {
        let predecessor = fixture();
        let repository = CurrentRepositoryV3 {
            schema: CURRENT_REPOSITORY_SCHEMA_V3.into(),
            frontier_id: predecessor.frontier_id,
            profile_root: predecessor.profile_root,
            origin_id: "vro_0123456789abcdef".into(),
            origin_root: root('b'),
            accepted_claims: predecessor.accepted_claims,
            pending_claims: Vec::new(),
            proposals: Vec::new(),
            submissions: Vec::new(),
            registrations: Vec::new(),
            verifications: Vec::new(),
            artifacts: Vec::new(),
            authority_keyset_root: predecessor.authority_keyset_root,
            authority_policy_root: predecessor.authority_policy_root,
        };
        repository.verify().unwrap();
        let bytes = repository.canonical_bytes().unwrap();
        assert_eq!(CurrentRepositoryV3::parse(&bytes).unwrap(), repository);
        let mut tampered = repository;
        tampered.origin_id = "vre_0123456789abcdef".into();
        assert!(tampered.verify().is_err());
    }

    #[test]
    fn current_profile_is_native_closed_metadata() {
        let current = CurrentFrontierProfileV2 {
            schema: CURRENT_FRONTIER_PROFILE_SCHEMA_V2.into(),
            frontier_id: "vfr_0123456789abcdef".into(),
            name: "Example".into(),
            summary: "A bounded example Frontier.".into(),
            scope: FrontierProfileScopeV2 {
                question: "What is true?".into(),
                includes: vec!["Exact claims.".into()],
                excludes: vec!["Unbounded claims.".into()],
            },
            maintainers: vec!["Example Maintainer".into()],
            license: FrontierProfileLicenseV2 {
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
