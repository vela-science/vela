//! Current repository write edges.
//!
//! The protocol/runtime owns repository validation. This module retains only
//! the operating-system edges that protocol code must not implement:
//! descriptor-bound atomic file replacement and the independently stored
//! sequence-one repository-authority pin.

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
use std::path::Component;
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
use rustix::fd::OwnedFd;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use vela_protocol::canonical;

pub const AUTHORITY_TRUST_ANCHOR_SCHEMA_V1: &str = "vela.authority-trust-anchor.v1";

/// An out-of-band pin for the first repository-authority record.
///
/// The sequence-1 authority-record root binds the Frontier, initial keyset,
/// policy authorization, principal, events, and execution claim. The local pin
/// selects the intended chain; it grants no authority and is never read from
/// repository-controlled bytes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorityTrustAnchorV1 {
    pub schema: String,
    pub frontier_id: String,
    pub first_authority_record_root: String,
}

impl AuthorityTrustAnchorV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORITY_TRUST_ANCHOR_SCHEMA_V1 {
            return Err(format!(
                "authority trust anchor schema must be {AUTHORITY_TRUST_ANCHOR_SCHEMA_V1}"
            ));
        }
        validate_frontier_id(&self.frontier_id)?;
        validate_sha256_root(
            "authority trust anchor first_authority_record_root",
            &self.first_authority_record_root,
        )
    }

    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        canonical::sha256_canonical(self).map(|digest| format!("sha256:{digest}"))
    }

    pub fn verify_sequence_one(
        &self,
        frontier_id: &str,
        first_authority_record_root: &str,
    ) -> Result<(), String> {
        self.validate()?;
        if self.frontier_id != frontier_id
            || self.first_authority_record_root != first_authority_record_root
        {
            return Err(
                "authority trust anchor does not select the verified sequence-1 record".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedAuthorityTrustAnchorV1 {
    pub path: PathBuf,
    pub root: String,
    pub anchor: AuthorityTrustAnchorV1,
}

/// The exact leaf state a repository-file replacement is allowed to consume.
///
/// This is deliberately stronger than "the path looked safe when the command
/// started". The preimage is retained together with pinned repository and
/// parent-directory descriptors and is checked again immediately before the
/// atomic install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryFilePreimage {
    Absent,
    Exact(Vec<u8>),
}

/// Permission handling for an installed repository file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryFileReplacementMode {
    /// Preserve the permission bits of an exact existing preimage.
    PreserveExisting,
    /// Install with one explicit permission mode.
    Exact(u32),
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RepositoryFileIdentity {
    device: u64,
    inode: u64,
    mode: u32,
}

// `rustix::fs::Stat` exposes normalized `u64`/`u32` fields on Linux and
// Android. Apple uses libc's narrower `dev_t`/`mode_t`, so only that ABI needs
// explicit widening.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn stat_device(stat: &rustix::fs::Stat) -> u64 {
    stat.st_dev
}

#[cfg(target_vendor = "apple")]
fn stat_device(stat: &rustix::fs::Stat) -> u64 {
    stat.st_dev as u64
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn stat_mode(stat: &rustix::fs::Stat) -> u32 {
    stat.st_mode
}

#[cfg(target_vendor = "apple")]
fn stat_mode(stat: &rustix::fs::Stat) -> u32 {
    stat.st_mode as u32
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
impl RepositoryFileIdentity {
    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat_device(stat),
            inode: stat.st_ino,
            mode: stat_mode(stat),
        }
    }

    fn same_object(self, stat: &rustix::fs::Stat) -> bool {
        self.device == stat_device(stat) && self.inode == stat.st_ino
    }
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
#[derive(Debug)]
struct PinnedRepositoryDirectory {
    name: OsString,
    descriptor: OwnedFd,
    identity: RepositoryFileIdentity,
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
struct RepositoryReplacementTemporary<'a> {
    parent: &'a OwnedFd,
    name: String,
    armed: bool,
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
impl RepositoryReplacementTemporary<'_> {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
impl Drop for RepositoryReplacementTemporary<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = rustix::fs::unlinkat(
                self.parent,
                self.name.as_str(),
                rustix::fs::AtFlags::empty(),
            );
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
struct RepositoryRootWriteLock<'a> {
    descriptor: &'a OwnedFd,
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
impl RepositoryRootWriteLock<'_> {
    fn try_acquire<'a>(
        descriptor: &'a OwnedFd,
        label: &Path,
    ) -> Result<RepositoryRootWriteLock<'a>, String> {
        use rustix::fs::FlockOperation;

        rustix::fs::flock(descriptor, FlockOperation::NonBlockingLockExclusive)
            .map_err(|error| {
                if error == rustix::io::Errno::AGAIN || error == rustix::io::Errno::WOULDBLOCK {
                    format!(
                        "repository replacement busy for {}; retry after the current Vela writer finishes",
                        label.display()
                    )
                } else {
                    format!("lock repository before replacing {}: {error}", label.display())
                }
            })?;
        Ok(RepositoryRootWriteLock { descriptor })
    }
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
impl Drop for RepositoryRootWriteLock<'_> {
    fn drop(&mut self) {
        let _ = rustix::fs::flock(self.descriptor, rustix::fs::FlockOperation::Unlock);
    }
}

