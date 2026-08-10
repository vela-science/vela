//! Bounded, symlink-safe reads for files retained inside a repository.
//!
//! The read happens through one already-open descriptor. Path identity is
//! checked before and after the bounded read, so a concurrent rename or
//! symlink swap cannot substitute different bytes after validation on the
//! supported Linux and macOS targets.

use std::io::Read;
use std::path::{Component, Path};

pub(crate) const PUBLIC_ARTIFACT_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub(crate) const PUBLIC_ARTIFACT_TOTAL_MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct BoundedFileError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    /// True only when a descriptor was successfully opened. Tests use this to
    /// prove pagination selects IDs before it touches retained material.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) opened: bool,
}

impl BoundedFileError {
    fn new(code: &'static str, message: impl Into<String>, opened: bool) -> Self {
        Self {
            code,
            message: message.into(),
            opened,
        }
    }

    /// This module's code as one of the CLI's published `error.code` values.
    ///
    /// The twelve codes here distinguish causes a caller of *this* module acts
    /// on; the published set is what a caller of the *binary* acts on, and the
    /// four ways an open or a read can fail underneath us are one fact to
    /// someone holding a path that did not produce bytes. Everything a caller
    /// could actually respond to differently — it is not there, it is a
    /// symlink, it is too large, it moved while we read it, the path is not one
    /// we accept — keeps its own name.
    pub(crate) fn published_code(&self) -> &'static str {
        match self.code {
            "missing" => "file_missing",
            "not_regular" => "file_not_regular",
            "symlink" => "file_symlink",
            "oversized" => "file_oversized",
            "path_escape" => "file_path_escape",
            "path_invalid" => "file_path_invalid",
            "path_changed" => "file_path_changed",
            _ => "file_unreadable",
        }
    }
}

impl std::fmt::Display for BoundedFileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

fn read_open_file(
    file: impl Read,
    path: &Path,
    max_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, BoundedFileError> {
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            BoundedFileError::new(
                "read_failed",
                format!("read {label} {}: {error}", path.display()),
                true,
            )
        })?;
    if bytes.len() as u64 > max_bytes {
        return Err(BoundedFileError::new(
            "oversized",
            format!(
                "{label} {} exceeds the {max_bytes}-byte limit",
                path.display()
            ),
            true,
        ));
    }
    Ok(bytes)
}

/// Read one explicitly supplied file without allowing its path to swap to a
/// symlink or different inode while Vela is reading it. Unlike
/// [`read_bounded_repository_file`], this accepts a path outside the repository so
/// portable Submission and Verification files can be retained directly.
pub(crate) fn read_bounded_file(
    path: &Path,
    max_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, BoundedFileError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        BoundedFileError::new(
            if error.kind() == std::io::ErrorKind::NotFound {
                "missing"
            } else {
                "inspect_failed"
            },
            format!("inspect {label} {}: {error}", path.display()),
            false,
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(BoundedFileError::new(
            "symlink",
            format!("{label} must not be a symlink: {}", path.display()),
            false,
        ));
    }
    if !metadata.is_file() {
        return Err(BoundedFileError::new(
            "not_regular",
            format!("{label} must be a regular file: {}", path.display()),
            false,
        ));
    }
    if metadata.len() > max_bytes {
        return Err(BoundedFileError::new(
            "oversized",
            format!(
                "{label} {} exceeds the {max_bytes}-byte limit",
                path.display()
            ),
            false,
        ));
    }

    let inspected_identity = same_file::Handle::from_path(path).map_err(|error| {
        BoundedFileError::new(
            "inspect_failed",
            format!("identify {label} {}: {error}", path.display()),
            false,
        )
    })?;
    let file = std::fs::File::open(path).map_err(|error| {
        BoundedFileError::new(
            "open_failed",
            format!("open {label} {}: {error}", path.display()),
            false,
        )
    })?;
    let opened = file.metadata().map_err(|error| {
        BoundedFileError::new(
            "inspect_open_failed",
            format!("inspect open {label} {}: {error}", path.display()),
            true,
        )
    })?;
    if !opened.is_file() {
        return Err(BoundedFileError::new(
            "not_regular",
            format!("{label} must be a regular file: {}", path.display()),
            true,
        ));
    }
    let opened_identity = same_file::Handle::from_file(file.try_clone().map_err(|error| {
        BoundedFileError::new(
            "inspect_open_failed",
            format!("clone open {label} {}: {error}", path.display()),
            true,
        )
    })?)
    .map_err(|error| {
        BoundedFileError::new(
            "inspect_open_failed",
            format!("identify open {label} {}: {error}", path.display()),
            true,
        )
    })?;
    verify_handle_identity(&inspected_identity, &opened_identity, path, label, true)?;
    if opened.len() > max_bytes {
        return Err(BoundedFileError::new(
            "oversized",
            format!(
                "{label} {} exceeds the {max_bytes}-byte limit",
                path.display()
            ),
            true,
        ));
    }
    verify_named_path_identity(path, &opened_identity, label, true)?;

    let bytes = read_open_file(file, path, max_bytes, label)?;
    verify_named_path_identity(path, &opened_identity, label, true)?;
    Ok(bytes)
}

