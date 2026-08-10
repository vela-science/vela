//! Repository-authority replay and exceptional human-decision support.
//!
//! Predecessor writers are absent from the current product. Current decisions
//! use the repository authority already retained on disk.

use std::path::Path;
use std::process::Command;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_authority::runtime_authentication::{AuthenticationRequest, LocalOsSession};
use vela_edge::repository_write::{
    AUTHORITY_TRUST_ANCHOR_SCHEMA_V1, AuthorityTrustAnchorV1,
    install_authority_trust_anchor_from_home, load_authority_trust_anchor_from_home,
    rebind_authority_trust_anchor_from_home,
};
use vela_protocol::authorization::{
    AUTHORIZATION_MODEL_SCHEMA_V1, AUTHORIZATION_PROFILE_V1, AUTHORIZATION_REQUEST_SCHEMA_V1,
    AuthorityActionV1, AuthorityMemberV1, AuthorityResourceTypeV1, AuthorityRoleV1,
    AuthorizationModelV1, AuthorizationRequestV1, AuthorizationResourceV1,
};

/// A placeholder root for the two request fields the preflight overwrites.
///
/// The request has to validate before it reaches the evaluator, and both
/// fields are full roots, so they cannot simply be absent.
const NULL_AUTHENTICATION_ROOT: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
use vela_protocol::authority::{
    AUTHORITY_KEY_ALGORITHM, AUTHORITY_KEY_PURPOSE, AUTHORITY_KEYSET_SCHEMA_V1,
    AuthorityEnvelopeV1, AuthorityEventV1, AuthorityKeyV1, AuthorityKeysetV1, AuthorityRecordV1,
    PrincipalSnapshotV1, SemanticApprovalV1,
};
use vela_protocol::authority_history::{
    AUTHORITY_INITIALIZE_ACTION, AUTHORITY_INITIALIZED_EVENT_KIND, AuthorityHistoryInput,
    AuthorityHistoryVerification, AuthorityInitializationV1, verify_authority_history,
};
use vela_protocol::canonical::to_canonical_bytes;
use vela_protocol::events::{EventKind, NULL_HASH, StateActor, StateTarget};
use vela_protocol::principal::PrincipalClass;
use vela_protocol::repository::{REPOSITORY_SCHEMA_V4, RepositoryV4};
use vela_protocol::repository_origin::RepositoryOriginV1;

use crate::authority_transaction::{
    AuthorityEventDraft, AuthorityHistorySnapshot, AuthorityObjectDraft,
    AuthorityTransactionRequest, execute_authority_transaction, execution_binary_sha256,
};
use crate::repository_authority_provider::{
    SshAgentRepositoryAuthoritySigner, select_repository_authority_identity,
};
use vela_repository::{ContentDigest, WriteClass};

use super::{fail_return, print_json};

