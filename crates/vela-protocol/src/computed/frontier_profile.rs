//! Closed, non-authoritative Frontier Repository Profile v1.
//!
//! The profile is human-editable repository metadata. It is deliberately
//! separate from replayed scientific state and grants no actor, policy, or
//! acceptance authority. This module defines and roots the value object only;
//! repository migration and v0.1 loader compatibility are owned by later
//! implementation slices.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::events::{
    EVENT_KIND_FRONTIER_REPOSITORY_BOUND, EVENT_SCHEMA, NULL_HASH, StateEvent, compute_event_id,
    event_content_preimage_bytes,
};
use crate::frontier_repository::{
    ExactFrontierDependencyV1, FRONTIER_IDENTITY_SCHEMA, FrontierIdentityOrigin,
    FrontierIdentityV1, exact_dependency_root, repository_boundary_event_content_root,
    repository_boundary_payload_from_event_shape, validate_repository_boundary_event_set,
};
use crate::project::Project;
use crate::scientific_state::scientific_state_root_v2;

pub const FRONTIER_PROFILE_SCHEMA_V1: &str = "vela.frontier-profile.v1";
pub const FRONTIER_CREATED_SCHEMA_V1: &str = crate::events::FRONTIER_CREATED_SCHEMA_V1;

pub const FRONTIER_PROFILE_NAME_MAX_BYTES: usize = 256;
pub const FRONTIER_PROFILE_SUMMARY_MAX_BYTES: usize = 2 * 1024;
pub const FRONTIER_PROFILE_QUESTION_MAX_BYTES: usize = 4 * 1024;
pub const FRONTIER_PROFILE_SCOPE_ITEM_MAX_BYTES: usize = 2 * 1024;
pub const FRONTIER_PROFILE_MAINTAINER_MAX_BYTES: usize = 256;
pub const FRONTIER_PROFILE_LICENSE_MAX_BYTES: usize = 256;

/// Human-facing discovery, onboarding, and stewardship metadata for one
/// Frontier repository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierProfileV1 {
    pub schema: String,
    /// Checked assertion about the separately derived Frontier identity. This
    /// display handle is never itself an identity source.
    pub frontier_id: String,
    pub name: String,
    pub summary: String,
    pub scope: FrontierProfileScopeV1,
    pub maintainers: Vec<String>,
    pub license: FrontierProfileLicenseV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierProfileScopeV1 {
    pub question: String,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierProfileLicenseV1 {
    pub content: String,
    pub code: String,
    pub data: String,
}

/// Security-bearing state associated with a Profile v1 repository.
///
/// Every field is derived from canonical identity-event bytes. In particular,
/// no value in `frontier.yaml`, `.vela/config.toml`, or `.vela/settings.toml`
/// can supply or override this record. Repository-context verification of Git
/// anchors and actor authority remains a higher-layer prerequisite for writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveFrontierAuthorityV1 {
    pub frontier_id: String,
    pub identity_root: String,
    pub dependency_root: String,
    pub dependencies: Vec<ExactFrontierDependencyV1>,
    pub identity_event_root: String,
}

/// Read-only roots for a validated Profile v1 repository.
///
/// This is an in-process projection, not a new protocol wire object. It gives
/// lock/proof writers one explicit source for the profile and scientific-state
/// roots without adding profile metadata to [`Project`] or its reducer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierProfileProjectionV1 {
    pub profile_root: String,
    pub frontier_id: String,
    pub identity_root: String,
    pub dependency_root: String,
    pub dependencies: Vec<ExactFrontierDependencyV1>,
    pub identity_event_root: String,
    pub scientific_state_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrontierCreatedPayloadV1 {
    pub schema: String,
    pub name_at_creation: String,
    pub creator: String,
    pub profile_schema: String,
    pub dependency_root: String,
    pub created_at: String,
}