pub(crate) fn read_bounded_repository_file(
    repository_path: &Path,
    relative: &Path,
    max_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, BoundedFileError> {
    if relative.is_absolute()
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(BoundedFileError::new(
            "path_invalid",
            format!(
                "{label} path must be normalized and repository-relative: {}",
                relative.display()
            ),
            false,
        ));
    }
    let root = repository_path.canonicalize().map_err(|error| {
        BoundedFileError::new(
            "repository_unresolvable",
            format!("canonicalize repository for {label}: {error}"),
            false,
        )
    })?;
    let components = relative.components().collect::<Vec<_>>();
    let mut current = root.clone();
    let mut inspected_identity = None;
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            unreachable!("relative components were validated")
        };
        current.push(component);
        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(BoundedFileError::new(
                    "missing",
                    format!("{label} is missing: {}", current.display()),
                    false,
                ));
            }
            Err(error) => {
                return Err(BoundedFileError::new(
                    "inspect_failed",
                    format!("inspect {label} {}: {error}", current.display()),
                    false,
                ));
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(BoundedFileError::new(
                "symlink",
                format!("{label} path traverses a symlink: {}", current.display()),
                false,
            ));
        }
        let is_leaf = index + 1 == components.len();
        if is_leaf {
            if !metadata.is_file() {
                return Err(BoundedFileError::new(
                    "not_regular",
                    format!("{label} must be a regular file: {}", current.display()),
                    false,
                ));
            }
            if metadata.len() > max_bytes {
                return Err(BoundedFileError::new(
                    "oversized",
                    format!(
                        "{label} {} exceeds the {max_bytes}-byte limit",
                        current.display()
                    ),
                    false,
                ));
            }
            inspected_identity = Some(same_file::Handle::from_path(&current).map_err(|error| {
                BoundedFileError::new(
                    "inspect_failed",
                    format!("identify {label} {}: {error}", current.display()),
                    false,
                )
            })?);
        } else if !metadata.is_dir() {
            return Err(BoundedFileError::new(
                "ancestor_not_directory",
                format!(
                    "{label} ancestor must be a directory: {}",
                    current.display()
                ),
                false,
            ));
        }
    }

    let file = std::fs::File::open(&current).map_err(|error| {
        BoundedFileError::new(
            "open_failed",
            format!("open {label} {}: {error}", current.display()),
            false,
        )
    })?;
    let opened = file.metadata().map_err(|error| {
        BoundedFileError::new(
            "inspect_open_failed",
            format!("inspect open {label} {}: {error}", current.display()),
            true,
        )
    })?;
    if !opened.is_file() {
        return Err(BoundedFileError::new(
            "not_regular",
            format!("{label} must be a regular file: {}", current.display()),
            true,
        ));
    }
    let inspected_identity = inspected_identity.ok_or_else(|| {
        BoundedFileError::new(
            "path_invalid",
            format!(
                "{label} path must name a repository-relative file: {}",
                relative.display()
            ),
            true,
        )
    })?;
    let opened_identity = same_file::Handle::from_file(file.try_clone().map_err(|error| {
        BoundedFileError::new(
            "inspect_open_failed",
            format!("clone open {label} {}: {error}", current.display()),
            true,
        )
    })?)
    .map_err(|error| {
        BoundedFileError::new(
            "inspect_open_failed",
            format!("identify open {label} {}: {error}", current.display()),
            true,
        )
    })?;
    verify_handle_identity(&inspected_identity, &opened_identity, &current, label, true)?;
    if opened.len() > max_bytes {
        return Err(BoundedFileError::new(
            "oversized",
            format!(
                "{label} {} exceeds the {max_bytes}-byte limit",
                current.display()
            ),
            true,
        ));
    }
    verify_open_path_identity(&root, &current, &opened_identity, label, true)?;

    let bytes = read_open_file(file, &current, max_bytes, label)?;
    verify_open_path_identity(&root, &current, &opened_identity, label, true)?;
    Ok(bytes)
}

