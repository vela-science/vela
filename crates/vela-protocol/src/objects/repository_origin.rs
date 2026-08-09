//! Where one repository's canonical history begins: `vela.repository-origin.v1`.
//!
//! ## What is no longer here
//!
//! This modelled two kinds of beginning. A genesis opened a fresh lineage at
//! generation 1; a *compaction* opened generation N over a predecessor repository,
//! and carried an eleven-field block binding that predecessor's remote, tag,
//! commit, tree, four roots, an archive digest, an object manifest and an
//! equivalence report. It was written for one pre-release repair and used for
//! it once.
//!
//! No live repository needs it. `vela-science/math` is generation 1 at a fresh
//! genesis, and the epoch-1 repositories that were compacted are archived and
//! unreadable by the current binary — their history is preserved by their Git
//! tags and the binaries of their era, which is where a historical epoch
//! belongs.
//!
//! Continuity between lineages, if some future migration needs it, is a
//! separately signed attestation over exact commits, trees, roots and an
//! equivalence report — evidence beside the repository rather than a permanent
//! field on the object every repository must carry. Keeping the machinery for a
//! repair that has already happened is the ceremony this cut removes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const REPOSITORY_ORIGIN_V1_SCHEMA: &str = "vela.repository-origin.v1";
pub const ORIGIN_HANDLE_PREFIX: &str = "vro_";

/// The genesis of one repository lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepositoryOriginV1 {
    #[schemars(schema_with = "crate::wire_schema::repository_origin_schema_tag")]
    pub schema: String,
    #[schemars(schema_with = "crate::wire_schema::repository_id")]
    pub repository_id: String,
    /// Always 1. Retained because a reader that finds any other value is
    /// looking at an object this runtime does not define, and should say so
    /// rather than silently treat it as a genesis.
    pub generation: u64,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub profile_root: String,
    #[schemars(schema_with = "crate::wire_schema::sha256_root")]
    pub initial_object_set_root: String,
    #[schemars(schema_with = "crate::wire_schema::text")]
    pub reason: String,
}

impl RepositoryOriginV1 {
    pub fn genesis(
        repository_id: String,
        profile_root: String,
        reason: String,
    ) -> Result<Self, String> {
        let value = Self {
            schema: REPOSITORY_ORIGIN_V1_SCHEMA.into(),
            repository_id,
            generation: 1,
            profile_root,
            initial_object_set_root: empty_object_set_root()?,
            reason,
        };
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 128 * 1024 {
            return Err("repository origin exceeds the 128 KiB encoded limit".into());
        }
        let value: Self = crate::canonical::from_json_slice_strict(bytes)
            .map_err(|error| format!("parse repository origin v1: {error}"))?;
        value.verify()?;
        if value.canonical_bytes()? != bytes {
            return Err("repository origin bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_semantics()?;
        crate::canonical::to_canonical_bytes(self)
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        Ok(crate::canonical::sha256_root(&self.canonical_bytes()?))
    }

    /// The readable `vro_` handle for this origin's canonical root.
    ///
    /// Stored beside the full root until this cut, over a preimage built by
    /// clearing itself — the same convention the signed objects shed.
    pub fn id(&self) -> Result<String, String> {
        crate::shape::derive_handle(ORIGIN_HANDLE_PREFIX, &self.canonical_root()?)
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != REPOSITORY_ORIGIN_V1_SCHEMA {
            return Err(format!(
                "repository origin schema must be `{REPOSITORY_ORIGIN_V1_SCHEMA}`"
            ));
        }
        require_prefixed("repository_id", &self.repository_id, "vrepo_")?;
        require_sha256("profile_root", &self.profile_root)?;
        require_sha256("initial_object_set_root", &self.initial_object_set_root)?;
        require_text("reason", &self.reason)?;
        if self.generation != 1 {
            return Err("repository origin generation must be exactly 1".into());
        }
        if self.initial_object_set_root != empty_object_set_root()? {
            return Err("repository origin must bind the empty initial object set".into());
        }
        Ok(())
    }
}

fn empty_object_set_root() -> Result<String, String> {
    Ok(format!(
        "sha256:{}",
        crate::canonical::sha256_canonical(&Vec::<String>::new())?
    ))
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "repository origin {field} must be non-empty, trimmed text"
        ));
    }
    Ok(())
}

fn require_prefixed(field: &str, value: &str, prefix: &str) -> Result<(), String> {
    require_text(field, value)?;
    if !value.starts_with(prefix) {
        return Err(format!(
            "repository origin {field} must start with `{prefix}`"
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    if crate::shape::is_full_sha256_root(value) {
        Ok(())
    } else {
        Err(format!(
            "repository origin {field} must be a full sha256: digest"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn genesis() -> RepositoryOriginV1 {
        RepositoryOriginV1::genesis(
            format!("vrepo_{}", "a".repeat(32)),
            root('a'),
            "Create one current repository.".into(),
        )
        .unwrap()
    }

    #[test]
    fn genesis_is_one_closed_current_origin() {
        let origin = genesis();
        assert_eq!(origin.generation, 1);
        assert_eq!(
            origin.id().unwrap(),
            crate::shape::derive_handle("vro_", &origin.canonical_root().unwrap()).unwrap()
        );
        RepositoryOriginV1::parse(&origin.canonical_bytes().unwrap()).unwrap();
    }

    /// An origin does not carry its own handle, so it cannot disagree with its
    /// own bytes and cannot be carried across an edit.
    #[test]
    fn the_handle_is_derived_and_not_a_field() {
        let mut value = serde_json::to_value(genesis()).unwrap();
        assert!(value.get("origin_id").is_none());
        value["origin_id"] = serde_json::json!("vro_0000000000000000");
        assert!(RepositoryOriginV1::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    /// The compaction fields are gone, and an object still carrying them is a
    /// generation this runtime does not read rather than one it tolerates.
    #[test]
    fn a_predecessor_block_is_no_longer_a_field_this_runtime_reads() {
        let mut value = serde_json::to_value(genesis()).unwrap();
        value["kind"] = serde_json::json!("compaction");
        value["predecessor"] = serde_json::json!({"remote": "https://example.invalid/x.git"});
        assert!(RepositoryOriginV1::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn a_generation_beyond_the_first_fails_closed() {
        let mut origin = genesis();
        origin.generation = 2;
        assert!(origin.verify().is_err());
    }
}