/// A two-phase, descriptor-relative repository-file replacement.
///
/// Preparation opens the repository root and every directory below it with
/// no-follow semantics, records their identities, and binds either an exact
/// regular-file preimage or exact absence. Installation writes and fsyncs a
/// temporary through the pinned parent descriptor. Installation holds one
/// non-blocking advisory exclusive lock on the pinned repository-root
/// descriptor from the first preimage revalidation through install, readback,
/// and parent fsync. Every Vela caller of this edge therefore serializes on the
/// same repository inode; a concurrent Vela writer receives a precise busy
/// error rather than hanging. Non-cooperating repository writers are still
/// detected by exact preimage and displaced-file checks. They can make a
/// losing exchange transiently observable before rollback because POSIX has no
/// conditional replace-existing primitive; this edge is therefore limited to
/// non-authoritative, replaceable repository files such as settings and target
/// indexes. The lock does not claim to constrain a process that can already
/// mutate repository bytes directly.
///
/// With the lock held, installation revalidates the complete named path, then
/// performs an atomic no-clobber create or exchange. The displaced file from
/// an exchange is checked against the prepared preimage before it is removed.
/// Caught failures clean their reserved temporary path. A process crash may
/// leave the reserved `.vela-replace-*` recovery artifact, but cannot redirect
/// the write outside the pinned repository directory.
///
/// Linux and Apple platforms expose the required no-replace/exchange rename
/// primitives. Unsupported platforms fail closed at preparation rather than
/// falling back to a path-based rename with a known TOCTOU gap.
#[derive(Debug)]
pub struct PreparedRepositoryFileReplacement {
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    root_path: PathBuf,
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    relative_path: PathBuf,
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    root_descriptor: OwnedFd,
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    root_identity: RepositoryFileIdentity,
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    directories: Vec<PinnedRepositoryDirectory>,
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    leaf_name: OsString,
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    preimage: RepositoryFilePreimage,
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    preimage_identity: Option<RepositoryFileIdentity>,
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    replacement: Vec<u8>,
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    replacement_mode: u32,
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    max_bytes: u64,
    #[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
    #[allow(dead_code)]
    unavailable: (),
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
impl PreparedRepositoryFileReplacement {
    /// Prepare against an explicit exact preimage. `None` means the leaf must
    /// be absent; `Some(bytes)` means a regular non-symlink file with those
    /// exact bytes.
    pub fn prepare_exact(
        root_path: &Path,
        relative_path: &Path,
        expected: Option<&[u8]>,
        replacement: &[u8],
        mode: RepositoryFileReplacementMode,
        max_bytes: u64,
    ) -> Result<Self, String> {
        let expected = match expected {
            Some(bytes) => RepositoryFilePreimage::Exact(bytes.to_vec()),
            None => RepositoryFilePreimage::Absent,
        };
        Self::prepare(
            root_path,
            relative_path,
            Some(expected),
            replacement,
            mode,
            max_bytes,
        )
    }

    /// Prepare against the regular-file bytes or exact absence observed
    /// through the pinned parent descriptor.
    pub fn prepare_observed(
        root_path: &Path,
        relative_path: &Path,
        replacement: &[u8],
        mode: RepositoryFileReplacementMode,
        max_bytes: u64,
    ) -> Result<Self, String> {
        Self::prepare(root_path, relative_path, None, replacement, mode, max_bytes)
    }

    fn prepare(
        root_path: &Path,
        relative_path: &Path,
        expected: Option<RepositoryFilePreimage>,
        replacement: &[u8],
        mode: RepositoryFileReplacementMode,
        max_bytes: u64,
    ) -> Result<Self, String> {
        use rustix::fs::{FileType, Mode, OFlags};

        if max_bytes == 0 || replacement.len() as u64 > max_bytes {
            return Err(format!(
                "{} replacement exceeds the {} byte repository-file limit",
                relative_path.display(),
                max_bytes
            ));
        }
        let mut components = Vec::new();
        for component in relative_path.components() {
            match component {
                Component::Normal(component) => components.push(component.to_os_string()),
                _ => {
                    return Err(format!(
                        "repository replacement path '{}' must be a non-empty normalized relative path",
                        relative_path.display()
                    ));
                }
            }
        }
        let leaf_name = components
            .pop()
            .ok_or_else(|| "repository replacement path must contain a file name".to_string())?;
        let label = relative_path.display().to_string();

        let root_metadata = fs::symlink_metadata(root_path)
            .map_err(|error| format!("inspect repository root for {label}: {error}"))?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(format!(
                "{label} must be beneath real non-symlink repository directories"
            ));
        }
        let root_descriptor = rustix::fs::open(
            root_path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|error| format!("open repository root for {label}: {error}"))?;
        let root_stat = rustix::fs::fstat(&root_descriptor)
            .map_err(|error| format!("identify repository root for {label}: {error}"))?;
        if FileType::from_raw_mode(root_stat.st_mode) != FileType::Directory {
            return Err(format!(
                "{label} must be beneath real non-symlink repository directories"
            ));
        }
        let root_identity = RepositoryFileIdentity::from_stat(&root_stat);

        let mut directories: Vec<PinnedRepositoryDirectory> = Vec::new();
        for name in components {
            let parent = directories
                .last()
                .map_or(&root_descriptor, |directory| &directory.descriptor);
            let descriptor = rustix::fs::openat(
                parent,
                &name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .map_err(|error| format!("open pinned parent of {label}: {error}"))?;
            let stat = rustix::fs::fstat(&descriptor)
                .map_err(|error| format!("identify pinned parent of {label}: {error}"))?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
                return Err(format!(
                    "{label} must be beneath real non-symlink repository directories"
                ));
            }
            directories.push(PinnedRepositoryDirectory {
                name,
                descriptor,
                identity: RepositoryFileIdentity::from_stat(&stat),
            });
        }
        let parent = directories
            .last()
            .map_or(&root_descriptor, |directory| &directory.descriptor);
        let observed = Self::read_named_preimage(parent, &leaf_name, max_bytes, &label)?;
        let (preimage, preimage_identity) = match (expected, observed) {
            (Some(RepositoryFilePreimage::Absent), None) => (RepositoryFilePreimage::Absent, None),
            (Some(RepositoryFilePreimage::Absent), Some(_)) => {
                return Err(format!("{label} appeared before replacement"));
            }
            (Some(RepositoryFilePreimage::Exact(expected)), Some((observed, identity)))
                if observed == expected =>
            {
                (RepositoryFilePreimage::Exact(expected), Some(identity))
            }
            (Some(RepositoryFilePreimage::Exact(_)), Some(_)) => {
                return Err(format!("{label} changed before replacement"));
            }
            (Some(RepositoryFilePreimage::Exact(_)), None) => {
                return Err(format!("{label} disappeared before replacement"));
            }
            (None, Some((observed, identity))) => {
                (RepositoryFilePreimage::Exact(observed), Some(identity))
            }
            (None, None) => (RepositoryFilePreimage::Absent, None),
        };
        let replacement_mode = match mode {
            RepositoryFileReplacementMode::Exact(mode) if mode & !0o777 != 0 => {
                return Err(format!(
                    "{label} replacement mode must contain only ordinary permission bits"
                ));
            }
            RepositoryFileReplacementMode::Exact(mode) => mode,
            RepositoryFileReplacementMode::PreserveExisting => {
                preimage_identity
                    .ok_or_else(|| format!("{label} has no existing mode to preserve"))?
                    .mode
                    & 0o777
            }
        };
        Ok(Self {
            root_path: root_path.to_path_buf(),
            relative_path: relative_path.to_path_buf(),
            root_descriptor,
            root_identity,
            directories,
            leaf_name,
            preimage,
            preimage_identity,
            replacement: replacement.to_vec(),
            replacement_mode,
            max_bytes,
        })
    }

