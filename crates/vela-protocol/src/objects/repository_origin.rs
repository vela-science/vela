//! Single current repository origin used after the pre-release compaction.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REPOSITORY_ORIGIN_V1_SCHEMA: &str = "vela.repository-origin.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryOriginKind {
    Genesis,
    Compaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryOriginPredecessorV1 {
    pub remote: String,
    pub tag: String,
    pub commit: String,
    pub tree: String,
    pub repository_root: String,
    pub authority_head_root: String,
    pub archive_sha256: String,
    pub object_manifest_root: String,
    pub equivalence_report_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryOriginV1 {
    pub schema: String,
    pub origin_id: String,
    pub frontier_id: String,
    pub generation: u64,
    pub profile_root: String,
    pub initial_object_set_root: String,
    pub kind: RepositoryOriginKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predecessor: Option<RepositoryOriginPredecessorV1>,
    pub reason: String,
}

impl RepositoryOriginV1 {
    pub fn genesis(
        frontier_id: String,
        profile_root: String,
        reason: String,
    ) -> Result<Self, String> {
        Self::build(
            frontier_id,
            1,
            profile_root,
            empty_object_set_root()?,
            RepositoryOriginKind::Genesis,
            None,
            reason,
        )
    }

    pub fn compaction(
        frontier_id: String,
        generation: u64,
        profile_root: String,
        initial_object_set_root: String,
        predecessor: RepositoryOriginPredecessorV1,
        reason: String,
    ) -> Result<Self, String> {
        Self::build(
            frontier_id,
            generation,
            profile_root,
            initial_object_set_root,
            RepositoryOriginKind::Compaction,
            Some(predecessor),
            reason,
        )
    }

    fn build(
        frontier_id: String,
        generation: u64,
        profile_root: String,
        initial_object_set_root: String,
        kind: RepositoryOriginKind,
        predecessor: Option<RepositoryOriginPredecessorV1>,
        reason: String,
    ) -> Result<Self, String> {
        let mut value = Self {
            schema: REPOSITORY_ORIGIN_V1_SCHEMA.into(),
            origin_id: String::new(),
            frontier_id,
            generation,
            profile_root,
            initial_object_set_root,
            kind,
            predecessor,
            reason,
        };
        value.validate_semantics()?;
        value.origin_id = value.derive_id()?;
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 128 * 1024 {
            return Err("repository origin exceeds the 128 KiB encoded limit".into());
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse repository origin v1: {error}"))?;
        value.verify()?;
        if value.canonical_bytes()? != bytes {
            return Err("repository origin bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()?;
        let expected = self.derive_id()?;
        if self.origin_id != expected {
            return Err(format!(
                "repository origin id mismatch: declared {}, rebuilt {expected}",
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
        if self.schema != REPOSITORY_ORIGIN_V1_SCHEMA {
            return Err(format!(
                "repository origin schema must be `{REPOSITORY_ORIGIN_V1_SCHEMA}`"
            ));
        }
        require_prefixed("frontier_id", &self.frontier_id, "vfr_")?;
        require_sha256("profile_root", &self.profile_root)?;
        require_sha256("initial_object_set_root", &self.initial_object_set_root)?;
        require_text("reason", &self.reason)?;
        match (&self.kind, &self.predecessor) {
            (RepositoryOriginKind::Genesis, None) => {
                if self.generation != 1 {
                    return Err("repository genesis origin generation must be exactly 1".into());
                }
                if self.initial_object_set_root != empty_object_set_root()? {
                    return Err(
                        "repository genesis origin must bind the empty initial object set".into(),
                    );
                }
            }
            (RepositoryOriginKind::Compaction, Some(predecessor)) => {
                if self.generation <= 1 {
                    return Err("repository compaction origin generation must exceed 1".into());
                }
                predecessor.validate()?;
            }
            (RepositoryOriginKind::Genesis, Some(_)) => {
                return Err("repository genesis origin cannot carry a predecessor".into());
            }
            (RepositoryOriginKind::Compaction, None) => {
                return Err("repository compaction origin requires a predecessor".into());
            }
        }
        Ok(())
    }
}

impl RepositoryOriginPredecessorV1 {
    fn validate(&self) -> Result<(), String> {
        if !self.remote.starts_with("https://github.com/") || !self.remote.ends_with(".git") {
            return Err(
                "repository origin predecessor remote must be canonical GitHub HTTPS".into(),
            );
        }
        if !self.tag.starts_with("pre-compaction/") {
            return Err("repository origin predecessor tag must use `pre-compaction/`".into());
        }
        require_git_oid("predecessor.commit", &self.commit)?;
        require_git_oid("predecessor.tree", &self.tree)?;
        for (field, value) in [
            ("predecessor.repository_root", &self.repository_root),
            ("predecessor.authority_head_root", &self.authority_head_root),
            ("predecessor.archive_sha256", &self.archive_sha256),
            (
                "predecessor.object_manifest_root",
                &self.object_manifest_root,
            ),
            (
                "predecessor.equivalence_report_root",
                &self.equivalence_report_root,
            ),
        ] {
            require_sha256(field, value)?;
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
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("repository origin {field} must be a full sha256: digest"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "repository origin {field} must be a full sha256: digest"
        ));
    }
    Ok(())
}

fn require_git_oid(field: &str, value: &str) -> Result<(), String> {
    if !matches!(value.len(), 40 | 64)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "repository origin {field} must be a full Git object id"
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

    fn predecessor() -> RepositoryOriginPredecessorV1 {
        RepositoryOriginPredecessorV1 {
            remote: "https://github.com/vela-science/fixture-frontier.git".into(),
            tag: "pre-compaction/2026-07-29".into(),
            commit: "a".repeat(40),
            tree: "b".repeat(40),
            repository_root: root('1'),
            authority_head_root: root('2'),
            archive_sha256: root('3'),
            object_manifest_root: root('4'),
            equivalence_report_root: root('5'),
        }
    }

    #[test]
    fn genesis_is_one_closed_current_origin() {
        let origin = RepositoryOriginV1::genesis(
            "vfr_fixture".into(),
            root('a'),
            "Create one current Frontier.".into(),
        )
        .unwrap();
        assert_eq!(origin.kind, RepositoryOriginKind::Genesis);
        assert!(origin.predecessor.is_none());
        RepositoryOriginV1::parse(&origin.canonical_bytes().unwrap()).unwrap();
    }

    #[test]
    fn compaction_binds_exact_predecessor_and_equivalence() {
        let origin = RepositoryOriginV1::compaction(
            "vfr_fixture".into(),
            2,
            root('a'),
            root('b'),
            predecessor(),
            "Compact pre-release state without changing Standing.".into(),
        )
        .unwrap();
        assert_eq!(origin.kind, RepositoryOriginKind::Compaction);
        assert!(origin.predecessor.is_some());
        RepositoryOriginV1::parse(&origin.canonical_bytes().unwrap()).unwrap();
    }

    #[test]
    fn origin_kind_and_predecessor_cannot_be_substituted() {
        let mut origin = RepositoryOriginV1::compaction(
            "vfr_fixture".into(),
            2,
            root('a'),
            root('b'),
            predecessor(),
            "Compact pre-release state without changing Standing.".into(),
        )
        .unwrap();
        origin.kind = RepositoryOriginKind::Genesis;
        assert!(origin.verify().is_err());
    }

    #[test]
    fn compaction_equivalence_root_is_load_bearing() {
        let mut origin = RepositoryOriginV1::compaction(
            "vfr_fixture".into(),
            2,
            root('a'),
            root('b'),
            predecessor(),
            "Compact pre-release state without changing Standing.".into(),
        )
        .unwrap();
        origin.predecessor.as_mut().unwrap().equivalence_report_root = root('f');
        assert!(origin.verify().is_err());
    }
}
