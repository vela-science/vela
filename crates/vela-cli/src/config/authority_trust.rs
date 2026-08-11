//! CLI-owned descriptor-hardened storage for the independently held
//! sequence-one repository-authority pin. It grants no authority.

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
use rustix::fd::OwnedFd;

use serde::{Deserialize, Serialize};
use vela_protocol::canonical;

pub const AUTHORITY_TRUST_ANCHOR_SCHEMA_V1: &str = "vela.authority-trust-anchor.v1";
#[cfg(any(target_os = "linux", target_vendor = "apple"))]
const AUTHORITY_TRUST_ANCHOR_MAX_BYTES: u64 = 4 * 1024;
#[cfg(any(target_os = "linux", target_vendor = "apple"))]
const AUTHORITY_TRUST_ANCHOR_REBIND_MISSING: &str =
    "authority trust-anchor rebind requires an existing exact pin";
#[cfg(any(target_os = "linux", target_vendor = "apple"))]
const AUTHORITY_TRUST_ANCHOR_REBIND_MISMATCH: &str =
    "authority trust-anchor rebind preimage does not match the installed pin";

/// An out-of-band pin for the first repository-authority record.
///
/// The sequence-1 authority-record root binds the repository, initial keyset,
/// policy authorization, principal, events, and execution claim. The local pin
/// selects the intended chain; it grants no authority and is never read from
/// repository-controlled bytes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorityTrustAnchorV1 {
    pub schema: String,
    pub repository_id: String,
    pub first_authority_record_root: String,
}

impl AuthorityTrustAnchorV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUTHORITY_TRUST_ANCHOR_SCHEMA_V1 {
            return Err(format!(
                "authority trust anchor schema must be {AUTHORITY_TRUST_ANCHOR_SCHEMA_V1}"
            ));
        }
        validate_repository_id(&self.repository_id)?;
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
        repository_id: &str,
        first_authority_record_root: &str,
    ) -> Result<(), String> {
        self.validate()?;
        if self.repository_id != repository_id
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

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RepositoryFileIdentity {
    device: u64,
    inode: u64,
    owner: u32,
    mode: u32,
}

// `rustix::fs::Stat` exposes normalized `u64`/`u32` fields on Linux. Apple uses
// libc's narrower `dev_t`/`mode_t`, so only that ABI needs explicit widening.
#[cfg(target_os = "linux")]
fn stat_device(stat: &rustix::fs::Stat) -> u64 {
    stat.st_dev
}

#[cfg(target_vendor = "apple")]
fn stat_device(stat: &rustix::fs::Stat) -> u64 {
    stat.st_dev as u64
}

#[cfg(target_os = "linux")]
fn stat_mode(stat: &rustix::fs::Stat) -> u32 {
    stat.st_mode
}

#[cfg(target_vendor = "apple")]
fn stat_mode(stat: &rustix::fs::Stat) -> u32 {
    stat.st_mode as u32
}

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
impl RepositoryFileIdentity {
    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat_device(stat),
            inode: stat.st_ino,
            owner: stat.st_uid,
            mode: stat_mode(stat),
        }
    }
}

#[cfg(unix)]
fn require_trusted_parent_owner_mode(owner: u32, mode: u32, label: &Path) -> Result<(), String> {
    if owner != rustix::process::geteuid().as_raw() {
        return Err(format!(
            "trust parent directory '{}' is not owned by the current operating-system account",
            label.display()
        ));
    }
    let mode = mode & 0o777;
    if mode & 0o022 != 0 {
        return Err(format!(
            "trust parent directory '{}' may not be group- or world-writable; observed mode {mode:04o}",
            label.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn require_private_owner_mode(
    owner: u32,
    mode: u32,
    label: &Path,
    expected_mode: u32,
    kind: &str,
) -> Result<(), String> {
    if owner != rustix::process::geteuid().as_raw() {
        return Err(format!(
            "trust {kind} '{}' is not owned by the current operating-system account",
            label.display()
        ));
    }
    let mode = mode & 0o777;
    if mode != expected_mode {
        return Err(format!(
            "trust {kind} '{}' must have exact mode {expected_mode:04o}, observed {mode:04o}",
            label.display()
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
#[derive(Debug)]
struct PinnedRepositoryDirectory {
    name: OsString,
    descriptor: OwnedFd,
    identity: RepositoryFileIdentity,
}

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
struct RepositoryReplacementTemporary<'a> {
    parent: &'a OwnedFd,
    name: String,
    armed: bool,
}

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
impl RepositoryReplacementTemporary<'_> {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
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

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
struct RepositoryRootWriteLock<'a> {
    descriptor: &'a OwnedFd,
}

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
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
                        "authority trust-anchor replacement busy for {}; retry after the current Vela writer finishes",
                        label.display()
                    )
                } else {
                    format!(
                        "lock authority trust store before replacing {}: {error}",
                        label.display()
                    )
                }
            })?;
        Ok(RepositoryRootWriteLock { descriptor })
    }
}

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
impl Drop for RepositoryRootWriteLock<'_> {
    fn drop(&mut self) {
        let _ = rustix::fs::flock(self.descriptor, rustix::fs::FlockOperation::Unlock);
    }
}