    fn parent_descriptor(&self) -> &OwnedFd {
        self.directories
            .last()
            .map_or(&self.root_descriptor, |directory| &directory.descriptor)
    }

    fn read_open_file(file: OwnedFd, max_bytes: u64, label: &str) -> Result<Vec<u8>, String> {
        let mut file = fs::File::from(file);
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(max_bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("read pinned {label}: {error}"))?;
        if bytes.len() as u64 > max_bytes {
            return Err(format!(
                "{label} exceeds the {max_bytes} byte repository-file limit"
            ));
        }
        Ok(bytes)
    }

    fn read_named_preimage(
        parent: &OwnedFd,
        leaf_name: &OsString,
        max_bytes: u64,
        label: &str,
    ) -> Result<Option<(Vec<u8>, RepositoryFileIdentity)>, String> {
        use rustix::fs::{FileType, Mode, OFlags};

        let descriptor = match rustix::fs::openat(
            parent,
            leaf_name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(error) if error == rustix::io::Errno::NOENT => return Ok(None),
            Err(error) => {
                return Err(format!("open pinned repository file {label}: {error}"));
            }
        };
        let stat = rustix::fs::fstat(&descriptor)
            .map_err(|error| format!("identify pinned repository file {label}: {error}"))?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(format!(
                "{label} must be absent or a regular non-symlink file"
            ));
        }
        let identity = RepositoryFileIdentity::from_stat(&stat);
        let bytes = Self::read_open_file(descriptor, max_bytes, label)?;
        Ok(Some((bytes, identity)))
    }

    fn revalidate_named_path(&self) -> Result<(), String> {
        use rustix::fs::{AtFlags, FileType, Mode, OFlags};

        let label = self.relative_path.display();
        let named_root = rustix::fs::open(
            &self.root_path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|error| {
            format!("repository parent of {label} changed before replacement: {error}")
        })?;
        let root_stat = rustix::fs::fstat(&named_root)
            .map_err(|error| format!("reidentify repository root for {label}: {error}"))?;
        if !self.root_identity.same_object(&root_stat) {
            return Err(format!(
                "repository parent of {label} changed before replacement"
            ));
        }
        let mut parent = &self.root_descriptor;
        for directory in &self.directories {
            let stat = rustix::fs::statat(parent, &directory.name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|error| format!("reidentify pinned parent of {label}: {error}"))?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
                || !directory.identity.same_object(&stat)
            {
                return Err(format!(
                    "repository parent of {label} changed before replacement"
                ));
            }
            parent = &directory.descriptor;
        }
        Ok(())
    }

    fn revalidate_preimage(&self) -> Result<(), String> {
        let label = self.relative_path.display().to_string();
        let observed = Self::read_named_preimage(
            self.parent_descriptor(),
            &self.leaf_name,
            self.max_bytes,
            &label,
        )?;
        match (&self.preimage, self.preimage_identity, observed) {
            (RepositoryFilePreimage::Absent, None, None) => Ok(()),
            (
                RepositoryFilePreimage::Exact(expected),
                Some(expected_identity),
                Some((observed, observed_identity)),
            ) if observed == *expected && observed_identity == expected_identity => Ok(()),
            (RepositoryFilePreimage::Absent, _, Some(_)) => {
                Err(format!("{label} appeared before replacement"))
            }
            (RepositoryFilePreimage::Exact(_), _, None) => {
                Err(format!("{label} disappeared before replacement"))
            }
            _ => Err(format!("{label} changed before replacement")),
        }
    }

    fn verify_installed_leaf(
        &self,
        temporary_identity: RepositoryFileIdentity,
    ) -> Result<(), String> {
        let label = self.relative_path.display().to_string();
        let (bytes, identity) = Self::read_named_preimage(
            self.parent_descriptor(),
            &self.leaf_name,
            self.max_bytes,
            &label,
        )?
        .ok_or_else(|| format!("installed {label} disappeared during exact readback"))?;
        if bytes != self.replacement || identity != temporary_identity {
            return Err(format!(
                "installed {label} does not match its planned replacement"
            ));
        }
        self.revalidate_named_path()
    }

    /// Install the prepared replacement.
    pub fn install(self) -> Result<bool, String> {
        self.install_with_hook(|| Ok(()))
    }

    /// Install with one hook immediately before named-path/preimage
    /// revalidation. This is public so callers can supply cancellation checks
    /// and deterministic race tests without weakening the production path.
    pub fn install_with_hook(
        self,
        before_replace: impl FnOnce() -> Result<(), String>,
    ) -> Result<bool, String> {
        self.install_with_hooks(|| Ok(()), before_replace)
    }

    fn install_with_hooks(
        self,
        after_temporary_created: impl FnOnce() -> Result<(), String>,
        before_replace: impl FnOnce() -> Result<(), String>,
    ) -> Result<bool, String> {
        use rand_core::RngCore;
        use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};

        // The lock is advisory by design: it serializes every Vela replacement
        // prepared against this repository inode. A process with direct write
        // access to repository bytes remains outside this cooperative lease
        // and is handled by the exact preimage/readback checks below.
        let _root_lock =
            RepositoryRootWriteLock::try_acquire(&self.root_descriptor, &self.relative_path)?;
        self.revalidate_named_path()?;
        self.revalidate_preimage()?;
        if matches!(
            &self.preimage,
            RepositoryFilePreimage::Exact(bytes)
                if *bytes == self.replacement
                    && self.preimage_identity.is_some_and(|identity| {
                        identity.mode & 0o777 == self.replacement_mode
                    })
        ) {
            return Ok(false);
        }

        let parent = self.parent_descriptor();
        let mut random = rand_core::OsRng;
        let (temporary_name, temporary) = loop {
            let candidate = format!(
                ".vela-replace-{}-{:016x}",
                std::process::id(),
                random.next_u64()
            );
            match rustix::fs::openat(
                parent,
                candidate.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::from_raw_mode(0o600),
            ) {
                Ok(file) => break (candidate, file),
                Err(error) if error == rustix::io::Errno::EXIST => continue,
                Err(error) => {
                    return Err(format!(
                        "create pinned temporary replacement for {}: {error}",
                        self.relative_path.display()
                    ));
                }
            }
        };
        let mut temporary_cleanup = RepositoryReplacementTemporary {
            parent,
            name: temporary_name,
            armed: true,
        };
        after_temporary_created()?;

        // Keep an incomplete temporary private. Apply the requested final mode
        // only after all bytes have been written, then capture the identity
        // after chmod so a restrictive umask cannot create a false readback
        // mismatch.
        rustix::fs::fchmod(&temporary, Mode::from_raw_mode(0o600)).map_err(|error| {
            format!(
                "make temporary replacement private for {}: {error}",
                self.relative_path.display()
            )
        })?;
        let mut temporary = fs::File::from(temporary);
        temporary.write_all(&self.replacement).map_err(|error| {
            format!(
                "write pinned temporary replacement for {}: {error}",
                self.relative_path.display()
            )
        })?;
        rustix::fs::fchmod(&temporary, Mode::from_raw_mode(self.replacement_mode as _)).map_err(
            |error| {
                format!(
                    "set temporary replacement mode for {}: {error}",
                    self.relative_path.display()
                )
            },
        )?;
        temporary.sync_all().map_err(|error| {
            format!(
                "fsync pinned temporary replacement for {}: {error}",
                self.relative_path.display()
            )
        })?;
        let temporary_stat = rustix::fs::fstat(&temporary).map_err(|error| {
            format!(
                "identify completed temporary replacement for {}: {error}",
                self.relative_path.display()
            )
        })?;
        let temporary_identity = RepositoryFileIdentity::from_stat(&temporary_stat);
        drop(temporary);

        before_replace()?;
        self.revalidate_named_path()?;
        self.revalidate_preimage()?;

        let parent = self.parent_descriptor();
        match &self.preimage {
            RepositoryFilePreimage::Absent => {
                rustix::fs::renameat_with(
                    parent,
                    temporary_cleanup.name.as_str(),
                    parent,
                    &self.leaf_name,
                    RenameFlags::NOREPLACE,
                )
                .map_err(|error| {
                    format!(
                        "atomically install absent {} without clobbering: {error}",
                        self.relative_path.display()
                    )
                })?;
                if let Err(error) = self.verify_installed_leaf(temporary_identity) {
                    let rollback = rustix::fs::renameat_with(
                        parent,
                        &self.leaf_name,
                        parent,
                        temporary_cleanup.name.as_str(),
                        RenameFlags::NOREPLACE,
                    );
                    return Err(match rollback {
                        Ok(()) => error,
                        Err(rollback) => {
                            format!("{error}; failed to roll back replacement: {rollback}")
                        }
                    });
                }
                if let Err(error) = rustix::fs::fsync(parent) {
                    let rollback = rustix::fs::renameat_with(
                        parent,
                        &self.leaf_name,
                        parent,
                        temporary_cleanup.name.as_str(),
                        RenameFlags::NOREPLACE,
                    );
                    let rollback_sync = rollback
                        .as_ref()
                        .ok()
                        .and_then(|()| rustix::fs::fsync(parent).ok());
                    return Err(match (rollback, rollback_sync) {
                        (Ok(()), Some(())) => format!(
                            "fsync pinned parent of {}: {error}; replacement was rolled back",
                            self.relative_path.display()
                        ),
                        (Ok(()), None) => format!(
                            "fsync pinned parent of {}: {error}; replacement was rolled back but rollback durability is uncertain",
                            self.relative_path.display()
                        ),
                        (Err(rollback), _) => format!(
                            "fsync pinned parent of {}: {error}; failed to roll back replacement: {rollback}",
                            self.relative_path.display()
                        ),
                    });
                }
                // The rename and its parent directory are durable; the
                // temporary name no longer exists.
                temporary_cleanup.disarm();
            }
            RepositoryFilePreimage::Exact(expected) => {
                rustix::fs::renameat_with(
                    parent,
                    temporary_cleanup.name.as_str(),
                    parent,
                    &self.leaf_name,
                    RenameFlags::EXCHANGE,
                )
                .map_err(|error| {
                    format!(
                        "atomically exchange {} replacement: {error}",
                        self.relative_path.display()
                    )
                })?;
                let displaced = Self::read_named_preimage(
                    parent,
                    &OsString::from(&temporary_cleanup.name),
                    self.max_bytes,
                    &self.relative_path.display().to_string(),
                );
                let displaced_matches = displaced.as_ref().is_ok_and(|value| {
                    value.as_ref().is_some_and(|(bytes, identity)| {
                        bytes == expected && Some(*identity) == self.preimage_identity
                    })
                });
                let installed = self.verify_installed_leaf(temporary_identity);
                if !displaced_matches || installed.is_err() {
                    let reason = installed.err().unwrap_or_else(|| {
                        format!(
                            "{} changed during atomic exchange",
                            self.relative_path.display()
                        )
                    });
                    let rollback = rustix::fs::renameat_with(
                        parent,
                        temporary_cleanup.name.as_str(),
                        parent,
                        &self.leaf_name,
                        RenameFlags::EXCHANGE,
                    );
                    return Err(match rollback {
                        Ok(()) => reason,
                        Err(rollback) => {
                            format!("{reason}; failed to roll back replacement: {rollback}")
                        }
                    });
                }

                // Persist the exact installed leaf while the displaced
                // preimage still exists, so an fsync error remains rollback
                // capable rather than returning an ambiguous changed state.
                if let Err(error) = rustix::fs::fsync(parent) {
                    let rollback = rustix::fs::renameat_with(
                        parent,
                        temporary_cleanup.name.as_str(),
                        parent,
                        &self.leaf_name,
                        RenameFlags::EXCHANGE,
                    );
                    let rollback_sync = rollback
                        .as_ref()
                        .ok()
                        .and_then(|()| rustix::fs::fsync(parent).ok());
                    return Err(match (rollback, rollback_sync) {
                        (Ok(()), Some(())) => format!(
                            "fsync pinned parent of {}: {error}; replacement was rolled back",
                            self.relative_path.display()
                        ),
                        (Ok(()), None) => format!(
                            "fsync pinned parent of {}: {error}; replacement was rolled back but rollback durability is uncertain",
                            self.relative_path.display()
                        ),
                        (Err(rollback), _) => format!(
                            "fsync pinned parent of {}: {error}; failed to roll back replacement: {rollback}",
                            self.relative_path.display()
                        ),
                    });
                }

                if let Err(error) =
                    rustix::fs::unlinkat(parent, temporary_cleanup.name.as_str(), AtFlags::empty())
                {
                    let rollback = rustix::fs::renameat_with(
                        parent,
                        temporary_cleanup.name.as_str(),
                        parent,
                        &self.leaf_name,
                        RenameFlags::EXCHANGE,
                    );
                    let rollback_sync = rollback
                        .as_ref()
                        .ok()
                        .and_then(|()| rustix::fs::fsync(parent).ok());
                    return Err(match (rollback, rollback_sync) {
                        (Ok(()), Some(())) => format!(
                            "remove displaced {} preimage: {error}; replacement was rolled back",
                            self.relative_path.display()
                        ),
                        (Ok(()), None) => format!(
                            "remove displaced {} preimage: {error}; replacement was rolled back but rollback durability is uncertain",
                            self.relative_path.display()
                        ),
                        (Err(rollback), _) => format!(
                            "remove displaced {} preimage: {error}; failed to roll back replacement: {rollback}",
                            self.relative_path.display()
                        ),
                    });
                }
                temporary_cleanup.disarm();

                // The installed leaf was already made durable while rollback
                // remained possible. This second sync persists only removal of
                // the displaced recovery artifact; an error cannot make the
                // successfully installed leaf semantically false.
                let _ = rustix::fs::fsync(parent);
            }
        }
        Ok(true)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