/// The authorization model a fresh repository starts with.
///
/// This used to mint a Cedar policy bundle: an entity snapshot, a schema
/// declaring five actions and an authentication context record, a policy text
/// naming the principal, a frozen tests root, and two live Cedar evaluations —
/// one asserting the bound principal is allowed and one asserting an unbound
/// principal is denied — before any of it could be written down. All of it
/// said one thing, and the closed model says that thing: this principal is a
/// human who holds both roles on this repository.
///
/// The two self-evaluations went with it. They were checking that a generated
/// policy compiled to the rule its generator intended; a model that *is* the
/// rule has nothing to check, and `authorization::tests` holds the evaluator
/// to both answers directly.
pub(crate) fn fresh_authority_model(
    repository_id: &str,
    principal_id: &str,
) -> Result<AuthorizationModelV1, String> {
    let model = AuthorizationModelV1 {
        schema: AUTHORIZATION_MODEL_SCHEMA_V1.into(),
        profile: AUTHORIZATION_PROFILE_V1.into(),
        repository_id: repository_id.to_string(),
        members: vec![
            AuthorityMemberV1 {
                principal_id: principal_id.to_string(),
                principal_class: PrincipalClass::Human,
                role: AuthorityRoleV1::Administrator,
            },
            AuthorityMemberV1 {
                principal_id: principal_id.to_string(),
                principal_class: PrincipalClass::Human,
                role: AuthorityRoleV1::Reviewer,
            },
        ],
        previous_model_root: None,
    };
    model.validate()?;
    Ok(model)
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedRepositoryAuthority {
    pub(crate) history: AuthorityHistorySnapshot,
    pub(crate) verification: AuthorityHistoryVerification,
}

pub(crate) fn cmd_authority_trust_pin(
    repository_path: &Path,
    record_root: &str,
    previous_record_root: Option<&str>,
    json_out: bool,
) {
    crate::ui::set_mode("authority trust pin", json_out);
    let result = pin_repository_authority(repository_path, record_root, previous_record_root)
        .unwrap_or_else(|error| fail_return(&error));
    if json_out {
        print_json(&result);
    } else {
        println!("repository authority pinned");
        println!("  repository: {}", result["repository_id"]);
        println!(
            "  sequence-1 record: {}",
            result["first_authority_record_root"]
        );
        println!("  local anchor: {}", result["authority_trust_anchor_root"]);
        println!("  operation: {}", result["operation"]);
        println!("  authority granted: none");
    }
}

fn pin_repository_authority(
    repository_path: &Path,
    record_root: &str,
    previous_record_root: Option<&str>,
) -> Result<Value, String> {
    let repository = crate::repository::verify_repository_at(repository_path, true)?;
    let origin = vela_protocol::repository_origin::RepositoryOriginV1::parse(
        &std::fs::read(repository_path.join(".vela/origin.json"))
            .map_err(|error| format!("read current repository origin: {error}"))?,
    )?;
    let authority = load_repository_authority(repository_path, &repository, &origin)?;
    let first_envelope = authority
        .history
        .authority_envelopes
        .first()
        .ok_or_else(|| "repository authority has no sequence-1 record".to_string())?;
    let first_record = authority_record_from_envelope(first_envelope)?;
    if first_record.content.sequence != 1
        || first_record
            .content
            .previous_authority_record_root
            .is_some()
    {
        return Err("first repository-authority record is not sequence 1".to_string());
    }
    let observed_root = first_record.root()?;
    if observed_root != record_root {
        return Err(format!(
            "independently supplied authority root {record_root} does not match sequence-1 record {observed_root}"
        ));
    }
    let anchor = AuthorityTrustAnchorV1 {
        schema: AUTHORITY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
        repository_id: repository.repository_id.clone(),
        first_authority_record_root: observed_root.clone(),
    };
    let user_home = crate::repository_write_policy::operating_system_account_home()
        .map_err(|error| error.to_string())?;
    let existing = load_authority_trust_anchor_from_home(&user_home, &repository.repository_id)?;
    let (installed, operation, writes) = match existing {
        Some(existing) if existing.anchor == anchor => (existing, "unchanged", Vec::new()),
        Some(existing) => {
            let previous_root = previous_record_root.ok_or_else(|| {
                format!(
                    "authority trust anchor already pins {}; to advance it, repeat with \
                     --previous-record-root {}",
                    existing.anchor.first_authority_record_root,
                    existing.anchor.first_authority_record_root,
                )
            })?;
            let expected = AuthorityTrustAnchorV1 {
                schema: AUTHORITY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
                repository_id: repository.repository_id.clone(),
                first_authority_record_root: previous_root.to_string(),
            };
            let rebound = rebind_authority_trust_anchor_from_home(&user_home, &expected, &anchor)?;
            let path = rebound.path.display().to_string();
            (rebound, "rebound", vec![path])
        }
        None => {
            if previous_record_root.is_some() {
                return Err(
                    "--previous-record-root requires an existing exact local pin".to_string(),
                );
            }
            let installed = install_authority_trust_anchor_from_home(&user_home, &anchor)?;
            let path = installed.path.display().to_string();
            (installed, "installed", vec![path])
        }
    };
    let boundary_event_id = authority
        .verification
        .initialization_event_id
        .clone()
        .ok_or_else(|| "repository authority has no initialization boundary".to_string())?;
    Ok(json!({
        "schema": "vela.authority-trust-pin-result.v2",
        "ok": true,
        "command": "authority.trust.pin",
        "repository_path": repository_path.display().to_string(),
        "repository_id": repository.repository_id,
        "first_authority_record_id": first_record.record_id,
        "first_authority_record_root": observed_root,
        "initial_authority_keyset_root": first_record.content.authority_keyset_root,
        "initial_authorization_model_root": first_record.content.authorization.model_root,
        "boundary_event_id": boundary_event_id,
        "authority_trust_anchor_root": installed.root,
        "authority_trust_anchor_path": installed.path.display().to_string(),
        "operation": operation,
        "writes": writes,
        "repository_writes": [],
        "authority_granted": false
    }))
}

/// Why `vela init` could not establish repository authority.
///
/// The two cases need opposite remedies, and a caller that cannot tell them
/// apart will hand an operator the wrong one: the trust pin is installed after
/// the record is signed, committed, and replay-verified, so a pin collision is
/// not a signing failure and no key operation can clear it.
pub(crate) enum RepositoryAuthorityInitError {
    /// Identity selection, signing, or repository genesis failed.
    Signing(String),
    /// A pre-existing local pin makes fresh initialization inapplicable. No
    /// authority record has been signed or installed yet.
    TrustPinBlocksInitialization {
        repository_id: String,
        pin_path: String,
        pinned_root: String,
    },
    /// The authority record was established, but the local trust pin for this
    /// repository already selects a different sequence-one root. The repository
    /// still cannot take an authority write until the pin is reconciled.
    TrustPinCollision {
        repository_id: String,
        record_root: String,
        pin_path: String,
        pinned_root: String,
    },
}

impl From<String> for RepositoryAuthorityInitError {
    fn from(error: String) -> Self {
        Self::Signing(error)
    }
}

impl From<&str> for RepositoryAuthorityInitError {
    fn from(error: &str) -> Self {
        Self::Signing(error.to_string())
    }
}

pub(crate) fn initialize_repository_authority(
    repository_path: &Path,
    key_selector: Option<&str>,
    reason: &str,
) -> Result<Value, RepositoryAuthorityInitError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("init requires a non-empty authority reason".into());
    }
    let profile = crate::repository::verify_bootstrap_at(repository_path)?;
    let profile_root = profile.profile_root()?;
    let initial_user_home = crate::repository_write_policy::operating_system_account_home()
        .map_err(|error| error.to_string())?;
    if let Some(existing) =
        load_authority_trust_anchor_from_home(&initial_user_home, &profile.repository_id)?
    {
        return Err(RepositoryAuthorityInitError::TrustPinBlocksInitialization {
            repository_id: profile.repository_id,
            pin_path: existing.path.display().to_string(),
            pinned_root: existing.anchor.first_authority_record_root,
        });
    }
    let identity = select_repository_authority_identity(key_selector)?;
    let recorded_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let local = local_session(&recorded_at)?;
    let principal = PrincipalSnapshotV1 {
        principal_id: local.principal_id.clone(),
        principal_class: PrincipalClass::Human,
        display_name: Some("Repository administrator".into()),
        affiliation: None,
        account_links: vec![local.principal_id.clone()],
    };
    let authority_keyset = AuthorityKeysetV1 {
        schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
        repository_id: profile.repository_id.clone(),
        generation: 1,
        threshold: 1,
        keys: vec![AuthorityKeyV1 {
            key_id: identity.key_id.clone(),
            algorithm: AUTHORITY_KEY_ALGORITHM.into(),
            public_key: identity.public_key.clone(),
            valid_from_sequence: 1,
            valid_through_sequence: None,
            purpose: AUTHORITY_KEY_PURPOSE.into(),
        }],
        previous_keyset_root: None,
        activation_record_root: None,
        closed: false,
    };
    authority_keyset.validate()?;
    let authorization_model = fresh_authority_model(&profile.repository_id, &local.principal_id)?;
    let keyset_root = authority_keyset.root()?;
    let model_root = authorization_model.root()?;
    let origin = RepositoryOriginV1::genesis(
        profile.repository_id.clone(),
        profile_root.clone(),
        reason.to_string(),
    )?;
    let origin_root = origin.canonical_root()?;
    let repository = RepositoryV4 {
        schema: REPOSITORY_SCHEMA_V4.into(),
        repository_id: profile.repository_id.clone(),
        profile_root: profile_root.clone(),
        origin_id: origin.id()?,
        origin_root: origin_root.clone(),
        accepted_claims: Vec::new(),
        pending_claims: Vec::new(),
        proposals: Vec::new(),
        proposal_withdrawals: Vec::new(),
        submissions: Vec::new(),
        verifications: Vec::new(),
        artifacts: Vec::new(),
        authority_keyset_root: keyset_root.clone(),
        authority_model_root: model_root.clone(),
    };
    repository.verify()?;
    let repository_root = repository.canonical_root()?;
    let initialization = AuthorityInitializationV1 {
        schema: vela_protocol::authority_history::AUTHORITY_INITIALIZATION_SCHEMA_V1.into(),
        repository_id: profile.repository_id.clone(),
        initial_event_log_root: empty_repository_event_log_root(),
        initial_actor_registry_root: empty_repository_actor_registry_root(),
        new_authority_keyset_root: keyset_root.clone(),
        new_authorization_model_root: model_root.clone(),
        new_principal_id: local.principal_id.clone(),
        minimum_writer_version: env!("CARGO_PKG_VERSION").into(),
        reason: reason.into(),
    };
    initialization.validate()?;
    let intent_digest = ContentDigest::hash(to_canonical_bytes(&json!({
        "schema": "vela.repository-origin-intent.v1",
        "repository_id": profile.repository_id,
        "profile_root": profile_root,
        "origin_root": origin_root,
        "repository_root": repository_root,
        "principal_id": principal.principal_id,
        "reason": reason,
    }))?)
    .as_str()
    .to_string();
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve current Vela binary: {error}"))?;
    let binary_sha256 = execution_binary_sha256(&executable)?;
    let journal_dir = crate::repository_ops::repository_transaction_journal_dir(repository_path)?;
    let barrier = match crate::repository_write_policy::acquire_fresh_repository_write_barrier(
        repository_path,
        &journal_dir,
    ) {
        Ok(barrier) => barrier,
        Err(error) => {
            let current_user_home = crate::repository_write_policy::operating_system_account_home()
                .map_err(|home_error| home_error.to_string())?;
            if let Some(existing) =
                load_authority_trust_anchor_from_home(&current_user_home, &profile.repository_id)?
            {
                return Err(RepositoryAuthorityInitError::TrustPinBlocksInitialization {
                    repository_id: profile.repository_id,
                    pin_path: existing.path.display().to_string(),
                    pinned_root: existing.anchor.first_authority_record_root,
                });
            }
            return Err(error.to_string().into());
        }
    };
    let mut authentication = local;
    let mut signer = SshAgentRepositoryAuthoritySigner::from_environment(
        identity.key_id.clone(),
        &identity.public_key,
    )?;
    let result = execute_authority_transaction(
        barrier,
        repository_path,
        AuthorityTransactionRequest {
            history: AuthorityHistorySnapshot {
                repository_id: profile.repository_id.clone(),
                initial_event_log_root: empty_repository_event_log_root(),
                initial_actor_registry_root: empty_repository_actor_registry_root(),
                authority_keyset,
                authorization_model,
                retained_authority_keysets: Vec::new(),
                retained_authorization_models: Vec::new(),
                authority_events: Vec::new(),
                authority_envelopes: Vec::new(),
            },
            intent_digest: intent_digest.clone(),
            principal: principal.clone(),
            authentication_request: AuthenticationRequest {
                principal_id: principal.principal_id.clone(),
                principal_class: PrincipalClass::Human,
                transaction_at: recorded_at.clone(),
            },
            authorization_request: AuthorizationRequestV1 {
                schema: AUTHORIZATION_REQUEST_SCHEMA_V1.into(),
                profile: AUTHORIZATION_PROFILE_V1.into(),
                model_root: model_root.clone(),
                repository_id: profile.repository_id.clone(),
                principal_id: principal.principal_id.clone(),
                principal_class: PrincipalClass::Human,
                action: AuthorityActionV1::AuthorityInitialize,
                resource: AuthorizationResourceV1 {
                    repository_id: profile.repository_id.clone(),
                    resource_type: AuthorityResourceTypeV1::Repository,
                    resource_id: profile.repository_id.clone(),
                },
                /* Both are replaced by `preflight_authority_action` from the
                verified session; a caller cannot decide its own recency. */
                authentication_root: NULL_AUTHENTICATION_ROOT.into(),
                recovery_recent: false,
                transaction_read_set_root: NULL_AUTHENTICATION_ROOT.into(),
                intent_digest: intent_digest.clone(),
            },
            /* `role` and the StateTarget type below are pre-ADR-0039 wire
            spellings inside a DSSE-signed preimage. `vela-science/math` holds
            both under a valid signature at genesis. Renaming them changes the
            bytes of future authority records only — replay of existing ones
            reads their own retained bytes and `validate_semantic_approvals`
            checks `role` for non-emptiness rather than against a vocabulary —
            but it is still a wire change, so it waits for one that has a
            reason of its own rather than riding a comment. */
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: principal.principal_id.clone(),
                role: "repository_administrator".into(),
                action: AUTHORITY_INITIALIZE_ACTION.into(),
                reason: reason.into(),
                approved_at: recorded_at.clone(),
                intent_digest,
            }],
            event_drafts: vec![AuthorityEventDraft {
                kind: EventKind::Other(AUTHORITY_INITIALIZED_EVENT_KIND.into()),
                target: StateTarget {
                    r#type: "repository".into(),
                    id: profile.repository_id.clone(),
                },
                actor: StateActor {
                    r#type: "human".into(),
                    id: principal.principal_id.clone(),
                },
                timestamp: recorded_at.clone(),
                reason: reason.into(),
                before_hash: NULL_HASH.into(),
                after_hash: NULL_HASH.into(),
                payload: serde_json::to_value(&initialization)
                    .map_err(|error| format!("encode authority initialization: {error}"))?,
                caveats: vec![
                    "Repository authority authenticates writes; it does not establish scientific truth."
                        .into(),
                ],
            }],
            object_drafts: vec![
                AuthorityObjectDraft {
                    path: ".vela/origin.json".into(),
                    object_kind: "repository_origin".into(),
                    class: WriteClass::CanonicalEvidence,
                    postimage: Some(origin.canonical_bytes()?),
                },
                AuthorityObjectDraft {
                    path: ".vela/repository.json".into(),
                    object_kind: "repository_manifest".into(),
                    class: WriteClass::CanonicalEvidence,
                    postimage: Some(repository.canonical_bytes()?),
                },
            ],
            next_authority_keyset: None,
            next_authorization_model: None,
            read_set: Vec::new(),
            vela_version: env!("CARGO_PKG_VERSION").into(),
            binary_sha256,
            recorded_at,
        },
        &mut authentication,
        &mut signer,
    )
    .map_err(|error| error.to_string())?;
    let verified = crate::repository::load_repository_at(repository_path, true)?;
    if verified.canonical_root()? != repository_root {
        return Err("native repository genesis replay produced a different manifest".into());
    }
    let add = Command::new("git")
        .current_dir(repository_path)
        .args([
            "add",
            "--",
            "README.md",
            "AGENTS.md",
            "CLAUDE.md",
            "vela.toml",
            ".gitignore",
            ".gitattributes",
            ".vela/origin.json",
            ".vela/repository.json",
            ".vela/authority",
        ])
        .output()
        .map_err(|error| format!("stage native repository genesis: {error}"))?;
    if !add.status.success() {
        return Err(format!(
            "native repository genesis is signed but staging failed: {}",
            String::from_utf8_lossy(&add.stderr).trim()
        )
        .into());
    }
    let commit = Command::new("git")
        .current_dir(repository_path)
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=Vela Agent",
            "-c",
            "user.email=agent@vela.space",
            "commit",
            "-m",
            "Initialize current Vela repository",
        ])
        .output()
        .map_err(|error| format!("commit native repository genesis: {error}"))?;
    if !commit.status.success() {
        return Err(format!(
            "native repository genesis is signed and staged but commit failed: {}",
            String::from_utf8_lossy(&commit.stderr).trim()
        )
        .into());
    }
    let verified = crate::repository::verify_repository_at(repository_path, true)?;
    if verified.canonical_root()? != repository_root {
        return Err("native repository genesis replay produced a different manifest".into());
    }
    let git_commit = vela_edge::git::text(repository_path, &["rev-parse", "HEAD^{commit}"])?;
    let git_tree = vela_edge::git::text(repository_path, &["rev-parse", "HEAD^{tree}"])?;
    let local_anchor = AuthorityTrustAnchorV1 {
        schema: AUTHORITY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
        repository_id: profile.repository_id.clone(),
        first_authority_record_root: result.authority_record_root.clone(),
    };
    let user_home = crate::repository_write_policy::operating_system_account_home()
        .map_err(|error| error.to_string())?;
    let installed_anchor = install_authority_trust_anchor_from_home(&user_home, &local_anchor)
        .map_err(|error| match load_authority_trust_anchor_from_home(
            &user_home,
            &profile.repository_id,
        ) {
            Ok(Some(existing)) if existing.anchor != local_anchor => {
                RepositoryAuthorityInitError::TrustPinCollision {
                    repository_id: profile.repository_id.clone(),
                    record_root: result.authority_record_root.clone(),
                    pin_path: existing.path.display().to_string(),
                    pinned_root: existing.anchor.first_authority_record_root.clone(),
                }
            }
            _ => RepositoryAuthorityInitError::Signing(format!(
                "repository authority initialized at {} but its local trust anchor could not be installed: {error}",
                result.authority_record_root
            )),
        })?;
    Ok(json!({
        "schema": "vela.authority-initialization-result.v3",
        "ok": true,
        "repository_path": repository_path.display().to_string(),
        "repository_id": profile.repository_id,
        "principal_id": principal.principal_id,
        "repository_key_id": identity.key_id,
        "repository_key_fingerprint": identity.fingerprint,
        "origin_id": origin.id()?,
        "origin_root": origin_root,
        "repository_root": repository_root,
        "git_commit": git_commit,
        "git_tree": git_tree,
        "authority_keyset_root": keyset_root,
        "model_root": model_root,
        "authority_record_id": result.authority_record_id,
        "authority_record_root": result.authority_record_root.clone(),
        "event_ids": result.event_ids,
        "after_event_log_root": result.after_event_log_root,
        "local_trust": {
            "schema": AUTHORITY_TRUST_ANCHOR_SCHEMA_V1,
            "repository_id": profile.repository_id,
            "first_authority_record_root": result.authority_record_root,
            "anchor_root": installed_anchor.root,
            "anchor_path": installed_anchor.path,
            "installed": true
        },
        "writes_now": true
    }))
}

