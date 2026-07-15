//! Bounded, symlink-safe reads for files retained inside a frontier.
//!
//! The read happens through one already-open descriptor. Path identity is
//! checked before and after the bounded read, so a concurrent rename or
//! symlink swap cannot substitute different bytes after validation on the
//! supported Unix and Windows targets.

use std::io::Read;
use std::path::{Component, Path};

pub(crate) const RECEIPT_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub(crate) const PUBLIC_ARTIFACT_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub(crate) const PUBLIC_ARTIFACT_TOTAL_MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct BoundedFileError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    /// True only when a descriptor was successfully opened. Tests use this to
    /// prove pagination selects IDs before it touches retained material.
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
}

impl std::fmt::Display for BoundedFileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

/// Read one explicitly supplied file without allowing its path to swap to a
/// symlink or different inode while Vela is reading it. Unlike
/// [`read_bounded_frontier_file`], this accepts a path outside the frontier so
/// portable Receipt v1 files can be landed directly.
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
    verify_metadata_identity(&metadata, &opened, path, label, true)?;
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
    verify_named_path_identity(path, &opened, label, true)?;

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
    verify_named_path_identity(path, &opened, label, true)?;
    Ok(bytes)
}

pub(crate) fn read_bounded_frontier_file(
    frontier: &Path,
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
                "{label} path must be normalized and frontier-relative: {}",
                relative.display()
            ),
            false,
        ));
    }
    let root = frontier.canonicalize().map_err(|error| {
        BoundedFileError::new(
            "frontier_unresolvable",
            format!("canonicalize frontier for {label}: {error}"),
            false,
        )
    })?;
    let components = relative.components().collect::<Vec<_>>();
    let mut current = root.clone();
    let mut inspected_leaf = None;
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
            inspected_leaf = Some(metadata);
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
    let inspected_leaf = inspected_leaf.ok_or_else(|| {
        BoundedFileError::new(
            "path_invalid",
            format!(
                "{label} path must name a frontier-relative file: {}",
                relative.display()
            ),
            true,
        )
    })?;
    verify_metadata_identity(&inspected_leaf, &opened, &current, label, true)?;
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
    verify_open_path_identity(&root, &current, &opened, label, true)?;

    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            BoundedFileError::new(
                "read_failed",
                format!("read {label} {}: {error}", current.display()),
                true,
            )
        })?;
    if bytes.len() as u64 > max_bytes {
        return Err(BoundedFileError::new(
            "oversized",
            format!(
                "{label} {} exceeds the {max_bytes}-byte limit",
                current.display()
            ),
            true,
        ));
    }
    verify_open_path_identity(&root, &current, &opened, label, true)?;
    Ok(bytes)
}

fn verify_open_path_identity(
    root: &Path,
    current: &Path,
    opened: &std::fs::Metadata,
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
                "{label} resolved outside the frontier: {}",
                canonical.display()
            ),
            descriptor_opened,
        ));
    }
    Ok(())
}

fn verify_named_path_identity(
    current: &Path,
    opened: &std::fs::Metadata,
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
    let named = std::fs::metadata(current).map_err(|error| {
        BoundedFileError::new(
            "path_changed",
            format!("reinspect named {label} {}: {error}", current.display()),
            descriptor_opened,
        )
    })?;
    verify_metadata_identity(&named, opened, current, label, descriptor_opened)
}