pub const REPOSITORY_FILE_MUTATION_UNAVAILABLE: &str = "repository-file mutation is unavailable on this platform because Vela cannot provide descriptor-relative no-clobber/exchange replacement";

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
impl PreparedRepositoryFileReplacement {
    pub fn prepare_exact(
        _root_path: &Path,
        _relative_path: &Path,
        _expected: Option<&[u8]>,
        _replacement: &[u8],
        _mode: RepositoryFileReplacementMode,
        _max_bytes: u64,
    ) -> Result<Self, String> {
        Err(REPOSITORY_FILE_MUTATION_UNAVAILABLE.to_string())
    }

    pub fn prepare_observed(
        _root_path: &Path,
        _relative_path: &Path,
        _replacement: &[u8],
        _mode: RepositoryFileReplacementMode,
        _max_bytes: u64,
    ) -> Result<Self, String> {
        Err(REPOSITORY_FILE_MUTATION_UNAVAILABLE.to_string())
    }

    pub fn install(self) -> Result<bool, String> {
        Err(REPOSITORY_FILE_MUTATION_UNAVAILABLE.to_string())
    }

    pub fn install_with_hook(
        self,
        _before_replace: impl FnOnce() -> Result<(), String>,
    ) -> Result<bool, String> {
        self.install()
    }
}

