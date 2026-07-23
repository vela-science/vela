//! Managed identity — the ergonomics layer that hides key files and ids.
//!
//! Signed commands can resolve a key and actor from one local identity.
//!
//! A `vela id create` writes a single profile to `~/.vela/identity.json`:
//! the generated key and actor id. User preferences live in
//! `~/.vela/config.toml`, not in identity state.
//!
//! Precedence for every resolver: an explicit flag wins, then a `VELA_*`
//! environment variable, then the stored profile. Nothing is silent: when
//! none resolves, the error names the exact next command to run.

use std::io::Read;
use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use crate::cli::{fail_return, parse_signing_key};

/// One stored identity. Written to `~/.vela/identity.json`. The private
/// key itself lives in its own file (`key_path`), never inline here, so
/// this file is safe to read for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Identity {
    /// Schema tag for forward migration.
    #[serde(default = "default_version")]
    pub version: String,
    /// The actor id used as reviewer / owner / proposer, e.g.
    /// `reviewer:alice` or `agent:my-bot`.
    pub actor_id: String,
    /// `human` or `agent` (classified from `actor_id` at create time).
    #[serde(default = "default_actor_type")]
    pub actor_type: String,
    /// Absolute path to the Ed25519 private key (hex seed).
    pub key_path: String,
    /// Hex-encoded Ed25519 public key. `vela actor add` verifies this against
    /// the configured private key during one-time empty-registry bootstrap.
    pub pubkey: String,
    /// v2 signer backend. Absence means the v1 `key_path` file backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer: Option<IdentitySigner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum IdentitySigner {
    File {
        key_path: String,
    },
    Helper {
        provider: String,
        key_id: String,
        public_key: String,
        protection_grade: String,
        mode: String,
        helper_sha256: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pending_source_removal: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pending_vela_binary_sha256: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ProtectedSignerProfile {
    pub(crate) provider: String,
    pub(crate) protection_grade: String,
    pub(crate) mode: vela_signer::ProtectionMode,
    pub(crate) helper_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SignerHealth {
    pub(crate) state: &'static str,
    pub(crate) platform: &'static str,
    pub(crate) integration: &'static str,
    pub(crate) integration_ready: bool,
    pub(crate) binary_version: &'static str,
    pub(crate) binary_path: String,
    pub(crate) binary_sha256: Option<String>,
    pub(crate) binary_pin_state: &'static str,
    pub(crate) pinned_binary_sha256: Option<String>,
    pub(crate) helper_path: Option<String>,
    pub(crate) helper_sha256: Option<String>,
    pub(crate) pinned_helper_sha256: Option<String>,
    pub(crate) next_action: Option<String>,
}

fn default_version() -> String {
    "1.0".to_string()
}
fn default_actor_type() -> String {
    "human".to_string()
}

/// `~/.vela` — the per-user Vela home.
pub(crate) fn vela_home() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".vela")
}

/// `~/.vela/identity.json`.
pub(crate) fn identity_path() -> PathBuf {
    vela_home().join("identity.json")
}

/// Load the stored identity, if any. `None` when the file is absent or
/// unreadable (treated as "not set up yet", not an error).
pub(crate) fn load_identity() -> Option<Identity> {
    let path = identity_path();
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Load security-sensitive identity metadata from the operating-system
/// account home, never from `HOME`. Administration commands use this reader
/// so an injected environment or symlink cannot substitute an identity file.
pub(crate) fn load_administrative_identity() -> Result<Identity, String> {
    let home =
        crate::frontier_txn::operating_system_account_home().map_err(|error| error.to_string())?;
    load_administrative_identity_from_home(&home)
}

fn load_administrative_identity_from_home(home: &Path) -> Result<Identity, String> {
    let vela_directory = home.join(".vela");
    let directory_metadata = std::fs::symlink_metadata(&vela_directory).map_err(|error| {
        format!(
            "inspect administrative identity directory '{}': {error}",
            vela_directory.display()
        )
    })?;
    if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
        return Err("administrative identity directory must be a real directory".to_string());
    }

    let path = vela_directory.join("identity.json");
    let linked = std::fs::symlink_metadata(&path).map_err(|error| {
        format!(
            "inspect administrative identity '{}': {error}",
            path.display()
        )
    })?;
    if linked.file_type().is_symlink() || !linked.is_file() {
        return Err("administrative identity must be a regular non-symlink file".to_string());
    }
    reject_insecure_administrative_identity(&path, &linked)?;

    let inspected = same_file::Handle::from_path(&path)
        .map_err(|error| format!("identify administrative identity: {error}"))?;
    let mut file = std::fs::File::open(&path)
        .map_err(|error| format!("open administrative identity: {error}"))?;
    let opened = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|error| format!("clone administrative identity descriptor: {error}"))?,
    )
    .map_err(|error| format!("identify open administrative identity: {error}"))?;
    if inspected != opened {
        return Err("administrative identity changed while it was opened".to_string());
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read administrative identity: {error}"))?;
    if bytes.len() > 64 * 1024 {
        return Err("administrative identity is unexpectedly large".to_string());
    }
    let final_link = std::fs::symlink_metadata(&path)
        .map_err(|error| format!("reinspect administrative identity: {error}"))?;
    let final_identity = same_file::Handle::from_path(&path)
        .map_err(|error| format!("reidentify administrative identity: {error}"))?;
    if final_link.file_type().is_symlink() || !final_link.is_file() || opened != final_identity {
        return Err("administrative identity changed while it was read".to_string());
    }
    reject_insecure_administrative_identity(&path, &final_link)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "parse administrative identity '{}': {error}",
            path.display()
        )
    })
}

