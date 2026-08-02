//! Local custody for bounded producer identities used by exact CLI intake.
//!
//! Signing key: the agent's own session identity, minted automatically
//! at `~/.vela/agents/<actor>/private.key` on first use (the CLI resolves the
//! exact actor before calling this boundary); `VELA_AGENT_KEY_HEX`
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
///    MINTED on first use.
///
/// Custody: minting is refused for anything but `agent:`/`ci:` actors —
/// human authority is never a side effect. The minted key signs only
/// agent-grade objects (leases,
/// records); every decision verb still refuses agent actors outright.
pub fn agent_signing_key(actor: &str) -> Result<SigningKey, String> {
    if let Some(key) = environment_key()? {
        return Ok(key);
    }
    if !actor.starts_with("agent:") && !actor.starts_with("ci:") && !actor.starts_with("verifier:")
    {
        return Err(format!(
            "agent key auto-mint is for agent:/ci:/verifier: actors, not '{actor}' — human authority uses the authenticated platform principal"
        ));
    }
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let base = std::path::PathBuf::from(home).join(".vela/agents");
    mint_or_load_agent_key(&base, actor)
}

/// Load the already-established identity for an exact producer.
///
/// Lifecycle actions such as Proposal withdrawal must prove continuity with
/// the retained Submission. They therefore fail if its key is unavailable
/// instead of silently minting a replacement that cannot verify.
pub fn existing_agent_signing_key(actor: &str) -> Result<SigningKey, String> {
    if let Some(key) = environment_key()? {
        return Ok(key);
    }
    validate_agent_actor(actor)?;
    let home = std::env::var("HOME").map_err(|_| "HOME unset".to_string())?;
    let key_path = actor_key_path(&std::path::PathBuf::from(home).join(".vela/agents"), actor);
    if !key_path.is_file() {
        return Err(format!(
            "producer identity key is unavailable at {}; withdrawal requires the exact key that signed the Submission",
            key_path.display()
        ));
    }
    load_agent_key(&key_path)
}

fn environment_key() -> Result<Option<SigningKey>, String> {
    let Ok(hex_str) = std::env::var(AGENT_KEY_ENV) else {
        return Ok(None);
    };
    let bytes = hex::decode(hex_str.trim()).map_err(|e| format!("decode {AGENT_KEY_ENV}: {e}"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("{AGENT_KEY_ENV} must be 32 hex bytes"))?;
    Ok(Some(SigningKey::from_bytes(&arr)))
}

fn validate_agent_actor(actor: &str) -> Result<(), String> {
    if actor.starts_with("agent:") || actor.starts_with("ci:") || actor.starts_with("verifier:") {
        Ok(())
    } else {
        Err(format!(
            "agent key custody is for agent:/ci:/verifier: actors, not '{actor}' — human authority uses the authenticated platform principal"
        ))
    }
}

fn actor_key_path(base: &Path, actor: &str) -> std::path::PathBuf {
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
    base.join(safe).join("private.key")
}

fn load_agent_key(key_path: &Path) -> Result<SigningKey, String> {
    let hex_str = std::fs::read_to_string(key_path)
        .map_err(|e| format!("read {}: {e}", key_path.display()))?;
    let bytes =
        hex::decode(hex_str.trim()).map_err(|e| format!("decode {}: {e}", key_path.display()))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("{} must be 32 hex bytes", key_path.display()))?;
    Ok(SigningKey::from_bytes(&arr))
}

/// The mint itself, factored for tests: `<base>/<sanitized-actor>/private.key`
/// (hex seed, 0600), created once and reused for the actor's lifetime on
/// this machine.
fn mint_or_load_agent_key(base: &Path, actor: &str) -> Result<SigningKey, String> {
    let key_path = actor_key_path(base, actor);
    let dir = key_path
        .parent()
        .ok_or_else(|| "agent key path has no parent".to_string())?;
    if key_path.exists() {
        return load_agent_key(&key_path);
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let mut seed = [0u8; 32];
    use rand_core::RngCore;
    rand_core::OsRng.fill_bytes(&mut seed);
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
