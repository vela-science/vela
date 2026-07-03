//! Identity and signing helpers: `vela id` keygen, the re-sign engine
//! (`cmd_id_sign`, now driven by the `vela sign` hygiene lane), and
//! signing-key parsing. Moved verbatim from `cli/mod.rs`.

use super::*;

/// Shared success print for `vela id create` / `vela id import`: shows the
/// identity and the single line a maintainer runs to register it, so the
/// onboarding handoff is one copy-paste.
pub(crate) fn print_identity_created(identity: &crate::cli_identity::Identity, json: bool) {
    if json {
        print_json(&json!({
            "ok": true,
            "command": "id.create",
            "actor_id": identity.actor_id,
            "actor_type": identity.actor_type,
            "pubkey": identity.pubkey,
            "key_path": identity.key_path,
            "hub_url": identity.hub_url,
        }));
        return;
    }
    println!("{} identity · {}", style::ok("ready"), identity.actor_id);
    println!("  public key: {}", identity.pubkey);
    println!("  key file:   {}", identity.key_path);
    println!("  hub:        {}", identity.hub_url);
    println!();
    println!("Next: a maintainer registers you on a frontier with");
    println!(
        "  vela actor add <frontier> {} --pubkey {}",
        identity.actor_id, identity.pubkey
    );
    println!("Then `vela land` and `vela sign` need no key flags.");
}

pub(crate) fn cmd_id_keygen(out: std::path::PathBuf, json: bool) {
    {
        {
            let public_key = sign::generate_keypair(&out).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "id.keygen",
                "output_dir": out.display().to_string(),
                "public_key": public_key,
            });
            if json {
                print_json(&payload);
            } else {
                println!("{} keypair · {}", style::ok("generated"), out.display());
                println!("  public key: {public_key}");
            }
        }
    }
}

pub(crate) fn cmd_id_sign(
    frontier: std::path::PathBuf,
    key: Option<std::path::PathBuf>,
    json: bool,
) {
    {
        {
            let key_path =
                crate::cli_identity::resolve_key_path(key.as_deref()).unwrap_or_else(|| {
                    fail_return("no signing key: pass --key <path> or run `vela id create` once")
                });
            let count = sign::sign_registered_events(&frontier, &key_path)
                .unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "id.sign",
                "frontier": frontier.display().to_string(),
                "private_key": key_path.display().to_string(),
                "signed": count,
            });
            if json {
                print_json(&payload);
            } else {
                println!(
                    "{} {count} event(s) in {}",
                    style::ok("signed"),
                    frontier.display()
                );
            }
        }
    }
}

pub(crate) fn parse_signing_key(hex_str: &str) -> ed25519_dalek::SigningKey {
    let bytes = hex::decode(hex_str)
        .unwrap_or_else(|e| fail_return(&format!("invalid private-key hex: {e}")));
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .unwrap_or_else(|_| fail_return("private key must be 32 bytes"));
    ed25519_dalek::SigningKey::from_bytes(&key_bytes)
}
