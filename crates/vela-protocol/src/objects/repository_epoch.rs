//! Exact predecessor boundary for a current-only Frontier repository.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REPOSITORY_EPOCH_V1_SCHEMA: &str = "vela.repository-epoch.v1";
pub const REPOSITORY_GENESIS_V1_SCHEMA: &str = "vela.repository-genesis.v1";

/// Native origin for a repository created directly in the current object
/// model. Unlike [`RepositoryEpochV1`], this object has no predecessor,
/// archive, equivalence report, or compatibility roots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryGenesisV1 {
    pub schema: String,
    pub epoch_id: String,
    pub frontier_id: String,
    pub epoch: u64,
    pub profile_root: String,
    pub initial_object_set_root: String,
    pub initial_event_log_root: String,
    pub initial_actor_registry_root: String,
    pub reason: String,
}

impl RepositoryGenesisV1 {
    pub fn build(
        frontier_id: String,
        profile_root: String,
        initial_object_set_root: String,
        reason: String,
    ) -> Result<Self, String> {
        let mut value = Self {
            schema: REPOSITORY_GENESIS_V1_SCHEMA.to_string(),
            epoch_id: String::new(),
            frontier_id,
            epoch: 1,
            profile_root,
            initial_object_set_root,
            initial_event_log_root: format!("sha256:{}", crate::events::event_log_hash(&[])),
            initial_actor_registry_root: format!("sha256:{}", hex::encode(Sha256::digest([]))),
            reason,
        };
        value.validate_semantics()?;
        value.epoch_id = value.derive_id()?;
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 64 * 1024 {
            return Err("Repository genesis exceeds the 64 KiB encoded limit".into());
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse repository genesis v1: {error}"))?;
        value.verify()?;
        if value.canonical_bytes()? != bytes {
            return Err("repository genesis bytes are not canonical JSON".into());
        }
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()?;
        let expected = self.derive_id()?;
        if self.epoch_id != expected {
            return Err(format!(
                "repository genesis id mismatch: declared {}, rebuilt {expected}",
                self.epoch_id
            ));
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

    fn derive_id(&self) -> Result<String, String> {
        let mut body = self.clone();
        body.epoch_id.clear();
        let bytes = crate::canonical::to_canonical_bytes(&body)?;
        Ok(format!("vre_{}", &hex::encode(Sha256::digest(bytes))[..16]))
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != REPOSITORY_GENESIS_V1_SCHEMA {
            return Err(format!(
                "repository genesis schema must be `{REPOSITORY_GENESIS_V1_SCHEMA}`"
            ));
        }
        require_prefixed("frontier_id", &self.frontier_id, "vfr_")?;
        if self.epoch != 1 {
            return Err("repository genesis epoch must be exactly 1".into());
        }
        require_sha256("profile_root", &self.profile_root)?;
        require_sha256("initial_object_set_root", &self.initial_object_set_root)?;
        require_sha256("initial_event_log_root", &self.initial_event_log_root)?;
        require_sha256(
            "initial_actor_registry_root",
            &self.initial_actor_registry_root,
        )?;
        let expected_event_root = format!("sha256:{}", crate::events::event_log_hash(&[]));
        let expected_actor_root = format!("sha256:{}", hex::encode(Sha256::digest([])));
        if self.initial_event_log_root != expected_event_root
            || self.initial_actor_registry_root != expected_actor_root
        {
            return Err(
                "repository genesis must bind the exact empty event and actor-registry roots"
                    .into(),
            );
        }
        require_text("reason", &self.reason)?;
        Ok(())
    }
}

/// Schema-dispatched current repository origin. Existing upgraded
/// repositories retain their exact predecessor epoch bytes; newly created
/// repositories use the smaller native genesis object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryBoundaryV1 {
    Predecessor(RepositoryEpochV1),
    Genesis(RepositoryGenesisV1),
}

impl RepositoryBoundaryV1 {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let schema = serde_json::from_slice::<serde_json::Value>(bytes)
            .map_err(|error| format!("parse repository boundary JSON: {error}"))?
            .get("schema")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "repository boundary has no schema".to_string())?
            .to_string();
        match schema.as_str() {
            REPOSITORY_EPOCH_V1_SCHEMA => {
                let epoch = RepositoryEpochV1::parse(bytes)?;
                if epoch.canonical_bytes()? != bytes {
                    return Err("repository epoch bytes are not canonical JSON".into());
                }
                Ok(Self::Predecessor(epoch))
            }
            REPOSITORY_GENESIS_V1_SCHEMA => Ok(Self::Genesis(RepositoryGenesisV1::parse(bytes)?)),
            _ => Err(format!("unsupported repository boundary schema `{schema}`")),
        }
    }

    pub fn frontier_id(&self) -> &str {
        match self {
            Self::Predecessor(value) => &value.frontier_id,
            Self::Genesis(value) => &value.frontier_id,
        }
    }

