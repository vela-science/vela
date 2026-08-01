//! Small, private, fsync-backed operation journals.
//!
//! These records are recovery plumbing. Git publication stores them below the
//! Git directory; frontier transactions store them below the ignored private
//! `.vela/operation-journals` directory. Neither location is scientific state
//! or part of the publication path set.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

#[cfg(test)]
const JOURNAL_SCHEMA: &str = "vela.operation-journal.internal.v1";

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
    ensure_durable_directory(parent)?;

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

/// Create a journal directory one component at a time and durably link every
/// newly created component from its parent before it can contain a commit
/// marker. `create_dir_all` alone is insufficient here: after a power loss the
/// journal file may have been fsynced while one of its newly-created ancestor
/// directory entries was not.
///
/// Existing symlink or non-directory components fail closed. Callers still
/// hold their higher-level operation lock; this function additionally checks
/// each component after creation so a path substitution cannot be silently
/// accepted as a journal directory.
fn ensure_durable_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "journal directory component must be a real directory: {}",
                    path.display()
                ));
            }
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "inspect journal directory component {}: {error}",
                path.display()
            ));
        }
    }

    let parent = path.parent().ok_or_else(|| {
        format!(
            "missing journal directory component has no parent: {}",
            path.display()
        )
    })?;
    ensure_durable_directory(parent)?;

    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(format!(
                "create journal directory component {}: {error}",
                path.display()
            ));
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "inspect created journal directory component {}: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "journal directory component must be a real directory: {}",
            path.display()
        ));
    }

    // The new directory's own metadata and the parent entry naming it are
    // independent durability boundaries on Unix filesystems.
    sync_directory(path)?;
    sync_directory(parent)
}

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("read operation journal {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse operation journal {}: {error}", path.display()))
}

#[cfg(test)]
fn remove(path: &Path) -> Result<(), String> {
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

    #[test]
    fn first_write_durably_creates_each_nested_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let nested = temporary
            .path()
            .join("frontier")
            .join("committed")
            .join("blobs");
        let journal = nested.join("vop_test.json");
        let fixture = Fixture {
            schema: JOURNAL_SCHEMA.to_string(),
            value: 11,
        };

        write_json(&journal, &fixture).unwrap();

        assert!(nested.is_dir());
        assert_eq!(read_json::<Fixture>(&journal).unwrap(), fixture);
    }

    #[cfg(unix)]
    #[test]
    fn nested_journal_creation_rejects_a_symlink_component() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let outside = temporary.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let linked = temporary.path().join("linked");
        symlink(&outside, &linked).unwrap();
        let journal = linked.join("committed").join("vop_test.json");
        let fixture = Fixture {
            schema: JOURNAL_SCHEMA.to_string(),
            value: 13,
        };

        let error = write_json(&journal, &fixture).unwrap_err();
        assert!(error.contains("must be a real directory"), "{error}");
        assert!(!outside.join("committed").exists());
    }
}
