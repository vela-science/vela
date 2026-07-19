use crate::cli::print_identity_created;
use crate::cli::{fail, fail_return, fail_usage, parse_signing_key, print_json};
use crate::cli_commands::*;
use colored::Colorize;
use serde_json::json;
use std::io::Write;
use std::process::{Command, Stdio};
use vela_protocol::cli_style as style;
use vela_protocol::repo;
use vela_protocol::sign;

pub(crate) fn cmd_id(action: IdAction) {
    use crate::cli_identity::{Identity, identity_path, load_identity, save_identity, vela_home};
    match action {
        IdAction::Protect {
            user_presence: _,
            remove_source_key: _,
            mode,
            json,
        } => {
            let mut identity =
                load_identity().unwrap_or_else(|| fail_return("no identity configured"));
            if identity.actor_type != "human"
                || identity.actor_id.starts_with("agent:")
                || identity.actor_id.starts_with("ci:")
            {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Custody,
                    "only a human reviewer identity may use protected decision signing",
                    None,
                );
            }
            let already_complete = matches!(
                identity.signer,
                Some(crate::cli_identity::IdentitySigner::Helper {
                    pending_source_removal: None,
                    ..
                })
            );
            let mut authorized_local_update = false;
            let pending_recovery = pending_enrollment_recovery(&identity).unwrap_or_else(|error| {
                crate::ui::fail_with(crate::ui::ErrorKind::Custody, &error, None)
            });
            if let Some(pending) = pending_recovery {
                if mode != pending.mode {
                    crate::ui::fail_with(
                        crate::ui::ErrorKind::Custody,
                        "finish the interrupted protected enrollment before changing its authentication mode",
                        Some(&format!("rerun with the original --mode {}", pending.mode)),
                    );
                }
                let vela_binary = std::env::current_exe()
                    .unwrap_or_else(|error| fail_return(&format!("resolve Vela binary: {error}")));
                let current_vela_sha256 = vela_signer::contract::file_sha256(&vela_binary)
                    .unwrap_or_else(|error| fail_return(&error));
                let helper = crate::cli_identity::signer_helper_path(&vela_binary)
                    .unwrap_or_else(|error| fail_return(&error));
                let current_helper_sha256 = vela_signer::contract::file_sha256(&helper)
                    .unwrap_or_else(|error| fail_return(&error));
                if current_vela_sha256 != pending.vela_binary_sha256
                    || current_helper_sha256 != pending.helper_sha256
                {
                    crate::ui::fail_with(
                        crate::ui::ErrorKind::Custody,
                        "the binaries changed during interrupted protected enrollment",
                        Some(
                            "restore the exact Vela package named by `vela id show --json`, then resume enrollment",
                        ),
                    );
                }
                let response = request_helper_rebind(
                    &identity,
                    &pending.vela_binary_sha256,
                    &pending.helper_sha256,
                    &pending.mode,
                    &pending.mode,
                    vela_signer::RebindPurpose::EnrollmentRecovery,
                )
                .unwrap_or_else(|error| {
                    crate::ui::fail_with(crate::ui::ErrorKind::Custody, &error, None)
                });
                if let Some(crate::cli_identity::IdentitySigner::Helper {
                    provider,
                    public_key,
                    protection_grade,
                    helper_sha256,
                    pending_source_removal,
                    pending_vela_binary_sha256,
                    ..
                }) = &mut identity.signer
                {
                    *provider = response.provider;
                    *public_key = response.public_key;
                    *protection_grade = response.protection_grade;
                    *helper_sha256 = response.helper_sha256;
                    *pending_source_removal = None;
                    *pending_vela_binary_sha256 = None;
                }
                save_identity(&identity).unwrap_or_else(|error| fail_return(&error));
                authorized_local_update = true;
            } else if !already_complete {
                let source = match &identity.signer {
                    Some(crate::cli_identity::IdentitySigner::File { key_path }) => {
                        key_path.clone()
                    }
                    Some(crate::cli_identity::IdentitySigner::Helper {
                        pending_source_removal: Some(path),
                        ..
                    }) => path.clone(),
                    _ => identity.key_path.clone(),
                };
                let enrollment = request_helper_enrollment(&identity, &source, &mode)
                    .unwrap_or_else(|error| {
                        crate::ui::fail_with(crate::ui::ErrorKind::Custody, &error, None)
                    });
                identity.version = "2.0".to_string();
                identity.key_path.clear();
                identity.signer = Some(crate::cli_identity::IdentitySigner::Helper {
                    provider: enrollment.provider.clone(),
                    key_id: enrollment.key_id.clone(),
                    public_key: enrollment.public_key.clone(),
                    protection_grade: enrollment.protection_grade.clone(),
                    mode: mode.clone(),
                    helper_sha256: enrollment.helper_sha256.clone(),
                    pending_source_removal: Some(source.clone()),
                    pending_vela_binary_sha256: Some(enrollment.vela_binary_sha256.clone()),
                });
                save_identity(&identity).unwrap_or_else(|error| fail_return(&error));
                if let Err(error) = std::fs::remove_file(&source) {
                    crate::ui::fail_with(
                        crate::ui::ErrorKind::Custody,
                        &format!(
                            "protected key installed, but plaintext source removal failed: {error}"
                        ),
                        Some(
                            "rerun `vela id protect --user-presence --remove-source-key`; protected decisions remain disabled until cleanup completes",
                        ),
                    );
                }
                if let Some(crate::cli_identity::IdentitySigner::Helper {
                    pending_source_removal,
                    pending_vela_binary_sha256,
                    ..
                }) = &mut identity.signer
                {
                    *pending_source_removal = None;
                    *pending_vela_binary_sha256 = None;
                }
                save_identity(&identity).unwrap_or_else(|error| fail_return(&error));
                authorized_local_update = true;
            } else if let Some(crate::cli_identity::IdentitySigner::Helper {
                helper_sha256: previous_helper_sha256,
                mode: previous_mode,
                public_key,
                provider,
                ..
            }) = identity.signer.as_ref()
            {
                if public_key != &identity.pubkey || provider != "os_store" {
                    crate::ui::fail_with(
                        crate::ui::ErrorKind::Custody,
                        "protected identity binding does not match its public profile",
                        None,
                    );
                }
                let vela_binary = std::env::current_exe()
                    .unwrap_or_else(|error| fail_return(&format!("resolve Vela binary: {error}")));
                let helper = crate::cli_identity::signer_helper_path(&vela_binary)
                    .unwrap_or_else(|error| fail_return(&error));
                let installed_helper_sha256 = vela_signer::contract::file_sha256(&helper)
                    .unwrap_or_else(|error| fail_return(&error));
                let (previous_vela_binary_sha256, binary_changed) = protected_binary_rebind_state()
                    .unwrap_or_else(|error| {
                        crate::ui::fail_with(crate::ui::ErrorKind::Custody, &error, None)
                    });
                if previous_helper_sha256 != &installed_helper_sha256
                    || previous_mode != &mode
                    || binary_changed
                {
                    let response = request_helper_rebind(
                        &identity,
                        &previous_vela_binary_sha256,
                        previous_helper_sha256,
                        previous_mode,
                        &mode,
                        vela_signer::RebindPurpose::Upgrade,
                    )
                    .unwrap_or_else(|error| {
                        crate::ui::fail_with(crate::ui::ErrorKind::Custody, &error, None)
                    });
                    if let Some(crate::cli_identity::IdentitySigner::Helper {
                        provider,
                        public_key,
                        protection_grade,
                        mode: configured_mode,
                        helper_sha256,
                        ..
                    }) = &mut identity.signer
                    {
                        *provider = response.provider;
                        *public_key = response.public_key;
                        *protection_grade = response.protection_grade;
                        *configured_mode = mode.clone();
                        *helper_sha256 = response.helper_sha256;
                    }
                    save_identity(&identity).unwrap_or_else(|error| fail_return(&error));
                    authorized_local_update = true;
                }
            }
            if authorized_local_update {
                crate::config::binary_pin::record_pin().unwrap_or_else(|error| {
                    crate::ui::fail_with(
                        crate::ui::ErrorKind::Custody,
                        &format!("record authenticated Vela binary pin: {error}"),
                        Some("rerun the same id protect command to resume the local update"),
                    )
                });
            }
            let binary_pin = match crate::config::binary_pin::pin_state().unwrap_or_else(|error| {
                crate::ui::fail_with(crate::ui::ErrorKind::Custody, &error, None)
            }) {
                crate::config::binary_pin::PinState::Match(pin) => pin,
                _ => crate::ui::fail_with(
                    crate::ui::ErrorKind::Custody,
                    "protected identity is not bound to the running Vela binary",
                    Some("rerun the same id protect command to authorize this exact installation"),
                ),
            };
            if json {
                print_json(&json!({
                    "ok": true,
                    "command": "id.protect",
                    "actor_id": identity.actor_id,
                    "public_key": identity.pubkey,
                    "signer": {"kind": "helper", "provider": "os_store", "mode": mode},
                    "plaintext_present": false,
                    "binary_pin": {"sha256": binary_pin.sha256, "version": binary_pin.version},
                }));
            } else {
                println!("{} protected identity", style::ok("ready"));
                println!("  actor: {}", identity.actor_id);
                println!("  approval: one exact decision card; authentication mode: {mode}");
                println!("  plaintext key: removed");
                println!(
                    "  binary pin: {} ({})",
                    &binary_pin.sha256[..16],
                    binary_pin.version
                );
            }
        }
        IdAction::Lock { json } => {
            let removed = vela_signer::system::lock_local_session().unwrap_or_else(|error| {
                crate::ui::fail_with(crate::ui::ErrorKind::Custody, &error, None)
            });
            if json {
                print_json(&json!({
                    "ok": true,
                    "command": "id.lock",
                    "session": "closed",
                    "changed": removed,
                    "identity_changed": false,
                    "frontier_changed": false,
                }));
            } else {
                println!("{} approval session closed", style::ok("locked"));
                println!("  protected identity: unchanged");
                println!("  frontier state: unchanged");
            }
        }
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
            let mut identity = Identity {
                version: "1.0".to_string(),
                actor_id: actor_id.clone(),
                actor_type: actor_type.to_string(),
                key_path: key_path.display().to_string(),
                pubkey: pubkey.clone(),
                signer: None,
            };
            if !agent {
                let enrollment = request_helper_enrollment(
                    &identity,
                    &key_path.display().to_string(),
                    "session",
                )
                .unwrap_or_else(|error| {
                    crate::ui::fail_with(crate::ui::ErrorKind::Custody, &error, None)
                });
                identity.version = "2.0".to_string();
                identity.key_path.clear();
                identity.signer = Some(crate::cli_identity::IdentitySigner::Helper {
                    provider: enrollment.provider,
                    key_id: enrollment.key_id,
                    public_key: enrollment.public_key,
                    protection_grade: enrollment.protection_grade,
                    mode: "session".to_string(),
                    helper_sha256: enrollment.helper_sha256,
                    pending_source_removal: Some(key_path.display().to_string()),
                    pending_vela_binary_sha256: Some(enrollment.vela_binary_sha256),
                });
                save_identity(&identity).unwrap_or_else(|error| fail_return(&error));
                std::fs::remove_file(&key_path).unwrap_or_else(|error| {
                    crate::ui::fail_with(
                        crate::ui::ErrorKind::Custody,
                        &format!(
                            "protected key installed, but temporary source removal failed: {error}"
                        ),
                        Some("rerun `vela id protect --user-presence --remove-source-key`"),
                    )
                });
                if let Some(crate::cli_identity::IdentitySigner::Helper {
                    pending_source_removal,
                    pending_vela_binary_sha256,
                    ..
                }) = &mut identity.signer
                {
                    *pending_source_removal = None;
                    *pending_vela_binary_sha256 = None;
                }
            }
            save_identity(&identity).unwrap_or_else(|e| fail_return(&e));
            if !agent {
                crate::config::binary_pin::record_pin().unwrap_or_else(|error| {
                    crate::ui::fail_with(
                        crate::ui::ErrorKind::Custody,
                        &format!("record authenticated Vela binary pin: {error}"),
                        Some(
                            "rerun `vela id protect --user-presence --remove-source-key` to resume",
                        ),
                    )
                });
            }
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
                signer: None,
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
            if matches!(
                load_identity().and_then(|identity| identity.signer),
                Some(crate::cli_identity::IdentitySigner::Helper {
                    pending_source_removal: None,
                    ..
                })
            ) {
                crate::ui::fail_with(
                    crate::ui::ErrorKind::Custody,
                    "a protected identity cannot move its Vela binary pin with the legacy pin command",
                    Some(
                        "run `vela id protect --user-presence --remove-source-key` to authorize the exact binary/helper update",
                    ),
                );
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
            let health = crate::cli_identity::signer_health(&identity);
            if json {
                let signer = match &identity.signer {
                    Some(crate::cli_identity::IdentitySigner::Helper {
                        provider,
                        protection_grade,
                        mode,
                        helper_sha256,
                        pending_source_removal,
                        pending_vela_binary_sha256,
                        ..
                    }) => {
                        let session_state = protection_mode(mode)
                            .map(|mode| {
                                vela_signer::system::local_session_state(
                                    &identity.actor_id,
                                    &identity.pubkey,
                                    provider,
                                    mode,
                                    helper_sha256,
                                    chrono::Utc::now(),
                                )
                            })
                            .unwrap_or(vela_signer::system::LocalSessionState::Invalid);
                        json!({
                            "kind": "protected",
                            "provider": provider,
                            "protection_grade": protection_grade,
                            "mode": mode,
                            "helper_sha256": helper_sha256,
                            "plaintext_present": pending_source_removal.is_some(),
                            "pending_vela_binary_sha256": pending_vela_binary_sha256,
                            "session": session_state.as_str(),
                        })
                    }
                    Some(crate::cli_identity::IdentitySigner::File { .. }) | None => {
                        json!({"kind": "file"})
                    }
                };
                print_json(&json!({
                    "ok": true,
                    "configured": true,
                    "actor_id": identity.actor_id,
                    "actor_type": identity.actor_type,
                    "pubkey": identity.pubkey,
                    "signer": signer,
                    "release": {
                        "version": health.binary_version,
                        "binary_path": health.binary_path,
                        "binary_sha256": health.binary_sha256,
                    },
                    "protected_backend": health,
                }));
            } else {
                println!("{}", style::ok("identity"));
                println!("  actor:  {}", identity.actor_id);
                println!("  pubkey: {}", identity.pubkey);
                println!(
                    "  binary: vela {} · {}",
                    health.binary_version,
                    health
                        .binary_sha256
                        .as_deref()
                        .unwrap_or("digest unavailable")
                );
                match &identity.signer {
                    Some(crate::cli_identity::IdentitySigner::Helper {
                        mode,
                        pending_source_removal,
                        helper_sha256,
                        provider,
                        ..
                    }) => {
                        let session_state = protection_mode(mode)
                            .map(|mode| {
                                vela_signer::system::local_session_state(
                                    &identity.actor_id,
                                    &identity.pubkey,
                                    provider,
                                    mode,
                                    helper_sha256,
                                    chrono::Utc::now(),
                                )
                            })
                            .unwrap_or(vela_signer::system::LocalSessionState::Invalid);
                        println!("  approval: protected · {mode}");
                        println!("  session:  {}", session_state.as_str());
                        println!(
                            "  secret:   {}",
                            if pending_source_removal.is_some() {
                                "cleanup required"
                            } else {
                                "protected; plaintext absent"
                            }
                        );
                        println!("  protection: {}", health.state);
                        if let Some(next_action) = &health.next_action {
                            println!("  next: {next_action}");
                        }
                    }
                    _ => println!("  key:    {}", identity.key_path),
                }
            }
        }
    }
}

