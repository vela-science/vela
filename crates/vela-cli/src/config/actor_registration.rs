use std::path::Path;

use serde_json::{Value, json};
use vela_protocol::events::{
    EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED, EVENT_SCHEMA, NULL_HASH, StateActor, StateEvent,
    StateTarget, compute_event_id,
};

const ACTIVATION_REASON: &str = "Activate signature enforcement after the exact anchored history.";
const ACTIVATION_CAVEAT: &str = "Unsigned anchor members remain legacy and unauthenticated; activation does not attribute them to the key holder.";

fn preview_root(
    preview: &vela_edge::actor_registration::ActorRegistrationPreview,
) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = vela_protocol::canonical::to_canonical_bytes(preview)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn render_preview(
    frontier: &Path,
    preview: &vela_edge::actor_registration::ActorRegistrationPreview,
    root: &str,
    json_output: bool,
) {
    if json_output {
        crate::cli::print_json(&json!({
            "ok": true,
            "command": "actor.activate.preview",
            "frontier": frontier.display().to_string(),
            "preview_root": root,
            "payload": preview.payload,
            "counts": {
                "anchored_unsigned": preview.anchored_unsigned,
                "anchored_signed": preview.anchored_signed,
                "post_anchor_unsigned": preview.post_anchor_unsigned,
                "post_anchor_signed": preview.post_anchor_signed,
            },
            "authority": "human_terminal_only",
            "legacy_authentication": "none",
        }));
        return;
    }
    crate::ui::header(
        "ACTOR",
        &preview.payload.actor_id,
        Some("temporal registration activation"),
    );
    println!("  frontier: {}", preview.payload.frontier_id);
    println!("  public key: {}", preview.payload.public_key);
    println!(
        "  anchor: {}  tree {}",
        preview.payload.anchor.git_commit, preview.payload.anchor.git_tree
    );
    println!(
        "  event log: {} events  {}",
        preview.payload.anchor.event_count, preview.payload.anchor.event_log_root
    );
    println!(
        "  actor registry: {}",
        preview.payload.anchor.actor_registry_root
    );
    println!(
        "  anchor actor events: {} unsigned legacy, {} signed",
        preview.anchored_unsigned, preview.anchored_signed
    );
    println!(
        "  later actor events: {} unsigned, {} signed",
        preview.post_anchor_unsigned, preview.post_anchor_signed
    );
    println!("  preview root: {root}");
    println!();
    println!("  unsigned anchored events remain unauthenticated.");
    println!("  every matching event absent from the anchor requires a valid signature.");
}