#[cfg(unix)]
fn reject_insecure_administrative_identity(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let owner = rustix::process::geteuid().as_raw();
    if metadata.uid() != owner {
        return Err(format!(
            "administrative identity '{}' is not owned by the current operating-system account",
            path.display()
        ));
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(format!(
            "administrative identity '{}' must not be accessible by group or others",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_insecure_administrative_identity(
    _path: &Path,
    _metadata: &std::fs::Metadata,
) -> Result<(), String> {
    Ok(())
}

/// Persist an identity, creating `~/.vela` if needed.
pub(crate) fn save_identity(identity: &Identity) -> Result<(), String> {
    let dir = vela_home();
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = identity_path();
    let json =
        serde_json::to_string_pretty(identity).map_err(|e| format!("serialize identity: {e}"))?;
    let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
    std::fs::write(&temporary, format!("{json}\n"))
        .map_err(|e| format!("write {}: {e}", temporary.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("protect {}: {e}", temporary.display()))?;
    }
    std::fs::rename(&temporary, &path).map_err(|e| format!("install {}: {e}", path.display()))
}

// ── Resolvers: flag > VELA_* env > profile > error-with-hint ──────────

const SETUP_HINT: &str = "no identity configured — run `vela id create --handle <your-name>` once \
     (generates a key, stores it, prints the line a maintainer runs to register you)";

/// Resolve the actor id. `--actor` / `--reviewer` / `--owner` flag wins,
/// then `$VELA_ACTOR_ID`, then the stored profile.
pub(crate) fn resolve_actor(flag: Option<&str>) -> String {
    if let Some(a) = flag.filter(|s| !s.trim().is_empty()) {
        return a.to_string();
    }
    if let Ok(a) = std::env::var("VELA_ACTOR_ID")
        && !a.trim().is_empty()
    {
        return a;
    }
    match load_identity() {
        Some(id) => id.actor_id,
        None => fail_return(SETUP_HINT),
    }
}

/// Resolve the acting identity for a DECISION verb (accept / reject /
/// sign) and refuse agent-lane actors with the custody exit code before
/// the engine is even reached. The engine remains the authority — this
/// pre-check only types the refusal (exit 4 instead of a generic 1).
pub(crate) fn resolve_decision_actor(flag: Option<&str>) -> String {
    let actor = resolve_actor(flag);
    if actor.starts_with("agent:") || actor.starts_with("ci:") {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Custody,
            &format!(
                "`{actor}` cannot decide: committing truth-bearing state is a key-custody human act"
            ),
            Some(
                "a human runs this under their own identity (`vela id show`); agents land drafts with `vela land`",
            ),
        );
    }
    actor
}

/// Resolve a signing key path. `--key` flag wins, then `$VELA_KEY_PATH`,
/// then the profile's `key_path`.
pub(crate) fn resolve_key_path(flag: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = flag {
        return Some(p.to_path_buf());
    }
    if let Ok(p) = std::env::var("VELA_KEY_PATH")
        && !p.trim().is_empty()
    {
        return Some(PathBuf::from(p));
    }
    load_identity().and_then(|id| match id.signer {
        Some(IdentitySigner::File { key_path }) => Some(PathBuf::from(key_path)),
        Some(IdentitySigner::Helper { .. }) => None,
        None => Some(PathBuf::from(id.key_path)),
    })
}

pub(crate) fn protected_signer_profile() -> Result<ProtectedSignerProfile, String> {
    let identity = load_identity().ok_or_else(|| SETUP_HINT.to_string())?;
    protected_signer_profile_for(&identity)
}

pub(crate) fn signer_health(identity: &Identity) -> SignerHealth {
    let platform = std::env::consts::OS;
    let (integration, integration_ready, integration_repair) =
        platform_signer_integration(platform);
    let binary = std::env::current_exe().ok();
    let binary_sha256 = binary
        .as_deref()
        .and_then(|path| vela_signer::contract::file_sha256(path).ok());
    let helper = binary
        .as_deref()
        .and_then(|path| signer_helper_path(path).ok());
    let helper_sha256 = helper
        .as_deref()
        .and_then(|path| vela_signer::contract::file_sha256(path).ok());
    let (binary_pin_state, pinned_binary_sha256, binary_pin_ready) =
        match crate::config::binary_pin::pin_state() {
            Ok(crate::config::binary_pin::PinState::Match(pin)) => {
                ("match", Some(format!("sha256:{}", pin.sha256)), true)
            }
            Ok(crate::config::binary_pin::PinState::Mismatch { pinned, .. }) => {
                ("mismatch", Some(format!("sha256:{}", pinned.sha256)), false)
            }
            Ok(crate::config::binary_pin::PinState::Unpinned) => ("unpinned", None, false),
            Err(_) => ("unavailable", None, false),
        };
    let (pinned_helper_sha256, incomplete) = match &identity.signer {
        Some(IdentitySigner::Helper {
            helper_sha256,
            pending_source_removal,
            pending_vela_binary_sha256,
            ..
        }) => (
            Some(helper_sha256.clone()),
            pending_source_removal.is_some() || pending_vela_binary_sha256.is_some(),
        ),
        _ => (None, false),
    };
    let protected = pinned_helper_sha256.is_some();
    let (state, next_action) = if incomplete {
        (
            "incomplete",
            Some(
                "vela id protect --user-presence --remove-source-key --mode session --json"
                    .to_string(),
            ),
        )
    } else if protected && !integration_ready {
        ("missing_integration", integration_repair)
    } else if let Some(pinned) = pinned_helper_sha256.as_ref() {
        if protected_backend_ready(helper_sha256.as_deref(), pinned, binary_pin_ready) {
            ("ready", None)
        } else {
            (
                "stale",
                Some(
                    "vela id protect --user-presence --remove-source-key --mode session --json"
                        .to_string(),
                ),
            )
        }
    } else {
        (
            "file_key",
            (identity.actor_type == "human").then(|| {
                "vela id protect --user-presence --remove-source-key --mode session --json"
                    .to_string()
            }),
        )
    };
    SignerHealth {
        state,
        platform,
        integration,
        integration_ready,
        binary_version: env!("CARGO_PKG_VERSION"),
        binary_path: binary
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "unavailable".to_string()),
        binary_sha256,
        binary_pin_state,
        pinned_binary_sha256,
        helper_path: helper.map(|path| path.display().to_string()),
        helper_sha256,
        pinned_helper_sha256,
        next_action,
    }
}

fn protected_backend_ready(
    observed_helper_sha256: Option<&str>,
    pinned_helper_sha256: &str,
    binary_pin_ready: bool,
) -> bool {
    observed_helper_sha256 == Some(pinned_helper_sha256) && binary_pin_ready
}

fn platform_signer_integration(platform: &str) -> (&'static str, bool, Option<String>) {
    match platform {
        "macos" => ("Keychain + LocalAuthentication", true, None),
        "windows" => ("Credential Manager + Windows Hello", true, None),
        "linux" => {
            let pkcheck = std::env::var_os("PATH").is_some_and(|path| {
                std::env::split_paths(&path).any(|directory| directory.join("pkcheck").is_file())
            });
            let policy = [
                "/usr/share/polkit-1/actions/science.vela.signer.policy",
                "/usr/local/share/polkit-1/actions/science.vela.signer.policy",
            ]
            .iter()
            .any(|path| Path::new(path).is_file());
            (
                "Secret Service + polkit",
                pkcheck && policy,
                (!(pkcheck && policy)).then(|| {
                    "re-run the provenance-verified Vela installer to install pkcheck support and science.vela.signer.policy"
                        .to_string()
                }),
            )
        }
        _ => (
            "file-key compatibility only",
            false,
            Some("protected human signing is unsupported on this platform".to_string()),
        ),
    }
}

pub(crate) fn protected_signer_profile_for(
    identity: &Identity,
) -> Result<ProtectedSignerProfile, String> {
    let expected_key_id = format!("{}:{}", identity.actor_id, identity.pubkey);
    match &identity.signer {
        Some(IdentitySigner::Helper {
            provider,
            key_id,
            public_key,
            protection_grade,
            mode,
            helper_sha256,
            pending_source_removal: None,
            ..
        }) => {
            if identity.actor_type != "human"
                || identity.actor_id.starts_with("agent:")
                || identity.actor_id.starts_with("ci:")
                || public_key != &identity.pubkey
                || key_id != &expected_key_id
            {
                return Err("protected identity actor/key binding is invalid".to_string());
            }
            if provider != "os_store" {
                return Err(format!(
                    "unsupported protected signer provider '{provider}'"
                ));
            }
            if !matches!(
                protection_grade.as_str(),
                "user_session" | "app_isolated" | "external_confirmed" | "hardware_nonexportable"
            ) {
                return Err(format!(
                    "unsupported protected signer grade '{protection_grade}'"
                ));
            }
            if helper_sha256.len() != 71
                || !helper_sha256.starts_with("sha256:")
                || !helper_sha256[7..]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err("protected signer helper digest is invalid".to_string());
            }
            Ok(ProtectedSignerProfile {
                provider: provider.clone(),
                protection_grade: protection_grade.clone(),
                mode: parse_protection_mode(mode)?,
                helper_sha256: helper_sha256.clone(),
            })
        }
        Some(IdentitySigner::Helper {
            pending_source_removal: Some(path),
            ..
        }) => Err(format!(
            "protected identity migration is incomplete; remove the plaintext source {path} by rerunning `vela id protect --user-presence --remove-source-key`"
        )),
        _ => Err(
            "this decision requires a user-presence protected identity; run `vela id protect --user-presence --remove-source-key`"
                .to_string(),
        ),
    }
}

fn parse_protection_mode(value: &str) -> Result<vela_signer::ProtectionMode, String> {
    match value {
        "session" => Ok(vela_signer::ProtectionMode::Session),
        "always" => Ok(vela_signer::ProtectionMode::Always),
        _ => Err(format!("unsupported protected signer mode '{value}'")),
    }
}

pub(crate) fn signer_helper_path(vela_binary: &Path) -> Result<PathBuf, String> {
    let directory = vela_binary
        .parent()
        .ok_or_else(|| "running Vela binary has no parent directory".to_string())?;
    #[cfg(target_os = "windows")]
    let helper = directory.join("vela-signer.exe");
    #[cfg(not(target_os = "windows"))]
    let helper = directory.join("vela-signer");
    if !helper.is_file() {
        return Err(format!(
            "pinned signer helper is missing at {}; reinstall the complete Vela package",
            helper.display()
        ));
    }
    Ok(helper)
}

/// Resolve and load the signing key, exiting with a setup hint when none
/// is configured. Use for commands where a key is mandatory.
pub(crate) fn resolve_signing_key(flag: Option<&Path>) -> SigningKey {
    match resolve_signing_key_opt(flag) {
        Some(key) => key,
        None => fail_return(SETUP_HINT),
    }
}

/// Resolve the signing key if one is configured, else `None`. Callers may use
/// this only for process writes whose protocol boundary explicitly permits an
/// unsigned record; truth-bearing decisions have no keyless bootstrap.
pub(crate) fn resolve_signing_key_opt(flag: Option<&Path>) -> Option<SigningKey> {
    let path = resolve_key_path(flag)?;
    let hex = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| fail_return(&format!("read key {}: {e}", path.display())));
    Some(parse_signing_key(hex.trim()))
}