/// Deterministic path for the independently distributed sequence-1 authority
/// root. Repository bytes and environment variables cannot redirect it.
pub fn authority_trust_anchor_path(user_home: &Path, frontier_id: &str) -> Result<PathBuf, String> {
    validate_frontier_id(frontier_id)?;
    Ok(user_home
        .join(".vela")
        .join("trust")
        .join("authorities")
        .join(format!("{frontier_id}.json")))
}

pub fn load_authority_trust_anchor_from_home(
    user_home: &Path,
    frontier_id: &str,
) -> Result<Option<LoadedAuthorityTrustAnchorV1>, String> {
    let path = authority_trust_anchor_path(user_home, frontier_id)?;
    let Some(anchor) = load_private_trust_document::<AuthorityTrustAnchorV1>(
        user_home,
        "authorities",
        &path,
        "authority trust anchor",
    )?
    else {
        return Ok(None);
    };
    anchor.validate()?;
    if anchor.frontier_id != frontier_id {
        return Err(format!(
            "authority trust anchor frontier {} does not match requested {frontier_id}",
            anchor.frontier_id
        ));
    }
    Ok(Some(LoadedAuthorityTrustAnchorV1 {
        root: anchor.root()?,
        path,
        anchor,
    }))
}

pub fn install_authority_trust_anchor_from_home(
    user_home: &Path,
    anchor: &AuthorityTrustAnchorV1,
) -> Result<LoadedAuthorityTrustAnchorV1, String> {
    anchor.validate()?;
    let path = authority_trust_anchor_path(user_home, &anchor.frontier_id)?;
    install_private_trust_document(
        user_home,
        "authorities",
        &path,
        anchor,
        "authority trust anchor",
        ".authority-trust-anchor-",
    )?;
    load_authority_trust_anchor_from_home(user_home, &anchor.frontier_id)?
        .ok_or_else(|| "installed authority trust anchor could not be read back".to_string())
}