/// Load repository authority for the current-origin repository.
///
/// A current origin starts a sequence-1 authority history over either its
/// archived predecessor roots or the protocol null roots.
pub(crate) fn load_repository_authority(
    repository_path: &Path,
    repository: &vela_protocol::repository::RepositoryV4,
    origin: &vela_protocol::repository_origin::RepositoryOriginV1,
) -> Result<LoadedRepositoryAuthority, String> {
    if repository.repository_id != origin.repository_id
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin.id()?
        || repository.origin_root != origin.canonical_root()?
    {
        return Err(
            "current repository authority loader received a mismatched repository origin".into(),
        );
    }
    /* Genesis is the only origin, so these are the empty roots. A compaction
    origin used to substitute its predecessor's archived roots here. */
    let initial_event_log_root = empty_repository_event_log_root();
    let initial_actor_registry_root = empty_repository_actor_registry_root();
    let authority_root = repository_path.join(".vela/authority");
    let retained_authority_keysets =
        read_authority_json_directory::<AuthorityKeysetV1>(&authority_root.join("keysets"))?;
    let retained_authorization_models =
        read_authority_json_directory::<AuthorizationModelV1>(&authority_root.join("models"))?;
    let authority_events =
        read_authority_json_directory::<AuthorityEventV1>(&authority_root.join("events"))?;
    let authority_envelopes =
        read_authority_json_directory::<AuthorityEnvelopeV1>(&authority_root.join("records"))?;
    let mut authority_envelopes = authority_envelopes
        .into_iter()
        .map(|envelope| Ok((authority_envelope_sequence(&envelope)?, envelope)))
        .collect::<Result<Vec<_>, String>>()?;
    authority_envelopes.sort_by_key(|(sequence, _)| *sequence);
    let authority_envelopes = authority_envelopes
        .into_iter()
        .map(|(_, envelope)| envelope)
        .collect::<Vec<_>>();
    let verification = verify_authority_history(AuthorityHistoryInput {
        repository_id: &repository.repository_id,
        initial_event_log_root: &initial_event_log_root,
        initial_actor_registry_root: &initial_actor_registry_root,
        authority_keysets: &retained_authority_keysets,
        authorization_models: &retained_authorization_models,
        authority_events: &authority_events,
        authority_envelopes: &authority_envelopes,
    })?;
    let active_keyset_root = verification
        .final_authority_keyset_root
        .as_deref()
        .ok_or_else(|| "repository authority has no active keyset root".to_string())?;
    let active_model_root = verification
        .final_authorization_model_root
        .as_deref()
        .ok_or_else(|| "repository authority has no active policy root".to_string())?;
    if active_keyset_root != repository.authority_keyset_root
        || active_model_root != repository.authority_model_root
    {
        return Err("repository manifest does not bind the verified authority heads".into());
    }
    let authority_keyset = retained_authority_keysets
        .iter()
        .find(|keyset| keyset.root().is_ok_and(|root| root == active_keyset_root))
        .cloned()
        .ok_or_else(|| "active repository-authority keyset is missing".to_string())?;
    let authorization_model = retained_authorization_models
        .iter()
        .find(|bundle| bundle.root().is_ok_and(|root| root == active_model_root))
        .cloned()
        .ok_or_else(|| "active repository policy is missing".to_string())?;
    Ok(LoadedRepositoryAuthority {
        history: AuthorityHistorySnapshot {
            repository_id: repository.repository_id.clone(),
            initial_event_log_root: initial_event_log_root.to_string(),
            initial_actor_registry_root: initial_actor_registry_root.to_string(),
            authority_keyset,
            authorization_model,
            retained_authority_keysets,
            retained_authorization_models,
            authority_events,
            authority_envelopes,
        },
        verification,
    })
}