/// A descriptor-relative replacement for one existing authority trust pin.
///
/// Preparation pins the account home, private authority directory chain, and
/// existing owner-only 0600 preimage with no-follow semantics. Installation
/// locks the account home, revalidates every identity/mode/owner/byte, and then
/// either returns the verified no-op or atomically exchanges and reads back.
/// The lock serializes Vela writers only; exact checks detect other writers.
/// Failed or uncertain rollback stops cleanup and names its recovery path;
/// no write can escape the pinned authority directory.
///
/// Linux and Apple platforms expose the required exchange rename primitive.
/// Unsupported platforms fail closed at preparation rather than
/// falling back to a path-based rename with a known TOCTOU gap.
#[cfg(any(target_os = "linux", target_vendor = "apple"))]
#[derive(Debug)]
struct PreparedAuthorityTrustAnchorReplacement {
    root_path: PathBuf,
    relative_path: PathBuf,
    root_descriptor: OwnedFd,
    root_identity: RepositoryFileIdentity,
    directories: Vec<PinnedRepositoryDirectory>,
    leaf_name: OsString,
    preimage: Vec<u8>,
    preimage_identity: RepositoryFileIdentity,
    replacement: Vec<u8>,
}

#[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
#[derive(Debug)]
struct PreparedAuthorityTrustAnchorReplacement;

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
impl PreparedAuthorityTrustAnchorReplacement {
    fn prepare(
        root_path: &Path,
        expected: &AuthorityTrustAnchorV1,
        replacement: &AuthorityTrustAnchorV1,
    ) -> Result<Self, String> {
        use rustix::fs::{FileType, Mode, OFlags};

        let mut preimage = serde_json::to_vec_pretty(expected)
            .map_err(|error| format!("serialize authority trust-anchor preimage: {error}"))?;
        preimage.push(b'\n');
        let mut replacement = serde_json::to_vec_pretty(replacement)
            .map_err(|error| format!("serialize authority trust-anchor replacement: {error}"))?;
        replacement.push(b'\n');
        let relative_path = PathBuf::from(".vela")
            .join("trust")
            .join("authorities")
            .join(format!("{}.json", expected.repository_id));
        let components = [".vela", "trust", "authorities"];
        let leaf_name = OsString::from(format!("{}.json", expected.repository_id));
        let label = relative_path.display().to_string();

        let root_descriptor = rustix::fs::open(
            root_path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|error| {
            if error == rustix::io::Errno::NOENT {
                AUTHORITY_TRUST_ANCHOR_REBIND_MISSING.to_string()
            } else {
                format!("open repository root for {label}: {error}")
            }
        })?;
        let root_stat = rustix::fs::fstat(&root_descriptor)
            .map_err(|error| format!("identify repository root for {label}: {error}"))?;
        if FileType::from_raw_mode(root_stat.st_mode) != FileType::Directory {
            return Err(format!(
                "{label} must be beneath real non-symlink repository directories"
            ));
        }
        let root_identity = RepositoryFileIdentity::from_stat(&root_stat);

        let mut directories: Vec<PinnedRepositoryDirectory> = Vec::new();
        let mut directory_label = PathBuf::new();
        for (index, name) in components.into_iter().enumerate() {
            let name = OsString::from(name);
            directory_label.push(&name);
            let parent = directories
                .last()
                .map_or(&root_descriptor, |directory| &directory.descriptor);
            let descriptor = rustix::fs::openat(
                parent,
                &name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .map_err(|error| {
                if error == rustix::io::Errno::NOENT {
                    AUTHORITY_TRUST_ANCHOR_REBIND_MISSING.to_string()
                } else {
                    format!("open pinned parent of {label}: {error}")
                }
            })?;
            let stat = rustix::fs::fstat(&descriptor)
                .map_err(|error| format!("identify pinned parent of {label}: {error}"))?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
                return Err(format!(
                    "{label} must be beneath real non-symlink repository directories"
                ));
            }
            let identity = RepositoryFileIdentity::from_stat(&stat);
            if index == 0 {
                require_trusted_parent_owner_mode(identity.owner, identity.mode, &directory_label)?;
            } else {
                require_private_owner_mode(
                    identity.owner,
                    identity.mode,
                    &directory_label,
                    0o700,
                    "directory",
                )?;
            }
            directories.push(PinnedRepositoryDirectory {
                name,
                descriptor,
                identity,
            });
        }
        let parent = directories
            .last()
            .map_or(&root_descriptor, |directory| &directory.descriptor);
        let (observed, preimage_identity) = Self::read_named_preimage(parent, &leaf_name, &label)?
            .ok_or_else(|| AUTHORITY_TRUST_ANCHOR_REBIND_MISSING.to_string())?;
        if observed != preimage {
            return Err(AUTHORITY_TRUST_ANCHOR_REBIND_MISMATCH.to_string());
        }
        require_private_owner_mode(
            preimage_identity.owner,
            preimage_identity.mode,
            &relative_path,
            0o600,
            "file",
        )?;
        Ok(Self {
            root_path: root_path.to_path_buf(),
            relative_path,
            root_descriptor,
            root_identity,
            directories,
            leaf_name,
            preimage,
            preimage_identity,
            replacement,
        })
    }

