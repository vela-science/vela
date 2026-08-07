//! Read-only reader for epoch-1 repositories. Frozen.
//!
//! Epoch 1 spelled the authority boundary `Frontier`: `frontier.toml`,
//! `frontier_id`, `vfr_`, and the `finding.*` Event kinds. ADR 0039 renamed all
//! of it. The four repositories written under that spelling are retained
//! exactly as signed, so something has to keep reading them.
//!
//! It cannot be the current types. Every canonical object carries
//! `#[serde(deny_unknown_fields)]`, and both `parse` paths re-serialize and
//! compare bytes, so a `#[serde(alias)]` on the renamed field would still fail
//! the canonical-bytes check. A compatibility branch inside the current types is
//! impossible rather than merely undesirable.
//!
//! So epoch 1 is a different schema family instead of an alias inside one. That
//! is what lets "no aliases" hold literally: nothing in the current path knows
//! these spellings exist, and nothing here changes when the current path moves.
//!
//! **This module is frozen.** It describes bytes that were signed and cannot be
//! rewritten. A change here is only ever a bug fix in how those bytes are read,
//! never a change in what they mean. It is read-only by construction: the
//! constructors are not copied, because epoch 1 admits nothing further.
//!
//! Canonicalization is shared with the current path on purpose. RFC 8785 is not
//! epoch-scoped, and a second implementation of it would be a second thing to
//! get wrong.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

/// Epoch 1 kept the repository profile at the repository root under the name
/// the boundary then had.
pub const EPOCH1_PROFILE_PATH: &str = "frontier.toml";

pub const EPOCH1_PROFILE_SCHEMA_V2: &str = "vela.frontier-profile.v2";
pub const EPOCH1_REPOSITORY_SCHEMA_V4: &str = "vela.repository.v4";
pub const EPOCH1_ORIGIN_SCHEMA_V1: &str = "vela.repository-origin.v1";

const PROFILE_NAME_MAX_BYTES: usize = 256;
const PROFILE_SUMMARY_MAX_BYTES: usize = 2 * 1024;
const PROFILE_QUESTION_MAX_BYTES: usize = 4 * 1024;
const PROFILE_SCOPE_ITEM_MAX_BYTES: usize = 2 * 1024;
const PROFILE_MAINTAINER_MAX_BYTES: usize = 256;
const PROFILE_LICENSE_MAX_BYTES: usize = 256;
const PROFILE_ENCODED_MAX_BYTES: usize = 64 * 1024;

/// The Event kinds epoch 1 used for a scientific transition.
///
/// The current path renamed these to `claim.*`. A Decision check that matches
/// on the current typed variants reads every one of these as untyped and
/// reports a non-scientific transition, which is exactly the regression this
/// module exists to prevent.
pub const EPOCH1_SCIENTIFIC_EVENT_KINDS: [&str; 4] = [
    "finding.asserted",
    "finding.noted",
    "finding.retracted",
    "finding.superseded",
];