fn request_helper_enrollment(
    identity: &crate::cli_identity::Identity,
    source: &str,
    mode: &str,
) -> Result<vela_signer::EnrollmentResponse, String> {
    use rand::RngCore;

    let vela_binary =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let helper = crate::cli_identity::signer_helper_path(&vela_binary)?;
    let helper_sha256 = vela_signer::contract::file_sha256(&helper)?;
    let source = std::fs::canonicalize(source)
        .map_err(|error| format!("resolve plaintext source key {source}: {error}"))?;
    let mut nonce = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let now = chrono::Utc::now();
    let request = vela_signer::EnrollmentRequest {
        schema: vela_signer::contract::ENROLLMENT_REQUEST_SCHEMA.to_string(),
        nonce: hex::encode(nonce),
        expires_at: (now + chrono::Duration::seconds(120))
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        vela_binary_path: vela_binary.display().to_string(),
        vela_binary_sha256: vela_signer::contract::file_sha256(&vela_binary)?,
        helper_sha256: helper_sha256.clone(),
        actor: identity.actor_id.clone(),
        public_key: identity.pubkey.clone(),
        source_path: source.display().to_string(),
        provider: "os_store".to_string(),
        protection_mode: match mode {
            "session" => vela_signer::ProtectionMode::Session,
            "always" => vela_signer::ProtectionMode::Always,
            _ => return Err(format!("unsupported protected signer mode '{mode}'")),
        },
        remove_source_after_install: true,
    };
    vela_signer::contract::validate_enrollment_request(&request, now)?;
    let bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("serialize signer enrollment: {error}"))?;
    let mut child = Command::new(&helper)
        .arg("enroll")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start signer helper {}: {error}", helper.display()))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "signer helper stdin is unavailable".to_string())?
        .write_all(&bytes)
        .map_err(|error| format!("write signer enrollment: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for signer enrollment: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "signer enrollment declined or failed: {}",
            crate::cli::safe_text::inline(String::from_utf8_lossy(&output.stderr).trim())
        ));
    }
    let response: vela_signer::EnrollmentResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse closed enrollment response: {error}"))?;
    if response.schema != vela_signer::contract::ENROLLMENT_RESPONSE_SCHEMA
        || response.nonce != request.nonce
        || response.actor != request.actor
        || response.public_key != request.public_key
        || response.vela_binary_sha256 != request.vela_binary_sha256
        || response.helper_sha256 != helper_sha256
        || response.provider != request.provider
        || response.protection_mode != request.protection_mode
        || response.source_removed
    {
        return Err("signer enrollment response does not match the exact request".to_string());
    }
    Ok(response)
}