/// Atomically move one independently retained authority pin to the exact
/// sequence-one record established by a verified repository-origin
/// transition.
///
/// This is not TOFU and does not derive trust from repository-controlled
/// bytes. The caller must supply the exact already-installed anchor as the
/// preimage and must have independently verified the signed transition before
/// invoking this edge. A concurrent change, symlink substitution, or
/// unexpected existing value fails closed.
pub fn rebind_authority_trust_anchor_from_home(
    user_home: &Path,
    expected: &AuthorityTrustAnchorV1,
    replacement: &AuthorityTrustAnchorV1,
) -> Result<LoadedAuthorityTrustAnchorV1, String> {
    expected.validate()?;
    replacement.validate()?;
    if expected.frontier_id != replacement.frontier_id {
        return Err("authority trust-anchor rebind cannot change Frontier identity".to_string());
    }
    let loaded = load_authority_trust_anchor_from_home(user_home, &expected.frontier_id)?
        .ok_or_else(|| {
            "authority trust-anchor rebind requires an existing exact pin".to_string()
        })?;
    if loaded.anchor != *expected {
        return Err(
            "authority trust-anchor rebind preimage does not match the installed pin".into(),
        );
    }
    if expected == replacement {
        return Ok(loaded);
    }

    let mut expected_bytes = serde_json::to_vec_pretty(expected)
        .map_err(|error| format!("serialize authority trust-anchor preimage: {error}"))?;
    expected_bytes.push(b'\n');
    let mut replacement_bytes = serde_json::to_vec_pretty(replacement)
        .map_err(|error| format!("serialize authority trust-anchor replacement: {error}"))?;
    replacement_bytes.push(b'\n');
    let relative = PathBuf::from(".vela")
        .join("trust")
        .join("authorities")
        .join(format!("{}.json", replacement.frontier_id));
    PreparedRepositoryFileReplacement::prepare_exact(
        user_home,
        &relative,
        Some(&expected_bytes),
        &replacement_bytes,
        RepositoryFileReplacementMode::PreserveExisting,
        4 * 1024,
    )?
    .install()?;
    let rebound = load_authority_trust_anchor_from_home(user_home, &replacement.frontier_id)?
        .ok_or_else(|| "rebound authority trust anchor could not be read back".to_string())?;
    if rebound.anchor != *replacement {
        return Err(
            "rebound authority trust anchor differs from the exact replacement".to_string(),
        );
    }
    Ok(rebound)
}

fn load_private_trust_document<T: DeserializeOwned>(
    user_home: &Path,
    namespace: &str,
    path: &Path,
    label: &str,
) -> Result<Option<T>, String> {
    let vela = user_home.join(".vela");
    let trust = vela.join("trust");
    let namespace_dir = trust.join(namespace);
    match safe_metadata(&vela)? {
        None => return Ok(None),
        Some(metadata) if metadata.file_type().is_dir() => {
            require_trusted_parent_directory(&vela, &metadata)?;
        }
        Some(_) => {
            return Err(format!(
                "trust path '{}' must be a real directory, not a symlink or other file",
                vela.display()
            ));
        }
    }
    for directory in [&trust, &namespace_dir] {
        match safe_metadata(directory)? {
            None => return Ok(None),
            Some(metadata) if metadata.file_type().is_dir() => {
                require_private_directory(directory, &metadata)?;
            }
            Some(_) => {
                return Err(format!(
                    "trust path '{}' must be a real directory, not a symlink or other file",
                    directory.display()
                ));
            }
        }
    }
    let Some(metadata) = safe_metadata(path)? else {
        return Ok(None);
    };
    if !metadata.file_type().is_file() {
        return Err(format!(
            "{label} '{}' must be a regular file and may not be a symlink",
            path.display()
        ));
    }
    require_private_file(path, &metadata)?;
    let inspected = same_file::Handle::from_path(path)
        .map_err(|error| format!("identify {label} '{}': {error}", path.display()))?;
    let mut file = fs::File::open(path)
        .map_err(|error| format!("open {label} '{}': {error}", path.display()))?;
    let opened = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|error| format!("clone {label} descriptor: {error}"))?,
    )
    .map_err(|error| format!("identify open {label}: {error}"))?;
    if inspected != opened {
        return Err(format!("{label} changed while it was opened"));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read {label} '{}': {error}", path.display()))?;
    if bytes.len() > 64 * 1024 {
        return Err(format!("{label} exceeds the 64 KiB limit"));
    }
    let final_metadata =
        safe_metadata(path)?.ok_or_else(|| format!("{label} disappeared while it was read"))?;
    let final_identity = same_file::Handle::from_path(path)
        .map_err(|error| format!("reidentify {label}: {error}"))?;
    require_private_file(path, &final_metadata)?;
    if opened != final_identity {
        return Err(format!("{label} changed while it was read"));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("parse {label} '{}': {error}", path.display()))
}