impl FrontierProfileV1 {
    /// Parse a YAML representation and validate the complete closed profile.
    ///
    /// Serde's `deny_unknown_fields` annotation applies to every mapping, so
    /// unknown nested fields fail at the same boundary as unknown top-level
    /// fields.
    pub fn from_yaml_str(source: &str) -> Result<Self, String> {
        reject_yaml_indirection(source)?;
        // Deserialize through Value first. Its mapping implementation rejects
        // duplicate keys before a struct could otherwise consume the map.
        let value: serde_yaml::Value = serde_yaml::from_str(source)
            .map_err(|error| format!("invalid {FRONTIER_PROFILE_SCHEMA_V1} YAML: {error}"))?;
        validate_yaml_structure(&value)?;
        let profile: Self = serde_yaml::from_value(value)
            .map_err(|error| format!("invalid {FRONTIER_PROFILE_SCHEMA_V1} YAML: {error}"))?;
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FRONTIER_PROFILE_SCHEMA_V1 {
            return Err(format!(
                "profile.schema must be `{FRONTIER_PROFILE_SCHEMA_V1}`"
            ));
        }
        validate_frontier_id(&self.frontier_id)?;
        validate_text("profile.name", &self.name, FRONTIER_PROFILE_NAME_MAX_BYTES)?;
        validate_text(
            "profile.summary",
            &self.summary,
            FRONTIER_PROFILE_SUMMARY_MAX_BYTES,
        )?;
        validate_text(
            "profile.scope.question",
            &self.scope.question,
            FRONTIER_PROFILE_QUESTION_MAX_BYTES,
        )?;
        let includes = validate_unique_text_list(
            "profile.scope.includes",
            &self.scope.includes,
            FRONTIER_PROFILE_SCOPE_ITEM_MAX_BYTES,
        )?;
        let excludes = validate_unique_text_list(
            "profile.scope.excludes",
            &self.scope.excludes,
            FRONTIER_PROFILE_SCOPE_ITEM_MAX_BYTES,
        )?;
        if includes.intersection(&excludes).next().is_some() {
            return Err(
                "profile.scope cannot contain the same statement in includes and excludes"
                    .to_string(),
            );
        }
        validate_unique_text_list(
            "profile.maintainers",
            &self.maintainers,
            FRONTIER_PROFILE_MAINTAINER_MAX_BYTES,
        )?;
        validate_text(
            "profile.license.content",
            &self.license.content,
            FRONTIER_PROFILE_LICENSE_MAX_BYTES,
        )?;
        validate_text(
            "profile.license.code",
            &self.license.code,
            FRONTIER_PROFILE_LICENSE_MAX_BYTES,
        )?;
        validate_text(
            "profile.license.data",
            &self.license.data,
            FRONTIER_PROFILE_LICENSE_MAX_BYTES,
        )?;
        Ok(())
    }

    /// Check the profile's readable Frontier ID against an identity that was
    /// derived independently from genesis or a valid repository boundary.
    pub fn assert_frontier_id(&self, bound_frontier_id: &str) -> Result<(), String> {
        validate_frontier_id(&self.frontier_id)?;
        validate_frontier_id(bound_frontier_id)?;
        if self.frontier_id != bound_frontier_id {
            return Err(format!(
                "profile.frontier_id `{}` does not match bound Frontier `{bound_frontier_id}`",
                self.frontier_id
            ));
        }
        Ok(())
    }

    /// Full SHA-256 over the protocol's canonical JSON representation.
    ///
    /// YAML key order, comments, quoting, whitespace, and final newlines are
    /// erased by parsing before this root is derived. Array order remains
    /// semantic and is therefore preserved.
    pub fn profile_root(&self) -> Result<String, String> {
        self.validate()?;
        crate::canonical::sha256_canonical(self).map(|digest| format!("sha256:{digest}"))
    }

