//! Repository-authority replay and exceptional human-decision support.
//!
//! Predecessor writers are absent from the current product. Current decisions
//! use the verified repository authority already retained by the Frontier.

use std::path::Path;
use std::process::Command;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_authority::runtime_authentication::{AuthenticationRequest, LocalOsSession};
use vela_authority::{CedarEvaluationInput, CedarPolicyMaterial};
use vela_edge::repository_write::{
    AUTHORITY_TRUST_ANCHOR_SCHEMA_V1, AuthorityTrustAnchorV1,
    install_authority_trust_anchor_from_home, load_authority_trust_anchor_from_home,
    rebind_authority_trust_anchor_from_home,
};
use vela_protocol::authority::{
    AUTHORITY_KEY_ALGORITHM, AUTHORITY_KEY_PURPOSE, AUTHORITY_KEYSET_SCHEMA_V1,
    AuthorityEnvelopeV1, AuthorityEventV1, AuthorityKeyV1, AuthorityKeysetV1, AuthorityRecordV1,
    POLICY_BUNDLE_SCHEMA_V1, PolicyBundleV1, PrincipalSnapshotV1, SemanticApprovalV1,
};
use vela_protocol::authority_history::{
    AUTHORITY_INITIALIZE_ACTION, AUTHORITY_INITIALIZED_EVENT_KIND, AuthorityHistoryInput,
    AuthorityHistoryVerification, AuthorityInitializationV1, verify_authority_history,
};
use vela_protocol::canonical::to_canonical_bytes;
use vela_protocol::current_repository::{CURRENT_REPOSITORY_SCHEMA_V3, CurrentRepositoryV3};
use vela_protocol::events::{EventKind, NULL_HASH, StateActor, StateTarget};
use vela_protocol::principal::PrincipalClass;
use vela_protocol::repository_origin::RepositoryOriginV1;

use crate::authority_transaction::{
    AuthorityEventDraft, AuthorityHistorySnapshot, AuthorityObjectDraft,
    AuthorityTransactionRequest, authority_policy_material_paths, execute_authority_transaction,
    execution_binary_sha256,
};
use crate::frontier_txn::{ContentDigest, FrontierTxn, WriteClass};
use crate::repository_authority_provider::{
    SshAgentRepositoryAuthoritySigner, select_repository_authority_identity,
};

use super::{fail_return, print_json};

const HUMAN_AUTHORITY_SCHEMA: &str = r#"
entity Frontier;
entity Human;
entity Proposal;
action "authority_rotate" appliesTo {
    principal: Human,
    resource: Frontier,
    context: {
        exact: Bool,
        authentication: {
            method: String,
            assurance: String,
            authenticated_at: String,
            observed_at: String,
            expires_at: String,
            user_presence: Bool,
            user_verification: Bool,
            recovery_recent: Bool
        }
    }
};
action "authority_close" appliesTo {
    principal: Human,
    resource: Frontier,
    context: {
        exact: Bool,
        authentication: {
            method: String,
            assurance: String,
            authenticated_at: String,
            observed_at: String,
            expires_at: String,
            user_presence: Bool,
            user_verification: Bool,
            recovery_recent: Bool
        }
    }
};
action "policy_rotate" appliesTo {
    principal: Human,
    resource: Frontier,
    context: {
        exact: Bool,
        authentication: {
            method: String,
            assurance: String,
            authenticated_at: String,
            observed_at: String,
            expires_at: String,
            user_presence: Bool,
            user_verification: Bool,
            recovery_recent: Bool
        }
    }
};
action "review_accept" appliesTo {
    principal: Human,
    resource: Proposal,
    context: {
        exact: Bool,
        authentication: {
            method: String,
            assurance: String,
            authenticated_at: String,
            observed_at: String,
            expires_at: String,
            user_presence: Bool,
            user_verification: Bool,
            recovery_recent: Bool
        }
    }
};
action "review_reject" appliesTo {
    principal: Human,
    resource: Proposal,
    context: {
        exact: Bool,
        authentication: {
            method: String,
            assurance: String,
            authenticated_at: String,
            observed_at: String,
            expires_at: String,
            user_presence: Bool,
            user_verification: Bool,
            recovery_recent: Bool
        }
    }
};
"#;

const FRESH_AUTHORITY_INITIALIZE_SCHEMA: &str = r#"
action "authority_initialize" appliesTo {
    principal: Human,
    resource: Frontier,
    context: {
        exact: Bool,
        authentication: {
            method: String,
            assurance: String,
            authenticated_at: String,
            observed_at: String,
            expires_at: String,
            user_presence: Bool,
            user_verification: Bool,
            recovery_recent: Bool
        }
    }
};"#;