    pub fn epoch_id(&self) -> &str {
        match self {
            Self::Predecessor(value) => &value.epoch_id,
            Self::Genesis(value) => &value.epoch_id,
        }
    }

    pub fn reason(&self) -> &str {
        match self {
            Self::Predecessor(value) => &value.reason,
            Self::Genesis(value) => &value.reason,
        }
    }

    pub fn canonical_root(&self) -> Result<String, String> {
        match self {
            Self::Predecessor(value) => value.canonical_root(),
            Self::Genesis(value) => value.canonical_root(),
        }
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            Self::Predecessor(value) => value.canonical_bytes(),
            Self::Genesis(value) => value.canonical_bytes(),
        }
    }

    pub fn predecessor_roots(&self) -> Option<&PredecessorRoots> {
        match self {
            Self::Predecessor(value) => Some(&value.predecessor_roots),
            Self::Genesis(_) => None,
        }
    }

    pub fn genesis(&self) -> Option<&RepositoryGenesisV1> {
        match self {
            Self::Genesis(value) => Some(value),
            Self::Predecessor(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredecessorRoots {
    pub event_log: String,
    pub scientific_state: String,
    pub compatibility_snapshot: String,
    pub proposal_state: String,
    pub actor_registry: String,
    pub artifact_registry: String,
    pub authority_head: String,
    pub authority_event_log: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryEpochV1 {
    pub schema: String,
    pub epoch_id: String,
    pub frontier_id: String,
    pub epoch: u64,
    pub predecessor_remote: String,
    pub predecessor_tag: String,
    pub predecessor_commit: String,
    pub predecessor_tree: String,
    pub predecessor_profile_schema: String,
    pub predecessor_roots: PredecessorRoots,
    pub predecessor_git_object_manifest_root: String,
    pub archive_bundle_sha256: String,
    pub imported_claim_set_root: String,
    pub retained_current_object_set_root: String,
    pub archived_object_index_root: String,
    pub equivalence_report_root: String,
    pub reason: String,
}

impl RepositoryEpochV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        frontier_id: String,
        epoch: u64,
        predecessor_remote: String,
        predecessor_tag: String,
        predecessor_commit: String,
        predecessor_tree: String,
        predecessor_profile_schema: String,
        predecessor_roots: PredecessorRoots,
        predecessor_git_object_manifest_root: String,
        archive_bundle_sha256: String,
        imported_claim_set_root: String,
        retained_current_object_set_root: String,
        archived_object_index_root: String,
        equivalence_report_root: String,
        reason: String,
    ) -> Result<Self, String> {
        let mut value = Self {
            schema: REPOSITORY_EPOCH_V1_SCHEMA.to_string(),
            epoch_id: String::new(),
            frontier_id,
            epoch,
            predecessor_remote,
            predecessor_tag,
            predecessor_commit,
            predecessor_tree,
            predecessor_profile_schema,
            predecessor_roots,
            predecessor_git_object_manifest_root,
            archive_bundle_sha256,
            imported_claim_set_root,
            retained_current_object_set_root,
            archived_object_index_root,
            equivalence_report_root,
            reason,
        };
        value.validate_semantics()?;
        value.epoch_id = value.derive_id()?;
        value.verify()?;
        Ok(value)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("Repository epoch exceeds the 2 MiB encoded limit".into());
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse repository epoch v1: {error}"))?;
        value.verify()?;
        Ok(value)
    }

    pub fn verify(&self) -> Result<(), String> {
        self.validate_semantics()?;
        let expected = self.derive_id()?;
        if self.epoch_id != expected {
            return Err(format!(
                "repository epoch id mismatch: declared {}, rebuilt {expected}",
                self.epoch_id
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
        let mut body = self.clone();
        body.epoch_id.clear();
        let bytes = crate::canonical::to_canonical_bytes(&body)?;
        Ok(format!("vre_{}", &hex::encode(Sha256::digest(bytes))[..16]))
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != REPOSITORY_EPOCH_V1_SCHEMA {
            return Err(format!(
                "repository epoch schema must be `{REPOSITORY_EPOCH_V1_SCHEMA}`"
            ));
        }
        require_prefixed("frontier_id", &self.frontier_id, "vfr_")?;
        if self.epoch == 0 {
            return Err("repository epoch must be positive".into());
        }
        require_text("predecessor_remote", &self.predecessor_remote)?;
        if !self.predecessor_remote.starts_with("https://github.com/")
            || !self.predecessor_remote.ends_with(".git")
        {
            return Err(
                "repository epoch predecessor_remote must be canonical GitHub HTTPS".into(),
            );
        }
        require_text("predecessor_tag", &self.predecessor_tag)?;
        if !self.predecessor_tag.starts_with("pre-current-epoch/") {
            return Err("repository epoch predecessor_tag must use `pre-current-epoch/`".into());
        }
        require_git_oid("predecessor_commit", &self.predecessor_commit)?;
        require_git_oid("predecessor_tree", &self.predecessor_tree)?;
        require_text(
            "predecessor_profile_schema",
            &self.predecessor_profile_schema,
        )?;
        for (field, root) in [
            (
                "predecessor_roots.event_log",
                &self.predecessor_roots.event_log,
            ),
            (
                "predecessor_roots.scientific_state",
                &self.predecessor_roots.scientific_state,
            ),
            (
                "predecessor_roots.compatibility_snapshot",
                &self.predecessor_roots.compatibility_snapshot,
            ),
            (
                "predecessor_roots.proposal_state",
                &self.predecessor_roots.proposal_state,
            ),
            (
                "predecessor_roots.actor_registry",
                &self.predecessor_roots.actor_registry,
            ),
            (
                "predecessor_roots.artifact_registry",
                &self.predecessor_roots.artifact_registry,
            ),
            (
                "predecessor_roots.authority_head",
                &self.predecessor_roots.authority_head,
            ),
            (
                "predecessor_roots.authority_event_log",
                &self.predecessor_roots.authority_event_log,
            ),
        ] {
            require_sha256(field, root)?;
        }
        for (field, root) in [
            (
                "predecessor_git_object_manifest_root",
                &self.predecessor_git_object_manifest_root,
            ),
            ("archive_bundle_sha256", &self.archive_bundle_sha256),
            ("imported_claim_set_root", &self.imported_claim_set_root),
            (
                "retained_current_object_set_root",
                &self.retained_current_object_set_root,
            ),
            (
                "archived_object_index_root",
                &self.archived_object_index_root,
            ),
            ("equivalence_report_root", &self.equivalence_report_root),
        ] {
            require_sha256(field, root)?;
        }
        require_text("reason", &self.reason)?;
        Ok(())
    }
}

fn require_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(format!(
            "repository epoch {field} must be non-empty, trimmed text"
        ));
    }
    Ok(())
}

fn require_prefixed(field: &str, value: &str, prefix: &str) -> Result<(), String> {
    require_text(field, value)?;
    if !value.starts_with(prefix) {
        return Err(format!("repository epoch {field} must start with {prefix}"));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), String> {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("repository epoch {field} must be a full sha256: digest"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "repository epoch {field} must be a full sha256: digest"
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
            "repository epoch {field} must be a full Git object id"
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

    fn fixture() -> RepositoryEpochV1 {
        RepositoryEpochV1::build(
            "vfr_fixture".into(),
            1,
            "https://github.com/vela-science/fixture-frontier.git".into(),
            "pre-current-epoch/2026-07-27".into(),
            "a".repeat(40),
            "b".repeat(40),
            "vela.frontier-profile.v1".into(),
            PredecessorRoots {
                event_log: root('a'),
                scientific_state: root('b'),
                compatibility_snapshot: root('c'),
                proposal_state: root('d'),
                actor_registry: root('e'),
                artifact_registry: root('f'),
                authority_head: root('1'),
                authority_event_log: root('2'),
            },
            root('3'),
            root('4'),
            root('5'),
            root('6'),
            root('7'),
            root('8'),
            "Adopt the current repository contract.".into(),
        )
        .unwrap()
    }

    #[test]
    fn repository_epoch_is_content_addressed() {
        let value = fixture();
        assert!(value.epoch_id.starts_with("vre_"));
        RepositoryEpochV1::parse(&value.canonical_bytes().unwrap()).unwrap();
    }

    #[test]
    fn repository_epoch_rejects_predecessor_substitution() {
        let mut value = fixture();
        value.predecessor_commit = "9".repeat(40);
        assert!(value.verify().is_err());
    }

    #[test]
    fn native_genesis_has_no_predecessor_contract() {
        let value = RepositoryGenesisV1::build(
            "vfr_0123456789abcdef".into(),
            root('a'),
            root('b'),
            "Establish repository authority.".into(),
        )
        .unwrap();
        let bytes = value.canonical_bytes().unwrap();
        let parsed = RepositoryBoundaryV1::parse(&bytes).unwrap();
        assert_eq!(parsed.frontier_id(), "vfr_0123456789abcdef");
        assert_eq!(parsed.epoch_id(), value.epoch_id);
        assert!(parsed.predecessor_roots().is_none());
        let json = String::from_utf8(bytes).unwrap();
        assert!(!json.contains("predecessor"));
        assert!(!json.contains("archive"));
        assert!(!json.contains("equivalence"));
    }

    #[test]
    fn native_genesis_rejects_nonempty_history_roots() {
        let mut value = RepositoryGenesisV1::build(
            "vfr_0123456789abcdef".into(),
            root('a'),
            root('b'),
            "Establish repository authority.".into(),
        )
        .unwrap();
        value.initial_event_log_root = root('c');
        assert!(value.verify().is_err());
    }
}
