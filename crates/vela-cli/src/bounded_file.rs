//! Bounded, symlink-safe reads for files retained inside a repository.
//!
//! The read happens through one already-open descriptor. Path identity is
//! checked before and after the bounded read, so a concurrent rename or
//! symlink swap cannot substitute different bytes after validation on the
//! supported Linux and macOS targets.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::io::Read;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::Component;
use std::path::Path;

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

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundedFileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl BoundedFileIdentity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;

        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_bounded_descriptor(path: &Path) -> std::io::Result<std::fs::File> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(|error| std::io::Error::from_raw_os_error(error.raw_os_error()))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn path_changed_while_opening(error: &std::io::Error) -> bool {
    let code = error.raw_os_error();
    [
        rustix::io::Errno::LOOP,
        rustix::io::Errno::NOENT,
        rustix::io::Errno::NOTDIR,
    ]
    .into_iter()
    .any(|errno| code == Some(errno.raw_os_error()))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_inspected_file(path: &Path, label: &str) -> Result<std::fs::File, BoundedFileError> {
    #[cfg(test)]
    run_before_bounded_open_hook(path);
    open_bounded_descriptor(path).map_err(|error| {
        if path_changed_while_opening(&error) {
            BoundedFileError::new(
                "path_changed",
                format!("{label} path changed before open: {}", path.display()),
                false,
            )
        } else {
            BoundedFileError::new(
                "open_failed",
                format!("open {label} {}: {error}", path.display()),
                false,
            )
        }
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
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
#[cfg(any(target_os = "linux", target_os = "macos"))]
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

    let inspected_identity = BoundedFileIdentity::from_metadata(&metadata);
    let file = open_inspected_file(path, label)?;
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
    let opened_identity = BoundedFileIdentity::from_metadata(&opened);
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

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn read_bounded_file(
    path: &Path,
    _max_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, BoundedFileError> {
    Err(BoundedFileError::new(
        "inspect_failed",
        format!(
            "inspect {label} {}: descriptor-hardened bounded-file reads are unavailable on this platform",
            path.display()
        ),
        false,
    ))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
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
            inspected_identity = Some(BoundedFileIdentity::from_metadata(&metadata));
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

    let file = open_inspected_file(&current, label)?;
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
    let opened_identity = BoundedFileIdentity::from_metadata(&opened);
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

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn read_bounded_repository_file(
    _repository_path: &Path,
    relative: &Path,
    _max_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, BoundedFileError> {
    Err(BoundedFileError::new(
        "inspect_failed",
        format!(
            "inspect {label} {}: descriptor-hardened bounded-file reads are unavailable on this platform",
            relative.display()
        ),
        false,
    ))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verify_open_path_identity(
    root: &Path,
    current: &Path,
    opened: &BoundedFileIdentity,
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verify_named_path_identity(
    current: &Path,
    opened: &BoundedFileIdentity,
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
    let named = BoundedFileIdentity::from_metadata(&linked);
    verify_handle_identity(&named, opened, current, label, descriptor_opened)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verify_handle_identity(
    inspected: &BoundedFileIdentity,
    opened: &BoundedFileIdentity,
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

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
type BeforeBoundedOpenHook = Box<dyn FnOnce(&Path)>;

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
thread_local! {
    static BEFORE_BOUNDED_OPEN_HOOK: std::cell::RefCell<Option<BeforeBoundedOpenHook>> =
        std::cell::RefCell::new(None);
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
fn set_before_bounded_open_hook(hook: impl FnOnce(&Path) + 'static) {
    BEFORE_BOUNDED_OPEN_HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
fn run_before_bounded_open_hook(path: &Path) {
    BEFORE_BOUNDED_OPEN_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook(path);
        }
    });
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::fs;
    use std::io::ErrorKind::{Interrupted, Other};
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{Duration, Instant};

    const SWAP_CHILD: &str = "VELA_TEST_BOUNDED_FILE_SWAP_CHILD";
    const SWAP_TEST: &str =
        "bounded_file::tests::coordinated_path_swaps_fail_closed_without_blocking";

    struct OneError(Option<std::io::ErrorKind>);

    impl Read for OneError {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            if let Some(kind) = self.0.take() {
                return Err(kind.into());
            }
            Ok(0)
        }
    }

    fn create_fifo(path: &Path) {
        assert!(
            Command::new("/usr/bin/mkfifo")
                .arg(path)
                .status()
                .unwrap()
                .success()
        );
    }

    fn run_swap_child(case: &str) {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let repository = temporary.path().join("repository");
        let relative = Path::new("records/receipt.json");
        let leaf = repository.join(relative);
        fs::create_dir_all(leaf.parent().unwrap()).unwrap();
        fs::write(&leaf, b"original").unwrap();
        let sentinel = temporary.path().join("sentinel");
        fs::write(&sentinel, b"sentinel").unwrap();
        let repository_reader = case.starts_with("repository-");
        let expected_path = if repository_reader {
            repository.canonicalize().unwrap().join(relative)
        } else {
            leaf.clone()
        };

        match case {
            "standalone-fifo" | "repository-fifo" => set_before_bounded_open_hook(|path| {
                fs::remove_file(path).unwrap();
                create_fifo(path);
            }),
            "standalone-symlink" | "repository-symlink" => {
                let sentinel = sentinel.clone();
                set_before_bounded_open_hook(move |path| {
                    fs::remove_file(path).unwrap();
                    symlink(&sentinel, path).unwrap();
                });
            }
            "standalone-replacement" | "repository-replacement" => {
                let replacement = temporary.path().join("replacement");
                fs::write(&replacement, b"replacement").unwrap();
                set_before_bounded_open_hook(move |path| fs::rename(&replacement, path).unwrap());
            }
            "standalone-missing" | "repository-missing" => {
                set_before_bounded_open_hook(|path| fs::remove_file(path).unwrap());
            }
            "repository-notdir" => {
                let records = repository.join("records");
                let retained = repository.join("retained-records");
                set_before_bounded_open_hook(move |_| {
                    fs::rename(&records, &retained).unwrap();
                    fs::write(&records, b"not a directory").unwrap();
                });
            }
            "repository-escape" => {
                let outside = temporary.path().join("outside");
                fs::create_dir(&outside).unwrap();
                fs::hard_link(&leaf, outside.join("receipt.json")).unwrap();
                let records = repository.join("records");
                let retained = repository.join("retained-records");
                set_before_bounded_open_hook(move |_| {
                    fs::rename(&records, &retained).unwrap();
                    symlink(&outside, &records).unwrap();
                });
            }
            _ => panic!("unknown bounded-file swap case: {case}"),
        }

        let error = if repository_reader {
            read_bounded_repository_file(&repository, relative, 128, "receipt").unwrap_err()
        } else {
            read_bounded_file(&leaf, 128, "receipt").unwrap_err()
        };
        let expected = match case {
            "standalone-fifo" | "repository-fifo" => (
                "not_regular",
                true,
                format!(
                    "receipt must be a regular file: {}",
                    expected_path.display()
                ),
            ),
            "standalone-symlink" | "repository-symlink" | "standalone-missing"
            | "repository-missing" | "repository-notdir" => (
                "path_changed",
                false,
                format!(
                    "receipt path changed before open: {}",
                    expected_path.display()
                ),
            ),
            "standalone-replacement" | "repository-replacement" => (
                "path_changed",
                true,
                format!(
                    "receipt path changed while open: {}",
                    expected_path.display()
                ),
            ),
            "repository-escape" => (
                "path_escape",
                true,
                format!(
                    "receipt resolved outside the repository: {}",
                    temporary
                        .path()
                        .join("outside/receipt.json")
                        .canonicalize()
                        .unwrap()
                        .display()
                ),
            ),
            _ => unreachable!(),
        };
        assert_eq!(
            (error.code, error.opened, error.message),
            (expected.0, expected.1, expected.2),
            "case: {case}"
        );
        assert_eq!(fs::read(&sentinel).unwrap(), b"sentinel");
    }

    #[test]
    fn coordinated_path_swaps_fail_closed_without_blocking() {
        if let Ok(case) = std::env::var(SWAP_CHILD) {
            run_swap_child(&case);
            return;
        }
        for case in [
            "standalone-fifo",
            "repository-fifo",
            "standalone-symlink",
            "repository-symlink",
            "standalone-replacement",
            "repository-replacement",
            "standalone-missing",
            "repository-missing",
            "repository-notdir",
            "repository-escape",
        ] {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", SWAP_TEST, "--nocapture"])
                .env(SWAP_CHILD, case)
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            let status = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Ok(None) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        panic!("bounded-file swap case blocked past its deadline: {case}");
                    }
                    Err(error) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        panic!("poll bounded-file swap case {case}: {error}");
                    }
                }
            };
            assert!(status.success(), "bounded-file swap child failed: {case}");
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
        let inspected =
            BoundedFileIdentity::from_metadata(&fs::symlink_metadata(&inspected_path).unwrap());
        let substituted =
            BoundedFileIdentity::from_metadata(&fs::symlink_metadata(&substituted_path).unwrap());

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