    /// Bind the editable profile to independently derived identity-event state
    /// and compute the closed scientific-state root.
    pub fn project(&self, project: &Project) -> Result<FrontierProfileProjectionV1, String> {
        self.validate()?;
        let authority = EffectiveFrontierAuthorityV1::from_events(&project.events)?;
        self.assert_frontier_id(&authority.frontier_id)?;
        let scientific_state_root = scientific_state_root_v2(
            project,
            &authority.identity_root,
            &authority.dependency_root,
        )?;
        Ok(FrontierProfileProjectionV1 {
            profile_root: self.profile_root()?,
            frontier_id: authority.frontier_id,
            identity_root: authority.identity_root,
            dependency_root: authority.dependency_root,
            dependencies: authority.dependencies,
            identity_event_root: authority.identity_event_root,
            scientific_state_root,
        })
    }
}

impl EffectiveFrontierAuthorityV1 {
    /// Derive effective identity and dependencies from the complete canonical
    /// event set. Profile and settings bytes are deliberately not parameters.
    pub fn from_events(events: &[StateEvent]) -> Result<Self, String> {
        let boundary_events = events
            .iter()
            .filter(|event| event.kind.as_str() == EVENT_KIND_FRONTIER_REPOSITORY_BOUND)
            .collect::<Vec<_>>();

        if boundary_events.is_empty() {
            return authority_from_profile_v1_genesis(events);
        }

        let errors = validate_repository_boundary_event_set(events);
        if !errors.is_empty() {
            return Err(format!(
                "repository identity-event set is invalid: {}",
                errors.join("; ")
            ));
        }

        let mut by_root = BTreeMap::new();
        let mut referenced_parents = BTreeSet::new();
        for event in boundary_events {
            let root = repository_boundary_event_content_root(event)?;
            let payload = repository_boundary_payload_from_event_shape(event)?;
            if let Some(parent) = payload.previous_identity_event_root.as_ref() {
                referenced_parents.insert(parent.clone());
            }
            if by_root.insert(root.clone(), payload).is_some() {
                return Err(format!(
                    "repository identity-event set contains duplicate root {root}"
                ));
            }
        }

        let heads = by_root
            .iter()
            .filter(|(root, _)| !referenced_parents.contains(*root))
            .collect::<Vec<_>>();
        let [(identity_event_root, payload)] = heads.as_slice() else {
            return Err(format!(
                "repository identity-event set must have exactly one boundary head, found {}",
                heads.len()
            ));
        };
        Ok(Self {
            frontier_id: payload.frontier_id.clone(),
            identity_root: payload.identity_root.clone(),
            dependency_root: payload.dependency_root.clone(),
            dependencies: payload.dependencies.clone(),
            identity_event_root: (*identity_event_root).clone(),
        })
    }
}