fn request_helper_rebind(
    identity: &crate::cli_identity::Identity,
    previous_vela_binary_sha256: &str,
    previous_helper_sha256: &str,
    previous_mode: &str,
    mode: &str,
    purpose: vela_signer::RebindPurpose,
) -> Result<vela_signer::RebindResponse, String> {
    use rand::RngCore;

    let vela_binary =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let helper = crate::cli_identity::signer_helper_path(&vela_binary)?;
    let helper_sha256 = vela_signer::contract::file_sha256(&helper)?;
    let mut nonce = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let now = chrono::Utc::now();
    let request = vela_signer::RebindRequest {
        schema: vela_signer::contract::REBIND_REQUEST_SCHEMA.to_string(),
        purpose,
        nonce: hex::encode(nonce),
        expires_at: (now + chrono::Duration::seconds(120))
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        vela_binary_path: vela_binary.display().to_string(),
        vela_binary_sha256: vela_signer::contract::file_sha256(&vela_binary)?,
        previous_vela_binary_sha256: previous_vela_binary_sha256.to_string(),
        helper_sha256,
        previous_helper_sha256: previous_helper_sha256.to_string(),
        actor: identity.actor_id.clone(),
        public_key: identity.pubkey.clone(),
        provider: "os_store".to_string(),
        previous_protection_mode: protection_mode(previous_mode)?,
        protection_mode: protection_mode(mode)?,
    };
    vela_signer::validate_rebind_request(&request, now)?;
    let bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("serialize signer rebind: {error}"))?;
    let mut child = Command::new(&helper)
        .arg("rebind")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start signer helper {}: {error}", helper.display()))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "signer helper stdin is unavailable".to_string())?
        .write_all(&bytes)
        .map_err(|error| format!("write signer rebind: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for signer rebind: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "signer update authorization failed: {}",
            crate::cli::safe_text::inline(String::from_utf8_lossy(&output.stderr).trim())
        ));
    }
    let response: vela_signer::RebindResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse closed rebind response: {error}"))?;
    vela_signer::validate_rebind_response(&request, &response)?;
    Ok(response)
}

