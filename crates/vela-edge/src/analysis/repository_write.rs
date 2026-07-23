//! Fail-closed repository verification at the canonical write boundary.
//!
//! This module does not perform a write. It combines the protocol-owned
//! Profile v1 projection and reducer replay with the edge-owned Git boundary
//! verifier. Administrator-bound repositories additionally require an
//! independently retained, user-local trust anchor. Repository bytes cannot
//! supply that anchor; the caller supplies the trusted home/config root and is
//! responsible for binding it across the write attempt.

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
use std::path::Component;
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
use rustix::fd::OwnedFd;

use serde::{Deserialize, Serialize};
use vela_protocol::events::EVENT_KIND_FRONTIER_REPOSITORY_BOUND;
use vela_protocol::frontier_profile::{EffectiveFrontierAuthorityV1, FrontierProfileProjectionV1};
use vela_protocol::frontier_repo::{
    FrontierProfileFile, read_repository_control_text, read_repository_profile,
};
use vela_protocol::frontier_repository::{
    FrontierRepositoryBoundaryMode, repository_boundary_event_content_root,
    repository_boundary_payload_from_event_shape, repository_identity_event_content_root,
};
use vela_protocol::frontier_settings::FrontierSettingsV1;
use vela_protocol::project::Project;
use vela_protocol::{canonical, reducer};

use super::frontier_repository::{
    RepositoryBoundaryContext, RepositoryTrustAnchor, verify_repository_artifact_projection,
    verify_repository_boundary_context_with_trust_anchor, verify_repository_finding_projection,
    verify_repository_unreplayed_sidecars,
};

pub const REPOSITORY_TRUST_ANCHOR_SCHEMA_V1: &str = "vela.repository-trust-anchor.v1";

/// An out-of-band pin for the first administrator boundary in a Frontier.
///
/// This file is deliberately outside the repository. It grants no actor or
/// scientific authority. For native and migrated repositories alike, it
/// prevents repository-controlled bytes from substituting the initially
/// reviewed administrator boundary or key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepositoryTrustAnchorV1 {
    pub schema: String,
    pub frontier_id: String,
    pub identity_root: String,
    pub boundary_content_root: String,
    pub administrator_actor_id: String,
    pub administrator_public_key: String,
}

impl RepositoryTrustAnchorV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != REPOSITORY_TRUST_ANCHOR_SCHEMA_V1 {
            return Err(format!(
                "trust anchor schema must be {REPOSITORY_TRUST_ANCHOR_SCHEMA_V1}"
            ));
        }
        validate_frontier_id(&self.frontier_id)?;
        validate_sha256_root("trust anchor identity_root", &self.identity_root)?;
        validate_sha256_root(
            "trust anchor boundary_content_root",
            &self.boundary_content_root,
        )?;
        validate_actor_id(
            "trust anchor administrator_actor_id",
            &self.administrator_actor_id,
        )?;
        validate_lower_hex(
            "trust anchor administrator_public_key",
            &self.administrator_public_key,
            64,
        )
    }

    /// Canonical content root independent of JSON formatting and key order.
    pub fn root(&self) -> Result<String, String> {
        self.validate()?;
        canonical::sha256_canonical(self).map(|digest| format!("sha256:{digest}"))
    }

    fn boundary_anchor(&self) -> RepositoryTrustAnchor {
        RepositoryTrustAnchor {
            boundary_content_root: self.boundary_content_root.clone(),
            administrator_public_key: self.administrator_public_key.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedRepositoryTrustAnchorV1 {
    pub path: PathBuf,
    pub root: String,
    pub anchor: RepositoryTrustAnchorV1,
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
/// Linux/Android and Apple platforms expose the required no-replace/exchange
/// rename primitives. Other platforms fail closed at preparation rather than
/// falling back to a path-based rename with a known TOCTOU gap. Native Windows
/// is intentionally included in that fail-closed set until a runtime-tested
/// implementation can retain the displaced exact preimage through a
/// handle-relative atomic exchange; neither path-based `ReplaceFileW` nor
/// replace-only `FileRenameInfoEx` supplies that complete contract.
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
        use rand::RngCore;
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
        let mut random = rand::rngs::OsRng;
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

#[cfg(windows)]
pub const REPOSITORY_FILE_MUTATION_UNAVAILABLE: &str = "native Windows repository-file mutation is blocked in this build: Vela has not validated a handle-relative atomic exchange and rollback primitive that preserves exact-preimage and reparse-point guarantees; run this mutation from WSL2 with the checkout on its Linux filesystem, or from a supported Unix host (read, check, and reproduce remain available)";

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    windows
)))]
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

/// Deterministic trust-anchor path below an explicitly supplied user home.
///
/// This function does not consult environment or repository overrides.
/// Production callers resolve and bind the trusted home once; tests inject a
/// temporary home directory.
pub fn repository_trust_anchor_path(
    user_home: &Path,
    frontier_id: &str,
) -> Result<PathBuf, String> {
    validate_frontier_id(frontier_id)?;
    Ok(user_home
        .join(".vela")
        .join("trust")
        .join("frontiers")
        .join(format!("{frontier_id}.json")))
}

/// Load a user-local trust anchor without following any symlink below
/// `user_home`.
pub fn load_repository_trust_anchor_from_home(
    user_home: &Path,
    frontier_id: &str,
) -> Result<Option<LoadedRepositoryTrustAnchorV1>, String> {
    let path = repository_trust_anchor_path(user_home, frontier_id)?;
    let vela = user_home.join(".vela");
    let trust = vela.join("trust");
    let frontiers = trust.join("frontiers");
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
    for directory in [&trust, &frontiers] {
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
    let Some(metadata) = safe_metadata(&path)? else {
        return Ok(None);
    };
    if !metadata.file_type().is_file() {
        return Err(format!(
            "trust anchor '{}' must be a regular file and may not be a symlink",
            path.display()
        ));
    }
    require_private_file(&path, &metadata)?;
    let inspected = same_file::Handle::from_path(&path)
        .map_err(|error| format!("identify trust anchor '{}': {error}", path.display()))?;
    let mut file = fs::File::open(&path)
        .map_err(|error| format!("open trust anchor '{}': {error}", path.display()))?;
    let opened = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|error| format!("clone trust-anchor descriptor: {error}"))?,
    )
    .map_err(|error| format!("identify open trust anchor: {error}"))?;
    if inspected != opened {
        return Err("trust anchor changed while it was opened".to_string());
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read trust anchor '{}': {error}", path.display()))?;
    if bytes.len() > 64 * 1024 {
        return Err("trust anchor exceeds the 64 KiB limit".to_string());
    }
    let final_metadata = safe_metadata(&path)?
        .ok_or_else(|| "trust anchor disappeared while it was read".to_string())?;
    let final_identity = same_file::Handle::from_path(&path)
        .map_err(|error| format!("reidentify trust anchor: {error}"))?;
    require_private_file(&path, &final_metadata)?;
    if opened != final_identity {
        return Err("trust anchor changed while it was read".to_string());
    }
    let anchor: RepositoryTrustAnchorV1 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse trust anchor '{}': {error}", path.display()))?;
    anchor.validate()?;
    if anchor.frontier_id != frontier_id {
        return Err(format!(
            "trust anchor frontier {} does not match requested {frontier_id}",
            anchor.frontier_id
        ));
    }
    Ok(Some(LoadedRepositoryTrustAnchorV1 {
        root: anchor.root()?,
        path,
        anchor,
    }))
}

