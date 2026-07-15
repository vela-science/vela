//! Identity helpers: `vela id` key generation and signing-key parsing.
//! Moved verbatim from `cli/mod.rs`.

use super::*;

/// Shared success print for `vela id create` / `vela id import`: shows the
/// identity and the one-time empty-registry bootstrap command.
pub(crate) fn print_identity_created(identity: &crate::cli_identity::Identity, json: bool) {
    if json {
        print_json(&json!({
            "ok": true,
            "command": "id.create",
            "actor_id": identity.actor_id,
            "actor_type": identity.actor_type,
            "pubkey": identity.pubkey,
            "key_path": identity.key_path,
        }));
        return;
    }
    println!("{} identity · {}", style::ok("ready"), identity.actor_id);
    println!("  public key: {}", identity.pubkey);
    println!("  key file:   {}", identity.key_path);
    println!();
    println!("Next: bootstrap a new frontier's empty actor registry with");
    println!("  vela actor add <frontier>");
    println!("Established registries change only through signed governance.");
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

pub(crate) fn parse_signing_key(hex_str: &str) -> ed25519_dalek::SigningKey {
    let bytes = hex::decode(hex_str)
        .unwrap_or_else(|e| fail_return(&format!("invalid private-key hex: {e}")));
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .unwrap_or_else(|_| fail_return("private key must be 32 bytes"));
    ed25519_dalek::SigningKey::from_bytes(&key_bytes)
}