fn protected_binary_rebind_state() -> Result<(String, bool), String> {
    use crate::config::binary_pin::PinState;
    match crate::config::binary_pin::pin_state()? {
        PinState::Match(pin) => Ok((format!("sha256:{}", pin.sha256), false)),
        PinState::Mismatch { pinned, .. } => Ok((format!("sha256:{}", pinned.sha256), true)),
        PinState::Unpinned => Ok((format!("sha256:{}", "0".repeat(64)), true)),
    }
}

struct PendingEnrollmentRecovery {
    vela_binary_sha256: String,
    helper_sha256: String,
    mode: String,
}

fn pending_enrollment_recovery(
    identity: &crate::cli_identity::Identity,
) -> Result<Option<PendingEnrollmentRecovery>, String> {
    let Some(crate::cli_identity::IdentitySigner::Helper {
        provider,
        public_key,
        mode,
        helper_sha256,
        pending_source_removal: Some(source),
        pending_vela_binary_sha256,
        ..
    }) = &identity.signer
    else {
        return Ok(None);
    };
    if std::path::Path::new(source).is_file() {
        return Ok(None);
    }
    if provider != "os_store" || public_key != &identity.pubkey {
        return Err("interrupted protected identity binding is invalid".to_string());
    }
    let vela_binary_sha256 = pending_vela_binary_sha256.clone().ok_or_else(|| {
        "interrupted enrollment predates recoverable binary binding; restore the plaintext source from backup before retrying"
            .to_string()
    })?;
    Ok(Some(PendingEnrollmentRecovery {
        vela_binary_sha256,
        helper_sha256: helper_sha256.clone(),
        mode: mode.clone(),
    }))
}

