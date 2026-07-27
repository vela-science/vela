//! Managed producer identity.
//!
//! `vela id create --agent` writes one file-backed agent profile. Human
//! authority uses an authenticated principal and repository authority, not a
//! Vela identity or personal signing key.
//!
//! Precedence for every resolver: an explicit flag wins, then a `VELA_*`
//! environment variable, then the stored profile. Nothing is silent: when
//! none resolves, the error names the exact next command to run.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cli::fail_return;

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