fn empty_repository_event_log_root() -> String {
    format!("sha256:{}", vela_protocol::events::event_log_hash(&[]))
}

fn empty_repository_actor_registry_root() -> String {
    format!("sha256:{}", hex::encode(Sha256::digest([])))
}

fn authority_envelope_sequence(envelope: &AuthorityEnvelopeV1) -> Result<u64, String> {
    Ok(authority_record_from_envelope(envelope)?.content.sequence)
}

pub(crate) fn authority_record_from_envelope(
    envelope: &AuthorityEnvelopeV1,
) -> Result<AuthorityRecordV1, String> {
    let payload = BASE64_STANDARD
        .decode(&envelope.payload)
        .map_err(|error| format!("authority envelope payload is not base64: {error}"))?;
    let record: AuthorityRecordV1 = serde_json::from_slice(&payload)
        .map_err(|error| format!("authority envelope record JSON is invalid: {error}"))?;
    if to_canonical_bytes(&record)? != payload {
        return Err("authority record payload is not canonical JSON".to_string());
    }
    record.validate()?;
    Ok(record)
}

fn read_authority_json_directory<T>(directory: &Path) -> Result<Vec<T>, String>
where
    T: serde::de::DeserializeOwned,
{
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let metadata = std::fs::symlink_metadata(directory)
        .map_err(|error| format!("inspect {}: {error}", directory.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "authority store {} must be a real directory",
            directory.display()
        ));
    }
    let mut paths = std::fs::read_dir(directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("enumerate {}: {error}", directory.display()))?;
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let metadata = std::fs::symlink_metadata(&path)
                .map_err(|error| format!("inspect {}: {error}", path.display()))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "authority store member {} must be a regular file",
                    path.display()
                ));
            }
            let bytes = std::fs::read(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            serde_json::from_slice(&bytes)
                .map_err(|error| format!("decode {}: {error}", path.display()))
        })
        .collect()
}

