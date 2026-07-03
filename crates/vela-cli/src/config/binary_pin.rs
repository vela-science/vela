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
    std::fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&pin).map_err(|e| e.to_string())?
        ),
    )
    .map_err(|e| e.to_string())?;
    Ok(pin)
}

pub(crate) fn load_pin() -> Option<BinaryPin> {
    let path = pin_path()?;
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Ceremony gate: Ok(None) = no pin recorded (allowed, but the
/// ceremony says so); Ok(Some(pin)) = verified; Err = MISMATCH, the
/// ceremony must refuse.
pub(crate) fn verify_for_ceremony() -> Result<Option<BinaryPin>, String> {
    let Some(pin) = load_pin() else {
        return Ok(None);
    };
    let (exe, sha) = current_binary_sha()?;
    if sha != pin.sha256 {
        return Err(format!(
            "the running binary does not match your pin: {} now, {} pinned ({} at {}). If you \
             upgraded deliberately, re-pin with `vela id pin-binary`; if you did not, do NOT \
             sign — inspect {} first.",
            &sha[..16],
            &pin.sha256[..16],
            pin.version,
            pin.pinned_at,
            exe.display()
        ));
    }
    Ok(Some(pin))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_binary_hashes() {
        let (_, sha) = current_binary_sha().expect("hash self");
        assert_eq!(sha.len(), 64);
    }
}