/// Validate and derive the identity of a new Profile v1 Frontier directly
/// from its structural genesis event. The generic event validator/reducer must
/// separately learn this closed payload before v1 initialization can ship;
/// keeping the derivation here lets repository loading stay fail-closed rather
/// than falling back to legacy metadata.
fn authority_from_profile_v1_genesis(
    events: &[StateEvent],
) -> Result<EffectiveFrontierAuthorityV1, String> {
    let genesis_events = events
        .iter()
        .filter(|event| {
            event.kind.as_str() == "frontier.created"
                && event
                    .payload
                    .get("schema")
                    .and_then(serde_json::Value::as_str)
                    == Some(FRONTIER_CREATED_SCHEMA_V1)
        })
        .collect::<Vec<_>>();
    let [event] = genesis_events.as_slice() else {
        return Err(format!(
            "Profile v1 requires exactly one frontier.created genesis or one valid repository-boundary chain, found {} genesis events and no boundaries",
            genesis_events.len()
        ));
    };
    if event.schema != EVENT_SCHEMA || event.id != compute_event_id(event) {
        return Err("Profile v1 frontier.created core or content ID is invalid".to_string());
    }
    if event.target.r#type != "frontier"
        || event.actor.r#type != "frontier"
        || event.before_hash != NULL_HASH
        || event.after_hash != NULL_HASH
        || event.signature.is_some()
        || !event.caveats.is_empty()
    {
        return Err("Profile v1 frontier.created core shape is invalid".to_string());
    }
    let payload: FrontierCreatedPayloadV1 = serde_json::from_value(event.payload.clone())
        .map_err(|error| format!("invalid {FRONTIER_CREATED_SCHEMA_V1} payload: {error}"))?;
    if payload.schema != FRONTIER_CREATED_SCHEMA_V1 {
        return Err(format!(
            "frontier.created payload.schema must be {FRONTIER_CREATED_SCHEMA_V1}"
        ));
    }
    if payload.profile_schema != FRONTIER_PROFILE_SCHEMA_V1 {
        return Err(format!(
            "frontier.created profile_schema must be {FRONTIER_PROFILE_SCHEMA_V1}"
        ));
    }
    validate_text(
        "frontier.created.name_at_creation",
        &payload.name_at_creation,
        FRONTIER_PROFILE_NAME_MAX_BYTES,
    )?;
    validate_text(
        "frontier.created.creator",
        &payload.creator,
        FRONTIER_PROFILE_MAINTAINER_MAX_BYTES,
    )?;
    chrono::DateTime::parse_from_rfc3339(&payload.created_at)
        .map_err(|error| format!("frontier.created created_at must be RFC3339: {error}"))?;
    if payload.name_at_creation != event.target.id
        || payload.creator != event.actor.id
        || payload.created_at != event.timestamp
    {
        return Err("frontier.created payload identity disagrees with its event core".to_string());
    }
    let empty_dependency_root = exact_dependency_root(&[])?;
    if payload.dependency_root != empty_dependency_root {
        return Err(
            "Profile v1 frontier.created must bind the canonical empty dependency root".to_string(),
        );
    }

    let digest = hex::encode(Sha256::digest(event_content_preimage_bytes(event)));
    let identity_event_root = format!("sha256:{digest}");
    let identity = FrontierIdentityV1 {
        schema: FRONTIER_IDENTITY_SCHEMA.to_string(),
        frontier_id: format!("vfr_{}", &digest[..16]),
        origin: FrontierIdentityOrigin::Genesis,
        origin_commitment: identity_event_root.clone(),
        legacy_identity_preimage_root: None,
    };
    Ok(EffectiveFrontierAuthorityV1 {
        frontier_id: identity.frontier_id.clone(),
        identity_root: identity.root()?,
        dependency_root: empty_dependency_root,
        dependencies: Vec::new(),
        identity_event_root,
    })
}

