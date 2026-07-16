use crate::cli::print_identity_created;
use crate::cli::{fail, fail_return, fail_usage, parse_signing_key, print_json};
use crate::cli_commands::*;
use colored::Colorize;
use serde_json::json;
use vela_protocol::cli_style as style;
use vela_protocol::repo;
use vela_protocol::sign;

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
            if load_identity().is_some() && !force {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Exists,
                    &format!("an identity already exists ({})", identity_path().display()),
                    Some("run `vela id show` to inspect it, or pass --force to overwrite"),
                );
            }
            let handle = handle
                .or_else(|| std::env::var("USER").ok())
                .map(|h| h.trim().to_string())
                .filter(|h| !h.is_empty())
                .unwrap_or_else(|| "anon".to_string());
            let actor_type = if agent { "agent" } else { "human" };
            let actor_id = format!("{}:{}", if agent { "agent" } else { "reviewer" }, handle);
            let key_dir = vela_home().join("keys").join(&handle);
            let pubkey = sign::generate_keypair(&key_dir).unwrap_or_else(|e| fail_return(&e));
            let key_path = key_dir.join("private.key");
            let identity = Identity {
                version: "1.0".to_string(),
                actor_id: actor_id.clone(),
                actor_type: actor_type.to_string(),
                key_path: key_path.display().to_string(),
                pubkey: pubkey.clone(),
            };
            save_identity(&identity).unwrap_or_else(|e| fail_return(&e));
            print_identity_created(&identity, json);
        }
        IdAction::Import {
            key,
            handle,
            agent,
            force,
            json,
        } => {
            if load_identity().is_some() && !force {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Exists,
                    &format!("an identity already exists ({})", identity_path().display()),
                    Some("run `vela id show` to inspect it, or pass --force to overwrite"),
                );
            }
            let hex = std::fs::read_to_string(&key)
                .unwrap_or_else(|e| fail_return(&format!("read key {}: {e}", key.display())));
            let signing = parse_signing_key(hex.trim());
            let pubkey = hex::encode(signing.verifying_key().to_bytes());
            let handle = handle
                .or_else(|| std::env::var("USER").ok())
                .map(|h| h.trim().to_string())
                .filter(|h| !h.is_empty())
                .unwrap_or_else(|| "anon".to_string());
            let actor_id = format!("{}:{}", if agent { "agent" } else { "reviewer" }, handle);
            let identity = Identity {
                version: "1.0".to_string(),
                actor_id: actor_id.clone(),
                actor_type: if agent { "agent" } else { "human" }.to_string(),
                key_path: key.display().to_string(),
                pubkey: pubkey.clone(),
            };
            save_identity(&identity).unwrap_or_else(|e| fail_return(&e));
            print_identity_created(&identity, json);
        }
        IdAction::PinBinary { status, yes } => {
            use crate::config::binary_pin;
            if status {
                match binary_pin::verify_for_ceremony() {
                    Ok(Some(pin)) => println!(
                        "  · pinned {} ({} at {}) — matches the running binary",
                        &pin.sha256[..16],
                        pin.version,
                        pin.pinned_at
                    ),
                    Ok(None) => println!(
                        "  · no binary pin recorded — ceremonies run unpinned (run `vela id pin-binary`)"
                    ),
                    Err(e) => crate::ui::fail_with(crate::ui::ErrorKind::Custody, &e, None),
                }
                return;
            }
            // A pin is a HUMAN act: agents may not move the trust anchor.
            let actor = crate::cli_identity::resolve_decision_actor(None);
            if !yes {
                crate::ui::ensure_can_prompt(
                    "the binary pin",
                    "pass --yes to pin non-interactively",
                );
                if !crate::cli::prompt::confirm(&format!(
                    "pin the running vela binary as {actor}'s ceremony anchor? [y/N] "
                )) {
                    println!("not pinned.");
                    return;
                }
            }
            match binary_pin::record_pin() {
                Ok(pin) => {
                    println!(
                        "  · pinned {} ({}) — ceremonies now verify the binary first",
                        &pin.sha256[..16],
                        pin.version
                    );
                    if binary_pin::is_dev_build_path(std::path::Path::new(&pin.binary_path)) {
                        eprintln!(
                            "  {} this is a build-tree binary ({}); its hash changes on every \
                             `cargo build`. Pin your installed release (e.g. ~/.cargo/bin/vela) \
                             so the pin stays stable.",
                            style::warn("note"),
                            pin.binary_path
                        );
                    }
                }
                Err(e) => crate::ui::fail_with(crate::ui::ErrorKind::Domain, &e, None),
            }
        }
        IdAction::Show { json } => {
            let Some(identity) = load_identity() else {
                if json {
                    print_json(&json!({"ok": false, "configured": false}));
                } else {
                    println!(
                        "{} no identity configured — run `vela id create --handle <your-name>`",
                        style::warn("none")
                    );
                }
                return;
            };
            if json {
                print_json(&json!({
                    "ok": true,
                    "configured": true,
                    "actor_id": identity.actor_id,
                    "actor_type": identity.actor_type,
                    "pubkey": identity.pubkey,
                    "key_path": identity.key_path,
                }));
            } else {
                println!("{}", style::ok("identity"));
                println!("  actor:  {}", identity.actor_id);
                println!("  pubkey: {}", identity.pubkey);
                println!("  key:    {}", identity.key_path);
            }
        }
    }
}

