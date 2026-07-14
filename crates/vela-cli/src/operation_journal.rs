//! Small, private, fsync-backed operation journals.
//!
//! These records are recovery plumbing. They are deliberately stored below
//! Git's private directory rather than in the frontier, so they can never be
//! mistaken for scientific state or swept into a publication commit.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

pub(crate) const JOURNAL_SCHEMA: &str = "vela.operation-journal.internal.v1";

/// Derive a stable, filesystem-safe operation id from the complete planning
/// identity. Retrying the same plan therefore finds the same journal.
pub(crate) fn operation_id(kind: &str, planning_identity: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"vela.operation-journal.internal.v1\0");
    digest.update(kind.as_bytes());
    digest.update([0]);
    digest.update(planning_identity);
    format!("vop_{}", hex::encode(digest.finalize()))
}

pub(crate) fn path(dir: &Path, operation_id: &str) -> PathBuf {
    dir.join(format!("{operation_id}.json"))
}

/// Atomically replace a journal and fsync both the file and containing
/// directory. The caller owns the schema of `value`.
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("journal path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create journal directory {}: {error}", parent.display()))?;

    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize operation journal: {error}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("create journal temporary file: {error}"))?;
    temporary
        .write_all(&bytes)
        .and_then(|()| temporary.write_all(b"\n"))
        .map_err(|error| format!("write operation journal: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("fsync operation journal: {error}"))?;
    temporary.persist(path).map_err(|error| {
        format!(
            "install operation journal {}: {}",
            path.display(),
            error.error
        )
    })?;
    sync_directory(parent)
}

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("read operation journal {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse operation journal {}: {error}", path.display()))
}

pub(crate) fn remove(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => {
            if let Some(parent) = path.parent() {
                sync_directory(parent)?;
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "remove operation journal {}: {error}",
            path.display()
        )),
    }
}

/// Keep only the newest `retain` completed operation records. Active journals
/// live in the parent directory and are therefore never considered here.
pub(crate) fn prune_json(dir: &Path, retain: usize) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|error| {
            format!(
                "read completed journal directory {}: {error}",
                dir.display()
            )
        })?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                let modified = entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                Some((modified, path))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let remove_count = entries.len().saturating_sub(retain);
    for (_, path) in entries.into_iter().take(remove_count) {
        fs::remove_file(&path)
            .map_err(|error| format!("prune completed journal {}: {error}", path.display()))?;
    }
    if remove_count > 0 {
        sync_directory(dir)?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("fsync journal directory {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Fixture {
        schema: String,
        value: u64,
    }

    #[test]
    fn journal_round_trip_is_atomic_and_removable() {
        let temporary = tempfile::tempdir().unwrap();
        let id = operation_id("test", b"same plan");
        assert_eq!(id, operation_id("test", b"same plan"));
        assert_ne!(id, operation_id("test", b"different plan"));
        let journal = path(temporary.path(), &id);
        let fixture = Fixture {
            schema: JOURNAL_SCHEMA.to_string(),
            value: 7,
        };

        write_json(&journal, &fixture).unwrap();
        assert_eq!(read_json::<Fixture>(&journal).unwrap(), fixture);
        assert!(journal.is_file());
        remove(&journal).unwrap();
        remove(&journal).unwrap();
        assert!(!journal.exists());
    }
}