fn validate_frontier_id(value: &str) -> Result<(), String> {
    let Some(suffix) = value.strip_prefix("vfr_") else {
        return Err("profile.frontier_id must be vfr_<16 lowercase hex>".to_string());
    };
    if suffix.len() != 16
        || !suffix
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
    {
        return Err("profile.frontier_id must be vfr_<16 lowercase hex>".to_string());
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if value.len() > max_bytes {
        return Err(format!("{field} must be at most {max_bytes} UTF-8 bytes"));
    }
    let normalized = value.nfc().collect::<String>();
    if normalized != value {
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

fn validate_unique_text_list(
    field: &str,
    values: &[String],
    max_item_bytes: usize,
) -> Result<BTreeSet<String>, String> {
    let mut observed = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        validate_text(&format!("{field}[{index}]"), value, max_item_bytes)?;
        let normalized = value.nfc().collect::<String>();
        if !observed.insert(normalized) {
            return Err(format!("{field} must not contain duplicate values"));
        }
    }
    Ok(observed)
}

/// Reject YAML anchors, aliases, and explicit tags before Serde resolves them
/// and erases the syntactic distinction. Quoted indicator characters remain
/// ordinary text. Unquoted indicator tokens are forbidden even when a parser
/// would resolve them to the same in-memory value.
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
            if character == '\n' {
                comment = false;
            }
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
                    "{FRONTIER_PROFILE_SCHEMA_V1} forbids YAML anchors and aliases"
                ));
            }
            '!' if previous.is_none_or(|value| {
                value.is_whitespace() || matches!(value, '-' | '[' | '{' | ',' | ':')
            }) && characters.get(index + 1).is_some_and(|value| {
                !value.is_whitespace() && !matches!(value, ']' | '}' | ',' | '#')
            }) =>
            {
                return Err(format!(
                    "{FRONTIER_PROFILE_SCHEMA_V1} forbids explicit YAML tags"
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
                        "{FRONTIER_PROFILE_SCHEMA_V1} forbids YAML merge keys"
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
            // Explicit YAML tags are not part of this closed profile
            // language. The ordinary implicit scalar resolution performed by
            // the parser is sufficient and portable for these string fields.
            return Err(format!(
                "{FRONTIER_PROFILE_SCHEMA_V1} forbids explicit YAML tags"
            ));
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_profile() -> FrontierProfileV1 {
        FrontierProfileV1 {
            schema: FRONTIER_PROFILE_SCHEMA_V1.to_string(),
            frontier_id: "vfr_0123456789abcdef".to_string(),
            name: "Example Frontier".to_string(),
            summary: "A bounded repository-profile fixture.".to_string(),
            scope: FrontierProfileScopeV1 {
                question: "Which exact result does this Frontier maintain?".to_string(),
                includes: vec!["Rooted evidence".to_string()],
                excludes: vec!["Unbounded discovery".to_string()],
            },
            maintainers: vec!["maintainer:example".to_string()],
            license: FrontierProfileLicenseV1 {
                content: "CC-BY-4.0".to_string(),
                code: "Apache-2.0".to_string(),
                data: "CC0-1.0".to_string(),
            },
        }
    }

    #[test]
    fn frontier_profile_v1_validates_closed_value() {
        valid_profile().validate().unwrap();
    }

    #[test]
    fn frontier_profile_v1_rejects_invalid_identity_text_and_duplicates() {
        let mut profile = valid_profile();
        profile.frontier_id = "vfr_0123456789ABCDEf".to_string();
        assert!(profile.validate().unwrap_err().contains("frontier_id"));

        let mut profile = valid_profile();
        profile.summary = " \n\t".to_string();
        assert!(profile.validate().unwrap_err().contains("summary"));

        let mut profile = valid_profile();
        profile.maintainers.push("maintainer:example".to_string());
        assert!(profile.validate().unwrap_err().contains("duplicate"));

        let mut profile = valid_profile();
        profile.scope.includes.push("Rooted evidence".to_string());
        assert!(profile.validate().unwrap_err().contains("duplicate"));
    }

    #[test]
    fn frontier_profile_v1_rejects_oversized_text() {
        let mut profile = valid_profile();
        profile.name = "x".repeat(FRONTIER_PROFILE_NAME_MAX_BYTES + 1);
        assert!(profile.validate().unwrap_err().contains("at most"));
    }

    #[test]
    fn frontier_profile_v1_requires_nfc_and_rejects_control_text() {
        let mut profile = valid_profile();
        profile.summary = "Cafe\u{301}".to_string();
        assert!(profile.validate().unwrap_err().contains("Unicode NFC"));

        let mut profile = valid_profile();
        profile.summary = "Visible\tseparator".to_string();
        assert!(
            profile
                .validate()
                .unwrap_err()
                .contains("control character")
        );
    }

    #[test]
    fn profile_frontier_id_is_assertion_not_identity_source() {
        let profile = valid_profile();
        profile.assert_frontier_id("vfr_0123456789abcdef").unwrap();
        let error = profile
            .assert_frontier_id("vfr_fedcba9876543210")
            .unwrap_err();
        assert!(error.contains("does not match bound Frontier"));
    }
}