/// Resolve co-authorship provenance for a signed write: the non-human (AI / CI)
/// that drafted or assisted. This is the GitHub `Co-authored-by` pattern made
/// automatic. An agent harness exports `VELA_CO_AUTHOR` (an `agent:`/`ci:` id)
/// and optionally `VELA_GENERATED_BY` (a free-text model string); every signed
/// write then records the AI as a contribution while the human reviewer stays
/// the accountable signer. Same precedence as the rest: an explicit flag wins,
/// then the env var. Returns `None` when neither is set, so the event stays
/// byte-identical to the pre-redesign shape.
pub(crate) fn resolve_co_author_provenance(
    co_author: Option<&str>,
    generated_by: Option<&str>,
) -> Option<vela_protocol::provenance::Provenance> {
    let id = co_author
        .map(str::to_string)
        .or_else(|| std::env::var("VELA_CO_AUTHOR").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let generated_by = generated_by
        .map(str::to_string)
        .or_else(|| std::env::var("VELA_GENERATED_BY").ok())
        .unwrap_or_default();
    Some(vela_protocol::provenance::Provenance {
        machine_contributions: vec![vela_protocol::provenance::MachineContribution {
            id,
            class: String::new(),
            role: "drafted".to_string(),
            tool: String::new(),
            generated_by,
            authority: "none".to_string(),
        }],
        ..Default::default()
    })
}

#[cfg(test)]
mod protected_profile_tests {
    use super::*;

    fn write_private_administrative_identity(home: &Path, value: &Identity) -> PathBuf {
        let directory = home.join(".vela");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("identity.json");
        std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        path
    }

    fn identity() -> Identity {
        let public_key = "4".repeat(64);
        Identity {
            version: "2.0".to_string(),
            actor_id: "reviewer:test".to_string(),
            actor_type: "human".to_string(),
            key_path: String::new(),
            pubkey: public_key.clone(),
            signer: Some(IdentitySigner::Helper {
                provider: "os_store".to_string(),
                key_id: format!("reviewer:test:{public_key}"),
                public_key,
                protection_grade: "user_session".to_string(),
                mode: "session".to_string(),
                helper_sha256: format!("sha256:{}", "a".repeat(64)),
                pending_source_removal: None,
                pending_vela_binary_sha256: None,
            }),
        }
    }

    #[test]
    fn protected_profile_requires_exact_actor_key_and_helper_bindings() {
        let profile = protected_signer_profile_for(&identity()).unwrap();
        assert_eq!(profile.provider, "os_store");
        assert_eq!(profile.mode, vela_signer::ProtectionMode::Session);

        let mut wrong_key_id = identity();
        if let Some(IdentitySigner::Helper { key_id, .. }) = &mut wrong_key_id.signer {
            *key_id = "reviewer:other:deadbeef".to_string();
        }
        assert!(protected_signer_profile_for(&wrong_key_id).is_err());

        let mut wrong_public_key = identity();
        if let Some(IdentitySigner::Helper { public_key, .. }) = &mut wrong_public_key.signer {
            *public_key = "5".repeat(64);
        }
        assert!(protected_signer_profile_for(&wrong_public_key).is_err());

        let mut wrong_helper = identity();
        if let Some(IdentitySigner::Helper { helper_sha256, .. }) = &mut wrong_helper.signer {
            *helper_sha256 = format!("sha256:{}", "A".repeat(64));
        }
        assert!(protected_signer_profile_for(&wrong_helper).is_err());
    }

    #[test]
    fn unsupported_platform_never_claims_protected_signer_readiness() {
        let (integration, ready, repair) = platform_signer_integration("unsupported-test-os");
        assert_eq!(integration, "file-key compatibility only");
        assert!(!ready);
        assert!(repair.unwrap().contains("unsupported"));
    }

    #[test]
    fn protected_backend_requires_both_helper_and_binary_pins() {
        let helper = format!("sha256:{}", "a".repeat(64));
        assert!(protected_backend_ready(Some(&helper), &helper, true));
        assert!(!protected_backend_ready(Some(&helper), &helper, false));
        assert!(!protected_backend_ready(None, &helper, true));
    }

    #[test]
    fn protected_profile_rejects_agents_unknown_modes_and_incomplete_migrations() {
        let mut agent = identity();
        agent.actor_id = "agent:test".to_string();
        agent.actor_type = "agent".to_string();
        assert!(protected_signer_profile_for(&agent).is_err());

        let mut wrong_mode = identity();
        if let Some(IdentitySigner::Helper { mode, .. }) = &mut wrong_mode.signer {
            *mode = "forever".to_string();
        }
        assert!(protected_signer_profile_for(&wrong_mode).is_err());

        let mut incomplete = identity();
        if let Some(IdentitySigner::Helper {
            pending_source_removal,
            ..
        }) = &mut incomplete.signer
        {
            *pending_source_removal = Some("/tmp/plaintext.key".to_string());
        }
        let error = protected_signer_profile_for(&incomplete).unwrap_err();
        assert!(error.contains("migration is incomplete"));
    }

    #[test]
    fn administrative_identity_reader_accepts_only_private_regular_files() {
        let home = tempfile::tempdir().unwrap();
        let path = write_private_administrative_identity(home.path(), &identity());
        assert_eq!(
            load_administrative_identity_from_home(home.path())
                .unwrap()
                .actor_id,
            "reviewer:test"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt, symlink};

            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            let error = load_administrative_identity_from_home(home.path()).unwrap_err();
            assert!(error.contains("group or others"), "{error}");

            let external = tempfile::tempdir().unwrap();
            let target = external.path().join("identity.json");
            std::fs::copy(&path, &target).unwrap();
            std::fs::remove_file(&path).unwrap();
            symlink(&target, &path).unwrap();
            let error = load_administrative_identity_from_home(home.path()).unwrap_err();
            assert!(error.contains("regular non-symlink"), "{error}");
        }
    }
}