    fn parent_descriptor(&self) -> &OwnedFd {
        self.directories
            .last()
            .map_or(&self.root_descriptor, |directory| &directory.descriptor)
    }

    fn read_open_file(file: OwnedFd, label: &str) -> Result<Vec<u8>, String> {
        let mut file = fs::File::from(file);
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(AUTHORITY_TRUST_ANCHOR_MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("read pinned {label}: {error}"))?;
        if bytes.len() as u64 > AUTHORITY_TRUST_ANCHOR_MAX_BYTES {
            return Err(format!(
                "{label} exceeds the {AUTHORITY_TRUST_ANCHOR_MAX_BYTES} byte repository-file limit"
            ));
        }
        Ok(bytes)
    }

    fn read_named_preimage(
        parent: &OwnedFd,
        leaf_name: &OsString,
        label: &str,
    ) -> Result<Option<(Vec<u8>, RepositoryFileIdentity)>, String> {
        use rustix::fs::{FileType, Mode, OFlags};

        let descriptor = match rustix::fs::openat(
            parent,
            leaf_name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
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
            return Err(format!("{label} must be a regular non-symlink file"));
        }
        let identity = RepositoryFileIdentity::from_stat(&stat);
        let bytes = Self::read_open_file(descriptor, label)?;
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
        if RepositoryFileIdentity::from_stat(&root_stat) != self.root_identity {
            return Err(format!(
                "repository parent of {label} changed before replacement"
            ));
        }
        let mut parent = &self.root_descriptor;
        for directory in &self.directories {
            let stat = rustix::fs::statat(parent, &directory.name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|error| format!("reidentify pinned parent of {label}: {error}"))?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
                || RepositoryFileIdentity::from_stat(&stat) != directory.identity
            {
                return Err(format!(
                    "repository parent of {label} changed before replacement"
                ));
            }
            parent = &directory.descriptor;
        }
        Ok(())
    }

    fn loaded_from_bytes(&self, bytes: &[u8]) -> Result<LoadedAuthorityTrustAnchorV1, String> {
        let anchor: AuthorityTrustAnchorV1 = serde_json::from_slice(bytes).map_err(|error| {
            format!(
                "parse verified authority trust anchor '{}': {error}",
                self.relative_path.display()
            )
        })?;
        anchor.validate()?;
        Ok(LoadedAuthorityTrustAnchorV1 {
            path: self.root_path.join(&self.relative_path),
            root: anchor.root()?,
            anchor,
        })
    }

    fn revalidate_preimage(&self) -> Result<LoadedAuthorityTrustAnchorV1, String> {
        let label = self.relative_path.display().to_string();
        let observed =
            Self::read_named_preimage(self.parent_descriptor(), &self.leaf_name, &label)?;
        match observed {
            Some((observed, observed_identity))
                if observed == self.preimage && observed_identity == self.preimage_identity =>
            {
                self.loaded_from_bytes(&observed)
            }
            None => Err(AUTHORITY_TRUST_ANCHOR_REBIND_MISSING.to_string()),
            _ => Err(AUTHORITY_TRUST_ANCHOR_REBIND_MISMATCH.to_string()),
        }
    }