fn verify_metadata_identity(
    inspected: &std::fs::Metadata,
    opened: &std::fs::Metadata,
    current: &Path,
    label: &str,
    descriptor_opened: bool,
) -> Result<(), BoundedFileError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if opened.dev() != inspected.dev() || opened.ino() != inspected.ino() {
            return Err(BoundedFileError::new(
                "path_changed",
                format!("{label} path changed while open: {}", current.display()),
                descriptor_opened,
            ));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if opened.volume_serial_number() != inspected.volume_serial_number()
            || opened.file_index() != inspected.file_index()
        {
            return Err(BoundedFileError::new(
                "path_changed",
                format!("{label} path changed while open: {}", current.display()),
                descriptor_opened,
            ));
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (inspected, opened);
        return Err(BoundedFileError::new(
            "identity_unsupported",
            format!(
                "{label} cannot bind file identity on this platform: {}",
                current.display()
            ),
            descriptor_opened,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn reads_a_valid_normalized_frontier_relative_file() {
        let frontier = tempfile::tempdir().unwrap();
        let relative = Path::new("records/receipts/receipt.json");
        let absolute = frontier.path().join(relative);
        fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        fs::write(&absolute, b"bounded receipt bytes").unwrap();

        let bytes = read_bounded_frontier_file(
            frontier.path(),
            relative,
            b"bounded receipt bytes".len() as u64,
            "receipt",
        )
        .unwrap();

        assert_eq!(bytes, b"bounded receipt bytes");
    }

    #[test]
    fn standalone_reader_accepts_an_absolute_file_and_rejects_size_before_open() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("receipt.json");
        fs::write(&path, b"bounded receipt bytes").unwrap();
        assert_eq!(
            read_bounded_file(&path, b"bounded receipt bytes".len() as u64, "receipt").unwrap(),
            b"bounded receipt bytes"
        );

        let error = read_bounded_file(&path, 4, "receipt").unwrap_err();
        assert_eq!(error.code, "oversized");
        assert!(!error.opened);
    }

    #[cfg(unix)]
    #[test]
    fn standalone_reader_rejects_a_symlink_leaf_before_open() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let real = directory.path().join("real.json");
        let linked = directory.path().join("linked.json");
        fs::write(&real, b"receipt").unwrap();
        symlink(&real, &linked).unwrap();

        let error = read_bounded_file(&linked, 128, "receipt").unwrap_err();
        assert_eq!(error.code, "symlink");
        assert!(!error.opened);
    }

    #[test]
    fn inspected_leaf_identity_must_match_the_open_descriptor() {
        let directory = tempfile::tempdir().unwrap();
        let inspected_path = directory.path().join("inspected.json");
        let substituted_path = directory.path().join("substituted.json");
        fs::write(&inspected_path, b"inspected").unwrap();
        fs::write(&substituted_path, b"substituted").unwrap();
        let inspected = fs::symlink_metadata(&inspected_path).unwrap();
        let substituted = fs::File::open(&substituted_path)
            .unwrap()
            .metadata()
            .unwrap();

        let error =
            verify_metadata_identity(&inspected, &substituted, &inspected_path, "receipt", true)
                .unwrap_err();
        assert_eq!(error.code, "path_changed");
        assert!(error.opened);
    }

    #[test]
    fn rejects_oversized_input_before_opening_a_descriptor() {
        let frontier = tempfile::tempdir().unwrap();
        let relative = Path::new("records/receipt.json");
        let absolute = frontier.path().join(relative);
        fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        fs::write(&absolute, b"12345").unwrap();

        let error =
            read_bounded_frontier_file(frontier.path(), relative, 4, "receipt").unwrap_err();

        assert_eq!(error.code, "oversized");
        assert!(!error.opened, "size metadata must reject before File::open");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_leaf_before_opening_a_descriptor() {
        use std::os::unix::fs::symlink;

        let frontier = tempfile::tempdir().unwrap();
        fs::write(frontier.path().join("real.json"), b"real bytes").unwrap();
        symlink("real.json", frontier.path().join("receipt.json")).unwrap();

        let error =
            read_bounded_frontier_file(frontier.path(), Path::new("receipt.json"), 128, "receipt")
                .unwrap_err();

        assert_eq!(error.code, "symlink");
        assert!(!error.opened);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_ancestor_before_opening_a_descriptor() {
        use std::os::unix::fs::symlink;

        let frontier = tempfile::tempdir().unwrap();
        let real_directory = frontier.path().join("real-records");
        fs::create_dir(&real_directory).unwrap();
        fs::write(real_directory.join("receipt.json"), b"real bytes").unwrap();
        symlink("real-records", frontier.path().join("records")).unwrap();

        let error = read_bounded_frontier_file(
            frontier.path(),
            Path::new("records/receipt.json"),
            128,
            "receipt",
        )
        .unwrap_err();

        assert_eq!(error.code, "symlink");
        assert!(!error.opened);
    }

    #[test]
    fn accepts_only_normalized_frontier_relative_paths() {
        let frontier = tempfile::tempdir().unwrap();
        let absolute = frontier.path().join("receipt.json");
        fs::write(&absolute, b"receipt").unwrap();
        let invalid = [
            PathBuf::from("./receipt.json"),
            PathBuf::from("records/../receipt.json"),
            absolute,
        ];

        for path in invalid {
            let error =
                read_bounded_frontier_file(frontier.path(), &path, 128, "receipt").unwrap_err();
            assert_eq!(error.code, "path_invalid", "path: {}", path.display());
            assert!(!error.opened, "path: {}", path.display());
        }
    }
}