const AUTHORITY_POLICY_TESTS_ROOT: &str =
    // sha256("authority-policy-tests.v1|authority_initialize,authority_rotate,\
    // authority_close,policy_rotate,review_accept,review_reject|exact=true|\
    // bound-human-principal|hostile:unbound-principal")
    "sha256:33deb37ad783ce995a3d3463eea05a4d3c3f762e9a5dd46aa9ce079d41a5e56e";

pub(crate) fn fresh_authority_policy_for_frontier(
    frontier: &Path,
    frontier_id: &str,
    principal_id: &str,
    observed_at: &str,
) -> Result<(PolicyBundleV1, CedarEvaluationInput), String> {
    let _ = frontier;
    let _ = observed_at;
    let entities = json!([
        {
            "uid": {"type": "Frontier", "id": frontier_id},
            "attrs": {},
            "parents": []
        },
        {
            "uid": {"type": "Human", "id": principal_id},
            "attrs": {},
            "parents": []
        }
    ]);
    let schema = format!("{HUMAN_AUTHORITY_SCHEMA}\n{FRESH_AUTHORITY_INITIALIZE_SCHEMA}\n");
    let human_policy = human_authority_policy(principal_id, true)?;
    let policies = format!("{human_policy}\n");
    let bundle = PolicyBundleV1 {
        schema: POLICY_BUNDLE_SCHEMA_V1.into(),
        frontier_id: frontier_id.to_string(),
        cedar_schema_root: ContentDigest::hash(schema.as_bytes()).as_str().into(),
        policies_root: ContentDigest::hash(policies.as_bytes()).as_str().into(),
        entities_root: ContentDigest::hash(to_canonical_bytes(&entities)?)
            .as_str()
            .into(),
        tests_root: AUTHORITY_POLICY_TESTS_ROOT.into(),
        engine: vela_protocol::authority::CEDAR_ENGINE.into(),
        engine_version: vela_protocol::authority::CEDAR_ENGINE_VERSION.into(),
        restricted_profile: vela_protocol::authority::CEDAR_PROFILE_V1.into(),
        previous_bundle_root: None,
        authority_summary: "No automatic scientific admission; one local human principal may decide reviews and administer repository authority. Producer and verifier signatures authenticate routine evidence outside this policy.".into(),
    };
    let authorization = CedarEvaluationInput {
        schema,
        policies,
        entities,
        principal: format!("Human::{}", serde_json::to_string(principal_id).unwrap()),
        principal_class: PrincipalClass::Human,
        action: AUTHORITY_INITIALIZE_ACTION.into(),
        resource: format!("Frontier::{}", serde_json::to_string(frontier_id).unwrap()),
        context: json!({"exact": true}),
    };
    let evaluation = vela_authority::evaluate(&CedarEvaluationInput {
        context: json!({
            "exact": true,
            "authentication": {
                "method": "local_os_session",
                "assurance": "local_session",
                "authenticated_at": "2026-07-24T00:00:00Z",
                "observed_at": "2026-07-24T00:00:00Z",
                "expires_at": "2026-07-24T01:00:00Z",
                "user_presence": false,
                "user_verification": false,
                "recovery_recent": false
            }
        }),
        ..authorization.clone()
    });
    if !evaluation.valid
        || !evaluation.diagnostics.is_empty()
        || evaluation.decision != vela_protocol::authority::CedarDecision::Allow
    {
        return Err(format!(
            "initial authority policy does not authorize exact initialization: {:?}",
            evaluation.diagnostics
        ));
    }
    let other_principal = vela_authority::evaluate(&CedarEvaluationInput {
        principal: r#"Human::"unbound-principal""#.into(),
        context: json!({
            "exact": true,
            "authentication": {
                "method": "local_os_session",
                "assurance": "local_session",
                "authenticated_at": "2026-07-24T00:00:00Z",
                "observed_at": "2026-07-24T00:00:00Z",
                "expires_at": "2026-07-24T01:00:00Z",
                "user_presence": false,
                "user_verification": false,
                "recovery_recent": false
            }
        }),
        ..authorization.clone()
    });
    if !other_principal.valid
        || !other_principal.diagnostics.is_empty()
        || other_principal.decision != vela_protocol::authority::CedarDecision::Deny
    {
        return Err("initial authority policy does not deny an unbound human principal".into());
    }
    Ok((bundle, authorization))
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedRepositoryAuthority {
    pub(crate) history: AuthorityHistorySnapshot,
    pub(crate) policy_material: CedarPolicyMaterial,
    pub(crate) verification: AuthorityHistoryVerification,
}

pub(crate) fn cmd_authority_init(
    frontier: &Path,
    key_selector: Option<&str>,
    reason: &str,
    json_out: bool,
) {
    crate::ui::set_mode("authority init", json_out);
    let result = initialize_repository_authority(frontier, key_selector, reason)
        .unwrap_or_else(|error| fail_return(&error));
    if json_out {
        print_json(&result);
    } else {
        println!("repository authority initialized");
        println!("  key: {}", result["repository_key_fingerprint"]);
        println!("  authority record: {}", result["authority_record_id"]);
        println!("  authority root: {}", result["authority_record_root"]);
        println!("  local trust: installed (independent consumers pin the published root once)");
    }
}

pub(crate) fn cmd_authority_trust_pin(
    frontier: &Path,
    record_root: &str,
    previous_record_root: Option<&str>,
    json_out: bool,
) {
    crate::ui::set_mode("authority trust pin", json_out);
    let result = pin_repository_authority(frontier, record_root, previous_record_root)
        .unwrap_or_else(|error| fail_return(&error));
    if json_out {
        print_json(&result);
    } else {
        println!("repository authority pinned");
        println!("  frontier: {}", result["frontier_id"]);
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
    frontier: &Path,
    record_root: &str,
    previous_record_root: Option<&str>,
) -> Result<Value, String> {
    let repository = crate::current_repository::verify_current_repository_at(frontier, true)?;
    let origin = vela_protocol::repository_origin::RepositoryOriginV1::parse(
        &std::fs::read(frontier.join(".vela/origin.json"))
            .map_err(|error| format!("read current repository origin: {error}"))?,
    )?;
    let authority = load_current_repository_authority(frontier, &repository, &origin)?;
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
        frontier_id: repository.frontier_id.clone(),
        first_authority_record_root: observed_root.clone(),
    };
    let user_home =
        crate::frontier_txn::operating_system_account_home().map_err(|error| error.to_string())?;
    let existing = load_authority_trust_anchor_from_home(&user_home, &repository.frontier_id)?;
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
                frontier_id: repository.frontier_id.clone(),
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
        "schema": "vela.authority-trust-pin-result.v1",
        "ok": true,
        "command": "authority.trust.pin",
        "frontier": frontier.display().to_string(),
        "frontier_id": repository.frontier_id,
        "first_authority_record_id": first_record.record_id,
        "first_authority_record_root": observed_root,
        "initial_authority_keyset_root": first_record.content.authority_keyset_root,
        "initial_policy_bundle_root": first_record.content.authorization.policy_bundle_root,
        "boundary_event_id": boundary_event_id,
        "authority_trust_anchor_root": installed.root,
        "authority_trust_anchor_path": installed.path.display().to_string(),
        "operation": operation,
        "writes": writes,
        "frontier_writes": [],
        "authority_granted": false
    }))
}