    fn verify_installed_leaf(
        &self,
        temporary_identity: RepositoryFileIdentity,
    ) -> Result<LoadedAuthorityTrustAnchorV1, String> {
        let label = self.relative_path.display().to_string();
        let (bytes, identity) =
            Self::read_named_preimage(self.parent_descriptor(), &self.leaf_name, &label)?
                .ok_or_else(|| format!("installed {label} disappeared during exact readback"))?;
        require_private_owner_mode(
            identity.owner,
            identity.mode,
            &self.relative_path,
            0o600,
            "file",
        )?;
        if bytes != self.replacement || identity != temporary_identity {
            return Err(format!(
                "installed {label} does not match its planned replacement"
            ));
        }
        self.revalidate_named_path()?;
        self.loaded_from_bytes(&bytes)
    }

    fn rollback_exchange(
        &self,
        parent: &OwnedFd,
        temporary_cleanup: &mut RepositoryReplacementTemporary<'_>,
        reason: String,
    ) -> String {
        use rustix::fs::RenameFlags;

        let recovery_path = self
            .root_path
            .join(self.relative_path.parent().unwrap_or_else(|| Path::new("")))
            .join(&temporary_cleanup.name);
        let rollback = rustix::fs::renameat_with(
            parent,
            temporary_cleanup.name.as_str(),
            parent,
            &self.leaf_name,
            RenameFlags::EXCHANGE,
        );
        match rollback {
            Ok(()) => match rustix::fs::fsync(parent) {
                Ok(()) => format!("{reason}; replacement was rolled back"),
                Err(error) => {
                    temporary_cleanup.disarm();
                    format!(
                        "{reason}; replacement was rolled back but rollback durability is uncertain: {error}; automatic cleanup stopped; inspect the exact pin and recovery path '{}'",
                        recovery_path.display()
                    )
                }
            },
            Err(rollback) => {
                temporary_cleanup.disarm();
                format!(
                    "{reason}; failed to roll back replacement: {rollback}; automatic cleanup stopped; inspect the exact pin and recovery path '{}'",
                    recovery_path.display()
                )
            }
        }
    }

    fn install(self) -> Result<LoadedAuthorityTrustAnchorV1, String> {
        self.install_with_hooks(|| Ok(()), || Ok(()), || {})
    }

    #[cfg(test)]
    fn install_with_hook(
        self,
        before_replace: impl FnOnce() -> Result<(), String>,
    ) -> Result<LoadedAuthorityTrustAnchorV1, String> {
        self.install_with_hooks(|| Ok(()), before_replace, || {})
    }

    fn install_with_hooks(
        self,
        after_temporary_created: impl FnOnce() -> Result<(), String>,
        before_replace: impl FnOnce() -> Result<(), String>,
        after_exchange: impl FnOnce(),
    ) -> Result<LoadedAuthorityTrustAnchorV1, String> {
        use rand_core::RngCore;
        use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};