fn install_private_trust_document<T: Serialize + DeserializeOwned + PartialEq>(
    user_home: &Path,
    namespace: &str,
    path: &Path,
    document: &T,
    label: &str,
    temporary_prefix: &str,
) -> Result<(), String> {
    let vela = user_home.join(".vela");
    let trust = vela.join("trust");
    let namespace_dir = trust.join(namespace);
    ensure_trusted_parent_directory(&vela)?;
    ensure_private_directory(&trust)?;
    ensure_private_directory(&namespace_dir)?;

    if path.exists() {
        let existing = load_private_trust_document::<T>(user_home, namespace, path, label)?
            .ok_or_else(|| format!("{label} disappeared during install"))?;
        if existing == *document {
            return Ok(());
        }
        return Err(format!(
            "refusing to replace existing {label} '{}'",
            path.display()
        ));
    }

    let mut bytes = serde_json::to_vec_pretty(document)
        .map_err(|error| format!("serialize {label}: {error}"))?;
    bytes.push(b'\n');
    let mut temporary = tempfile::Builder::new()
        .prefix(temporary_prefix)
        .tempfile_in(&namespace_dir)
        .map_err(|error| format!("create {label} temporary file: {error}"))?;
    set_private_file_permissions(temporary.path())?;
    temporary
        .write_all(&bytes)
        .map_err(|error| format!("write {label} temporary file: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("sync {label} temporary file: {error}"))?;
    temporary.persist_noclobber(path).map_err(|error| {
        format!(
            "atomically install {label} '{}': {}",
            path.display(),
            error.error
        )
    })?;
    set_private_file_permissions(path)?;
    if let Ok(directory) = fs::File::open(&namespace_dir) {
        directory
            .sync_all()
            .map_err(|error| format!("sync {label} directory: {error}"))?;
    }
    Ok(())
}

fn validate_frontier_id(value: &str) -> Result<(), String> {
    let Some(suffix) = value.strip_prefix("vfr_") else {
        return Err("trust anchor frontier_id must be vfr_<16 lowercase hex>".to_string());
    };
    if suffix.len() != 16
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("trust anchor frontier_id must be vfr_<16 lowercase hex>".to_string());
    }
    Ok(())
}

fn validate_sha256_root(field: &str, value: &str) -> Result<(), String> {
    let Some(digest) = value.strip_prefix("sha256:") else {
        return Err(format!(
            "{field} must be a full sha256:<64 lowercase hex> root"
        ));
    };
    validate_lower_hex(field, digest, 64)
}

fn validate_lower_hex(field: &str, value: &str, length: usize) -> Result<(), String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{field} must be exactly {length} lowercase hex characters"
        ));
    }
    Ok(())
}

fn safe_metadata(path: &Path) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "trust path '{}' may not be a symlink",
                    path.display()
                ));
            }
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("inspect trust path '{}': {error}", path.display())),
    }
}

fn ensure_private_directory(path: &Path) -> Result<(), String> {
    match safe_metadata(path)? {
        Some(metadata) if metadata.file_type().is_dir() => {
            require_private_directory(path, &metadata)?;
        }
        Some(_) => {
            return Err(format!(
                "trust path '{}' must be a real directory",
                path.display()
            ));
        }
        None => {
            let parent = path
                .parent()
                .ok_or_else(|| format!("trust directory '{}' has no parent", path.display()))?;
            if !parent.is_dir() {
                return Err(format!(
                    "trust directory parent '{}' does not exist",
                    parent.display()
                ));
            }
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder
                .create(path)
                .map_err(|error| format!("create trust directory '{}': {error}", path.display()))?;
        }
    }
    set_private_directory_permissions(path)
}

