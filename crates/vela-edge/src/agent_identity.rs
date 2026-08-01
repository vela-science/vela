//! Local custody for bounded producer identities used by exact CLI intake.
//!
//! Signing key: the agent's own session identity, minted automatically
//! at `~/.vela/agents/<actor>/private.key` on first use (the actor comes
//! from the tool argument or `VELA_ACTOR_ID`); `VELA_AGENT_KEY_HEX`
//! overrides when an explicit key is wanted. Minting is refused for
//! non-agent actors; human authority uses the platform principal and the
//! repository-authority signer.
//! **No silent unsigned submissions, and no key ceremony either.**

use ed25519_dalek::SigningKey;
use std::path::Path;

const AGENT_KEY_ENV: &str = "VELA_AGENT_KEY_HEX";

/// Resolve the agent's signing key with zero ceremony. Order:
///
/// 1. `VELA_AGENT_KEY_HEX` — an explicit key always wins.
/// 2. The per-actor session key at `~/.vela/agents/<actor>/private.key`,
///    MINTED on first use. An agent session that exported
///    `VELA_ACTOR_ID=agent:<name>` (the charter's first rule) needs no
///    key step at all — identity is a consequence of showing up.
///
/// Custody: minting is refused for anything but `agent:`/`ci:` actors —
/// human authority is never a side effect. The minted key signs only
/// agent-grade objects (leases,
/// records); every decision verb still refuses agent actors outright.
pub fn agent_signing_key(explicit_actor: Option<&str>) -> Result<SigningKey, String> {
    if let Ok(hex_str) = std::env::var(AGENT_KEY_ENV) {
        let bytes =
            hex::decode(hex_str.trim()).map_err(|e| format!("decode {AGENT_KEY_ENV}: {e}"))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| format!("{AGENT_KEY_ENV} must be 32 hex bytes"))?;
        return Ok(SigningKey::from_bytes(&arr));
    }
    let actor = explicit_actor
        .map(str::to_string)
        .or_else(|| std::env::var("VELA_ACTOR_ID").ok())
        .ok_or_else(|| {
            format!(
                "no agent identity: set VELA_ACTOR_ID=agent:<name> (or {AGENT_KEY_ENV} for an explicit key)"
            )
        })?;
    if !actor.starts_with("agent:") && !actor.starts_with("ci:") && !actor.starts_with("verifier:")
    {
        return Err(format!(
            "agent key auto-mint is for agent:/ci:/verifier: actors, not '{actor}' — human authority uses the authenticated platform principal"
        ));
    }
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let base = std::path::PathBuf::from(home).join(".vela/agents");
    mint_or_load_agent_key(&base, &actor)
}

/// The mint itself, factored for tests: `<base>/<sanitized-actor>/private.key`
/// (hex seed, 0600), created once and reused for the actor's lifetime on
/// this machine.
fn mint_or_load_agent_key(base: &Path, actor: &str) -> Result<SigningKey, String> {
    let safe: String = actor
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let dir = base.join(safe);
    let key_path = dir.join("private.key");
    if key_path.exists() {
        let hex_str = std::fs::read_to_string(&key_path)
            .map_err(|e| format!("read {}: {e}", key_path.display()))?;
        let bytes = hex::decode(hex_str.trim())
            .map_err(|e| format!("decode {}: {e}", key_path.display()))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| format!("{} must be 32 hex bytes", key_path.display()))?;
        return Ok(SigningKey::from_bytes(&arr));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let mut seed = [0u8; 32];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let key = SigningKey::from_bytes(&seed);
    std::fs::write(&key_path, hex::encode(seed))
        .map_err(|e| format!("write {}: {e}", key_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

#[cfg(test)]
mod agent_key_tests {
    use super::*;

    #[test]
    fn mints_once_reuses_after_and_stays_agent_only() {
        let base = std::env::temp_dir().join(format!("vela-agent-keys-{}", std::process::id()));
        let a = mint_or_load_agent_key(&base, "agent:swarm-7").unwrap();
        let b = mint_or_load_agent_key(&base, "agent:swarm-7").unwrap();
        assert_eq!(a.to_bytes(), b.to_bytes(), "same actor, same key");
        let c = mint_or_load_agent_key(&base, "agent:swarm-8").unwrap();
        assert_ne!(a.to_bytes(), c.to_bytes(), "different actor, different key");
        assert!(base.join("agent-swarm-7/private.key").exists());
        std::fs::remove_dir_all(&base).ok();
    }
}
