//! CLI edge for the protected one-shot actor-bootstrap possession proof.
//!
//! This module does not install `.vela/actors.json`. It only constructs the
//! exact closed request, invokes the pinned one-shot signer helper, and
//! verifies the returned proof. The caller must keep its repository
//! transaction barrier held while this exchange runs and bind the same request
//! and response into the transaction read set.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use vela_protocol::frontier_profile::FrontierProfileV1;
use vela_protocol::frontier_repo::{FrontierProfileFile, read_repository_profile};
use vela_protocol::sign::ActorRecord;

use crate::config::binary_pin::PinState;
use crate::config::cli_identity;

const ACTOR_BOOTSTRAP_CONSEQUENCE: &str = concat!(
    "Register this one human key as the first repository actor. ",
    "This proves key possession only; it does not accept scientific state or activate policy."
);

#[derive(Debug, Clone)]
pub(crate) struct ActorBootstrapProofInputs {
    pub(crate) frontier: PathBuf,
    pub(crate) profile: FrontierProfileV1,
    pub(crate) actor_record: ActorRecord,
    pub(crate) actor_registry_root_before: String,
    pub(crate) event_log_root: String,
    pub(crate) event_count: u64,
    pub(crate) snapshot_root: String,
    pub(crate) reason: String,
    pub(crate) observed_at: String,
    /// Derived from an identity loaded through
    /// `cli_identity::load_administrative_identity`, never from ambient HOME.
    pub(crate) signer: cli_identity::ProtectedSignerProfile,
}

#[derive(Debug, Clone)]
pub(crate) struct ActorBootstrapProofExchange {
    pub(crate) request: vela_signer::ActorBootstrapProofRequest,
    pub(crate) response: vela_signer::ActorBootstrapProofResponse,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActorBootstrapInstallResult {
    pub(crate) schema: String,
    pub(crate) ok: bool,
    pub(crate) command: String,
    pub(crate) frontier_id: String,
    pub(crate) actor_id: String,
    pub(crate) public_key: String,
    pub(crate) actor_record_root: String,
    pub(crate) actor_registry_root_before: String,
    pub(crate) actor_registry_root_after: String,
    pub(crate) proof_request_root: String,
    pub(crate) operation_id: String,
    pub(crate) transaction_plan_root: String,
    pub(crate) canonical_delta_root: String,
    pub(crate) next_action: String,
}

pub(crate) fn install_protected_actor_bootstrap(
    frontier: &Path,
    identity: &cli_identity::Identity,
    actor_record: ActorRecord,
) -> Result<ActorBootstrapInstallResult, String> {
    install_protected_actor_bootstrap_with(frontier, identity, actor_record, |inputs| {
        request_protected_actor_bootstrap_proof(inputs)
    })
}

fn install_protected_actor_bootstrap_with(
    frontier: &Path,
    identity: &cli_identity::Identity,
    actor_record: ActorRecord,
    proof: impl FnOnce(ActorBootstrapProofInputs) -> Result<ActorBootstrapProofExchange, String>,
) -> Result<ActorBootstrapInstallResult, String> {
    if identity.actor_type != "human"
        || identity.actor_id.starts_with("agent:")
        || identity.actor_id.starts_with("ci:")
        || identity.actor_id != actor_record.id
        || identity.pubkey != actor_record.public_key
    {
        return Err(
            "actor bootstrap requires one exact protected human identity matching the candidate actor"
                .to_string(),
        );
    }
    let signer = cli_identity::protected_signer_profile_for(identity)?;
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)?;
    let barrier = crate::frontier_txn::FrontierTxn::acquire_actor_registry_write_barrier(
        frontier,
        &journal_dir,
    )
    .map_err(|error| error.to_string())?;
    let before = vela_protocol::repo::load_from_path(frontier)?;
    if !before.actors.is_empty() {
        return Err(
            "actor registry is already established; bootstrap cannot extend or replace it"
                .to_string(),
        );
    }
    let profile = match read_repository_profile(frontier)? {
        Some(FrontierProfileFile::V1(profile)) => profile,
        _ => return Err("actor bootstrap requires vela.frontier-profile.v1".to_string()),
    };
    let actor_bytes = vela_protocol::frontier_repo::read_repository_control_text(
        frontier,
        Path::new(".vela/actors.json"),
        ".vela/actors.json",
    )?
    .ok_or_else(|| "actor bootstrap requires the canonical empty actor registry".to_string())?
    .into_bytes();
    let actor_registry_root_before =
        format!("sha256:{}", hex::encode(Sha256::digest(&actor_bytes)));
    let canonical_empty_root = vela_signer::actor_registry_file_root(&[])?;
    if actor_registry_root_before != canonical_empty_root {
        return Err(
            "actor bootstrap requires byte-exact canonical empty .vela/actors.json".to_string(),
        );
    }
    let event_log_root = format!(
        "sha256:{}",
        vela_protocol::events::event_log_hash(&before.events)
    );
    let snapshot_root = format!("sha256:{}", vela_protocol::events::snapshot_hash(&before));
    let reason = "Establish the first protected repository administrator identity.".to_string();
    let exchange = proof(ActorBootstrapProofInputs {
        frontier: frontier.to_path_buf(),
        profile,
        actor_record: actor_record.clone(),
        actor_registry_root_before: actor_registry_root_before.clone(),
        event_log_root,
        event_count: before.events.len() as u64,
        snapshot_root,
        reason,
        observed_at: actor_record.created_at.clone(),
        signer,
    })?;
    vela_signer::validate_actor_bootstrap_response(&exchange.request, &exchange.response)?;
    if exchange.request.actor_record != actor_record
        || exchange.request.actor_registry_root_before != actor_registry_root_before
    {
        return Err("actor-bootstrap proof does not bind the planned registry delta".to_string());
    }

