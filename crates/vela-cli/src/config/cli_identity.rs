//! Managed identity — the ergonomics layer that hides key files and ids.
//!
//! Signed commands can resolve a key and actor from one local identity.
//!
//! A `vela id create` writes a single profile to `~/.vela/identity.json`:
//! the generated key and actor id. Routing and publication preferences live in
//! `config.toml`, not in identity state.
//!
//! Precedence for every resolver: an explicit flag wins, then a `VELA_*`
//! environment variable, then the stored profile. Nothing is silent: when
//! none resolves, the error names the exact next command to run.

use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use crate::cli::{fail_return, parse_signing_key};

/// The default public hub. Matches the constant baked into the registry
/// commands so an unconfigured user still reaches the live hub.
pub(crate) const DEFAULT_HUB: &str = "https://hub.constellate.science";

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

fn protected_signer_profile_for(identity: &Identity) -> Result<ProtectedSignerProfile, String> {
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
}