fn verify_open_path_identity(
    root: &Path,
    current: &Path,
    opened: &same_file::Handle,
    label: &str,
    descriptor_opened: bool,
) -> Result<(), BoundedFileError> {
    verify_named_path_identity(current, opened, label, descriptor_opened)?;
    let canonical = current.canonicalize().map_err(|error| {
        BoundedFileError::new(
            "path_changed",
            format!("canonicalize {label} {}: {error}", current.display()),
            descriptor_opened,
        )
    })?;
    if !canonical.starts_with(root) {
        return Err(BoundedFileError::new(
            "path_escape",
            format!(
                "{label} resolved outside the repository: {}",
                canonical.display()
            ),
            descriptor_opened,
        ));
    }
    Ok(())
}

fn verify_named_path_identity(
    current: &Path,
    opened: &same_file::Handle,
    label: &str,
    descriptor_opened: bool,
) -> Result<(), BoundedFileError> {
    let linked = std::fs::symlink_metadata(current).map_err(|error| {
        BoundedFileError::new(
            "path_changed",
            format!("reinspect {label} {}: {error}", current.display()),
            descriptor_opened,
        )
    })?;
    if linked.file_type().is_symlink() || !linked.is_file() {
        return Err(BoundedFileError::new(
            "path_changed",
            format!(
                "{label} path must remain a non-symlink regular file: {}",
                current.display()
            ),
            descriptor_opened,
        ));
    }
    let named = same_file::Handle::from_path(current).map_err(|error| {
        BoundedFileError::new(
            "path_changed",
            format!("reinspect named {label} {}: {error}", current.display()),
            descriptor_opened,
        )
    })?;
    verify_handle_identity(&named, opened, current, label, descriptor_opened)
}

