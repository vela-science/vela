//! Binary integrity pin: the clear-signing invariant's first gap,
//! closed. The sign ceremonies render what you are about to sign —
//! but the renderer is a binary on a machine an agent can write to.
//! Pinning records the binary's sha256 under a HUMAN act
//! (`vela id pin-binary`, confirm-gated); every ceremony start
//! re-hashes the running binary and refuses on mismatch.
//!
//! Threat model, stated plainly: this raises the bar against an agent
//! swapping the `vela` binary between your deliberate upgrade and your
//! next ceremony. It cannot stop an actor that can rewrite the pin
//! file itself — keep `~/.vela/binary-pin.json` outside agent-writable
//! sandboxes and consider `chmod 400`. The hardware story
//! (`ed25519-sk`, docs/THREAT_MODEL.md) is the full answer.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct BinaryPin {
    pub schema: String,
    pub sha256: String,
    pub binary_path: String,
    pub pinned_at: String,
    pub version: String,
}

fn pin_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".vela").join("binary-pin.json"))
}

fn current_binary_sha() -> Result<(PathBuf, String), String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let bytes = std::fs::read(&exe).map_err(|e| format!("read {}: {e}", exe.display()))?;
    Ok((exe, hex::encode(Sha256::digest(&bytes))))
}

/// The human act: record the running binary's hash. Confirm-gated at
/// the call site (cli_admin); this just writes.
pub(crate) fn record_pin() -> Result<BinaryPin, String> {
    let (exe, sha) = current_binary_sha()?;
    let pin = BinaryPin {
        schema: "vela.binary-pin.v0.1".to_string(),
        sha256: sha,
        binary_path: exe.display().to_string(),
        pinned_at: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let path = pin_path().ok_or("no HOME")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
    let body = format!(
        "{}\n",
        serde_json::to_string_pretty(&pin).map_err(|e| e.to_string())?
    );
    std::fs::write(&temporary, body).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    std::fs::rename(&temporary, &path).map_err(|e| e.to_string())?;
    Ok(pin)
}

pub(crate) fn load_pin() -> Option<BinaryPin> {
    let path = pin_path()?;
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Whether `path` looks like a cargo build artifact (`…/target/debug/…` or
/// `…/target/release/…`) rather than an installed release. A dev build's hash
/// changes on every `cargo build`, so pinning it guarantees the next ceremony
/// mismatches — the footgun the `vela` → `scripts/vela` wrapper makes easy to
/// hit. Callers warn when this is true.
pub(crate) fn is_dev_build_path(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/target/debug/") || s.contains("/target/release/")
}

/// The comparison the ceremony reasons over, so it can render old -> new and
/// offer an inline re-pin instead of aborting to a separate command.
pub(crate) enum PinState {
    /// No pin recorded — ceremonies run unpinned (opt-in).
    Unpinned,
    /// The running binary matches the pin.
    Match(BinaryPin),
    /// The running binary changed since the pin was set.
    Mismatch {
        pinned: BinaryPin,
        current_sha: String,
        current_version: String,
        current_path: PathBuf,
    },
}

/// Classify the running binary against the pin. Never prompts, never writes.
pub(crate) fn pin_state() -> Result<PinState, String> {
    let (exe, sha) = current_binary_sha()?;
    let Some(pin) = load_pin() else {
        return Ok(PinState::Unpinned);
    };
    if sha == pin.sha256 {
        Ok(PinState::Match(pin))
    } else {
        Ok(PinState::Mismatch {
            pinned: pin,
            current_sha: sha,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            current_path: exe,
        })
    }
}

/// Ceremony gate for the SCRIPTED (non-interactive) forms: Ok(None) = no pin
/// (allowed, noted); Ok(Some) = verified; Err = MISMATCH -> refuse. A script
/// cannot vouch for a new binary, so a mismatch is always fatal here; the
/// interactive ceremony ([`PinState`]) offers an inline re-pin instead.
pub(crate) fn verify_for_ceremony() -> Result<Option<BinaryPin>, String> {
    match pin_state()? {
        PinState::Unpinned => Ok(None),
        PinState::Match(pin) => Ok(Some(pin)),
        PinState::Mismatch {
            pinned,
            current_sha,
            current_path,
            ..
        } => Err(format!(
            "the running binary does not match your pin: {} now, {} pinned ({} at {}). If you \
             upgraded deliberately, re-run `vela sign` interactively to re-pin in place; if you \
             did not, do NOT sign — inspect {} first.",
            &current_sha[..16],
            &pinned.sha256[..16],
            pinned.version,
            pinned.pinned_at,
            current_path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_binary_hashes() {
        let (_, sha) = current_binary_sha().expect("hash self");
        assert_eq!(sha.len(), 64);
    }

    #[test]
    fn dev_build_paths_are_flagged() {
        use std::path::Path;
        // Build-tree binaries: hash churns on every `cargo build`.
        assert!(is_dev_build_path(Path::new(
            "/Users/x/personal/vela/vendor/vela/target/debug/vela"
        )));
        assert!(is_dev_build_path(Path::new("/repo/target/release/vela")));
        // Installed releases: stable to pin.
        assert!(!is_dev_build_path(Path::new("/Users/x/.cargo/bin/vela")));
        assert!(!is_dev_build_path(Path::new("/usr/local/bin/vela")));
    }
}