fn initialize_repository_authority(
    frontier: &Path,
    key_selector: Option<&str>,
    reason: &str,
) -> Result<Value, String> {
    initialize_current_repository_authority(frontier, key_selector, reason)
}

fn initialize_current_repository_authority(
    frontier: &Path,
    key_selector: Option<&str>,
    reason: &str,
) -> Result<Value, String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("authority init requires a non-empty reason".into());
    }
    let profile = crate::current_repository::verify_current_bootstrap_at(frontier)?;
    let profile_root = profile.profile_root()?;
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
        frontier_id: profile.frontier_id.clone(),
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
    let (policy_bundle, authorization_input) = fresh_authority_policy_for_frontier(
        frontier,
        &profile.frontier_id,
        &local.principal_id,
        &recorded_at,
    )?;
    let keyset_root = authority_keyset.root()?;
    let policy_root = policy_bundle.root()?;
    let origin = RepositoryOriginV1::genesis(
        profile.frontier_id.clone(),
        profile_root.clone(),
        reason.to_string(),
    )?;
    let origin_root = origin.canonical_root()?;
    let repository = CurrentRepositoryV3 {
        schema: CURRENT_REPOSITORY_SCHEMA_V3.into(),
        frontier_id: profile.frontier_id.clone(),
        profile_root: profile_root.clone(),
        origin_id: origin.origin_id.clone(),
        origin_root: origin_root.clone(),
        accepted_claims: Vec::new(),
        pending_claims: Vec::new(),
        proposals: Vec::new(),
        submissions: Vec::new(),
        registrations: Vec::new(),
        verifications: Vec::new(),
        artifacts: Vec::new(),
        authority_keyset_root: keyset_root.clone(),
        authority_policy_root: policy_root.clone(),
    };
    repository.verify()?;
    let repository_root = repository.canonical_root()?;
    let initialization = AuthorityInitializationV1 {
        schema: vela_protocol::authority_history::AUTHORITY_INITIALIZATION_SCHEMA_V1.into(),
        frontier_id: profile.frontier_id.clone(),
        initial_event_log_root: empty_repository_event_log_root(),
        initial_actor_registry_root: empty_repository_actor_registry_root(),
        new_authority_keyset_root: keyset_root.clone(),
        new_policy_bundle_root: policy_root.clone(),
        new_principal_id: local.principal_id.clone(),
        minimum_writer_version: env!("CARGO_PKG_VERSION").into(),
        reason: reason.into(),
    };
    initialization.validate()?;
    let intent_digest = ContentDigest::hash(to_canonical_bytes(&json!({
        "schema": "vela.repository-origin-intent.v1",
        "frontier_id": profile.frontier_id,
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
    let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)?;
    let barrier =
        FrontierTxn::acquire_repository_authority_initialization_barrier(frontier, &journal_dir)
            .map_err(|error| error.to_string())?;
    let mut authentication = local;
    let mut signer = SshAgentRepositoryAuthoritySigner::from_environment(
        identity.key_id.clone(),
        &identity.public_key,
    )?;
    let result = execute_authority_transaction(
        barrier,
        frontier,
        AuthorityTransactionRequest {
            history: AuthorityHistorySnapshot {
                frontier_id: profile.frontier_id.clone(),
                initial_event_log_root: empty_repository_event_log_root(),
                initial_actor_registry_root: empty_repository_actor_registry_root(),
                initial_snapshots_preexisting: false,
                authority_keyset,
                policy_bundle,
                retained_authority_keysets: Vec::new(),
                retained_policy_bundles: Vec::new(),
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
            authorization_input,
            semantic_approvals: vec![SemanticApprovalV1 {
                principal_id: principal.principal_id.clone(),
                role: "frontier_administrator".into(),
                action: AUTHORITY_INITIALIZE_ACTION.into(),
                reason: reason.into(),
                approved_at: recorded_at.clone(),
                intent_digest,
            }],
            event_drafts: vec![AuthorityEventDraft {
                kind: EventKind::Other(AUTHORITY_INITIALIZED_EVENT_KIND.into()),
                target: StateTarget {
                    r#type: "frontier".into(),
                    id: profile.frontier_id.clone(),
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
            derived_drafts: Vec::new(),
            next_authority_keyset: None,
            next_policy_bundle: None,
            next_policy_material: None,
            read_set: Vec::new(),
            vela_version: env!("CARGO_PKG_VERSION").into(),
            binary_sha256,
            recorded_at,
        },
        &mut authentication,
        &mut signer,
    )
    .map_err(|error| error.to_string())?;
    let verified = crate::current_repository::load_current_repository_at(frontier, true)?;
    if verified.canonical_root()? != repository_root {
        return Err("native repository genesis replay produced a different manifest".into());
    }
    let add = Command::new("git")
        .current_dir(frontier)
        .args([
            "add",
            "--",
            "README.md",
            "SCOPE.md",
            "VELA.md",
            "frontier.yaml",
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
        ));
    }
    let commit = Command::new("git")
        .current_dir(frontier)
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
        ));
    }
    let verified = crate::current_repository::verify_current_repository_at(frontier, true)?;
    if verified.canonical_root()? != repository_root {
        return Err("native repository genesis replay produced a different manifest".into());
    }
    let git_commit = crate::git_hardened::text(frontier, &["rev-parse", "HEAD^{commit}"])?;
    let git_tree = crate::git_hardened::text(frontier, &["rev-parse", "HEAD^{tree}"])?;
    let local_anchor = AuthorityTrustAnchorV1 {
        schema: AUTHORITY_TRUST_ANCHOR_SCHEMA_V1.to_string(),
        frontier_id: profile.frontier_id.clone(),
        first_authority_record_root: result.authority_record_root.clone(),
    };
    let user_home =
        crate::frontier_txn::operating_system_account_home().map_err(|error| error.to_string())?;
    let installed_anchor =
        install_authority_trust_anchor_from_home(&user_home, &local_anchor).map_err(|error| {
            format!(
                "repository authority initialized at {} but its local trust anchor could not be installed: {error}",
                result.authority_record_root
            )
        })?;
    Ok(json!({
        "schema": "vela.authority-initialization-result.v2",
        "ok": true,
        "frontier": frontier.display().to_string(),
        "frontier_id": profile.frontier_id,
        "principal_id": principal.principal_id,
        "repository_key_id": identity.key_id,
        "repository_key_fingerprint": identity.fingerprint,
        "origin_id": origin.origin_id,
        "origin_root": origin_root,
        "repository_root": repository_root,
        "git_commit": git_commit,
        "git_tree": git_tree,
        "authority_keyset_root": keyset_root,
        "policy_bundle_root": policy_root,
        "authority_record_id": result.authority_record_id,
        "authority_record_root": result.authority_record_root.clone(),
        "event_ids": result.event_ids,
        "after_event_log_root": result.after_event_log_root,
        "local_trust": {
            "schema": AUTHORITY_TRUST_ANCHOR_SCHEMA_V1,
            "frontier_id": profile.frontier_id,
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
pub(crate) fn load_current_repository_authority(
    frontier: &Path,
    repository: &vela_protocol::current_repository::CurrentRepositoryV3,
    origin: &vela_protocol::repository_origin::RepositoryOriginV1,
) -> Result<LoadedRepositoryAuthority, String> {
    if repository.frontier_id != origin.frontier_id
        || repository.profile_root != origin.profile_root
        || repository.origin_id != origin.origin_id
        || repository.origin_root != origin.canonical_root()?
    {
        return Err(
            "current repository authority loader received a mismatched repository origin".into(),
        );
    }
    let genesis_event_log_root = empty_repository_event_log_root();
    let genesis_actor_registry_root = empty_repository_actor_registry_root();
    let initial_event_log_root = origin
        .predecessor
        .as_ref()
        .map_or(genesis_event_log_root.as_str(), |predecessor| {
            predecessor.archived_event_log_root.as_str()
        });
    let initial_actor_registry_root = origin
        .predecessor
        .as_ref()
        .map_or(genesis_actor_registry_root.as_str(), |predecessor| {
            predecessor.archived_actor_registry_root.as_str()
        });
    let authority_root = frontier.join(".vela/authority");
    let retained_authority_keysets =
        read_authority_json_directory::<AuthorityKeysetV1>(&authority_root.join("keysets"))?;
    let retained_policy_bundles =
        read_authority_json_directory::<PolicyBundleV1>(&authority_root.join("policies"))?;
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
        frontier_id: &repository.frontier_id,
        initial_event_log_root,
        initial_actor_registry_root,
        initial_snapshots_preexisting: origin.predecessor.is_some(),
        authority_keysets: &retained_authority_keysets,
        policy_bundles: &retained_policy_bundles,
        authority_events: &authority_events,
        authority_envelopes: &authority_envelopes,
    })?;
    let active_keyset_root = verification
        .final_authority_keyset_root
        .as_deref()
        .ok_or_else(|| "repository authority has no active keyset root".to_string())?;
    let active_policy_root = verification
        .final_policy_bundle_root
        .as_deref()
        .ok_or_else(|| "repository authority has no active policy root".to_string())?;
    if active_keyset_root != repository.authority_keyset_root
        || active_policy_root != repository.authority_policy_root
    {
        return Err("repository manifest does not bind the verified authority heads".into());
    }
    let authority_keyset = retained_authority_keysets
        .iter()
        .find(|keyset| keyset.root().is_ok_and(|root| root == active_keyset_root))
        .cloned()
        .ok_or_else(|| "active repository-authority keyset is missing".to_string())?;
    let policy_bundle = retained_policy_bundles
        .iter()
        .find(|bundle| bundle.root().is_ok_and(|root| root == active_policy_root))
        .cloned()
        .ok_or_else(|| "active repository policy is missing".to_string())?;
    let policy_material = load_retained_policy_material(frontier, &policy_bundle)?;
    Ok(LoadedRepositoryAuthority {
        history: AuthorityHistorySnapshot {
            frontier_id: repository.frontier_id.clone(),
            initial_event_log_root: initial_event_log_root.to_string(),
            initial_actor_registry_root: initial_actor_registry_root.to_string(),
            initial_snapshots_preexisting: origin.predecessor.is_some(),
            authority_keyset,
            policy_bundle,
            retained_authority_keysets,
            retained_policy_bundles,
            authority_events,
            authority_envelopes,
        },
        policy_material,
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

fn load_retained_policy_material(
    frontier: &Path,
    bundle: &PolicyBundleV1,
) -> Result<CedarPolicyMaterial, String> {
    let paths = authority_policy_material_paths(bundle).map_err(|error| error.to_string())?;
    if !paths.iter().all(|path| frontier.join(path).is_file()) {
        return Err(
            "fresh repository authority is missing its retained Cedar policy material".into(),
        );
    }
    let material = CedarPolicyMaterial {
        schema: std::fs::read_to_string(frontier.join(&paths[0]))
            .map_err(|error| format!("read retained Cedar schema: {error}"))?,
        policies: std::fs::read_to_string(frontier.join(&paths[1]))
            .map_err(|error| format!("read retained Cedar policies: {error}"))?,
        entities: serde_json::from_slice(
            &std::fs::read(frontier.join(&paths[2]))
                .map_err(|error| format!("read retained Cedar entities: {error}"))?,
        )
        .map_err(|error| format!("decode retained Cedar entities: {error}"))?,
    };
    material.validate_against(bundle)?;
    Ok(material)
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

fn human_authority_policy(
    principal_id: &str,
    include_initialization: bool,
) -> Result<String, String> {
    if principal_id.trim().is_empty() || principal_id.chars().any(char::is_control) {
        return Err("repository administrator principal is invalid".into());
    }
    let principal = serde_json::to_string(principal_id)
        .map_err(|error| format!("encode repository administrator principal: {error}"))?;
    let initialization = if include_initialization {
        "        Action::\"authority_initialize\",\n"
    } else {
        ""
    };
    Ok(format!(
        r#"permit (
    principal == Human::{principal},
    action in [
{initialization}        Action::"authority_rotate",
        Action::"authority_close",
        Action::"policy_rotate",
        Action::"review_accept",
        Action::"review_reject"
    ],
    resource
) when {{
    context.exact &&
    !context.authentication.recovery_recent
}};"#
    ))
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
    #[cfg(unix)]
    let subject = format!("uid:{}", rustix::process::geteuid().as_raw());
    #[cfg(windows)]
    let subject = local_windows_subject()?;
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

#[cfg(windows)]
fn local_device_identifier() -> Result<String, String> {
    local_windows_subject()
}

#[cfg(windows)]
fn local_windows_subject() -> Result<String, String> {
    let output = Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .map_err(|error| format!("inspect Windows account SID: {error}"))?;
    if !output.status.success() {
        return Err("Windows account SID command failed".into());
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "Windows account SID output is not UTF-8".to_string())?;
    let sid = text
        .split(',')
        .nth(1)
        .map(|value| value.trim().trim_matches('"'))
        .filter(|value| value.starts_with("S-1-"))
        .ok_or_else(|| "Windows account SID is unavailable".to_string())?;
    Ok(format!("sid:{sid}"))
}

fn canonical_root(value: &impl Serialize) -> Result<String, String> {
    Ok(ContentDigest::hash(to_canonical_bytes(value)?)
        .as_str()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn helper_approval_time_is_canonicalized() {
        assert_eq!(
            canonical_whole_second_time("authority approval", "2026-07-25T15:42:06.768282000Z")
                .unwrap(),
            "2026-07-25T15:42:06Z"
        );
        assert!(canonical_whole_second_time("authority approval", "not-a-time").is_err());
    }

    #[test]
    fn fresh_policy_contains_only_human_decision_and_administration_actions() {
        let temporary = TempDir::new().unwrap();
        let (bundle, authorization) = fresh_authority_policy_for_frontier(
            temporary.path(),
            "vfr_0000000000000000",
            "local:device-fixture|uid:501",
            "2026-07-27T00:00:00Z",
        )
        .unwrap();
        assert_eq!(bundle.tests_root, AUTHORITY_POLICY_TESTS_ROOT);
        assert!(authorization.schema.contains("action \"review_accept\""));
        assert!(authorization.schema.contains("action \"review_reject\""));
        assert!(!authorization.schema.contains("entity Agent"));
        assert!(!authorization.schema.contains("action \"work_claim\""));
        assert!(
            !authorization
                .schema
                .contains("action \"submission_register\"")
        );
        assert!(
            !authorization
                .schema
                .contains("action \"verification_import\"")
        );
        assert!(!authorization.schema.contains("receipt_land"));
    }
}