pub(crate) fn canonical_whole_second_time(name: &str, value: &str) -> Result<String, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| {
            value
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        })
        .map_err(|error| format!("{name} is not RFC3339: {error}"))
}

pub(crate) fn local_session(observed_at: &str) -> Result<LocalOsSession, String> {
    let observed = DateTime::parse_from_rfc3339(observed_at)
        .map_err(|error| format!("local session observation time is invalid: {error}"))?
        .with_timezone(&Utc);
    let device = local_device_identifier()?;
    let subject = format!("uid:{}", rustix::process::geteuid().as_raw());
    let issuer = format!(
        "device-sha256:{}",
        hex::encode(Sha256::digest(device.as_bytes()))
    );
    let principal_id = format!("local:{issuer}|{subject}");
    let session_root = canonical_root(&json!({
        "schema": "vela.local-os-session-root.v1",
        "issuer": issuer,
        "subject": subject,
        "observed_at": observed_at,
    }))?;
    Ok(LocalOsSession {
        principal_id,
        issuer,
        subject,
        session_root,
        authenticated_at: observed_at.into(),
        expires_at: (observed + Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true),
        recovery_recent: false,
    })
}

#[cfg(target_os = "macos")]
fn local_device_identifier() -> Result<String, String> {
    let output = Command::new("/usr/sbin/ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .map_err(|error| format!("inspect macOS platform identity: {error}"))?;
    if !output.status.success() {
        return Err("macOS platform identity command failed".into());
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "macOS platform identity output is not UTF-8".to_string())?;
    text.lines()
        .find_map(|line| {
            line.split_once("\"IOPlatformUUID\" = \"")
                .and_then(|(_, value)| value.strip_suffix('"'))
                .map(str::to_string)
        })
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "macOS platform identity is unavailable".to_string())
}