fn verify_handle_identity(
    inspected: &same_file::Handle,
    opened: &same_file::Handle,
    current: &Path,
    label: &str,
    descriptor_opened: bool,
) -> Result<(), BoundedFileError> {
    if inspected != opened {
        return Err(BoundedFileError::new(
            "path_changed",
            format!("{label} path changed while open: {}", current.display()),
            descriptor_opened,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::ErrorKind::{Interrupted, Other};
    use std::path::PathBuf;

    struct OneError(Option<std::io::ErrorKind>);

    impl Read for OneError {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            if let Some(kind) = self.0.take() {
                return Err(kind.into());
            }
            Ok(0)
        }
    }

    #[test]
    fn readers_accept_short_and_exact_files_and_reject_oversize_before_open() {
        let repository = tempfile::tempdir().unwrap();
        let relative = Path::new("records/receipts/receipt.json");
        let absolute = repository.path().join(relative);
        fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        fs::write(&absolute, b"12345").unwrap();

        assert_eq!(
            read_bounded_file(&absolute, 6, "receipt").unwrap(),
            b"12345"
        );
        let exact = read_bounded_repository_file(repository.path(), relative, 5, "receipt");
        assert_eq!(exact.unwrap(), b"12345");
        for error in [
            read_bounded_file(&absolute, 4, "receipt").unwrap_err(),
            read_bounded_repository_file(repository.path(), relative, 4, "receipt").unwrap_err(),
        ] {
            assert_eq!((error.code, error.opened), ("oversized", false));
        }
    }

    #[test]
    fn opened_reader_retries_interruptions_maps_errors_and_bounds_consumption() {
        let path = Path::new("opened.fixture");
        assert_eq!(
            read_open_file(OneError(Some(Interrupted)), path, 4, "receipt").unwrap(),
            b""
        );

        let failure = read_open_file(OneError(Some(Other)), path, 4, "receipt").unwrap_err();
        assert_eq!(
            (failure.code, failure.message.as_str(), failure.opened),
            (
                "read_failed",
                "read receipt opened.fixture: other error",
                true
            )
        );

        let mut large = std::io::Cursor::new([0; 1024]);
        let error = read_open_file(&mut large, path, 4, "receipt").unwrap_err();
        assert_eq!(
            (error.code, error.message.as_str(), error.opened),
            (
                "oversized",
                "receipt opened.fixture exceeds the 4-byte limit",
                true
            )
        );
        assert_eq!(large.position(), 5);
    }

    #[cfg(unix)]
    #[test]
    fn readers_reject_symlink_and_fifo_leaves_before_open() {
        use std::os::unix::fs::symlink;

        let repository = tempfile::tempdir().unwrap();
        fs::write(repository.path().join("real"), b"receipt").unwrap();
        let linked = repository.path().join("linked");
        symlink("real", &linked).unwrap();
        for error in [
            read_bounded_file(&linked, 128, "receipt").unwrap_err(),
            read_bounded_repository_file(repository.path(), Path::new("linked"), 128, "receipt")
                .unwrap_err(),
        ] {
            assert_eq!((error.code, error.opened), ("symlink", false));
        }

        let fifo = repository.path().join("fifo");
        assert!(
            std::process::Command::new("/usr/bin/mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        for error in [
            read_bounded_file(&fifo, 128, "receipt").unwrap_err(),
            read_bounded_repository_file(repository.path(), Path::new("fifo"), 128, "receipt")
                .unwrap_err(),
        ] {
            assert_eq!((error.code, error.opened), ("not_regular", false));
        }

        let records = repository.path().join("real-records");
        fs::create_dir(&records).unwrap();
        fs::write(records.join("receipt.json"), b"receipt").unwrap();
        symlink("real-records", repository.path().join("records")).unwrap();
        let error = read_bounded_repository_file(
            repository.path(),
            Path::new("records/receipt.json"),
            128,
            "receipt",
        )
        .unwrap_err();
        assert_eq!((error.code, error.opened), ("symlink", false));
    }

    #[test]
    fn inspected_leaf_identity_must_match_the_open_descriptor() {
        let directory = tempfile::tempdir().unwrap();
        let inspected_path = directory.path().join("inspected.json");
        let substituted_path = directory.path().join("substituted.json");
        fs::write(&inspected_path, b"inspected").unwrap();
        fs::write(&substituted_path, b"substituted").unwrap();
        let inspected = same_file::Handle::from_path(&inspected_path).unwrap();
        let substituted = same_file::Handle::from_path(&substituted_path).unwrap();

        let error =
            verify_handle_identity(&inspected, &substituted, &inspected_path, "receipt", true)
                .unwrap_err();
        assert_eq!((error.code, error.opened), ("path_changed", true));
    }

    #[test]
    fn accepts_only_normalized_repository_relative_paths() {
        let repository_path = tempfile::tempdir().unwrap();
        let absolute = repository_path.path().join("receipt.json");
        fs::write(&absolute, b"receipt").unwrap();
        let invalid = [
            PathBuf::from("./receipt.json"),
            PathBuf::from("records/../receipt.json"),
            absolute,
        ];

        for path in invalid {
            let error = read_bounded_repository_file(repository_path.path(), &path, 128, "receipt")
                .unwrap_err();
            assert!(error.code == "path_invalid" && !error.opened, "{path:?}");
        }
    }
}