        // The lock is advisory by design: it serializes every Vela replacement
        // prepared against this repository inode. A process with direct write
        // access to repository bytes remains outside this cooperative lease
        // and is handled by the exact preimage/readback checks below.
        let _root_lock =
            RepositoryRootWriteLock::try_acquire(&self.root_descriptor, &self.relative_path)?;
        self.revalidate_named_path()?;
        let loaded_preimage = self.revalidate_preimage()?;
        if self.preimage == self.replacement {
            return Ok(loaded_preimage);
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

        // Keep an incomplete temporary private and capture identity only after
        // bytes and restrictive mode are final.
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
        rustix::fs::fchmod(&temporary, Mode::from_raw_mode(0o600)).map_err(|error| {
            format!(
                "set temporary replacement mode for {}: {error}",
                self.relative_path.display()
            )
        })?;
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
        after_exchange();
        let displaced = Self::read_named_preimage(
            parent,
            &OsString::from(&temporary_cleanup.name),
            &self.relative_path.display().to_string(),
        );
        let displaced_matches = displaced.as_ref().is_ok_and(|value| {
            value.as_ref().is_some_and(|(bytes, identity)| {
                bytes == &self.preimage && *identity == self.preimage_identity
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
            return Err(self.rollback_exchange(parent, &mut temporary_cleanup, reason));
        }

        // Persist the installed leaf while the displaced preimage still makes
        // an fsync error rollback capable.
        if let Err(error) = rustix::fs::fsync(parent) {
            let reason = format!(
                "fsync pinned parent of {}: {error}",
                self.relative_path.display()
            );
            return Err(self.rollback_exchange(parent, &mut temporary_cleanup, reason));
        }

        if let Err(error) =
            rustix::fs::unlinkat(parent, temporary_cleanup.name.as_str(), AtFlags::empty())
        {
            let reason = format!(
                "remove displaced {} preimage: {error}",
                self.relative_path.display()
            );
            return Err(self.rollback_exchange(parent, &mut temporary_cleanup, reason));
        }
        temporary_cleanup.disarm();

        // The leaf is already durable. This sync only persists removal of the
        // displaced recovery artifact and cannot falsify the installed leaf.
        let _ = rustix::fs::fsync(parent);
        self.verify_installed_leaf(temporary_identity)
    }
}

#[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
const AUTHORITY_TRUST_ANCHOR_REBIND_UNAVAILABLE: &str = "authority trust-anchor rebind is unavailable on this platform because Vela cannot provide descriptor-relative exchange replacement";

#[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
impl PreparedAuthorityTrustAnchorReplacement {
    fn prepare(
        _root_path: &Path,
        _expected: &AuthorityTrustAnchorV1,
        _replacement: &AuthorityTrustAnchorV1,
    ) -> Result<Self, String> {
        Err(AUTHORITY_TRUST_ANCHOR_REBIND_UNAVAILABLE.to_string())
    }

    fn install(self) -> Result<LoadedAuthorityTrustAnchorV1, String> {
        Err(AUTHORITY_TRUST_ANCHOR_REBIND_UNAVAILABLE.to_string())
    }
}

/// Deterministic path for the independently distributed sequence-1 authority
/// root. Repository bytes and environment variables cannot redirect it.
pub fn authority_trust_anchor_path(
    user_home: &Path,
    repository_id: &str,
) -> Result<PathBuf, String> {
    validate_repository_id(repository_id)?;
    Ok(user_home
        .join(".vela")
        .join("trust")
        .join("authorities")
        .join(format!("{repository_id}.json")))
}

pub fn load_authority_trust_anchor_from_home(
    user_home: &Path,
    repository_id: &str,
) -> Result<Option<LoadedAuthorityTrustAnchorV1>, String> {
    let path = authority_trust_anchor_path(user_home, repository_id)?;
    let Some(anchor) = load_authority_trust_document(user_home, &path)? else {
        return Ok(None);
    };
    anchor.validate()?;
    if anchor.repository_id != repository_id {
        return Err(format!(
            "authority trust anchor repository {} does not match requested {repository_id}",
            anchor.repository_id
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
    let path = authority_trust_anchor_path(user_home, &anchor.repository_id)?;
    install_authority_trust_document(user_home, &path, anchor)?;
    load_authority_trust_anchor_from_home(user_home, &anchor.repository_id)?
        .ok_or_else(|| "installed authority trust anchor could not be read back".to_string())
}

#[cfg(all(test, any(target_os = "linux", target_vendor = "apple")))]
std::thread_local! {
    static AUTHORITY_REBIND_AFTER_PREPARE_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

#[cfg(all(test, any(target_os = "linux", target_vendor = "apple")))]
fn set_authority_rebind_after_prepare_hook(hook: impl FnOnce() + 'static) {
    AUTHORITY_REBIND_AFTER_PREPARE_HOOK.with(|slot| slot.replace(Some(Box::new(hook))));
}

#[cfg(all(test, any(target_os = "linux", target_vendor = "apple")))]
fn run_authority_rebind_after_prepare_hook() {
    AUTHORITY_REBIND_AFTER_PREPARE_HOOK.with(|slot| {
        if let Some(hook) = slot.take() {
            hook();
        }
    });
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
    if expected.repository_id != replacement.repository_id {
        return Err("authority trust-anchor rebind cannot change repository identity".to_string());
    }
    let prepared =
        PreparedAuthorityTrustAnchorReplacement::prepare(user_home, expected, replacement)?;
    #[cfg(all(test, any(target_os = "linux", target_vendor = "apple")))]
    run_authority_rebind_after_prepare_hook();
    prepared.install()
}

fn load_authority_trust_document(
    user_home: &Path,
    path: &Path,
) -> Result<Option<AuthorityTrustAnchorV1>, String> {
    let label = "authority trust anchor";
    let vela = user_home.join(".vela");
    let trust = vela.join("trust");
    let namespace_dir = trust.join("authorities");
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

fn install_authority_trust_document(
    user_home: &Path,
    path: &Path,
    document: &AuthorityTrustAnchorV1,
) -> Result<(), String> {
    let label = "authority trust anchor";
    let vela = user_home.join(".vela");
    let trust = vela.join("trust");
    let namespace_dir = trust.join("authorities");
    ensure_trusted_parent_directory(&vela)?;
    ensure_private_directory(&trust)?;
    ensure_private_directory(&namespace_dir)?;

    if path.exists() {
        let existing = load_authority_trust_document(user_home, path)?
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
        .prefix(".authority-trust-anchor-")
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

fn validate_repository_id(value: &str) -> Result<(), String> {
    if !vela_protocol::is_repository_id(value) {
        return Err(
            "trust anchor repository_id must be lowercase canonical RFC 9562 UUIDv4".to_string(),
        );
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
    if value.len() != length || !value.bytes().all(vela_protocol::is_lower_hex) {
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

    require_trusted_parent_owner_mode(metadata.uid(), metadata.permissions().mode(), path)
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

    require_private_owner_mode(
        metadata.uid(),
        metadata.permissions().mode(),
        path,
        expected_mode,
        kind,
    )
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
            repository_id: "01234567-89ab-4def-8123-456789abcdef".to_string(),
            first_authority_record_root: format!("sha256:{}", "4".repeat(64)),
        }
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    fn replacement_anchor(
        original: &AuthorityTrustAnchorV1,
        root_digit: char,
    ) -> AuthorityTrustAnchorV1 {
        let mut replacement = original.clone();
        replacement.first_authority_record_root =
            format!("sha256:{}", root_digit.to_string().repeat(64));
        replacement
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    fn anchor_bytes(anchor: &AuthorityTrustAnchorV1) -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(anchor).unwrap();
        bytes.push(b'\n');
        bytes
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    fn expect_error<T: std::fmt::Debug>(result: Result<T, String>, expected: &str) -> String {
        let error = result.unwrap_err();
        assert!(error.contains(expected), "{error}");
        error
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    struct AnchorFixture {
        home: tempfile::TempDir,
        original: AuthorityTrustAnchorV1,
        replacement: AuthorityTrustAnchorV1,
        installed: LoadedAuthorityTrustAnchorV1,
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    impl AnchorFixture {
        fn new() -> Self {
            let home = tempfile::tempdir().unwrap();
            let original = authority_trust_anchor();
            let replacement = replacement_anchor(&original, '5');
            let installed =
                install_authority_trust_anchor_from_home(home.path(), &original).unwrap();
            Self {
                home,
                original,
                replacement,
                installed,
            }
        }

        fn prepare(&self) -> PreparedAuthorityTrustAnchorReplacement {
            PreparedAuthorityTrustAnchorReplacement::prepare(
                self.home.path(),
                &self.original,
                &self.replacement,
            )
            .unwrap()
        }

        fn rebind_to(
            &self,
            replacement: &AuthorityTrustAnchorV1,
        ) -> Result<LoadedAuthorityTrustAnchorV1, String> {
            rebind_authority_trust_anchor_from_home(self.home.path(), &self.original, replacement)
        }

        fn try_load(&self) -> Result<Option<LoadedAuthorityTrustAnchorV1>, String> {
            load_authority_trust_anchor_from_home(self.home.path(), &self.original.repository_id)
        }

        fn load(&self) -> LoadedAuthorityTrustAnchorV1 {
            self.try_load().unwrap().unwrap()
        }

        fn public_race(
            &self,
            replacement: &AuthorityTrustAnchorV1,
            expected_error: &str,
            hook: impl FnOnce() + 'static,
        ) -> String {
            set_authority_rebind_after_prepare_hook(hook);
            expect_error(self.rebind_to(replacement), expected_error)
        }

        fn assert_pin(&self, expected: &AuthorityTrustAnchorV1) {
            assert_eq!(
                fs::read(&self.installed.path).unwrap(),
                anchor_bytes(expected)
            );
        }

        fn assert_clean(&self) {
            assert_no_replacement_temporaries(self.installed.path.parent().unwrap());
        }
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    fn replace_anchor_path(path: &Path, anchor: &AuthorityTrustAnchorV1) {
        use std::os::unix::fs::PermissionsExt;

        let staged = path.with_extension("race");
        fs::write(&staged, anchor_bytes(anchor)).unwrap();
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o600)).unwrap();
        fs::rename(staged, path).unwrap();
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    fn assert_no_replacement_temporaries(directory: &Path) {
        assert!(fs::read_dir(directory).unwrap().flatten().all(|entry| {
            !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".vela-replace-")
        }));
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    fn assert_mode(path: &Path, expected: u32) {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777,
            expected
        );
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    #[test]
    fn authority_trust_anchor_replacement_cleans_rolls_back_and_serializes() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = AnchorFixture::new();
        expect_error(
            fixture.prepare().install_with_hooks(
                || Err("injected failure after temporary creation".to_string()),
                || Ok(()),
                || {},
            ),
            "injected failure",
        );
        fixture.assert_pin(&fixture.original);
        fixture.assert_clean();

        let rollback = AnchorFixture::new();
        expect_error(
            rollback.prepare().install_with_hooks(
                || Ok(()),
                || Ok(()),
                || {
                    fs::set_permissions(
                        &rollback.installed.path,
                        fs::Permissions::from_mode(0o640),
                    )
                    .unwrap();
                },
            ),
            "replacement was rolled back",
        );
        rollback.assert_pin(&rollback.original);
        assert_mode(&rollback.installed.path, 0o600);
        rollback.assert_clean();

        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, mpsc};

        let fixture = AnchorFixture::new();
        let second_anchor = replacement_anchor(&fixture.original, '6');
        let first = fixture.prepare();
        let second = PreparedAuthorityTrustAnchorReplacement::prepare(
            fixture.home.path(),
            &fixture.original,
            &second_anchor,
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
        expect_error(
            second.install_with_hook(|| {
                observed_second_hook.store(true, Ordering::SeqCst);
                Ok(())
            }),
            "authority trust-anchor replacement busy",
        );
        assert!(!second_hook_ran.load(Ordering::SeqCst));

        release_first_tx.send(()).unwrap();
        first_thread.join().unwrap().unwrap();
        fixture.assert_pin(&fixture.replacement);
        fixture.assert_clean();
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    #[test]
    fn public_rebind_rejects_preparation_races() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let equal = AnchorFixture::new();
        let equal_path = equal.installed.path.clone();
        let equal_anchor = equal.original.clone();
        equal.public_race(&equal.original, "preimage does not match", move || {
            replace_anchor_path(&equal_path, &equal_anchor)
        });
        equal.assert_pin(&equal.original);
        equal.assert_clean();

        for raced_anchor in ['5', '6'] {
            let fixture = AnchorFixture::new();
            let raced = replacement_anchor(&fixture.original, raced_anchor);
            let raced_path = fixture.installed.path.clone();
            let hook_anchor = raced.clone();
            fixture.public_race(&fixture.replacement, "preimage does not match", move || {
                replace_anchor_path(&raced_path, &hook_anchor)
            });
            fixture.assert_pin(&raced);
            fixture.assert_clean();
        }

        let mode = AnchorFixture::new();
        let mode_path = mode.installed.path.clone();
        mode.public_race(&mode.replacement, "preimage does not match", move || {
            fs::set_permissions(&mode_path, fs::Permissions::from_mode(0o640)).unwrap()
        });
        assert_mode(&mode.installed.path, 0o640);
        mode.assert_pin(&mode.original);
        mode.assert_clean();

        expect_error(mode.rebind_to(&mode.replacement), "exact mode 0600");

        let symlinked = AnchorFixture::new();
        let sentinel = symlinked.home.path().join("sentinel");
        fs::write(&sentinel, b"sentinel").unwrap();
        let symlink_path = symlinked.installed.path.clone();
        let hook_sentinel = sentinel.clone();
        symlinked.public_race(
            &symlinked.replacement,
            "open pinned repository file",
            move || {
                fs::remove_file(&symlink_path).unwrap();
                symlink(hook_sentinel, symlink_path).unwrap();
            },
        );
        assert_eq!(fs::read(sentinel).unwrap(), b"sentinel");
        symlinked.assert_clean();

        let fifo = AnchorFixture::new();
        let fifo_path = fifo.installed.path.clone();
        fifo.public_race(&fifo.replacement, "regular non-symlink file", move || {
            fs::remove_file(&fifo_path).unwrap();
            assert!(
                std::process::Command::new("/usr/bin/mkfifo")
                    .arg(fifo_path)
                    .status()
                    .unwrap()
                    .success()
            );
        });
        fifo.assert_clean();

        let fixture = AnchorFixture::new();
        let authorities = fixture.home.path().join(".vela/trust/authorities");
        let real_authorities = fixture.home.path().join("real-authorities");
        let sentinel_directory = fixture.home.path().join("sentinel-authorities");
        fs::create_dir(&sentinel_directory).unwrap();
        fs::set_permissions(&sentinel_directory, fs::Permissions::from_mode(0o700)).unwrap();
        let sentinel = sentinel_directory.join(format!("{}.json", fixture.original.repository_id));
        fs::write(&sentinel, b"sentinel").unwrap();
        let error = fixture.public_race(&fixture.replacement, "parent", {
            let authorities = authorities.clone();
            let real_authorities = real_authorities.clone();
            let sentinel_directory = sentinel_directory.clone();
            move || {
                fs::rename(&authorities, real_authorities).unwrap();
                symlink(sentinel_directory, authorities).unwrap();
            }
        });
        assert!(error.contains("changed"), "{error}");
        assert_eq!(fs::read(&sentinel).unwrap(), b"sentinel");
        assert_eq!(
            fs::read(real_authorities.join(fixture.installed.path.file_name().unwrap())).unwrap(),
            anchor_bytes(&fixture.original)
        );
        assert_no_replacement_temporaries(&real_authorities);
    }

    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    #[test]
    fn authority_trust_anchor_public_contract_is_exact() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let current_owner = rustix::process::geteuid().as_raw();
        let wrong_owner = current_owner
            .checked_add(1)
            .unwrap_or_else(|| current_owner.saturating_sub(1));
        expect_error(
            require_private_owner_mode(wrong_owner, 0o600, Path::new("pin.json"), 0o600, "file"),
            "not owned",
        );

        expect_error(
            require_trusted_parent_owner_mode(wrong_owner, 0o600, Path::new(".vela")),
            "not owned",
        );
        let fixture = AnchorFixture::new();
        assert_eq!(fixture.installed.anchor, fixture.original);
        assert_eq!(fixture.installed.root, fixture.original.root().unwrap());
        assert_eq!(
            fixture.installed.path,
            authority_trust_anchor_path(fixture.home.path(), &fixture.original.repository_id)
                .unwrap()
        );
        assert_eq!(fixture.load(), fixture.installed);
        for (path, mode) in [
            (fixture.home.path().join(".vela"), 0o700),
            (fixture.home.path().join(".vela/trust"), 0o700),
            (fixture.home.path().join(".vela/trust/authorities"), 0o700),
            (fixture.installed.path.clone(), 0o600),
        ] {
            assert_mode(&path, mode);
        }
        expect_error(
            install_authority_trust_anchor_from_home(fixture.home.path(), &fixture.replacement),
            "refusing to replace existing authority trust anchor",
        );

        for (case, mode, expected) in [
            ("pin", 0o640, "exact mode 0600"),
            ("trust", 0o750, "exact mode 0700"),
            ("vela", 0o777, "may not be group- or world-writable"),
        ] {
            let fixture = AnchorFixture::new();
            let path = match case {
                "pin" => fixture.installed.path.clone(),
                "trust" => fixture.home.path().join(".vela/trust"),
                _ => fixture.home.path().join(".vela"),
            };
            fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
            expect_error(fixture.try_load(), expected);
        }
        let fixture = AnchorFixture::new();
        let target = fixture.home.path().join("outside-anchor.json");
        fs::rename(&fixture.installed.path, &target).unwrap();
        symlink(&target, &fixture.installed.path).unwrap();

        expect_error(fixture.try_load(), "may not be a symlink");
        let absent = tempfile::tempdir().unwrap();
        let original = authority_trust_anchor();
        expect_error(
            rebind_authority_trust_anchor_from_home(
                absent.path(),
                &original,
                &replacement_anchor(&original, '5'),
            ),
            "requires an existing exact pin",
        );
        assert!(!absent.path().join(".vela").exists());

        let fixture = AnchorFixture::new();
        let before = same_file::Handle::from_path(&fixture.installed.path).unwrap();
        let unchanged = fixture.rebind_to(&fixture.original).unwrap();
        assert_eq!(unchanged.anchor, fixture.original);
        assert_eq!(unchanged, fixture.load());
        assert_eq!(
            before,
            same_file::Handle::from_path(&fixture.installed.path).unwrap()
        );
        fixture.assert_pin(&fixture.original);
        assert_mode(&fixture.installed.path, 0o600);
        fixture.assert_clean();

        let rebound = fixture.rebind_to(&fixture.replacement).unwrap();
        assert_eq!(rebound, fixture.load());
        let wrong_preimage = replacement_anchor(&fixture.original, '6');
        expect_error(
            rebind_authority_trust_anchor_from_home(
                fixture.home.path(),
                &wrong_preimage,
                &fixture.original,
            ),
            "preimage does not match",
        );
        fixture.assert_pin(&fixture.replacement);
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