#[cfg(target_os = "linux")]
fn local_device_identifier() -> Result<String, String> {
    let value = std::fs::read_to_string("/etc/machine-id")
        .map_err(|error| format!("read Linux machine identity: {error}"))?;
    let value = value.trim();
    if value.len() < 16 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Linux machine identity is malformed".into());
    }
    Ok(value.to_ascii_lowercase())
}

fn canonical_root(value: &impl Serialize) -> Result<String, String> {
    Ok(ContentDigest::hash(to_canonical_bytes(value)?)
        .as_str()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_approval_time_is_canonicalized() {
        assert_eq!(
            canonical_whole_second_time("authority approval", "2026-07-25T15:42:06.768282000Z")
                .unwrap(),
            "2026-07-25T15:42:06Z"
        );
        assert!(canonical_whole_second_time("authority approval", "not-a-time").is_err());
    }

    /// The model a fresh repository starts with authorizes exactly the human
    /// decision and administration actions, and nothing else.
    ///
    /// This used to read the generated Cedar schema text for the absence of
    /// `entity Agent`, `action "work_claim"`, `action "verification_import"`
    /// and `receipt_land` — asserting that a policy generator had not emitted
    /// tokens nobody asked it to emit. The action vocabulary is a closed Rust
    /// enum now, so those four cannot be named at all, and what is worth
    /// checking is what the model decides.
    #[test]
    fn a_fresh_model_authorizes_only_human_decision_and_administration() {
        use vela_protocol::authorization::{
            AUTHORIZATION_REQUEST_SCHEMA_V1, AuthorizationDecisionV1, evaluate_authorization_v1,
        };

        let repository_id = "00000000-0000-4000-8000-000000000000";
        let principal = "local:device-fixture|uid:501";
        let model = fresh_authority_model(repository_id, principal).unwrap();

        let request = |action: AuthorityActionV1, principal_id: &str| AuthorizationRequestV1 {
            schema: AUTHORIZATION_REQUEST_SCHEMA_V1.into(),
            profile: AUTHORIZATION_PROFILE_V1.into(),
            model_root: model.root().unwrap(),
            repository_id: repository_id.into(),
            principal_id: principal_id.into(),
            principal_class: PrincipalClass::Human,
            action,
            resource: match action.required_resource_type() {
                AuthorityResourceTypeV1::Repository => AuthorizationResourceV1 {
                    repository_id: repository_id.into(),
                    resource_type: AuthorityResourceTypeV1::Repository,
                    resource_id: repository_id.into(),
                },
                AuthorityResourceTypeV1::Proposal => AuthorizationResourceV1 {
                    repository_id: repository_id.into(),
                    resource_type: AuthorityResourceTypeV1::Proposal,
                    resource_id: "vpr_0123456789abcdef".into(),
                },
            },
            authentication_root: NULL_AUTHENTICATION_ROOT.into(),
            transaction_read_set_root: NULL_AUTHENTICATION_ROOT.into(),
            intent_digest: NULL_AUTHENTICATION_ROOT.into(),
            recovery_recent: false,
        };

        for action in [
            AuthorityActionV1::AuthorityInitialize,
            AuthorityActionV1::AuthorityRotate,
            AuthorityActionV1::AuthorityClose,
            AuthorityActionV1::AuthorityModelUpdate,
            AuthorityActionV1::ReviewAccept,
            AuthorityActionV1::ReviewReject,
        ] {
            let evaluation =
                evaluate_authorization_v1(&model, &request(action, principal)).unwrap();
            assert_eq!(
                evaluation.decision,
                AuthorizationDecisionV1::Allow,
                "{action:?} is not authorized for the genesis principal"
            );
        }

        // Nobody else holds either role.
        let stranger = request(AuthorityActionV1::ReviewAccept, "local:stranger|uid:999");
        assert_eq!(
            evaluate_authorization_v1(&model, &stranger)
                .unwrap()
                .decision,
            AuthorizationDecisionV1::Deny
        );
    }
}