fn ensure_trusted_parent_directory(path: &Path) -> Result<(), String> {
    match safe_metadata(path)? {
        Some(metadata) if metadata.file_type().is_dir() => {
            require_trusted_parent_directory(path, &metadata)
        }
        Some(_) => Err(format!(
            "trust path '{}' must be a real directory, not a symlink or other file",
            path.display()
        )),
        None => {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(path).map_err(|error| {
                format!(
                    "create trust parent directory '{}': {error}",
                    path.display()
                )
            })?;
            let metadata = safe_metadata(path)?
                .ok_or_else(|| format!("trust parent '{}' disappeared", path.display()))?;
            require_trusted_parent_directory(path, &metadata)
        }
    }
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
        format!(
            "set trust directory permissions '{}': {error}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("set trust anchor permissions '{}': {error}", path.display()))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn require_private_directory(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    require_owned_mode(path, metadata, 0o700, "directory")
}

#[cfg(unix)]
fn require_trusted_parent_directory(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let owner = rustix::process::geteuid().as_raw();
    if metadata.uid() != owner {
        return Err(format!(
            "trust parent directory '{}' is not owned by the current operating-system account",
            path.display()
        ));
    }
    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o022 != 0 {
        return Err(format!(
            "trust parent directory '{}' may not be group- or world-writable; observed mode {mode:04o}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn require_private_file(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    require_owned_mode(path, metadata, 0o600, "file")
}

#[cfg(unix)]
fn require_owned_mode(
    path: &Path,
    metadata: &fs::Metadata,
    expected_mode: u32,
    kind: &str,
) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let owner = rustix::process::geteuid().as_raw();
    if metadata.uid() != owner {
        return Err(format!(
            "trust {kind} '{}' is not owned by the current operating-system account",
            path.display()
        ));
    }
    let mode = metadata.permissions().mode() & 0o777;
    if mode != expected_mode {
        return Err(format!(
            "trust {kind} '{}' must have exact mode {expected_mode:04o}, observed {mode:04o}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_private_directory(_path: &Path, _metadata: &fs::Metadata) -> Result<(), String> {
    Ok(())
}

#[cfg(not(unix))]
fn require_trusted_parent_directory(_path: &Path, _metadata: &fs::Metadata) -> Result<(), String> {
    Ok(())
}

#[cfg(not(unix))]
fn require_private_file(_path: &Path, _metadata: &fs::Metadata) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn authority_trust_anchor() -> AuthorityTrustAnchorV1 {
        AuthorityTrustAnchorV1 {
            schema: AUTHORITY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
            frontier_id: "vfr_0123456789abcdef".to_string(),
            first_authority_record_root: format!("sha256:{}", "4".repeat(64)),
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    fn no_repository_replacement_temporaries(directory: &Path) -> bool {
        std::fs::read_dir(directory)
            .unwrap()
            .flatten()
            .all(|entry| {
                !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".vela-replace-")
            })
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    #[test]
    fn repository_file_replacement_enforces_exact_mode_after_umask_and_on_equal_bytes() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        std::fs::write(&path, b"before").unwrap();

        let first = PreparedRepositoryFileReplacement::prepare_exact(
            directory.path(),
            Path::new("settings.toml"),
            Some(b"before"),
            b"after",
            RepositoryFileReplacementMode::Exact(0o666),
            1024,
        )
        .unwrap();
        assert!(first.install().unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"after");
        assert_eq!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o666
        );

        // Equal bytes are not a no-op when Exact mode still needs repair.
        let repair = PreparedRepositoryFileReplacement::prepare_exact(
            directory.path(),
            Path::new("settings.toml"),
            Some(b"after"),
            b"after",
            RepositoryFileReplacementMode::Exact(0o644),
            1024,
        )
        .unwrap();
        assert!(repair.install().unwrap());
        assert_eq!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );

        let unchanged = PreparedRepositoryFileReplacement::prepare_exact(
            directory.path(),
            Path::new("settings.toml"),
            Some(b"after"),
            b"after",
            RepositoryFileReplacementMode::Exact(0o644),
            1024,
        )
        .unwrap();
        assert!(!unchanged.install().unwrap());
        assert!(no_repository_replacement_temporaries(directory.path()));
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    #[test]
    fn repository_file_replacement_cleans_a_temporary_after_precommit_failure() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        std::fs::write(&path, b"before").unwrap();
        let prepared = PreparedRepositoryFileReplacement::prepare_exact(
            directory.path(),
            Path::new("settings.toml"),
            Some(b"before"),
            b"after",
            RepositoryFileReplacementMode::Exact(0o644),
            1024,
        )
        .unwrap();

        let error = prepared
            .install_with_hooks(
                || Err("injected failure after temporary creation".to_string()),
                || Ok(()),
            )
            .unwrap_err();
        assert!(error.contains("injected failure"), "{error}");
        assert_eq!(std::fs::read(&path).unwrap(), b"before");
        assert!(no_repository_replacement_temporaries(directory.path()));
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    #[test]
    fn repository_file_replacement_serializes_competing_prepared_writers() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, mpsc};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        std::fs::write(&path, b"before").unwrap();
        let first = PreparedRepositoryFileReplacement::prepare_exact(
            directory.path(),
            Path::new("settings.toml"),
            Some(b"before"),
            b"first",
            RepositoryFileReplacementMode::Exact(0o644),
            1024,
        )
        .unwrap();
        let second = PreparedRepositoryFileReplacement::prepare_exact(
            directory.path(),
            Path::new("settings.toml"),
            Some(b"before"),
            b"second",
            RepositoryFileReplacementMode::Exact(0o644),
            1024,
        )
        .unwrap();

        let (first_entered_tx, first_entered_rx) = mpsc::channel();
        let (release_first_tx, release_first_rx) = mpsc::channel();
        let first_thread = std::thread::spawn(move || {
            first.install_with_hook(|| {
                first_entered_tx.send(()).unwrap();
                release_first_rx.recv().unwrap();
                Ok(())
            })
        });
        first_entered_rx.recv().unwrap();

        let second_hook_ran = Arc::new(AtomicBool::new(false));
        let observed_second_hook = Arc::clone(&second_hook_ran);
        let second_error = second
            .install_with_hook(|| {
                observed_second_hook.store(true, Ordering::SeqCst);
                Ok(())
            })
            .unwrap_err();
        assert!(
            second_error.contains("repository replacement busy"),
            "{second_error}"
        );
        assert!(!second_hook_ran.load(Ordering::SeqCst));

        release_first_tx.send(()).unwrap();
        assert!(first_thread.join().unwrap().unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        assert!(no_repository_replacement_temporaries(directory.path()));
    }

    #[test]
    fn authority_trust_anchor_round_trip_is_minimal_rooted_and_non_replacing() {
        let home = tempfile::tempdir().unwrap();
        let anchor = authority_trust_anchor();
        let installed = install_authority_trust_anchor_from_home(home.path(), &anchor).unwrap();
        assert_eq!(installed.anchor, anchor);
        assert_eq!(installed.root, anchor.root().unwrap());
        assert_eq!(
            installed.path,
            home.path()
                .join(".vela/trust/authorities/vfr_0123456789abcdef.json")
        );
        let loaded = load_authority_trust_anchor_from_home(home.path(), "vfr_0123456789abcdef")
            .unwrap()
            .unwrap();
        assert_eq!(loaded, installed);

        let mut replacement = anchor;
        replacement.first_authority_record_root = format!("sha256:{}", "5".repeat(64));
        let error =
            install_authority_trust_anchor_from_home(home.path(), &replacement).unwrap_err();
        assert!(error.contains("refusing to replace existing authority trust anchor"));
    }

    #[test]
    fn authority_trust_anchor_rebind_requires_the_exact_installed_preimage() {
        let home = tempfile::tempdir().unwrap();
        let original = authority_trust_anchor();
        install_authority_trust_anchor_from_home(home.path(), &original).unwrap();

        let mut replacement = original.clone();
        replacement.first_authority_record_root = format!("sha256:{}", "5".repeat(64));
        let rebound =
            rebind_authority_trust_anchor_from_home(home.path(), &original, &replacement).unwrap();
        assert_eq!(rebound.anchor, replacement);

        let mut wrong_preimage = original;
        wrong_preimage.first_authority_record_root = format!("sha256:{}", "6".repeat(64));
        let error = rebind_authority_trust_anchor_from_home(
            home.path(),
            &wrong_preimage,
            &authority_trust_anchor(),
        )
        .unwrap_err();
        assert!(error.contains("preimage does not match"));
    }

    #[test]
    fn authority_trust_anchor_rejects_unknown_fields_and_invalid_roots() {
        let mut value = serde_json::to_value(authority_trust_anchor()).unwrap();
        value["extra"] = json!(true);
        assert!(serde_json::from_value::<AuthorityTrustAnchorV1>(value).is_err());

        let mut invalid = authority_trust_anchor();
        invalid.first_authority_record_root = "sha256:short".to_string();
        assert!(invalid.validate().is_err());
    }
}