fn protection_mode(value: &str) -> Result<vela_signer::ProtectionMode, String> {
    match value {
        "session" => Ok(vela_signer::ProtectionMode::Session),
        "always" => Ok(vela_signer::ProtectionMode::Always),
        _ => Err(format!("unsupported protected signer mode '{value}'")),
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
            let protected_enrollment_proves_possession = match &identity.signer {
                Some(crate::cli_identity::IdentitySigner::Helper {
                    key_id,
                    public_key,
                    pending_source_removal: None,
                    ..
                }) => {
                    if public_key != &identity.pubkey
                        || key_id != &format!("{}:{}", identity.actor_id, identity.pubkey)
                    {
                        fail_return::<()>(
                            "protected identity key binding does not match its profile",
                        );
                    }
                    true
                }
                Some(crate::cli_identity::IdentitySigner::Helper { .. }) => {
                    fail_return::<()>(
                        "protected identity enrollment is incomplete; finish plaintext cleanup before actor bootstrap",
                    );
                    unreachable!()
                }
                _ => false,
            };
            let id = identity.actor_id;
            let pubkey = identity.pubkey;
            let trimmed = pubkey.trim();
            if trimmed.len() != 64 || hex::decode(trimmed).is_err() {
                fail_return::<()>(
                    "configured identity contains an invalid Ed25519 public key; recreate or re-import it with `vela id`",
                );
            }
            if !protected_enrollment_proves_possession {
                let key = crate::cli_identity::resolve_signing_key(None);
                if sign::pubkey_hex(&key) != trimmed {
                    fail_return::<()>(
                        "configured identity pubkey does not match its private key; refusing actor-registry bootstrap",
                    );
                }
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

#[cfg(test)]
mod protected_enrollment_recovery_tests {
    use super::*;

    fn pending_identity(
        source: String,
        binary_digest: Option<String>,
    ) -> crate::cli_identity::Identity {
        let public_key = "4".repeat(64);
        crate::cli_identity::Identity {
            version: "2.0".to_string(),
            actor_id: "reviewer:recovery-test".to_string(),
            actor_type: "human".to_string(),
            key_path: String::new(),
            pubkey: public_key.clone(),
            signer: Some(crate::cli_identity::IdentitySigner::Helper {
                provider: "os_store".to_string(),
                key_id: format!("reviewer:recovery-test:{public_key}"),
                public_key,
                protection_grade: "user_session".to_string(),
                mode: "session".to_string(),
                helper_sha256: format!("sha256:{}", "a".repeat(64)),
                pending_source_removal: Some(source),
                pending_vela_binary_sha256: binary_digest,
            }),
        }
    }

    #[test]
    fn missing_source_resumes_only_with_the_recorded_binary_binding() {
        let identity = pending_identity(
            "/definitely/missing/vela-source.key".to_string(),
            Some(format!("sha256:{}", "b".repeat(64))),
        );
        let recovery = pending_enrollment_recovery(&identity).unwrap().unwrap();
        assert_eq!(recovery.mode, "session");
        assert_eq!(
            recovery.vela_binary_sha256,
            format!("sha256:{}", "b".repeat(64))
        );

        let unbound = pending_identity("/definitely/missing/vela-source.key".to_string(), None);
        assert!(pending_enrollment_recovery(&unbound).is_err());
    }

    #[test]
    fn existing_source_uses_the_normal_enrollment_resume_path() {
        let source = tempfile::NamedTempFile::new().unwrap();
        let identity = pending_identity(
            source.path().display().to_string(),
            Some(format!("sha256:{}", "b".repeat(64))),
        );
        assert!(pending_enrollment_recovery(&identity).unwrap().is_none());
    }
}
