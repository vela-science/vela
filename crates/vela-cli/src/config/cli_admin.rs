use crate::cli::{fail_return, parse_signing_key, print_identity_created, print_json};
use crate::cli_commands::IdAction;
use serde_json::json;
use vela_protocol::cli_style as style;
use vela_protocol::sign;

/// Manage file-backed producer identities.
///
/// Human scientific authority no longer uses a Vela key. Repository-authority
/// Frontiers authenticate the local human principal through the platform
/// provider and sign the covering transaction with the repository service key.
pub(crate) fn cmd_id(action: IdAction) {
    use crate::cli_identity::{Identity, identity_path, load_identity, save_identity, vela_home};

    match action {
        IdAction::Keygen { out, json } => crate::cli::cmd_id_keygen(out, json),
        IdAction::Create {
            handle,
            agent,
            force,
            json,
        } => {
            if !agent {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Usage,
                    "Vela no longer creates human signing identities",
                    Some(
                        "use `vela id create --agent --handle <name>` for producer work; human decisions use the authenticated platform principal",
                    ),
                );
            }
            if load_identity().is_some() && !force {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Exists,
                    &format!("an identity already exists ({})", identity_path().display()),
                    Some("run `vela id show` to inspect it, or pass --force to overwrite"),
                );
            }
            let handle = handle
                .or_else(|| std::env::var("USER").ok())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "agent".to_string());
            let key_dir = vela_home().join("keys").join(&handle);
            let public_key =
                sign::generate_keypair(&key_dir).unwrap_or_else(|error| fail_return(&error));
            let identity = Identity {
                version: "1.0".to_string(),
                actor_id: format!("agent:{handle}"),
                actor_type: "agent".to_string(),
                key_path: key_dir.join("private.key").display().to_string(),
                pubkey: public_key,
            };
            save_identity(&identity).unwrap_or_else(|error| fail_return(&error));
            print_identity_created(&identity, json);
        }
        IdAction::Import {
            key,
            handle,
            agent,
            force,
            json,
        } => {
            if !agent {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Usage,
                    "Vela imports private keys only for agent identities",
                    Some("pass --agent, or use the platform principal for human authority"),
                );
            }
            if load_identity().is_some() && !force {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Exists,
                    &format!("an identity already exists ({})", identity_path().display()),
                    Some("run `vela id show` to inspect it, or pass --force to overwrite"),
                );
            }
            let encoded = std::fs::read_to_string(&key).unwrap_or_else(|error| {
                fail_return(&format!("read key {}: {error}", key.display()))
            });
            let signing = parse_signing_key(encoded.trim());
            let handle = handle
                .or_else(|| std::env::var("USER").ok())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "agent".to_string());
            let identity = Identity {
                version: "1.0".to_string(),
                actor_id: format!("agent:{handle}"),
                actor_type: "agent".to_string(),
                key_path: key.display().to_string(),
                pubkey: hex::encode(signing.verifying_key().to_bytes()),
            };
            save_identity(&identity).unwrap_or_else(|error| fail_return(&error));
            print_identity_created(&identity, json);
        }
        IdAction::Show { json } => {
            let Some(identity) = load_identity() else {
                if json {
                    print_json(&json!({"ok": false, "configured": false}));
                } else {
                    println!("{} no agent identity configured", style::warn("none"));
                }
                return;
            };
            let signer = if identity.actor_type == "human" {
                json!({
                    "kind": "retired_human_identity",
                    "writable": false,
                    "remove": identity_path(),
                })
            } else {
                json!({"kind": "file_agent"})
            };
            if json {
                print_json(&json!({
                    "ok": true,
                    "configured": true,
                    "actor_id": identity.actor_id,
                    "actor_type": identity.actor_type,
                    "pubkey": identity.pubkey,
                    "signer": signer,
                }));
            } else {
                println!("{}", style::ok("identity"));
                println!("  actor:  {}", identity.actor_id);
                println!("  pubkey: {}", identity.pubkey);
                if identity.actor_type == "human" {
                    println!("  mode:   legacy read-only identity");
                    println!("  human authority: authenticated platform principal");
                } else {
                    println!("  key:    {}", identity.key_path);
                }
            }
        }
    }
}