    let mut after: vela_protocol::project::Project =
        serde_json::from_value(serde_json::to_value(&before).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    after.actors.push(actor_record.clone());
    let managed = vela_protocol::repo::render_vela_repo_files(frontier, &after)?;
    let writes = crate::frontier_txn::PlannedWrite::from_managed_files(managed)
        .map_err(|error| error.to_string())?;
    let proof_request_root = vela_signer::actor_bootstrap_request_root(&exchange.request)?;
    let result = serde_json::json!({
        "schema": "vela.actor-bootstrap-result.v1",
        "proof_request": exchange.request,
        "proof_response": exchange.response,
    });
    let read_set = ["frontier.yaml", ".vela/settings.toml", ".vela/actors.json"]
        .into_iter()
        .map(|path| {
            crate::frontier_txn::InputBinding::existing_file(
                frontier,
                crate::frontier_txn::RepoPath::parse(path.to_string())?,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let completed = crate::frontier_txn::execute_no_event_transaction(
        barrier,
        frontier,
        "actor-bootstrap",
        crate::frontier_txn::ContentDigest::parse(proof_request_root.clone())
            .map_err(|error| error.to_string())?,
        &actor_record.created_at,
        &before,
        writes,
        read_set,
        result,
    )
    .map_err(|error| error.to_string())?
    .ok_or_else(|| "actor bootstrap produced no canonical delta".to_string())?;

    let installed = vela_protocol::repo::load_from_path(frontier)?;
    if installed.actors != [actor_record.clone()] {
        return Err(
            "actor-bootstrap transaction completed without the exact one-actor registry"
                .to_string(),
        );
    }
    let actor_registry_root_after = vela_signer::actor_registry_file_root(&installed.actors)?;
    if actor_registry_root_after != exchange.response.actor_registry_root_after {
        return Err("installed actor registry root differs from the protected proof".to_string());
    }
    Ok(ActorBootstrapInstallResult {
        schema: "vela.actor-bootstrap-result.v1".to_string(),
        ok: true,
        command: "actor.add".to_string(),
        frontier_id: before.frontier_id().to_string(),
        actor_id: actor_record.id,
        public_key: actor_record.public_key,
        actor_record_root: exchange.response.actor_record_root,
        actor_registry_root_before,
        actor_registry_root_after,
        proof_request_root,
        operation_id: completed.operation_id,
        transaction_plan_root: completed.plan_root,
        canonical_delta_root: completed.canonical_delta_root,
        next_action: "commit the exact actor-bootstrap delta, then run `vela frontier bind` to create and independently pin the first signed repository boundary".to_string(),
    })
}

pub(crate) fn request_protected_actor_bootstrap_proof(
    inputs: ActorBootstrapProofInputs,
) -> Result<ActorBootstrapProofExchange, String> {
    let vela_binary =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let vela_binary_sha256 = vela_signer::contract::file_sha256(&vela_binary)?;
    match crate::config::binary_pin::pin_state()? {
        PinState::Match(pin) if format!("sha256:{}", pin.sha256) == vela_binary_sha256 => {}
        PinState::Match(_) => {
            return Err(
                "actor bootstrap binary pin differs from the running Vela bytes".to_string(),
            );
        }
        PinState::Mismatch { .. } => {
            return Err(
                "actor bootstrap requires the running Vela binary to match its human pin"
                    .to_string(),
            );
        }
        PinState::Unpinned => {
            return Err(
                "actor bootstrap requires a pinned Vela binary; run `vela id pin-binary` first"
                    .to_string(),
            );
        }
    }
    let helper = cli_identity::signer_helper_path(&vela_binary)?;
    let helper_sha256 = vela_signer::contract::file_sha256(&helper)?;
    if helper_sha256 != inputs.signer.helper_sha256 {
        return Err(
            "actor bootstrap helper bytes differ from the protected identity binding".to_string(),
        );
    }

    let signer = inputs.signer.clone();
    let request = build_actor_bootstrap_request(
        inputs,
        &vela_binary,
        &vela_binary_sha256,
        &helper_sha256,
        &signer,
    )?;
    let response = invoke_actor_bootstrap_helper(&helper, &request)?;
    vela_signer::validate_actor_bootstrap_response(&request, &response)?;
    Ok(ActorBootstrapProofExchange { request, response })
}

fn build_actor_bootstrap_request(
    inputs: ActorBootstrapProofInputs,
    vela_binary: &Path,
    vela_binary_sha256: &str,
    helper_sha256: &str,
    signer: &cli_identity::ProtectedSignerProfile,
) -> Result<vela_signer::ActorBootstrapProofRequest, String> {
    let frontier = inputs
        .frontier
        .canonicalize()
        .map_err(|error| format!("resolve actor-bootstrap Frontier: {error}"))?;
    let profile_root = inputs.profile.profile_root()?;
    let actor_record_root = vela_signer::actor_record_root(&inputs.actor_record)?;
    let actor_registry_root_after =
        vela_signer::actor_registry_file_root(std::slice::from_ref(&inputs.actor_record))?;
    let mut nonce = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let now = chrono::Utc::now();
    let request = vela_signer::ActorBootstrapProofRequest {
        schema: vela_signer::ACTOR_BOOTSTRAP_REQUEST_SCHEMA.to_string(),
        nonce: hex::encode(nonce),
        expires_at: (now + chrono::Duration::seconds(120))
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        vela_binary_path: vela_binary.display().to_string(),
        vela_binary_sha256: vela_binary_sha256.to_string(),
        helper_sha256: helper_sha256.to_string(),
        frontier_id: inputs.profile.frontier_id.clone(),
        frontier_path: frontier.display().to_string(),
        profile: inputs.profile,
        profile_root,
        actor_id: inputs.actor_record.id.clone(),
        actor_public_key: inputs.actor_record.public_key.clone(),
        actor_record: inputs.actor_record,
        actor_record_root,
        actor_registry_root_before: inputs.actor_registry_root_before,
        actor_registry_root_after,
        event_log_root: inputs.event_log_root,
        event_count: inputs.event_count,
        snapshot_root: inputs.snapshot_root,
        reason: inputs.reason,
        observed_at: inputs.observed_at,
        provider: signer.provider.clone(),
        protection_grade: signer.protection_grade.clone(),
        protection_mode: signer.mode,
        display: vela_signer::ActorBootstrapDisplay {
            frontier_name: String::new(),
            actor: String::new(),
            consequence: ACTOR_BOOTSTRAP_CONSEQUENCE.to_string(),
        },
    };
    let mut request = request;
    request.display.frontier_name = request.profile.name.clone();
    request.display.actor = request.actor_id.clone();
    vela_signer::validate_actor_bootstrap_request(&request, now)?;
    Ok(request)
}

fn invoke_actor_bootstrap_helper(
    helper: &Path,
    request: &vela_signer::ActorBootstrapProofRequest,
) -> Result<vela_signer::ActorBootstrapProofResponse, String> {
    let bytes = serde_json::to_vec(request)
        .map_err(|error| format!("encode actor-bootstrap proof request: {error}"))?;
    let mut child = Command::new(helper)
        .arg("prove-actor-bootstrap")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start pinned signer helper {}: {error}", helper.display()))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "signer helper stdin is unavailable".to_string())?
        .write_all(&bytes)
        .map_err(|error| format!("write actor-bootstrap proof request: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for actor-bootstrap signer helper: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "actor-bootstrap proof failed or was cancelled: {}",
            crate::cli::safe_text::inline(String::from_utf8_lossy(&output.stderr).trim())
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("decode actor-bootstrap proof response: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn fixture() -> (
        tempfile::TempDir,
        PathBuf,
        SigningKey,
        cli_identity::Identity,
        ActorRecord,
    ) {
        let temporary = tempfile::tempdir().unwrap();
        let frontier = temporary.path().join("frontier");
        vela_protocol::frontier_repo::initialize_profile_v1_minimal(
            &frontier,
            vela_protocol::frontier_repo::ProfileV1InitOptions {
                name: "Actor bootstrap fixture",
                scope: "Can protected proof install exactly one initial actor?",
                initialize_git: false,
            },
        )
        .unwrap();
        let key = SigningKey::from_bytes(&[0x73; 32]);
        let public_key = vela_protocol::sign::pubkey_hex(&key);
        let helper_sha256 = format!("sha256:{}", "a".repeat(64));
        let identity = cli_identity::Identity {
            version: "2.0".to_string(),
            actor_id: "reviewer:bootstrap".to_string(),
            actor_type: "human".to_string(),
            key_path: String::new(),
            pubkey: public_key.clone(),
            signer: Some(cli_identity::IdentitySigner::Helper {
                provider: "os_store".to_string(),
                key_id: format!("reviewer:bootstrap:{public_key}"),
                public_key: public_key.clone(),
                protection_grade: "user_session".to_string(),
                mode: "always".to_string(),
                helper_sha256,
                pending_source_removal: None,
                pending_vela_binary_sha256: None,
            }),
        };
        let observed_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let actor = ActorRecord {
            id: identity.actor_id.clone(),
            public_key,
            algorithm: "ed25519".to_string(),
            created_at: observed_at,
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        };
        (temporary, frontier, key, identity, actor)
    }

    fn proof_exchange(
        inputs: ActorBootstrapProofInputs,
        key: &SigningKey,
    ) -> ActorBootstrapProofExchange {
        let signer = inputs.signer.clone();
        let binary = std::env::current_exe().unwrap();
        let binary_sha256 = vela_signer::contract::file_sha256(&binary).unwrap();
        let helper_sha256 = signer.helper_sha256.clone();
        let request =
            build_actor_bootstrap_request(inputs, &binary, &binary_sha256, &helper_sha256, &signer)
                .unwrap();
        let mut response = vela_signer::ActorBootstrapProofResponse {
            schema: vela_signer::ACTOR_BOOTSTRAP_RESPONSE_SCHEMA.to_string(),
            request_root: vela_signer::actor_bootstrap_request_root(&request).unwrap(),
            frontier_id: request.frontier_id.clone(),
            profile_root: request.profile_root.clone(),
            actor_id: request.actor_id.clone(),
            actor_public_key: request.actor_public_key.clone(),
            actor_record_root: request.actor_record_root.clone(),
            actor_registry_root_before: request.actor_registry_root_before.clone(),
            actor_registry_root_after: request.actor_registry_root_after.clone(),
            helper_version: "test".to_string(),
            helper_sha256: request.helper_sha256.clone(),
            provider: request.provider.clone(),
            protection_grade: request.protection_grade.clone(),
            approved_at: request.observed_at.clone(),
            protection_mode: request.protection_mode,
            signature: String::new(),
        };
        let signature =
            key.sign(&vela_signer::actor_bootstrap_response_signing_bytes(&response).unwrap());
        response.signature = format!("v1:{}", hex::encode(signature.to_bytes()));
        vela_signer::validate_actor_bootstrap_response(&request, &response).unwrap();
        ActorBootstrapProofExchange { request, response }
    }

    fn contains_commit_marker(path: &Path) -> bool {
        let Ok(entries) = std::fs::read_dir(path) else {
            return false;
        };
        entries.flatten().any(|entry| {
            let path = entry.path();
            if path.is_dir() {
                contains_commit_marker(&path)
            } else {
                path.parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "committed")
            }
        })
    }

    #[test]
    fn protected_proof_installs_exact_actor_through_transaction() {
        let (_temporary, frontier, key, identity, actor) = fixture();
        let result =
            install_protected_actor_bootstrap_with(&frontier, &identity, actor.clone(), |inputs| {
                Ok(proof_exchange(inputs, &key))
            })
            .unwrap();
        assert_eq!(result.actor_id, actor.id);
        assert!(result.operation_id.starts_with("vop_"));
        assert!(result.canonical_delta_root.starts_with("sha256:"));
        let installed = vela_protocol::repo::load_from_path(&frontier).unwrap();
        assert_eq!(installed.actors, vec![actor]);
    }

    #[test]
    fn agent_and_failed_proof_cannot_consume_actor_slot() {
        let (_temporary, frontier, key, mut identity, actor) = fixture();
        let actors_path = frontier.join(".vela/actors.json");
        let before = std::fs::read(&actors_path).unwrap();
        identity.actor_id = "agent:bootstrap".to_string();
        identity.actor_type = "agent".to_string();
        let error =
            install_protected_actor_bootstrap_with(&frontier, &identity, actor.clone(), |_| {
                panic!("agent rejection must precede proof access")
            })
            .unwrap_err();
        assert!(error.contains("protected human identity"), "{error}");
        assert_eq!(std::fs::read(&actors_path).unwrap(), before);

        let (_temporary, frontier, _key, identity, actor) = fixture();
        let actors_path = frontier.join(".vela/actors.json");
        let before = std::fs::read(&actors_path).unwrap();
        let error = install_protected_actor_bootstrap_with(&frontier, &identity, actor, |_| {
            Err("protected key is absent".to_string())
        })
        .unwrap_err();
        assert!(error.contains("protected key is absent"), "{error}");
        assert_eq!(std::fs::read(&actors_path).unwrap(), before);
        assert!(!contains_commit_marker(
            &frontier.join(".vela/operation-journals")
        ));
        drop(key);
    }

    #[test]
    fn post_proof_repository_drift_fails_before_marker_or_actor_write() {
        let (_temporary, frontier, key, identity, actor) = fixture();
        let actors_path = frontier.join(".vela/actors.json");
        let before = std::fs::read(&actors_path).unwrap();
        let settings_path = frontier.join(".vela/settings.toml");
        let error = install_protected_actor_bootstrap_with(&frontier, &identity, actor, |inputs| {
            let exchange = proof_exchange(inputs, &key);
            std::fs::write(
                &settings_path,
                "schema = \"vela.frontier-settings.v1\"\n[work]\nlease_ttl_seconds = 3600\n",
            )
            .unwrap();
            Ok(exchange)
        })
        .unwrap_err();
        assert!(
            error.contains("authorization") || error.contains("stale"),
            "{error}"
        );
        assert_eq!(std::fs::read(&actors_path).unwrap(), before);
        assert!(!contains_commit_marker(
            &frontier.join(".vela/operation-journals")
        ));
    }
}