/// Atomically install an explicitly reviewed trust anchor.
///
/// This helper never derives or automatically pins a value. A matching
/// existing file is idempotent; a different file is never overwritten.
pub fn install_repository_trust_anchor_from_home(
    user_home: &Path,
    anchor: &RepositoryTrustAnchorV1,
) -> Result<LoadedRepositoryTrustAnchorV1, String> {
    anchor.validate()?;
    let path = repository_trust_anchor_path(user_home, &anchor.frontier_id)?;
    let vela = user_home.join(".vela");
    let trust = vela.join("trust");
    let frontiers = trust.join("frontiers");
    ensure_trusted_parent_directory(&vela)?;
    ensure_private_directory(&trust)?;
    ensure_private_directory(&frontiers)?;

    if path.exists() {
        let existing = load_repository_trust_anchor_from_home(user_home, &anchor.frontier_id)?
            .ok_or_else(|| "trust anchor disappeared during install".to_string())?;
        if existing.anchor == *anchor {
            return Ok(existing);
        }
        return Err(format!(
            "refusing to replace existing trust anchor '{}'",
            path.display()
        ));
    }

    let mut bytes = serde_json::to_vec_pretty(anchor)
        .map_err(|error| format!("serialize trust anchor: {error}"))?;
    bytes.push(b'\n');
    let mut temporary = tempfile::Builder::new()
        .prefix(".repository-trust-anchor-")
        .tempfile_in(&frontiers)
        .map_err(|error| format!("create trust-anchor temporary file: {error}"))?;
    set_private_file_permissions(temporary.path())?;
    temporary
        .write_all(&bytes)
        .map_err(|error| format!("write trust-anchor temporary file: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("sync trust-anchor temporary file: {error}"))?;
    temporary.persist_noclobber(&path).map_err(|error| {
        format!(
            "atomically install trust anchor '{}': {}",
            path.display(),
            error.error
        )
    })?;
    set_private_file_permissions(&path)?;
    if let Ok(directory) = fs::File::open(&frontiers) {
        directory
            .sync_all()
            .map_err(|error| format!("sync trust-anchor directory: {error}"))?;
    }
    load_repository_trust_anchor_from_home(user_home, &anchor.frontier_id)?
        .ok_or_else(|| "installed trust anchor could not be read back".to_string())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryWriteGateCode {
    FrontierProfileUpgradeRequired,
    FrontierProfileInvalid,
    FrontierSettingsMissing,
    FrontierSettingsInvalid,
    FrontierProfileMismatch,
    RepositoryIdentityInvalid,
    ReducerReplayFailed,
    ProposalParityFailed,
    RepositoryTrustAnchorRequired,
    RepositoryTrustAnchorInvalid,
    RepositoryBoundaryInvalid,
}

impl RepositoryWriteGateCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FrontierProfileUpgradeRequired => "frontier_profile_upgrade_required",
            Self::FrontierProfileInvalid => "frontier_profile_invalid",
            Self::FrontierSettingsMissing => "frontier_settings_missing",
            Self::FrontierSettingsInvalid => "frontier_settings_invalid",
            Self::FrontierProfileMismatch => "frontier_profile_mismatch",
            Self::RepositoryIdentityInvalid => "repository_identity_invalid",
            Self::ReducerReplayFailed => "reducer_replay_failed",
            Self::ProposalParityFailed => "proposal_parity_failed",
            Self::RepositoryTrustAnchorRequired => "repository_trust_anchor_required",
            Self::RepositoryTrustAnchorInvalid => "repository_trust_anchor_invalid",
            Self::RepositoryBoundaryInvalid => "repository_boundary_invalid",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryWriteGateError {
    pub code: RepositoryWriteGateCode,
    pub message: String,
}

impl RepositoryWriteGateError {
    fn new(code: RepositoryWriteGateCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for RepositoryWriteGateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for RepositoryWriteGateError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifiedRepositoryIdentity {
    Genesis {
        identity_event_root: String,
    },
    PinnedBoundary {
        origin: VerifiedBoundaryOrigin,
        boundary: RepositoryBoundaryContext,
        trust_anchor_root: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedBoundaryOrigin {
    Genesis,
    LegacyBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRepositoryWriteContext {
    pub frontier_id: String,
    pub profile: FrontierProfileProjectionV1,
    pub settings: FrontierSettingsV1,
    pub replayed_snapshot_hash: String,
    pub materialized_snapshot_hash: String,
    pub identity: VerifiedRepositoryIdentity,
}

/// Verify the minimum repository state required before a canonical write.
///
/// This intentionally does not inspect generated locks, proof packets,
/// scientific strict signals, or other derived views. It proves only the
/// repository generation, typed settings, event-derived profile authority,
/// reducer replay, proposal/decision parity, and (for every
/// administrator-bound repository) exact Git-bound identity under an
/// independently supplied trust anchor.
pub fn verify_repository_for_write(
    repo_path: &Path,
    project: &Project,
    trust_anchor: Option<&RepositoryTrustAnchorV1>,
) -> Result<VerifiedRepositoryWriteContext, RepositoryWriteGateError> {
    let profile = match read_repository_profile(repo_path) {
        Ok(Some(FrontierProfileFile::V1(profile))) => profile,
        Ok(Some(FrontierProfileFile::LegacyV0_1(_))) | Ok(None) => {
            return Err(RepositoryWriteGateError::new(
                RepositoryWriteGateCode::FrontierProfileUpgradeRequired,
                "canonical writes require a verified migration to vela.frontier-profile.v1",
            ));
        }
        Err(error) => {
            return Err(RepositoryWriteGateError::new(
                RepositoryWriteGateCode::FrontierProfileInvalid,
                error,
            ));
        }
    };
    profile.validate().map_err(|error| {
        RepositoryWriteGateError::new(RepositoryWriteGateCode::FrontierProfileInvalid, error)
    })?;

    let settings_path = repo_path.join(".vela/settings.toml");
    let settings_source = read_repository_control_text(
        repo_path,
        Path::new(".vela/settings.toml"),
        ".vela/settings.toml",
    )
    .map_err(|error| {
        RepositoryWriteGateError::new(RepositoryWriteGateCode::FrontierSettingsInvalid, error)
    })?
    .ok_or_else(|| {
        RepositoryWriteGateError::new(
            RepositoryWriteGateCode::FrontierSettingsMissing,
            format!("required '{}' is missing", settings_path.display()),
        )
    })?;
    let settings = FrontierSettingsV1::from_toml(&settings_source).map_err(|error| {
        RepositoryWriteGateError::new(RepositoryWriteGateCode::FrontierSettingsInvalid, error)
    })?;

    let authority =
        EffectiveFrontierAuthorityV1::from_events(&project.events).map_err(|error| {
            RepositoryWriteGateError::new(RepositoryWriteGateCode::RepositoryIdentityInvalid, error)
        })?;
    profile
        .assert_frontier_id(&authority.frontier_id)
        .map_err(|error| {
            RepositoryWriteGateError::new(RepositoryWriteGateCode::FrontierProfileMismatch, error)
        })?;
    let projection = profile.project(project).map_err(|error| {
        RepositoryWriteGateError::new(RepositoryWriteGateCode::RepositoryIdentityInvalid, error)
    })?;

    let replay = reducer::verify_replay(project);
    if !replay.ok {
        return Err(RepositoryWriteGateError::new(
            RepositoryWriteGateCode::ReducerReplayFailed,
            if replay.diffs.is_empty() {
                replay.note
            } else {
                replay.diffs.join(" | ")
            },
        ));
    }

    let mut proposal_conflicts = vela_protocol::proposals::verify_proposal_decision_parity(project);
    proposal_conflicts.extend(vela_protocol::proposals::verify_proposal_withdrawals(
        repo_path, project,
    ));
    if !proposal_conflicts.is_empty() {
        return Err(RepositoryWriteGateError::new(
            RepositoryWriteGateCode::ProposalParityFailed,
            proposal_conflicts.join(" | "),
        ));
    }

    let boundaries = project
        .events
        .iter()
        .filter(|event| event.kind.as_str() == EVENT_KIND_FRONTIER_REPOSITORY_BOUND)
        .collect::<Vec<_>>();
    let identity = if boundaries.is_empty() {
        verify_native_finding_projection(project)?;
        verify_native_unreplayed_sidecars(project)?;
        verify_genesis_artifact_projection(project)?;
        VerifiedRepositoryIdentity::Genesis {
            identity_event_root: projection.identity_event_root.clone(),
        }
    } else {
        let mut current = boundaries.iter().filter(|event| {
            repository_identity_event_content_root(event)
                .is_ok_and(|root| root == projection.identity_event_root)
        });
        let boundary = current.next().copied().ok_or_else(|| {
            RepositoryWriteGateError::new(
                RepositoryWriteGateCode::RepositoryIdentityInvalid,
                "effective repository identity head is absent from the boundary event set",
            )
        })?;
        if current.next().is_some() {
            return Err(RepositoryWriteGateError::new(
                RepositoryWriteGateCode::RepositoryIdentityInvalid,
                "effective repository identity head is ambiguous",
            ));
        }
        let chain_origin = identity_chain_origin(project, &projection.identity_event_root)?;
        let anchor = trust_anchor.ok_or_else(|| {
            RepositoryWriteGateError::new(
                RepositoryWriteGateCode::RepositoryTrustAnchorRequired,
                "a repository administrator boundary requires the user-local trust anchor",
            )
        })?;
        if anchor.boundary_content_root != chain_origin.first_boundary_root {
            return Err(RepositoryWriteGateError::new(
                RepositoryWriteGateCode::RepositoryTrustAnchorInvalid,
                format!(
                    "trust anchor must pin first administrator boundary {}, not {}",
                    chain_origin.first_boundary_root, anchor.boundary_content_root
                ),
            ));
        }
        verify_trust_anchor_against_events(anchor, &projection, project)?;
        let boundary_context = verify_repository_boundary_context_with_trust_anchor(
            project,
            repo_path,
            boundary,
            Some(&anchor.boundary_anchor()),
        )
        .map_err(|error| {
            RepositoryWriteGateError::new(RepositoryWriteGateCode::RepositoryBoundaryInvalid, error)
        })?;
        verify_repository_artifact_projection(project, repo_path, boundary).map_err(|error| {
            RepositoryWriteGateError::new(RepositoryWriteGateCode::ReducerReplayFailed, error)
        })?;
        if chain_origin.origin == VerifiedBoundaryOrigin::LegacyBoundary {
            let initial_boundary = project
                .events
                .iter()
                .find(|event| {
                    repository_boundary_event_content_root(event)
                        .is_ok_and(|root| root == anchor.boundary_content_root)
                })
                .ok_or_else(|| {
                    RepositoryWriteGateError::new(
                        RepositoryWriteGateCode::RepositoryTrustAnchorInvalid,
                        "trusted initial boundary is absent after anchor validation",
                    )
                })?;
            verify_repository_finding_projection(project, repo_path, initial_boundary).map_err(
                |error| {
                    RepositoryWriteGateError::new(
                        RepositoryWriteGateCode::ReducerReplayFailed,
                        error,
                    )
                },
            )?;
            verify_repository_unreplayed_sidecars(project, repo_path, initial_boundary).map_err(
                |error| {
                    RepositoryWriteGateError::new(
                        RepositoryWriteGateCode::ReducerReplayFailed,
                        error,
                    )
                },
            )?;
        } else {
            verify_native_finding_projection(project)?;
            verify_native_unreplayed_sidecars(project)?;
        }
        VerifiedRepositoryIdentity::PinnedBoundary {
            origin: chain_origin.origin,
            boundary: boundary_context,
            trust_anchor_root: anchor.root().map_err(|error| {
                RepositoryWriteGateError::new(
                    RepositoryWriteGateCode::RepositoryTrustAnchorInvalid,
                    error,
                )
            })?,
        }
    };

    Ok(VerifiedRepositoryWriteContext {
        frontier_id: projection.frontier_id.clone(),
        profile: projection,
        settings,
        replayed_snapshot_hash: replay.replayed_snapshot_hash,
        materialized_snapshot_hash: replay.materialized_snapshot_hash,
        identity,
    })
}

struct BoundaryChainOrigin {
    origin: VerifiedBoundaryOrigin,
    first_boundary_root: String,
}

fn identity_chain_origin(
    project: &Project,
    head_root: &str,
) -> Result<BoundaryChainOrigin, RepositoryWriteGateError> {
    let mut cursor = head_root.to_string();
    let mut visited = std::collections::BTreeSet::new();
    loop {
        if !visited.insert(cursor.clone()) {
            return Err(RepositoryWriteGateError::new(
                RepositoryWriteGateCode::RepositoryIdentityInvalid,
                "repository identity chain contains a cycle",
            ));
        }
        let matching = project
            .events
            .iter()
            .filter(|event| {
                repository_identity_event_content_root(event).is_ok_and(|root| root == cursor)
            })
            .collect::<Vec<_>>();
        let [event] = matching.as_slice() else {
            return Err(RepositoryWriteGateError::new(
                RepositoryWriteGateCode::RepositoryIdentityInvalid,
                format!(
                    "repository identity root {cursor} must identify exactly one event, found {}",
                    matching.len()
                ),
            ));
        };
        let payload = repository_boundary_payload_from_event_shape(event).map_err(|error| {
            RepositoryWriteGateError::new(RepositoryWriteGateCode::RepositoryIdentityInvalid, error)
        })?;
        if payload.mode == FrontierRepositoryBoundaryMode::TemporalizeExisting {
            return Ok(BoundaryChainOrigin {
                origin: VerifiedBoundaryOrigin::LegacyBoundary,
                first_boundary_root: cursor,
            });
        }
        let parent = payload.previous_identity_event_root.ok_or_else(|| {
            RepositoryWriteGateError::new(
                RepositoryWriteGateCode::RepositoryIdentityInvalid,
                "dependency-update boundary is missing its identity parent",
            )
        })?;
        let parent_event = project.events.iter().find(|event| {
            repository_identity_event_content_root(event).is_ok_and(|root| root == parent)
        });
        if parent_event.is_some_and(|event| event.kind.as_str() == "frontier.created") {
            return Ok(BoundaryChainOrigin {
                origin: VerifiedBoundaryOrigin::Genesis,
                first_boundary_root: cursor,
            });
        }
        cursor = parent;
    }
}

fn verify_genesis_artifact_projection(project: &Project) -> Result<(), RepositoryWriteGateError> {
    let replayed = reducer::replayed_projection(project).map_err(|error| {
        RepositoryWriteGateError::new(RepositoryWriteGateCode::ReducerReplayFailed, error)
    })?;
    let expected = canonical::sha256_canonical(&replayed.artifacts).map_err(|error| {
        RepositoryWriteGateError::new(
            RepositoryWriteGateCode::ReducerReplayFailed,
            error.to_string(),
        )
    })?;
    let observed = canonical::sha256_canonical(&project.artifacts).map_err(|error| {
        RepositoryWriteGateError::new(
            RepositoryWriteGateCode::ReducerReplayFailed,
            error.to_string(),
        )
    })?;
    if observed != expected {
        return Err(RepositoryWriteGateError::new(
            RepositoryWriteGateCode::ReducerReplayFailed,
            format!(
                "artifact registry is not reducer-reproducible: expected sha256:{expected}, observed sha256:{observed}"
            ),
        ));
    }
    Ok(())
}

fn verify_native_unreplayed_sidecars(project: &Project) -> Result<(), RepositoryWriteGateError> {
    for (name, count) in [
        ("review_events", project.review_events.len()),
        ("confidence_updates", project.confidence_updates.len()),
    ] {
        if count != 0 {
            return Err(RepositoryWriteGateError::new(
                RepositoryWriteGateCode::ReducerReplayFailed,
                format!(
                    "native Profile v1 {name} has {count} sidecar record(s) without reducer reconstruction"
                ),
            ));
        }
    }
    Ok(())
}

fn verify_native_finding_projection(project: &Project) -> Result<(), RepositoryWriteGateError> {
    let remnants = reducer::classify_provenance(project).remnant;
    if remnants.is_empty() {
        return Ok(());
    }
    let preview = remnants
        .iter()
        .take(8)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    let omitted = remnants.len().saturating_sub(8);
    let suffix = if omitted == 0 {
        String::new()
    } else {
        format!(" (+{omitted} more)")
    };
    Err(RepositoryWriteGateError::new(
        RepositoryWriteGateCode::ReducerReplayFailed,
        format!(
            "native Profile v1 contains {} non-evented finding remnant(s): {preview}{suffix}; every finding must be reconstructed by a finding.asserted or finding.superseded event",
            remnants.len()
        ),
    ))
}

fn verify_trust_anchor_against_events(
    anchor: &RepositoryTrustAnchorV1,
    projection: &FrontierProfileProjectionV1,
    project: &Project,
) -> Result<(), RepositoryWriteGateError> {
    anchor.validate().map_err(|error| {
        RepositoryWriteGateError::new(RepositoryWriteGateCode::RepositoryTrustAnchorInvalid, error)
    })?;
    if anchor.frontier_id != projection.frontier_id {
        return Err(RepositoryWriteGateError::new(
            RepositoryWriteGateCode::RepositoryTrustAnchorInvalid,
            format!(
                "trust anchor frontier {} does not match projected {}",
                anchor.frontier_id, projection.frontier_id
            ),
        ));
    }
    if anchor.identity_root != projection.identity_root {
        return Err(RepositoryWriteGateError::new(
            RepositoryWriteGateCode::RepositoryTrustAnchorInvalid,
            "trust anchor identity_root does not match the event-derived identity",
        ));
    }
    let matching = project
        .events
        .iter()
        .filter(|event| {
            repository_boundary_event_content_root(event)
                .is_ok_and(|root| root == anchor.boundary_content_root)
        })
        .collect::<Vec<_>>();
    let [root_boundary] = matching.as_slice() else {
        return Err(RepositoryWriteGateError::new(
            RepositoryWriteGateCode::RepositoryTrustAnchorInvalid,
            format!(
                "trust anchor boundary root must identify exactly one event, found {}",
                matching.len()
            ),
        ));
    };
    let payload = repository_boundary_payload_from_event_shape(root_boundary).map_err(|error| {
        RepositoryWriteGateError::new(RepositoryWriteGateCode::RepositoryTrustAnchorInvalid, error)
    })?;
    if payload.frontier_id != anchor.frontier_id
        || payload.identity_root != anchor.identity_root
        || payload.administrator_actor_id != anchor.administrator_actor_id
        || payload.administrator_public_key != anchor.administrator_public_key
    {
        return Err(RepositoryWriteGateError::new(
            RepositoryWriteGateCode::RepositoryTrustAnchorInvalid,
            "trust anchor fields do not match the exact initial boundary event",
        ));
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

fn validate_actor_id(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > 256
        || value.chars().any(|character| character.is_control())
    {
        return Err(format!(
            "{field} must be non-empty, at most 256 bytes, and contain no control characters"
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
            #[cfg_attr(windows, allow(unused_mut))]
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
            #[cfg_attr(windows, allow(unused_mut))]
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
    use ed25519_dalek::SigningKey;
    use serde_json::json;
    use vela_protocol::bundle::{ConfidenceUpdate, ReviewAction, ReviewEvent};
    use vela_protocol::events::{
        EVENT_SCHEMA, EventKind, NULL_HASH, StateActor, StateEvent, StateTarget,
    };
    use vela_protocol::frontier_profile::FRONTIER_PROFILE_SCHEMA_V1;
    use vela_protocol::frontier_repo::{
        ProfileV1InitOptions, initialize_profile_v1_minimal, read_repository_profile,
    };
    use vela_protocol::frontier_repository::{
        FRONTIER_REPOSITORY_BOUNDARY_SCHEMA, FrontierIdentityV1,
        FrontierRepositoryBoundaryPayloadV1, FrontierRepositoryTrustMode, exact_dependency_root,
        new_repository_boundary_event,
    };
    use vela_protocol::sign::{ActorRecord, pubkey_hex, sign_event};
    use vela_protocol::test_support::make_finding;

    #[cfg(windows)]
    #[test]
    fn native_windows_repository_file_mutation_is_explicitly_fail_closed() {
        let exact = PreparedRepositoryFileReplacement::prepare_exact(
            Path::new(r"C:\vela-fixture"),
            Path::new(".vela/settings.toml"),
            Some(b"old"),
            b"new",
            RepositoryFileReplacementMode::PreserveExisting,
            1024,
        )
        .unwrap_err();
        let observed = PreparedRepositoryFileReplacement::prepare_observed(
            Path::new(r"C:\vela-fixture"),
            Path::new("targets.json"),
            b"new",
            RepositoryFileReplacementMode::Exact(0o644),
            1024,
        )
        .unwrap_err();
        for error in [exact, observed] {
            assert_eq!(error, REPOSITORY_FILE_MUTATION_UNAVAILABLE);
            assert!(error.contains("native Windows"));
            assert!(error.contains("handle-relative atomic exchange"));
            assert!(error.contains("WSL2"));
            assert!(error.contains("read, check, and reproduce remain available"));
        }
    }

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn profile_v1_fixture() -> (tempfile::TempDir, Project) {
        let directory = tempfile::tempdir().unwrap();
        initialize_profile_v1_minimal(
            directory.path(),
            ProfileV1InitOptions {
                name: "Write gate fixture",
                scope: "Can the exact Profile v1 repository pass the write gate?",
                initialize_git: false,
            },
        )
        .unwrap();
        let project = vela_protocol::repo::load_from_path(directory.path()).unwrap();
        (directory, project)
    }

    fn trust_anchor() -> RepositoryTrustAnchorV1 {
        RepositoryTrustAnchorV1 {
            schema: REPOSITORY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
            frontier_id: "vfr_0123456789abcdef".to_string(),
            identity_root: format!("sha256:{}", "1".repeat(64)),
            boundary_content_root: format!("sha256:{}", "2".repeat(64)),
            administrator_actor_id: "reviewer:administrator".to_string(),
            administrator_public_key: "3".repeat(64),
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
    fn trust_anchor_round_trip_is_closed_rooted_and_private() {
        let home = tempfile::tempdir().unwrap();
        let anchor = trust_anchor();
        let installed = install_repository_trust_anchor_from_home(home.path(), &anchor).unwrap();
        assert_eq!(installed.anchor, anchor);
        assert_eq!(installed.root, anchor.root().unwrap());
        assert_eq!(
            installed.path,
            home.path()
                .join(".vela/trust/frontiers/vfr_0123456789abcdef.json")
        );
        let loaded = load_repository_trust_anchor_from_home(home.path(), "vfr_0123456789abcdef")
            .unwrap()
            .unwrap();
        assert_eq!(loaded, installed);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(home.path().join(".vela/trust/frontiers"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&installed.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn trust_anchor_is_never_created_by_load_and_never_replaced() {
        let home = tempfile::tempdir().unwrap();
        assert!(
            load_repository_trust_anchor_from_home(home.path(), "vfr_0123456789abcdef")
                .unwrap()
                .is_none()
        );
        assert!(!home.path().join(".vela").exists());
        let first = trust_anchor();
        install_repository_trust_anchor_from_home(home.path(), &first).unwrap();
        let mut different = first;
        different.identity_root = format!("sha256:{}", "4".repeat(64));
        assert!(
            install_repository_trust_anchor_from_home(home.path(), &different)
                .unwrap_err()
                .contains("refusing to replace")
        );
    }

    #[cfg(unix)]
    #[test]
    fn trust_anchor_loader_rejects_symlinked_pin() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let home = tempfile::tempdir().unwrap();
        let frontiers = home.path().join(".vela/trust/frontiers");
        fs::create_dir_all(&frontiers).unwrap();
        for directory in [
            home.path().join(".vela"),
            home.path().join(".vela/trust"),
            frontiers.clone(),
        ] {
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let target = home.path().join("anchor.json");
        fs::write(&target, serde_json::to_vec(&trust_anchor()).unwrap()).unwrap();
        symlink(&target, frontiers.join("vfr_0123456789abcdef.json")).unwrap();
        let error = load_repository_trust_anchor_from_home(home.path(), "vfr_0123456789abcdef")
            .unwrap_err();
        assert!(error.contains("may not be a symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn trust_anchor_loader_requires_exact_private_modes() {
        use std::os::unix::fs::PermissionsExt;

        let home = tempfile::tempdir().unwrap();
        let installed =
            install_repository_trust_anchor_from_home(home.path(), &trust_anchor()).unwrap();
        fs::set_permissions(&installed.path, fs::Permissions::from_mode(0o640)).unwrap();
        let file_error =
            load_repository_trust_anchor_from_home(home.path(), "vfr_0123456789abcdef")
                .unwrap_err();
        assert!(file_error.contains("exact mode 0600"), "{file_error}");

        fs::set_permissions(&installed.path, fs::Permissions::from_mode(0o600)).unwrap();
        let trust = home.path().join(".vela/trust");
        fs::set_permissions(&trust, fs::Permissions::from_mode(0o750)).unwrap();
        let directory_error =
            load_repository_trust_anchor_from_home(home.path(), "vfr_0123456789abcdef")
                .unwrap_err();
        assert!(
            directory_error.contains("exact mode 0700"),
            "{directory_error}"
        );

        fs::set_permissions(&trust, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(home.path().join(".vela"), fs::Permissions::from_mode(0o777)).unwrap();
        let parent_error =
            load_repository_trust_anchor_from_home(home.path(), "vfr_0123456789abcdef")
                .unwrap_err();
        assert!(
            parent_error.contains("may not be group- or world-writable"),
            "{parent_error}"
        );
    }

    #[test]
    fn trust_anchor_rejects_unknown_fields_and_noncanonical_ids() {
        let mut value = serde_json::to_value(trust_anchor()).unwrap();
        value["extra"] = json!(true);
        assert!(serde_json::from_value::<RepositoryTrustAnchorV1>(value).is_err());
        let mut bad = trust_anchor();
        bad.frontier_id = "../../escape".to_string();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn write_gate_accepts_exact_profile_v1_genesis() {
        let (directory, project) = profile_v1_fixture();
        let verified = verify_repository_for_write(directory.path(), &project, None).unwrap();
        assert_eq!(verified.frontier_id, project.frontier_id());
        assert!(matches!(
            verified.identity,
            VerifiedRepositoryIdentity::Genesis { .. }
        ));
        assert_eq!(
            verified.replayed_snapshot_hash,
            verified.materialized_snapshot_hash
        );
    }

    #[test]
    fn write_gate_rejects_non_evented_finding_before_native_boundary() {
        let (directory, mut project) = profile_v1_fixture();
        project
            .findings
            .push(make_finding("vf_native_remnant", 0.5, "computational"));
        let error = verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ReducerReplayFailed);
        assert!(error.message.contains("non-evented finding remnant"));
        assert!(error.message.contains("vf_native_remnant"));
    }

    #[test]
    fn write_gate_requires_pin_for_first_native_administrator_boundary() {
        let directory = tempfile::tempdir().unwrap();
        initialize_profile_v1_minimal(
            directory.path(),
            ProfileV1InitOptions {
                name: "Native boundary fixture",
                scope: "Can a genesis-rooted dependency boundary preserve native identity?",
                initialize_git: false,
            },
        )
        .unwrap();
        let mut project = vela_protocol::repo::load_from_path(directory.path()).unwrap();
        let genesis = project.events.first().unwrap().clone();
        let identity = FrontierIdentityV1::from_genesis_event(&genesis).unwrap();
        let key = SigningKey::from_bytes(&[29; 32]);
        let actor = ActorRecord {
            id: "reviewer:native-administrator".to_string(),
            public_key: pubkey_hex(&key),
            algorithm: "ed25519".to_string(),
            created_at: "2026-07-22T00:00:00Z".to_string(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        project.actors = vec![actor.clone()];
        vela_protocol::repo::save(
            &vela_protocol::repo::VelaSource::VelaRepo(directory.path().to_path_buf()),
            &project,
        )
        .unwrap();
        git(directory.path(), &["init", "-q", "-b", "main"]);
        git(directory.path(), &["config", "user.name", "Vela Test"]);
        git(
            directory.path(),
            &["config", "user.email", "vela@example.invalid"],
        );
        git(directory.path(), &["add", "."]);
        git(directory.path(), &["commit", "-qm", "native anchor"]);
        let commit = git(directory.path(), &["rev-parse", "HEAD"]);
        let facts = super::super::frontier_repository::derive_repository_anchor_facts(
            directory.path(),
            &commit,
        )
        .unwrap();
        let profile_root = match read_repository_profile(directory.path()).unwrap().unwrap() {
            FrontierProfileFile::V1(profile) => profile.profile_root().unwrap(),
            FrontierProfileFile::LegacyV0_1(_) => panic!("expected Profile v1"),
        };
        let mut boundary = new_repository_boundary_event(
            FrontierRepositoryBoundaryPayloadV1 {
                schema: FRONTIER_REPOSITORY_BOUNDARY_SCHEMA.to_string(),
                mode: FrontierRepositoryBoundaryMode::UpdateDependencies,
                frontier_id: identity.frontier_id.clone(),
                identity_root: identity.root().unwrap(),
                observed_profile_root: profile_root,
                dependency_root: exact_dependency_root(&[]).unwrap(),
                dependencies: Vec::new(),
                previous_identity_event_root: Some(
                    repository_identity_event_content_root(&genesis).unwrap(),
                ),
                legacy_identity_preimage_root: None,
                administrator_actor_id: actor.id,
                administrator_public_key: actor.public_key,
                administrator_algorithm: actor.algorithm,
                trust_mode: FrontierRepositoryTrustMode::Genesis,
                git_object_format: facts.git_object_format,
                anchor_git_commit: facts.git_commit,
                anchor_git_tree: facts.git_tree,
                anchor_event_log_root: facts.event_log_root,
                anchor_event_count: facts.event_count,
                anchor_snapshot_root: facts.snapshot_root,
                anchor_snapshot_schema: facts.snapshot_schema,
                anchor_proposal_root: facts.proposal_root,
                anchor_actor_registry_root: facts.actor_registry_root,
                anchor_artifact_registry_root: facts.artifact_registry_root,
                anchor_canonical_store_root: facts.canonical_store_root,
            },
            "bind exact native dependency state",
            "2026-07-22T00:01:00Z",
        )
        .unwrap();
        boundary.signature = Some(sign_event(&boundary, &key).unwrap());
        let boundary_payload = repository_boundary_payload_from_event_shape(&boundary).unwrap();
        let anchor = RepositoryTrustAnchorV1 {
            schema: REPOSITORY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
            frontier_id: boundary_payload.frontier_id,
            identity_root: boundary_payload.identity_root,
            boundary_content_root: repository_boundary_event_content_root(&boundary).unwrap(),
            administrator_actor_id: boundary_payload.administrator_actor_id,
            administrator_public_key: boundary_payload.administrator_public_key,
        };
        project.events.push(boundary);

        let missing = verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(
            missing.code,
            RepositoryWriteGateCode::RepositoryTrustAnchorRequired
        );

        let verified =
            verify_repository_for_write(directory.path(), &project, Some(&anchor)).unwrap();
        assert!(matches!(
            verified.identity,
            VerifiedRepositoryIdentity::PinnedBoundary {
                origin: VerifiedBoundaryOrigin::Genesis,
                ..
            }
        ));

        let mut remnant =
            serde_json::from_value::<Project>(serde_json::to_value(&project).unwrap()).unwrap();
        remnant.findings.push(make_finding(
            "vf_native_boundary_remnant",
            0.5,
            "computational",
        ));
        let error =
            verify_repository_for_write(directory.path(), &remnant, Some(&anchor)).unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ReducerReplayFailed);
        assert!(error.message.contains("non-evented finding remnant"));
        assert!(error.message.contains("vf_native_boundary_remnant"));

        let proposal = vela_protocol::proposals::new_proposal_at(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_native_boundary_decision".to_string(),
            },
            "agent:fixture",
            "agent",
            "record bounded evidence",
            json!({"note": "bounded evidence"}),
            Vec::new(),
            Vec::new(),
            "2026-07-22T00:02:00Z",
        );
        let proposal_id = proposal.id.clone();
        let proposal_kind = proposal.kind.clone();
        project.proposals.push(proposal);
        let mut decision = vela_protocol::events::new_review_decision_event(
            &proposal_id,
            &proposal_kind,
            "rejected",
            None,
            "reviewer:native-administrator",
            "bounded evidence is insufficient",
            Some("2026-07-22T00:03:00Z"),
        )
        .unwrap();
        decision.signature = Some(sign_event(&decision, &key).unwrap());
        let stored = project.proposals.last_mut().unwrap();
        stored.status = "rejected".to_string();
        stored.reviewed_by = Some("reviewer:native-administrator".to_string());
        stored.reviewed_at = Some("2026-07-22T00:03:00Z".to_string());
        stored.decision_reason = Some("bounded evidence is insufficient".to_string());
        project.events.push(decision);
        vela_protocol::repo::save_to_path(directory.path(), &project).unwrap();
        let reloaded = vela_protocol::repo::load_from_path(directory.path()).unwrap();
        verify_repository_for_write(directory.path(), &reloaded, Some(&anchor)).unwrap();

        let asserted = make_finding("vf_native_event_sourced", 0.9, "computational");
        let mut event = StateEvent {
            schema: EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: EventKind::FindingAsserted,
            target: StateTarget {
                r#type: "finding".to_string(),
                id: asserted.id.clone(),
            },
            actor: StateActor {
                r#type: "human".to_string(),
                id: "reviewer:native-administrator".to_string(),
            },
            timestamp: "2026-07-22T00:04:00Z".to_string(),
            reason: "assert one event-sourced native finding".to_string(),
            before_hash: NULL_HASH.to_string(),
            after_hash: vela_protocol::events::finding_hash(&asserted),
            payload: json!({"finding": asserted}),
            caveats: Vec::new(),
            signature: None,
        };
        event.id = vela_protocol::events::compute_event_id(&event);
        event.signature = Some(sign_event(&event, &key).unwrap());
        project.events.push(event);
        project.findings.push(make_finding(
            "vf_native_event_sourced",
            0.9,
            "computational",
        ));
        vela_protocol::repo::save_to_path(directory.path(), &project).unwrap();
        let reloaded = vela_protocol::repo::load_from_path(directory.path()).unwrap();
        verify_repository_for_write(directory.path(), &reloaded, Some(&anchor)).unwrap();
    }

    #[test]
    fn write_gate_refuses_v0_1_before_any_other_gate() {
        let directory = tempfile::tempdir().unwrap();
        vela_protocol::frontier_repo::initialize_minimal(
            directory.path(),
            vela_protocol::frontier_repo::InitOptions {
                name: "legacy",
                initialize_git: false,
            },
        )
        .unwrap();
        let project = vela_protocol::repo::load_from_path(directory.path()).unwrap();
        let error = verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(
            error.code,
            RepositoryWriteGateCode::FrontierProfileUpgradeRequired
        );
        assert_eq!(error.code.as_str(), "frontier_profile_upgrade_required");
    }

    #[test]
    fn write_gate_rejects_missing_and_malformed_settings() {
        let (directory, project) = profile_v1_fixture();
        fs::remove_file(directory.path().join(".vela/settings.toml")).unwrap();
        let missing = verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(
            missing.code,
            RepositoryWriteGateCode::FrontierSettingsMissing
        );

        fs::write(
            directory.path().join(".vela/settings.toml"),
            "schema = \"vela.frontier-settings.v1\"\nunknown = true\n",
        )
        .unwrap();
        let malformed = verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(
            malformed.code,
            RepositoryWriteGateCode::FrontierSettingsInvalid
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_gate_rejects_byte_identical_external_profile_and_settings_symlinks() {
        use std::os::unix::fs::symlink;

        for relative in ["frontier.yaml", ".vela/settings.toml"] {
            let (directory, project) = profile_v1_fixture();
            let path = directory.path().join(relative);
            let external = tempfile::tempdir().unwrap();
            let target = external.path().join("control-file");
            fs::copy(&path, &target).unwrap();
            fs::remove_file(&path).unwrap();
            symlink(&target, &path).unwrap();

            let error = verify_repository_for_write(directory.path(), &project, None).unwrap_err();
            let expected = if relative == "frontier.yaml" {
                RepositoryWriteGateCode::FrontierProfileInvalid
            } else {
                RepositoryWriteGateCode::FrontierSettingsInvalid
            };
            assert_eq!(error.code, expected, "{relative}: {}", error.message);
            assert!(
                error.message.contains("regular non-symlink"),
                "{relative}: {}",
                error.message
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn write_gate_rejects_symlinked_control_file_parent() {
        use std::os::unix::fs::symlink;

        let (directory, project) = profile_v1_fixture();
        let vela = directory.path().join(".vela");
        let external = tempfile::tempdir().unwrap();
        let moved = external.path().join("vela-control-directory");
        fs::rename(&vela, &moved).unwrap();
        symlink(&moved, &vela).unwrap();

        let error = verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::FrontierSettingsInvalid);
        assert!(
            error
                .message
                .contains("real non-symlink repository directories"),
            "{}",
            error.message
        );
    }

    #[test]
    fn write_gate_rejects_profile_identity_mismatch() {
        let (directory, project) = profile_v1_fixture();
        let path = directory.path().join("frontier.yaml");
        let source = fs::read_to_string(&path).unwrap();
        fs::write(
            path,
            source.replace(&project.frontier_id(), "vfr_0123456789abcdef"),
        )
        .unwrap();
        let error = verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::FrontierProfileMismatch);
    }

    #[test]
    fn write_gate_rejects_reducer_replay_failure() {
        let (directory, mut project) = profile_v1_fixture();
        let mut unsupported = StateEvent {
            schema: EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: EventKind::Other("unsupported.write-gate-fixture".to_string()),
            target: StateTarget {
                r#type: "frontier".to_string(),
                id: project.frontier_id(),
            },
            actor: StateActor {
                r#type: "agent".to_string(),
                id: "agent:fixture".to_string(),
            },
            timestamp: "2026-07-22T00:00:01Z".to_string(),
            reason: "force reducer failure".to_string(),
            before_hash: NULL_HASH.to_string(),
            after_hash: NULL_HASH.to_string(),
            payload: json!({}),
            caveats: vec![],
            signature: None,
        };
        unsupported.id = vela_protocol::events::compute_event_id(&unsupported);
        project.events.push(unsupported);
        let error = verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ReducerReplayFailed);
    }

    #[test]
    fn repository_write_gate_checks_full_proposal_decision_parity() {
        let (directory, mut project) = profile_v1_fixture();
        let proposal = vela_protocol::proposals::new_proposal_at(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_write_gate_fixture".to_string(),
            },
            "agent:fixture",
            "agent",
            "record bounded evidence",
            json!({"note": "bounded evidence"}),
            Vec::new(),
            Vec::new(),
            "2026-07-22T00:00:00Z",
        );
        let proposal_path = directory
            .path()
            .join(format!(".vela/proposals/{}.json", proposal.id));
        project.proposals.push(proposal);
        vela_protocol::repo::save_to_path(directory.path(), &project).unwrap();

        let pending_bytes = fs::read(&proposal_path).unwrap();
        for (status, field, value, expected) in [
            (
                "pending_review",
                "decision_reason",
                "forged pending decision",
                "terminal decision fields",
            ),
            (
                "rejected",
                "reviewed_by",
                "reviewer:forged",
                "NO decision event",
            ),
            (
                "applied",
                "applied_event_id",
                "vev_0123456789abcdef",
                "NO decision event",
            ),
        ] {
            let mut stored: serde_json::Value = serde_json::from_slice(&pending_bytes).unwrap();
            stored["status"] = json!(status);
            stored[field] = json!(value);
            fs::write(&proposal_path, serde_json::to_vec_pretty(&stored).unwrap()).unwrap();
            let edited = vela_protocol::repo::load_from_path(directory.path()).unwrap();
            let before = fs::read(&proposal_path).unwrap();

            let error = verify_repository_for_write(directory.path(), &edited, None).unwrap_err();
            assert_eq!(error.code, RepositoryWriteGateCode::ProposalParityFailed);
            assert!(error.message.contains(expected), "{}", error.message);
            assert_eq!(fs::read(&proposal_path).unwrap(), before);
        }

        fs::write(&proposal_path, &pending_bytes).unwrap();
        let mut decided = vela_protocol::repo::load_from_path(directory.path()).unwrap();
        let decision = vela_protocol::events::new_review_decision_event(
            &decided.proposals[0].id,
            &decided.proposals[0].kind,
            "rejected",
            None,
            "reviewer:fixture",
            "bounded evidence is insufficient",
            Some("2026-07-22T00:01:00Z"),
        )
        .unwrap();
        decided.proposals[0].status = "rejected".to_string();
        decided.proposals[0].reviewed_by = Some("reviewer:fixture".to_string());
        decided.proposals[0].reviewed_at = Some("2026-07-22T00:01:00Z".to_string());
        decided.proposals[0].decision_reason = Some("bounded evidence is insufficient".to_string());
        decided.events.push(decision);
        vela_protocol::repo::save_to_path(directory.path(), &decided).unwrap();
        let mut stored: serde_json::Value =
            serde_json::from_slice(&fs::read(&proposal_path).unwrap()).unwrap();
        stored.as_object_mut().unwrap().remove("reviewed_at");
        fs::write(&proposal_path, serde_json::to_vec_pretty(&stored).unwrap()).unwrap();
        let missing_decision_field = vela_protocol::repo::load_from_path(directory.path()).unwrap();
        let error = verify_repository_for_write(directory.path(), &missing_decision_field, None)
            .unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ProposalParityFailed);
        assert!(
            error.message.contains("stored decision fields"),
            "{}",
            error.message
        );
    }

    #[test]
    fn write_gate_rejects_deleted_decided_proposal() {
        let (directory, mut project) = profile_v1_fixture();
        let proposal = vela_protocol::proposals::new_proposal_at(
            "finding.note",
            StateTarget {
                r#type: "finding".to_string(),
                id: "vf_write_gate_fixture".to_string(),
            },
            "agent:fixture",
            "agent",
            "record bounded evidence",
            json!({"note": "bounded evidence"}),
            Vec::new(),
            Vec::new(),
            "2026-07-22T00:00:00Z",
        );
        let proposal_id = proposal.id.clone();
        let proposal_kind = proposal.kind.clone();
        project.proposals.push(proposal);
        let decision = vela_protocol::events::new_review_decision_event(
            &proposal_id,
            &proposal_kind,
            "rejected",
            None,
            "reviewer:fixture",
            "bounded evidence is insufficient",
            Some("2026-07-22T00:01:00Z"),
        )
        .unwrap();
        let stored = project.proposals.first_mut().unwrap();
        stored.status = "rejected".to_string();
        stored.reviewed_by = Some("reviewer:fixture".to_string());
        stored.reviewed_at = Some("2026-07-22T00:01:00Z".to_string());
        stored.decision_reason = Some("bounded evidence is insufficient".to_string());
        project.events.push(decision);
        vela_protocol::repo::save_to_path(directory.path(), &project).unwrap();
        verify_repository_for_write(directory.path(), &project, None).unwrap();

        let proposal_path = directory
            .path()
            .join(format!(".vela/proposals/{proposal_id}.json"));
        fs::remove_file(&proposal_path).unwrap();
        let deleted = vela_protocol::repo::load_from_path(directory.path()).unwrap();
        let error = verify_repository_for_write(directory.path(), &deleted, None).unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ProposalParityFailed);
        assert!(
            error.message.contains("does not exist"),
            "{}",
            error.message
        );
        assert!(!proposal_path.exists());
    }

    #[test]
    fn write_gate_rejects_artifact_cache_not_backed_by_reducer_events() {
        let (directory, mut project) = profile_v1_fixture();
        project.artifacts.push(
            serde_json::from_value(json!({
                "id": "va_0123456789abcdef",
                "kind": "code",
                "name": "Unbacked artifact cache entry",
                "content_hash": format!("sha256:{}", "a".repeat(64)),
                "storage_mode": "remote",
                "locator": "https://example.invalid/unbacked",
                "source_url": "https://example.invalid/unbacked",
                "target_findings": [],
                "provenance": {
                    "source_type": "data_release",
                    "doi": null,
                    "url": "https://example.invalid/unbacked",
                    "title": "Unbacked artifact cache entry",
                    "authors": [],
                    "year": 2026,
                    "funders": [],
                    "extraction": {},
                    "review": null,
                    "contributions": []
                },
                "metadata": {},
                "retracted": false,
                "access_tier": "public",
                "created": "2026-07-22T00:00:00Z"
            }))
            .unwrap(),
        );
        let error = verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(error.code, RepositoryWriteGateCode::ReducerReplayFailed);
        assert!(error.message.contains("artifact registry"));
    }

    #[test]
    fn write_gate_rejects_native_unreplayed_scientific_sidecars() {
        let (directory, mut project) = profile_v1_fixture();
        project.review_events.push(ReviewEvent {
            id: "rev_unbacked".to_string(),
            workspace: None,
            finding_id: "vf_unbacked".to_string(),
            reviewer: "reviewer:fixture".to_string(),
            reviewed_at: "2026-07-22T00:00:00Z".to_string(),
            scope: None,
            status: None,
            action: ReviewAction::Approved,
            reason: "unbacked review sidecar".to_string(),
            evidence_considered: Vec::new(),
            state_change: None,
        });
        let review_error =
            verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(
            review_error.code,
            RepositoryWriteGateCode::ReducerReplayFailed
        );
        assert!(review_error.message.contains("review_events"));

        project.review_events.clear();
        project.confidence_updates.push(ConfidenceUpdate {
            finding_id: "vf_unbacked".to_string(),
            previous_score: 0.1,
            new_score: 0.2,
            basis: "unbacked confidence sidecar".to_string(),
            updated_by: "agent:fixture".to_string(),
            updated_at: "2026-07-22T00:00:00Z".to_string(),
        });
        let confidence_error =
            verify_repository_for_write(directory.path(), &project, None).unwrap_err();
        assert_eq!(
            confidence_error.code,
            RepositoryWriteGateCode::ReducerReplayFailed
        );
        assert!(confidence_error.message.contains("confidence_updates"));
    }

    #[test]
    fn write_gate_does_not_require_generated_views_or_proof_packets() {
        let (directory, project) = profile_v1_fixture();
        for relative in ["frontier.json", "vela.lock"] {
            let path = directory.path().join(relative);
            if path.exists() {
                fs::remove_file(path).unwrap();
            }
        }
        let proof = directory.path().join("proof");
        if proof.exists() {
            fs::remove_dir_all(proof).unwrap();
        }
        verify_repository_for_write(directory.path(), &project, None).unwrap();
    }

    #[test]
    fn profile_fixture_really_uses_the_v1_schema() {
        let (directory, _) = profile_v1_fixture();
        let source = fs::read_to_string(directory.path().join("frontier.yaml")).unwrap();
        assert!(source.contains(FRONTIER_PROFILE_SCHEMA_V1));
    }
}