pub(crate) fn cmd_actor(action: ActorAction) {
    match action {
        ActorAction::Add {
            frontier,
            orcid,
            clearance,
            json,
        } => {
            // Registry bootstrap is deliberately one-shot and self-binding:
            // only the configured identity may become the first actor. There
            // is no arbitrary id/pubkey insertion path on an established
            // frontier; later membership and rotation need signed governance.
            let identity = crate::cli_identity::load_identity().unwrap_or_else(|| {
                fail_usage("no configured identity; run `vela id create --handle <your-name>` before bootstrapping the frontier actor registry")
            });
            let id = identity.actor_id;
            let pubkey = identity.pubkey;
            let trimmed = pubkey.trim();
            if trimmed.len() != 64 || hex::decode(trimmed).is_err() {
                fail_return::<()>(
                    "configured identity contains an invalid Ed25519 public key; recreate or re-import it with `vela id`",
                );
            }
            let key = crate::cli_identity::resolve_signing_key(None);
            if sign::pubkey_hex(&key) != trimmed {
                fail_return::<()>(
                    "configured identity pubkey does not match its private key; refusing actor-registry bootstrap",
                );
            }
            // v0.43: Validate ORCID shape if supplied. Stored in bare form.
            let orcid_normalized = orcid
                .as_deref()
                .map(|s| sign::validate_orcid(s).unwrap_or_else(|e| fail_return(&e)));
            // v0.51: parse clearance up front so a typo fails at the
            // CLI boundary rather than silently degrading.
            let clearance: Option<vela_protocol::access_tier::AccessTier> =
                clearance.as_deref().map(|s| {
                    vela_protocol::access_tier::AccessTier::parse(s)
                        .unwrap_or_else(|e| fail_return(&e))
                });

            let mut project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            if !project.actors.is_empty() {
                fail(
                    "actor registry is already established; `vela actor add` is bootstrap-only and cannot extend or replace it",
                );
            }
            project.actors.push(sign::ActorRecord {
                id: id.clone(),
                public_key: trimmed.to_string(),
                algorithm: "ed25519".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tier: None,
                orcid: orcid_normalized.clone(),
                access_clearance: clearance,
                revoked_at: None,
                revoked_reason: None,
            });
            repo::save_to_path(&frontier, &project).unwrap_or_else(|e| fail_return(&e));
            let payload = json!({
                "ok": true,
                "command": "actor.add",
                "frontier": frontier.display().to_string(),
                "actor_id": id,
                "public_key": trimmed,
                "orcid": orcid_normalized,
                "registered_count": project.actors.len(),
            });
            if json {
                print_json(&payload);
            } else {
                println!(
                    "{} actor {} (pubkey {})",
                    style::ok("registered"),
                    id,
                    &trimmed[..16]
                );
            }
        }
        ActorAction::List { frontier, json } => {
            let project = repo::load_from_path(&frontier).unwrap_or_else(|e| fail_return(&e));
            if json {
                let payload = json!({
                    "ok": true,
                    "command": "actor.list",
                    "frontier": frontier.display().to_string(),
                    "actors": project.actors,
                });
                print_json(&payload);
            } else {
                println!();
                println!(
                    "  {}",
                    format!("VELA · ACTOR · LIST · {}", frontier.display())
                        .to_uppercase()
                        .dimmed()
                );
                println!("  {}", style::tick_row(60));
                if project.actors.is_empty() {
                    println!("  (no actors registered)");
                } else {
                    for actor in &project.actors {
                        println!(
                            "  {:<28} {}…  registered {}",
                            actor.id,
                            &actor.public_key[..16],
                            actor.created_at
                        );
                    }
                }
            }
        }
        ActorAction::Activate {
            frontier,
            anchor,
            actor,
            preview,
            yes,
            confirm_root,
            json,
        } => crate::config::actor_registration::cmd_actor_activate(
            &frontier,
            &anchor,
            actor.as_deref(),
            preview,
            yes,
            confirm_root.as_deref(),
            json,
        ),
    }
}
