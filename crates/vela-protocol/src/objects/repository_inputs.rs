//! Portable exact-file input identities used by current derived projections.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

pub const RETAINED_OBJECT_MANIFEST_SCHEMA: &str = "vela.retained-object-manifest.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GitObjectFormat {
    Sha1,
    Sha256,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetainedObjectEntryV1 {
    pub path: String,
    pub git_mode: String,
    pub size: u64,
    /// Lowercase, bare SHA-256 digest of the exact retained bytes.
    pub sha256: String,
}

impl RetainedObjectEntryV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_repository_path(&self.path)?;
        if !matches!(self.git_mode.as_str(), "100644" | "100755") {
            return Err(
                "retained object git_mode must be tracked regular-file mode 100644 or 100755"
                    .to_string(),
            );
        }
        if !crate::shape::is_lower_hex_64(&self.sha256) {
            return Err("retained object sha256 must be 64 lowercase hex characters".into());
        }
        Ok(())
    }
}

/// The wire form is the exact canonical JSON list rather than an object
/// wrapper. The schema constant names that list in contracts and diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct RetainedObjectManifestV1(pub Vec<RetainedObjectEntryV1>);

impl RetainedObjectManifestV1 {
    pub fn validate(&self) -> Result<(), String> {
        let mut previous_path: Option<&str> = None;
        let mut portable_collision_keys = BTreeSet::new();
        for entry in &self.0 {
            entry.validate()?;
            if let Some(previous) = previous_path {
                if entry.path == previous {
                    return Err(format!("duplicate retained object path {:?}", entry.path));
                }
                if entry.path.as_str() < previous {
                    return Err("retained object entries must be sorted by path".into());
                }
            }
            previous_path = Some(&entry.path);

            let collision_key = entry
                .path
                .nfc()
                .flat_map(char::to_lowercase)
                .collect::<String>();
            if !portable_collision_keys.insert(collision_key) {
                return Err(format!(
                    "retained object path {:?} has a portable case-fold collision",
                    entry.path
                ));
            }
        }
        Ok(())
    }
}

fn validate_repository_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("retained object path must not be empty".into());
    }
    if path != path.nfc().collect::<String>() {
        return Err("retained object path must already be Unicode NFC".into());
    }
    if path.chars().any(char::is_control) {
        return Err("retained object path contains a forbidden control character".into());
    }
    if path.starts_with('/') || path.ends_with('/') || path.contains('\\') {
        return Err("retained object path must be a normalized relative repository path".into());
    }
    if path
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err("retained object path contains an empty, dot, or traversal segment".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_manifest_is_sorted_and_portable() {
        let entry = |path: &str, digest: char| RetainedObjectEntryV1 {
            path: path.into(),
            git_mode: "100644".into(),
            size: 1,
            sha256: digest.to_string().repeat(64),
        };
        assert!(
            RetainedObjectManifestV1(vec![entry("a", '1'), entry("b", '2')])
                .validate()
                .is_ok()
        );
        assert!(
            RetainedObjectManifestV1(vec![entry("A", '1'), entry("a", '2')])
                .validate()
                .is_err()
        );
    }
}