/// True when an epoch-1 Event kind moved Claim standing.
pub fn is_epoch1_scientific_event_kind(kind: &str) -> bool {
    EPOCH1_SCIENTIFIC_EVENT_KINDS.contains(&kind)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Epoch1ProfileScopeV2 {
    pub question: String,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Epoch1ProfileLicenseV2 {
    pub content: String,
    pub code: String,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Epoch1ProfileV2 {
    pub schema: String,
    pub frontier_id: String,
    pub name: String,
    pub summary: String,
    pub scope: Epoch1ProfileScopeV2,
    pub maintainers: Vec<String>,
    pub license: Epoch1ProfileLicenseV2,
}

impl Epoch1ProfileV2 {
    pub fn from_toml_str(source: &str) -> Result<Self, String> {
        if source.len() > PROFILE_ENCODED_MAX_BYTES {
            return Err(format!(
                "{EPOCH1_PROFILE_SCHEMA_V2} exceeds the 64 KiB encoded limit"
            ));
        }
        let profile: Self = toml::from_str(source)
            .map_err(|error| format!("invalid {EPOCH1_PROFILE_SCHEMA_V2} TOML: {error}"))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != EPOCH1_PROFILE_SCHEMA_V2 {
            return Err(format!(
                "profile.schema must be `{EPOCH1_PROFILE_SCHEMA_V2}`"
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Epoch1OriginKind {
    Genesis,
    Compaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Epoch1OriginPredecessorV1 {
    pub remote: String,
    pub tag: String,
    pub commit: String,
    pub tree: String,
    pub repository_root: String,
    pub authority_head_root: String,
    pub archived_event_log_root: String,
    pub archived_actor_registry_root: String,
    pub archive_sha256: String,
    pub object_manifest_root: String,
    pub equivalence_report_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Epoch1OriginV1 {
    pub schema: String,
    pub origin_id: String,
    pub frontier_id: String,
    pub generation: u64,
    pub profile_root: String,
    pub initial_object_set_root: String,
    pub kind: Epoch1OriginKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predecessor: Option<Epoch1OriginPredecessorV1>,
    pub reason: String,
}

impl Epoch1OriginV1 {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 128 * 1024 {
            return Err("epoch-1 repository origin exceeds the 128 KiB encoded limit".into());
        }
        let value: Self = crate::canonical::from_json_slice_strict(bytes)
            .map_err(|error| format!("parse epoch-1 repository origin: {error}"))?;
        value.verify()?;
        if value.canonical_bytes()? != bytes {
            return Err("epoch-1 repository origin bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()?;
        let expected = self.derive_id()?;
        if self.origin_id != expected {
            return Err(format!(
                "epoch-1 repository origin id mismatch: declared {}, rebuilt {expected}",
                self.origin_id
            ));
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_semantics()?;
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
        body.origin_id.clear();
        let bytes = crate::canonical::to_canonical_bytes(&body)?;
        Ok(format!("vro_{}", &hex::encode(Sha256::digest(bytes))[..16]))
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != EPOCH1_ORIGIN_SCHEMA_V1 {
            return Err(format!(
                "epoch-1 repository origin schema must be `{EPOCH1_ORIGIN_SCHEMA_V1}`"
            ));
        }
        require_prefixed("frontier_id", &self.frontier_id, "vfr_")?;
        require_sha256("profile_root", &self.profile_root)?;
        require_sha256("initial_object_set_root", &self.initial_object_set_root)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Epoch1ObjectRefV1 {
    pub schema: String,
    pub id: String,
    pub root: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Epoch1ClaimStandingRefV1 {
    pub claim_id: String,
    pub claim_root: String,
    pub standing: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Epoch1RepositoryV4 {
    pub schema: String,
    pub frontier_id: String,
    pub profile_root: String,
    pub origin_id: String,
    pub origin_root: String,
    pub accepted_claims: Vec<Epoch1ClaimStandingRefV1>,
    pub pending_claims: Vec<Epoch1ClaimStandingRefV1>,
    pub proposals: Vec<Epoch1ObjectRefV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proposal_withdrawals: Vec<Epoch1ObjectRefV1>,
    pub submissions: Vec<Epoch1ObjectRefV1>,
    pub verifications: Vec<Epoch1ObjectRefV1>,
    pub artifacts: Vec<Epoch1ObjectRefV1>,
    pub authority_keyset_root: String,
    pub authority_policy_root: String,
}

impl Epoch1RepositoryV4 {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("epoch-1 repository exceeds the 8 MiB encoded limit".into());
        }
        let value: Self = crate::canonical::from_json_slice_strict(bytes)
            .map_err(|error| format!("parse epoch-1 repository: {error}"))?;
        value.verify()?;
        if value.canonical_bytes()? != bytes {
            return Err("epoch-1 repository bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        if self.schema != EPOCH1_REPOSITORY_SCHEMA_V4 {
            return Err(format!(
                "epoch-1 repository schema must be `{EPOCH1_REPOSITORY_SCHEMA_V4}`"
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
                return Err(format!("epoch-1 repository repeats object path `{path}`"));
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
    references: &[Epoch1ClaimStandingRefV1],
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
                "epoch-1 repository {field} standing must be `{expected_standing}`"
            ));
        }
        if prior.is_some_and(|prior: &str| prior >= reference.claim_id.as_str()) {
            return Err(format!(
                "epoch-1 repository {field} must be strictly sorted by Claim ID"
            ));
        }
        if !roots.insert(reference.claim_root.as_str()) {
            return Err(format!(
                "epoch-1 repository {field} repeats Claim root {}",
                reference.claim_root
            ));
        }
        prior = Some(reference.claim_id.as_str());
    }
    Ok(())
}

fn verify_object_refs(field: &str, references: &[Epoch1ObjectRefV1]) -> Result<(), String> {
    let mut prior = None;
    let mut roots = BTreeSet::new();
    for reference in references {
        require_text(&format!("{field}.schema"), &reference.schema)?;
        require_text(&format!("{field}.id"), &reference.id)?;
        require_sha256(&format!("{field}.root"), &reference.root)?;
        require_path(&format!("{field}.path"), &reference.path)?;
        if prior.is_some_and(|prior: &str| prior >= reference.id.as_str()) {
            return Err(format!(
                "epoch-1 repository {field} must be strictly sorted by object ID"
            ));
        }
        if !roots.insert(reference.root.as_str()) {
            return Err(format!(
                "epoch-1 repository {field} repeats object root {}",
                reference.root
            ));
        }
        prior = Some(reference.id.as_str());
    }
    Ok(())
}

fn validate_frontier_id(value: &str) -> Result<(), String> {
    let suffix = value
        .strip_prefix("vfr_")
        .ok_or_else(|| "profile.frontier_id must be vfr_<16 lowercase hex>".to_string())?;
    if suffix.len() != 16 || !suffix.bytes().all(crate::shape::is_lower_hex) {
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

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "epoch-1 repository {field} must be non-empty, trimmed text"
        ));
    }
    Ok(())
}

fn require_prefixed(field: &str, value: &str, prefix: &str) -> Result<(), String> {
    require_text(field, value)?;
    if !value.starts_with(prefix) {
        return Err(format!("epoch-1 repository {field} must start with `{prefix}`"));
    }
    Ok(())
}

fn require_full_claim_id(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("vcl_")
        .ok_or_else(|| format!("epoch-1 repository {field} must be vcl_<64 lowercase hex>"))?;
    if !crate::shape::is_lower_hex_64(digest) {
        return Err(format!(
            "epoch-1 repository {field} must be vcl_<64 lowercase hex>"
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("epoch-1 repository {field} must be a full sha256: digest"))?;
    if !crate::shape::is_lower_hex_64(digest) {
        return Err(format!(
            "epoch-1 repository {field} must be a full sha256: digest"
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
            "epoch-1 repository {field} must be a safe relative path"
        ));
    }
    Ok(())
}