pub(crate) fn cmd_actor_activate(
    frontier: &Path,
    anchor: &str,
    actor_flag: Option<&str>,
    preview_only: bool,
    yes: bool,
    confirmed_root: Option<&str>,
    json_output: bool,
) {
    let project = vela_protocol::repo::load_from_path(frontier)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let actor = actor_flag
        .map(str::to_string)
        .or_else(|| crate::cli_identity::load_identity().map(|identity| identity.actor_id))
        .or_else(|| (project.actors.len() == 1).then(|| project.actors[0].id.clone()))
        .unwrap_or_else(|| {
            crate::cli::fail_usage(
                "actor activation requires --actor or a configured matching identity",
            )
        });
    let preview = vela_edge::actor_registration::preview_temporalize_existing(
        &project, frontier, &actor, anchor,
    )
    .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let root = preview_root(&preview).unwrap_or_else(|error| crate::cli::fail_return(&error));
    if preview_only {
        render_preview(frontier, &preview, &root, json_output);
        return;
    }
    if yes && confirmed_root.is_none() {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Usage,
            "actor activate --yes requires --confirm-root from an exact prior preview",
            Some(
                "run with --preview --json, inspect the roots, then echo preview_root through --confirm-root",
            ),
        );
    }
    if let Some(confirmed_root) = confirmed_root
        && confirmed_root != root
    {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Custody,
            &format!("confirmed preview root {confirmed_root} does not match current {root}"),
            Some("render a fresh preview and inspect the changed roots before signing"),
        );
    }
    if json_output && !yes {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Usage,
            "actor activate --json is preview-only unless --yes and --confirm-root are supplied",
            Some("run once with --preview --json, then confirm that exact root"),
        );
    }
    if !json_output {
        render_preview(frontier, &preview, &root, false);
    }
    crate::cli::sign_session::ceremony_binary_gate(!yes);
    let decision_actor = crate::cli_identity::resolve_decision_actor(None);
    if decision_actor != actor {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Custody,
            &format!(
                "configured decision actor {decision_actor} cannot activate registration for {actor}"
            ),
            Some("use the identity whose public key is registered for this actor"),
        );
    }
    if !yes {
        crate::ui::ensure_can_prompt(
            "actor registration activation",
            "pass --yes only after reviewing the exact preview",
        );
        if !crate::cli::prompt::confirm(&format!(
            "  sign and publish activation root {root}? [y/N] "
        )) {
            crate::ui::fail_with(
                crate::ui::ErrorKind::Usage,
                "not activated because confirmation was declined",
                Some("rerun the preview and confirm only after checking every root"),
            );
        }
    }

    let expected_root = root.clone();
    let expected_anchor = anchor.to_string();
    let expected_actor = actor.clone();
    let outcome = crate::workflow::transact_actor_registration(frontier, |original| {
        let current_preview = vela_edge::actor_registration::preview_temporalize_existing(
            original,
            frontier,
            &expected_actor,
            &expected_anchor,
        )?;
        let current_root = preview_root(&current_preview)?;
        if current_root != expected_root {
            return Err(format!(
                "activation preview changed before the key edge: expected {expected_root}, got {current_root}"
            ));
        }
        let key = crate::cli_identity::resolve_signing_key(None);
        let public_key = vela_protocol::sign::pubkey_hex(&key);
        if public_key != current_preview.payload.public_key {
            return Err(
                "configured private key does not match the anchored actor registry".to_string(),
            );
        }
        let activated_at = chrono::Utc::now().to_rfc3339();
        let mut event = StateEvent {
            schema: EVENT_SCHEMA.to_string(),
            id: String::new(),
            kind: EVENT_KIND_ACTOR_REGISTRATION_ACTIVATED.into(),
            target: StateTarget {
                r#type: "actor".to_string(),
                id: expected_actor.clone(),
            },
            actor: StateActor {
                r#type: "human".to_string(),
                id: expected_actor.clone(),
            },
            timestamp: activated_at.clone(),
            reason: ACTIVATION_REASON.to_string(),
            before_hash: NULL_HASH.to_string(),
            after_hash: NULL_HASH.to_string(),
            payload: serde_json::to_value(&current_preview.payload)
                .map_err(|error| error.to_string())?,
            caveats: vec![ACTIVATION_CAVEAT.to_string()],
            signature: None,
        };
        event.id = compute_event_id(&event);
        event.signature = Some(vela_protocol::sign::sign_event(&event, &key)?);
        vela_protocol::actor_registration::verify_activation_signature(&event, &public_key)?;
        let activation_event_id = event.id.clone();
        let mut candidate: vela_protocol::project::Project =
            serde_json::from_value(serde_json::to_value(original).map_err(|e| e.to_string())?)
                .map_err(|error| error.to_string())?;
        candidate.events.push(event);
        vela_protocol::project::recompute_stats(&mut candidate);
        Ok((candidate, activation_event_id, activated_at))
    })
    .unwrap_or_else(|error| crate::cli::fail_return(&error));

    let reloaded = vela_protocol::repo::load_from_path(frontier)
        .unwrap_or_else(|error| crate::cli::fail_return(&error));
    let report = vela_edge::actor_registration::assess(&reloaded, Some(frontier));
    if report.boundary(&actor).is_none_or(|boundary| {
        boundary.outcome != vela_edge::actor_registration::BoundaryOutcome::Valid
    }) {
        crate::ui::fail_with(
            crate::ui::ErrorKind::Domain,
            "installed actor-registration activation did not validate after reload",
            Some("inspect the recoverable transaction and Git publication before relying on it"),
        );
    }
    if json_output {
        let mut payload = outcome;
        if let Some(object) = payload.as_object_mut() {
            object.insert("preview_root".to_string(), Value::String(root));
            object.insert("actor_id".to_string(), Value::String(actor));
        }
        crate::cli::print_json(&payload);
    } else {
        println!();
        println!(
            "  activated {} with event {}",
            actor,
            outcome["activation_event_id"].as_str().unwrap_or("?")
        );
        if let Some(publication) = outcome.get("publication") {
            println!("  publication: {publication}");
        }
    }
}
