//! Repository-authority replay and exceptional human-decision support.
//!
//! Era-0 migration writers are intentionally absent from the current product.
//! Historical migration records remain replayable, while new decisions use the
//! verified repository authority already retained by the Frontier.

use std::path::Path;
use std::process::Command;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_authority::legacy_translation::translate_legacy_policy;
use vela_authority::runtime_authentication::{
    AuthenticationRequest, LocalOsSession, RuntimeSessionState,
};
use vela_authority::{CedarEvaluationInput, CedarPolicyMaterial};
use vela_edge::repository_write::{
    AUTHORITY_TRUST_ANCHOR_SCHEMA_V1, AuthorityTrustAnchorV1,
    install_authority_trust_anchor_from_home,
};
use vela_protocol::authority::{
    AUTHORITY_KEY_ALGORITHM, AUTHORITY_KEY_PURPOSE, AUTHORITY_KEYSET_SCHEMA_V1,
    AuthorityEnvelopeV1, AuthorityEventV1, AuthorityKeyV1, AuthorityKeysetV1, AuthorityRecordV1,
    POLICY_BUNDLE_SCHEMA_V1, PolicyBundleV1, PrincipalSnapshotV1, SemanticApprovalV1,
};
use vela_protocol::authority_history::{
    AUTHORITY_INITIALIZE_ACTION, AUTHORITY_INITIALIZED_EVENT_KIND, AUTHORITY_MIGRATION_ACTION,
    AuthorityHistoryInput, AuthorityHistoryVerification, AuthorityInitializationV1,
    AuthorityModelMigrationV1, POLICY_ROTATE_ACTION, initialization_payload_from_event,
    verify_authority_history,
};
use vela_protocol::canonical::to_canonical_bytes;
use vela_protocol::events::{
    EventKind, NULL_HASH, StateActor, StateEvent, StateTarget, event_log_hash,
};
use vela_protocol::principal_capability::PrincipalClass;
use vela_protocol::project::Project;

use crate::authority_transaction::{
    AuthorityEventDraft, AuthorityHistorySnapshot, AuthorityTransactionRequest,
    authority_policy_material_paths, execute_authority_transaction, execution_binary_sha256,
};
use crate::frontier_txn::{ContentDigest, FrontierTxn};
use crate::repository_authority_provider::{
    SshAgentRepositoryAuthoritySigner, select_repository_authority_identity,
};

use super::{fail_return, print_json};

const POLICY_SHADOW_TESTS_ROOT: &str =
    "sha256:20edc3cef4390127a3f49daf36583f5ac7fa1fd24d2ebd5f7cc75ca1c6e4e41b";

const AUTOMATIC_POLICY_SCHEMA: &str = r#"entity Service;
entity Frontier;
action "automatic_permit" appliesTo {
    principal: Service,
    resource: Frontier,
    context: {
        structuralValid: Bool,
        claimClass: String,
        assuranceLevel: Long,
        impactTier: Long,
        changedFindings: Long,
        downstreamDependents: Long,
        assertionTextMutated: Bool,
        targetContested: Bool,
        governanceMutation: Bool,
        independenceSatisfied: Bool,
        methodIntegritySound: Bool,
        credentialValid: Bool,
        hasUnknownFields: Bool,
        replayability: String,
        executionBindingPresent: Bool,
        executionBindingValid: Bool,
        packetRoot: String,
        profileRoot: String,
        verifierCapsuleRoot: String,
        resultContractRoot: String,
        producerCredentialRootPresent: Bool,
        producerCredentialRoot: String
    }
};"#;

const HUMAN_AUTHORITY_SCHEMA: &str = r#"
entity Human;
entity Proposal;
action "authority_model_migrate" appliesTo {
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

const ROUTINE_WORK_SCHEMA: &str = r#"
entity Agent;
action "work_claim" appliesTo {
    principal: Agent,
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

const ROUTINE_WORK_POLICY: &str = r#"
permit(principal, action == Action::"work_claim", resource)
when {
    context.exact &&
    context.authentication.method == "agent_event_signature" &&
    context.authentication.assurance == "single_factor"
};"#;

const ROUTINE_SUBMISSION_SCHEMA: &str = r#"
action "submission_register" appliesTo {
    principal: Agent,
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

const ROUTINE_SUBMISSION_POLICY: &str = r#"
permit(principal, action == Action::"submission_register", resource)
when {
    context.exact &&
    context.authentication.method == "agent_record_signature" &&
    context.authentication.assurance == "single_factor"
};"#;

const ROUTINE_VERIFICATION_SCHEMA: &str = r#"
action "verification_import" appliesTo {
    principal: Agent,
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

const ROUTINE_VERIFICATION_POLICY: &str = r#"
permit(principal, action == Action::"verification_import", resource)
when {
    context.exact &&
    context.authentication.method == "agent_record_signature" &&
    context.authentication.assurance == "single_factor"
};"#;

const ROUTINE_WORK_POLICY_TESTS_ROOT: &str =
    // sha256("routine-work-policy-tests.v3|work_claim:agent_event_signature|\
    // submission_register:agent_record_signature|verification_import:\
    // agent_record_signature|exact=true|hostile:false-exact,none-auth,human-principal")
    "sha256:cf062aaec11c083af44080cc55363f27591656b52acec538ad964ec301fcc698";

pub(crate) fn active_repository_key(
    authority: &LoadedRepositoryAuthority,
) -> Result<(String, String), String> {
    let sequence = u64::try_from(authority.verification.authority_record_count + 1)
        .map_err(|_| "authority sequence exceeds u64")?;
    let active = authority
        .history
        .authority_keyset
        .keys
        .iter()
        .filter(|key| {
            key.valid_from_sequence <= sequence
                && key
                    .valid_through_sequence
                    .is_none_or(|through| sequence <= through)
        })
        .collect::<Vec<_>>();
    if authority.history.authority_keyset.threshold != 1 || active.len() != 1 {
        return Err(format!(
            "routine local repository-authority writes require threshold one and exactly one active key; found {}",
            active.len()
        ));
    }
    Ok((active[0].key_id.clone(), active[0].public_key.clone()))
}

fn initial_policy_bundle_at_with_routine_work(
    frontier: &Path,
    project: &Project,
    principal_id: &str,
    observed_at: &str,
    include_routine_work: bool,
    action: &str,
) -> Result<(PolicyBundleV1, CedarEvaluationInput), String> {
    let snapshot = vela_protocol::acceptance_policy::load_active_policy_snapshot(frontier)?;
    if let Some(verified) = &snapshot.verified {
        vela_protocol::acceptance_policy::resolve_policy_authority(project, verified, observed_at)?;
    }
    let translated = snapshot
        .verified
        .as_ref()
        .map(|verified| translate_legacy_policy(&verified.policy, POLICY_SHADOW_TESTS_ROOT, None))
        .transpose()?;
    let automatic_schema = translated
        .as_ref()
        .map_or(AUTOMATIC_POLICY_SCHEMA, |value| value.cedar_schema.as_str());
    let automatic_policies = translated
        .as_ref()
        .map_or("", |value| value.cedar_policies.as_str());
    let mut entities = translated
        .as_ref()
        .map_or_else(
            || {
                json!([
                    {
                        "uid": {"type": "Service", "id": "repository-authority-shadow"},
                        "attrs": {},
                        "parents": []
                    },
                    {
                        "uid": {"type": "Frontier", "id": project.frontier_id()},
                        "attrs": {},
                        "parents": []
                    }
                ])
            },
            |value| value.cedar_entities.clone(),
        )
        .as_array()
        .cloned()
        .ok_or_else(|| "translated Cedar entities must be an array".to_string())?;
    entities.push(json!({
        "uid": {"type": "Human", "id": principal_id},
        "attrs": {},
        "parents": []
    }));
    let entities = Value::Array(entities);
    let authority_schema = if action == AUTHORITY_INITIALIZE_ACTION {
        format!("{HUMAN_AUTHORITY_SCHEMA}\n{FRESH_AUTHORITY_INITIALIZE_SCHEMA}")
    } else {
        HUMAN_AUTHORITY_SCHEMA.to_string()
    };
    let schema = if include_routine_work {
        format!(
            "{automatic_schema}\n{authority_schema}\n{ROUTINE_WORK_SCHEMA}\n{ROUTINE_SUBMISSION_SCHEMA}\n{ROUTINE_VERIFICATION_SCHEMA}\n"
        )
    } else {
        // Preserve the exact migration-policy bytes emitted before the
        // routine-work schema was added. HUMAN_AUTHORITY_SCHEMA now carries a
        // trailing newline for composition with the fresh-authority schema;
        // the historical migration writer did not.
        format!(
            "{automatic_schema}\n{}\n",
            HUMAN_AUTHORITY_SCHEMA.trim_end()
        )
    };
    let human_policy = human_authority_policy(principal_id, action == AUTHORITY_INITIALIZE_ACTION)?;
    let policies = if include_routine_work {
        format!(
            "{automatic_policies}\n{human_policy}\n{ROUTINE_WORK_POLICY}\n{ROUTINE_SUBMISSION_POLICY}\n{ROUTINE_VERIFICATION_POLICY}\n"
        )
    } else {
        // Preserve the exact sequence-1 bytes emitted by Vela 0.930.0-rc.7.
        format!("{automatic_policies}\n{human_policy}\n")
    };
    let bundle = PolicyBundleV1 {
        schema: POLICY_BUNDLE_SCHEMA_V1.into(),
        frontier_id: project.frontier_id(),
        cedar_schema_root: ContentDigest::hash(schema.as_bytes()).as_str().into(),
        policies_root: ContentDigest::hash(policies.as_bytes()).as_str().into(),
        entities_root: ContentDigest::hash(to_canonical_bytes(&entities)?)
            .as_str()
            .into(),
        tests_root: if include_routine_work {
            ROUTINE_WORK_POLICY_TESTS_ROOT
        } else {
            POLICY_SHADOW_TESTS_ROOT
        }
        .into(),
        engine: vela_protocol::authority::CEDAR_ENGINE.into(),
        engine_version: vela_protocol::authority::CEDAR_ENGINE_VERSION.into(),
        restricted_profile: vela_protocol::authority::CEDAR_PROFILE_V1.into(),
        previous_bundle_root: None,
        authority_summary: if snapshot.verified.is_some() {
            if include_routine_work {
                "Preserve the exact translated automatic lane; signed agents may coordinate exact work leases; one local human principal may decide reviews and administer repository authority.".into()
            } else {
                "Preserve the exact translated automatic lane; one local human principal may decide reviews and administer repository authority.".into()
            }
        } else if include_routine_work {
            "No automatic scientific admission; signed agents may coordinate exact work leases; one local human principal may decide reviews and administer repository authority.".into()
        } else {
            "No automatic scientific admission; one local human principal may decide reviews and administer repository authority.".into()
        },
    };
    let authorization = CedarEvaluationInput {
        schema,
        policies,
        entities,
        principal: format!("Human::{}", serde_json::to_string(principal_id).unwrap()),
        principal_class: PrincipalClass::Human,
        action: action.into(),
        resource: format!(
            "Frontier::{}",
            serde_json::to_string(&project.frontier_id()).unwrap()
        ),
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
            "initial authority policy does not authorize the exact migration: {:?}",
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
    if include_routine_work {
        verify_routine_work_policy(&authorization, project)?;
    }
    Ok((bundle, authorization))
}

pub(crate) fn fresh_authority_policy(
    frontier: &Path,
    project: &Project,
    principal_id: &str,
    observed_at: &str,
) -> Result<(PolicyBundleV1, CedarEvaluationInput), String> {
    initial_policy_bundle_at_with_routine_work(
        frontier,
        project,
        principal_id,
        observed_at,
        true,
        AUTHORITY_INITIALIZE_ACTION,
    )
}

fn verify_routine_work_policy(
    authorization: &CedarEvaluationInput,
    project: &Project,
) -> Result<(), String> {
    let agent_input = CedarEvaluationInput {
        principal: r#"Agent::"agent:policy-fixture""#.into(),
        principal_class: PrincipalClass::Agent,
        action: "work_claim".into(),
        resource: format!(
            "Frontier::{}",
            serde_json::to_string(&project.frontier_id()).unwrap()
        ),
        context: json!({
            "exact": true,
            "authentication": {
                "method": "agent_event_signature",
                "assurance": "single_factor",
                "authenticated_at": "2026-07-24T00:00:00Z",
                "observed_at": "2026-07-24T00:00:00Z",
                "expires_at": "2026-07-24T00:05:00Z",
                "user_presence": false,
                "user_verification": false,
                "recovery_recent": false
            }
        }),
        ..authorization.clone()
    };
    let allowed = vela_authority::evaluate(&agent_input);
    if !allowed.valid
        || !allowed.diagnostics.is_empty()
        || allowed.decision != vela_protocol::authority::CedarDecision::Allow
    {
        return Err(format!(
            "routine work policy does not authorize one exact signed-agent lease: {:?}",
            allowed.diagnostics
        ));
    }
    let submission_input = CedarEvaluationInput {
        action: "submission_register".into(),
        context: json!({
            "exact": true,
            "authentication": {
                "method": "agent_record_signature",
                "assurance": "single_factor",
                "authenticated_at": "2026-07-24T00:00:00Z",
                "observed_at": "2026-07-24T00:00:00Z",
                "expires_at": "2026-07-24T00:05:00Z",
                "user_presence": false,
                "user_verification": false,
                "recovery_recent": false
            }
        }),
        ..agent_input.clone()
    };
    let submission_allowed = vela_authority::evaluate(&submission_input);
    if !submission_allowed.valid
        || !submission_allowed.diagnostics.is_empty()
        || submission_allowed.decision != vela_protocol::authority::CedarDecision::Allow
    {
        return Err(format!(
            "routine work policy does not authorize one exact signed-agent Submission registration: {:?}",
            submission_allowed.diagnostics
        ));
    }
    let verification_input = CedarEvaluationInput {
        action: "verification_import".into(),
        ..submission_input.clone()
    };
    let verification_allowed = vela_authority::evaluate(&verification_input);
    if !verification_allowed.valid
        || !verification_allowed.diagnostics.is_empty()
        || verification_allowed.decision != vela_protocol::authority::CedarDecision::Allow
    {
        return Err(format!(
            "routine work policy does not authorize one exact signed Verification Record import: {:?}",
            verification_allowed.diagnostics
        ));
    }
    for hostile in [
        CedarEvaluationInput {
            context: json!({
                "exact": false,
                "authentication": {
                    "method": "agent_event_signature",
                    "assurance": "single_factor",
                    "authenticated_at": "2026-07-24T00:00:00Z",
                    "observed_at": "2026-07-24T00:00:00Z",
                    "expires_at": "2026-07-24T00:05:00Z",
                    "user_presence": false,
                    "user_verification": false,
                    "recovery_recent": false
                }
            }),
            ..agent_input.clone()
        },
        CedarEvaluationInput {
            context: json!({
                "exact": true,
                "authentication": {
                    "method": "none",
                    "assurance": "single_factor",
                    "authenticated_at": "2026-07-24T00:00:00Z",
                    "observed_at": "2026-07-24T00:00:00Z",
                    "expires_at": "2026-07-24T00:05:00Z",
                    "user_presence": false,
                    "user_verification": false,
                    "recovery_recent": false
                }
            }),
            ..agent_input.clone()
        },
        CedarEvaluationInput {
            action: "submission_register".into(),
            context: json!({
                "exact": true,
                "authentication": {
                    "method": "agent_event_signature",
                    "assurance": "single_factor",
                    "authenticated_at": "2026-07-24T00:00:00Z",
                    "observed_at": "2026-07-24T00:00:00Z",
                    "expires_at": "2026-07-24T00:05:00Z",
                    "user_presence": false,
                    "user_verification": false,
                    "recovery_recent": false
                }
            }),
            ..submission_input.clone()
        },
        CedarEvaluationInput {
            action: "submission_register".into(),
            context: json!({
                "exact": false,
                "authentication": {
                    "method": "agent_record_signature",
                    "assurance": "single_factor",
                    "authenticated_at": "2026-07-24T00:00:00Z",
                    "observed_at": "2026-07-24T00:00:00Z",
                    "expires_at": "2026-07-24T00:05:00Z",
                    "user_presence": false,
                    "user_verification": false,
                    "recovery_recent": false
                }
            }),
            ..submission_input.clone()
        },
    ] {
        let denied = vela_authority::evaluate(&hostile);
        if !denied.valid
            || !denied.diagnostics.is_empty()
            || denied.decision != vela_protocol::authority::CedarDecision::Deny
        {
            return Err(format!(
                "routine work policy does not fail closed for a hostile lease: {:?}",
                denied.diagnostics
            ));
        }
    }
    Ok(())
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
        println!(
            "  next: distribute the full authority root independently, then run `vela authority trust pin`"
        );
    }
}

pub(crate) fn cmd_authority_trust_pin(frontier: &Path, record_root: &str, json_out: bool) {
    crate::ui::set_mode("authority trust pin", json_out);
    let result =
        pin_repository_authority(frontier, record_root).unwrap_or_else(|error| fail_return(&error));
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
        println!("  authority granted: none");
    }
}

fn pin_repository_authority(frontier: &Path, record_root: &str) -> Result<Value, String> {
    let project = vela_protocol::repo::load_from_path(frontier)?;
    let authority = load_repository_authority(frontier, &project)?
        .ok_or_else(|| "Frontier has no repository-authority history to pin".to_string())?;
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
        frontier_id: project.frontier_id(),
        first_authority_record_root: observed_root.clone(),
    };
    let user_home =
        crate::frontier_txn::operating_system_account_home().map_err(|error| error.to_string())?;
    let installed = install_authority_trust_anchor_from_home(&user_home, &anchor)?;
    let boundary_event_id = authority
        .verification
        .initialization_event_id
        .clone()
        .or_else(|| authority.verification.migration_event_id.clone())
        .ok_or_else(|| {
            "repository authority has no initialization or migration boundary".to_string()
        })?;
    Ok(json!({
        "schema": "vela.authority-trust-pin-result.v1",
        "ok": true,
        "command": "authority.trust.pin",
        "frontier": frontier.display().to_string(),
        "frontier_id": project.frontier_id(),
        "first_authority_record_id": first_record.record_id,
        "first_authority_record_root": observed_root,
        "initial_authority_keyset_root": first_record.content.authority_keyset_root,
        "initial_policy_bundle_root": first_record.content.authorization.policy_bundle_root,
        "boundary_event_id": boundary_event_id,
        "authority_trust_anchor_root": installed.root,
        "authority_trust_anchor_path": installed.path.display().to_string(),
        "writes": [installed.path.display().to_string()],
        "frontier_writes": [],
        "authority_granted": false
    }))
}

fn initialize_repository_authority(
    frontier: &Path,
    key_selector: Option<&str>,
    reason: &str,
) -> Result<Value, String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("authority init requires a non-empty reason".into());
    }
    let project = vela_protocol::repo::load_from_path(frontier)?;
    if load_repository_authority(frontier, &project)?.is_some() {
        return Err("repository authority is already initialized".into());
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
        frontier_id: project.frontier_id(),
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
    let (policy_bundle, authorization_input) =
        fresh_authority_policy(frontier, &project, &local.principal_id, &recorded_at)?;
    let keyset_root = authority_keyset.root()?;
    let policy_root = policy_bundle.root()?;
    let actors_path = frontier.join(".vela/actors.json");
    let actors_bytes = std::fs::read(&actors_path)
        .map_err(|error| format!("read {}: {error}", actors_path.display()))?;
    let initial_event_log_root = format!("sha256:{}", event_log_hash(&project.events));
    let initialization = AuthorityInitializationV1 {
        schema: vela_protocol::authority_history::AUTHORITY_INITIALIZATION_SCHEMA_V1.into(),
        frontier_id: project.frontier_id(),
        initial_event_log_root,
        initial_actor_registry_root: ContentDigest::hash(&actors_bytes).as_str().into(),
        new_authority_keyset_root: keyset_root.clone(),
        new_policy_bundle_root: policy_root.clone(),
        new_principal_id: local.principal_id.clone(),
        minimum_writer_version: env!("CARGO_PKG_VERSION").into(),
        reason: reason.into(),
    };
    initialization.validate()?;
    let intent_digest = ContentDigest::hash(to_canonical_bytes(&initialization)?)
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
                frontier_id: project.frontier_id(),
                legacy_events: project.events.clone(),
                legacy_actor_registry_bytes: actors_bytes,
                legacy_active_policy_head_root: NULL_HASH.into(),
                legacy_policy_store_manifest_root: NULL_HASH.into(),
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
            runtime_session_state: RuntimeSessionState::default(),
            authorization_input,
            delegation: None,
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
                    id: project.frontier_id(),
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
            object_drafts: Vec::new(),
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
    let reloaded = vela_protocol::repo::load_from_path(frontier)?;
    let authority = load_repository_authority(frontier, &reloaded)?
        .ok_or_else(|| "repository authority disappeared after initialization".to_string())?;
    if authority
        .verification
        .final_authority_record_root
        .as_deref()
        != Some(result.authority_record_root.as_str())
    {
        return Err("fresh repository-authority replay produced a different head".into());
    }
    Ok(json!({
        "schema": "vela.authority-initialization-result.v1",
        "ok": true,
        "frontier": frontier.display().to_string(),
        "frontier_id": project.frontier_id(),
        "principal_id": principal.principal_id,
        "repository_key_id": identity.key_id,
        "repository_key_fingerprint": identity.fingerprint,
        "authority_keyset_root": keyset_root,
        "policy_bundle_root": policy_root,
        "authority_record_id": result.authority_record_id,
        "authority_record_root": result.authority_record_root.clone(),
        "event_ids": result.event_ids,
        "after_event_log_root": result.after_event_log_root,
        "consumer_pin": {
            "schema": AUTHORITY_TRUST_ANCHOR_SCHEMA_V1,
            "frontier_id": project.frontier_id(),
            "first_authority_record_root": result.authority_record_root,
            "command": format!(
                "vela authority trust pin {} --record-root {} --json",
                frontier.display(),
                result.authority_record_root
            )
        },
        "writes_now": true
    }))
}

/// Assess whether an offered target can enter the routine producer path.
///
/// Target freshness and lease availability do not imply authorization. Early
/// Era-1 Frontiers intentionally migrated before the narrow signed-agent work
/// policy existed. Name the one required policy rotation before a producer
/// reads a key or acquires a write barrier.
pub(crate) fn ensure_routine_producer_ready(
    frontier: &Path,
    project: &Project,
) -> Result<(), String> {
    let Some(authority) = load_repository_authority(frontier, project)? else {
        return Ok(());
    };
    ensure_routine_producer_material_ready(&authority.policy_material, project)
}

fn ensure_routine_producer_material_ready(
    material: &CedarPolicyMaterial,
    project: &Project,
) -> Result<(), String> {
    let has_work_claim = material.schema.contains("action \"work_claim\"");
    let has_submission_register = material.schema.contains("action \"submission_register\"");
    let has_historical_receipt_land = material.schema.contains("action \"receipt_land\"");
    match (
        has_work_claim,
        has_submission_register || has_historical_receipt_land,
    ) {
        (false, false) => {
            return Err(
                "signed-agent routine producer work is not enabled; run `vela authority enable-work . --reason <bounded-reason> --json`, inspect the exact plan, and request its protected approval"
                    .into(),
            );
        }
        (true, true) => {}
        _ => {
            return Err(
                "repository-authority routine producer schema is incomplete; work_claim and submission_register must be introduced together"
                    .into(),
            );
        }
    }
    let verification_input = CedarEvaluationInput {
        schema: material.schema.clone(),
        policies: material.policies.clone(),
        entities: material.entities.clone(),
        principal: r#"Human::"verification-only""#.into(),
        principal_class: PrincipalClass::Human,
        action: POLICY_ROTATE_ACTION.into(),
        resource: format!(
            "Frontier::{}",
            serde_json::to_string(&project.frontier_id())
                .expect("serializing a frontier ID cannot fail")
        ),
        context: json!({"exact": true}),
    };
    verify_routine_work_policy(&verification_input, project)?;
    Ok(())
}

impl LoadedRepositoryAuthority {
    /// Return Era-1 events in covering-record sequence.
    ///
    /// Event-log commitments are set based, but reducers for coordination
    /// records such as lease refresh/release require causal transaction
    /// order. The already-verified envelopes are the sole ordering source.
    pub(crate) fn ordered_events(&self) -> Result<Vec<&AuthorityEventV1>, String> {
        let by_id = self
            .history
            .authority_events
            .iter()
            .map(|event| (event.id.as_str(), event))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut ordered = Vec::with_capacity(by_id.len());
        for envelope in &self.history.authority_envelopes {
            let payload = BASE64_STANDARD
                .decode(&envelope.payload)
                .map_err(|error| format!("authority envelope payload is not base64: {error}"))?;
            let record: AuthorityRecordV1 = serde_json::from_slice(&payload)
                .map_err(|error| format!("authority envelope record JSON is invalid: {error}"))?;
            for event_id in &record.content.event_ids {
                if let Some(event) = by_id.get(event_id.as_str()) {
                    ordered.push(*event);
                } else if record.content.sequence != 1 {
                    return Err(format!(
                        "authority record {} references missing Era-1 event {event_id}",
                        record.content.sequence
                    ));
                }
            }
        }
        if ordered.len() != by_id.len() {
            return Err("verified authority history did not order every Era-1 event".into());
        }
        Ok(ordered)
    }
}

pub(crate) fn load_repository_authority(
    frontier: &Path,
    project: &Project,
) -> Result<Option<LoadedRepositoryAuthority>, String> {
    let migrations = project
        .events
        .iter()
        .filter(|event| event.kind == EventKind::AuthorityModelMigrated)
        .collect::<Vec<_>>();
    let migration_event = match migrations.as_slice() {
        [] => None,
        [event] => Some(*event),
        _ => {
            return Err("frontier contains multiple authority.model_migrated events".into());
        }
    };
    if migrations.len() > 1 {
        return Err("frontier contains multiple authority.model_migrated events".into());
    }
    let migration = migration_event
        .map(|event| {
            let migration: AuthorityModelMigrationV1 =
                serde_json::from_value(event.payload.clone())
                    .map_err(|error| format!("decode authority migration payload: {error}"))?;
            migration.validate()?;
            Ok::<_, String>(migration)
        })
        .transpose()?;

    let authority_root = frontier.join(".vela/authority");
    let retained_authority_keysets =
        read_authority_json_directory::<AuthorityKeysetV1>(&authority_root.join("keysets"))?;
    let retained_policy_bundles =
        read_authority_json_directory::<PolicyBundleV1>(&authority_root.join("policies"))?;
    let authority_events =
        read_authority_json_directory::<AuthorityEventV1>(&authority_root.join("events"))?;
    if migration.is_none() && authority_events.is_empty() {
        return Ok(None);
    }
    let initialization = if migration.is_none() {
        let initializations = authority_events
            .iter()
            .filter(|event| event.content.kind.as_str() == AUTHORITY_INITIALIZED_EVENT_KIND)
            .collect::<Vec<_>>();
        let [event] = initializations.as_slice() else {
            return Err(
                "fresh repository authority must retain exactly one authority.initialized event"
                    .into(),
            );
        };
        Some(initialization_payload_from_event(event)?)
    } else {
        None
    };
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
    let actors_path = frontier.join(".vela/actors.json");
    let legacy_actor_registry_bytes = std::fs::read(&actors_path)
        .map_err(|error| format!("read {}: {error}", actors_path.display()))?;
    let frontier_id = migration
        .as_ref()
        .map(|value| value.frontier_id.as_str())
        .or_else(|| {
            initialization
                .as_ref()
                .map(|value| value.frontier_id.as_str())
        })
        .ok_or_else(|| "repository authority has no boundary payload".to_string())?;
    let legacy_active_policy_head_root = migration
        .as_ref()
        .map(|value| value.legacy_active_policy_head_root.as_str())
        .unwrap_or(vela_protocol::events::NULL_HASH);
    let legacy_policy_store_manifest_root = migration
        .as_ref()
        .map(|value| value.legacy_policy_store_manifest_root.as_str())
        .unwrap_or(vela_protocol::events::NULL_HASH);
    let verification = verify_authority_history(AuthorityHistoryInput {
        frontier_id,
        legacy_events: &project.events,
        legacy_actor_registry_bytes: &legacy_actor_registry_bytes,
        legacy_active_policy_head_root,
        legacy_policy_store_manifest_root,
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
    let authority_keyset = retained_authority_keysets
        .iter()
        .find(|keyset| keyset.root().is_ok_and(|root| root == active_keyset_root))
        .cloned()
        .ok_or_else(|| "active repository-authority keyset snapshot is missing".to_string())?;
    let policy_bundle = retained_policy_bundles
        .iter()
        .find(|bundle| bundle.root().is_ok_and(|root| root == active_policy_root))
        .cloned()
        .ok_or_else(|| "active repository policy bundle snapshot is missing".to_string())?;
    let policy_material =
        if let (Some(migration_event), Some(migration)) = (migration_event, migration.as_ref()) {
            load_or_reconstruct_policy_material(
                frontier,
                project,
                migration_event,
                migration,
                &policy_bundle,
                verification.authority_record_count,
            )?
        } else {
            load_retained_policy_material(frontier, &policy_bundle)?
        };
    Ok(Some(LoadedRepositoryAuthority {
        history: AuthorityHistorySnapshot {
            frontier_id: frontier_id.to_string(),
            legacy_events: project.events.clone(),
            legacy_actor_registry_bytes,
            legacy_active_policy_head_root: legacy_active_policy_head_root.to_string(),
            legacy_policy_store_manifest_root: legacy_policy_store_manifest_root.to_string(),
            authority_keyset,
            policy_bundle,
            retained_authority_keysets,
            retained_policy_bundles,
            authority_events,
            authority_envelopes,
        },
        policy_material,
        verification,
    }))
}

fn authority_envelope_sequence(envelope: &AuthorityEnvelopeV1) -> Result<u64, String> {
    Ok(authority_record_from_envelope(envelope)?.content.sequence)
}

fn authority_record_from_envelope(
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

fn load_or_reconstruct_policy_material(
    frontier: &Path,
    project: &Project,
    migration_event: &StateEvent,
    migration: &AuthorityModelMigrationV1,
    bundle: &PolicyBundleV1,
    authority_record_count: usize,
) -> Result<CedarPolicyMaterial, String> {
    let paths = authority_policy_material_paths(bundle).map_err(|error| error.to_string())?;
    let present = paths
        .iter()
        .map(|path| frontier.join(path).is_file())
        .collect::<Vec<_>>();
    if present.iter().all(|present| *present) {
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
        return Ok(material);
    }
    if present.iter().any(|present| *present) {
        return Err("retained Cedar policy material is incomplete".into());
    }
    if authority_record_count != 1 || bundle.previous_bundle_root.is_some() {
        return Err(
            "retained Cedar policy material is missing after the initial authority record".into(),
        );
    }
    let expected_root = bundle.root()?;
    let candidates = [true, false]
        .into_iter()
        .map(|include_routine_work| {
            initial_policy_bundle_at_with_routine_work(
                frontier,
                project,
                &migration.new_principal_id,
                &migration_event.timestamp,
                include_routine_work,
                AUTHORITY_MIGRATION_ACTION,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let candidate_roots = candidates
        .iter()
        .map(|(candidate, _)| candidate.root())
        .collect::<Result<Vec<_>, _>>()?;
    let (_, authorization) = candidates
        .into_iter()
        .find(|(candidate, _)| candidate.root().is_ok_and(|root| root == expected_root))
        .ok_or_else(|| {
            format!(
                "initial Cedar policy material cannot be reconstructed to the retained bundle root {expected_root}; candidate roots: {}",
                candidate_roots
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    let material = CedarPolicyMaterial::from_evaluation(&authorization);
    material.validate_against(bundle)?;
    Ok(material)
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
{initialization}        Action::"authority_model_migrate",
        Action::"authority_rotate",
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

    const OBSERVED_AT: &str = "2026-07-24T12:00:00Z";
    const PRINCIPAL_ID: &str = "local:device-fixture|uid:501";

    fn fixture() -> TempDir {
        let temporary = TempDir::new().unwrap();
        let project = vela_protocol::project::assemble(
            "migration-fixture",
            Vec::new(),
            0,
            0,
            "Disposable authority-migration fixture.",
        );
        vela_protocol::repo::init_repo(temporary.path(), &project).unwrap();
        temporary
    }

    #[test]
    fn helper_approval_time_is_canonicalized_for_authority_observation() {
        let recorded_at = canonical_whole_second_time(
            "authority-migration approval",
            "2026-07-25T15:42:06.768282000Z",
        )
        .unwrap();
        assert_eq!(recorded_at, "2026-07-25T15:42:06Z");
        assert_eq!(
            canonical_whole_second_time(
                "authority-migration approval",
                "2026-07-25T11:42:06.768282000-04:00",
            )
            .unwrap(),
            "2026-07-25T15:42:06Z"
        );
        assert!(canonical_whole_second_time("authority-migration approval", "not-a-time").is_err());

        let mut session = local_session(&recorded_at).unwrap();
        let request = vela_authority::runtime_authentication::AuthenticationRequest {
            principal_id: session.principal_id.clone(),
            principal_class: PrincipalClass::Human,
            transaction_at: recorded_at.clone(),
        };
        let observation = vela_authority::runtime_authentication::authenticate_for_transaction(
            &mut session,
            &request,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(observation.authenticated_at, recorded_at);
        assert_eq!(observation.observed_at, recorded_at);
    }

    #[test]
    fn routine_producer_readiness_distinguishes_early_and_current_era_one_policy() {
        let fixture = fixture();
        let project = vela_protocol::repo::load_from_path(fixture.path()).unwrap();
        let (_, early_authorization) = initial_policy_bundle_at_with_routine_work(
            fixture.path(),
            &project,
            PRINCIPAL_ID,
            OBSERVED_AT,
            false,
            AUTHORITY_MIGRATION_ACTION,
        )
        .unwrap();
        assert!(
            ensure_routine_producer_material_ready(
                &CedarPolicyMaterial::from_evaluation(&early_authorization),
                &project,
            )
            .unwrap_err()
            .contains("vela authority enable-work")
        );

        let (_, current_authorization) = initial_policy_bundle_at_with_routine_work(
            fixture.path(),
            &project,
            PRINCIPAL_ID,
            OBSERVED_AT,
            true,
            AUTHORITY_MIGRATION_ACTION,
        )
        .unwrap();
        ensure_routine_producer_material_ready(
            &CedarPolicyMaterial::from_evaluation(&current_authorization),
            &project,
        )
        .unwrap();

        let mut incomplete = CedarPolicyMaterial::from_evaluation(&current_authorization);
        incomplete.schema = incomplete
            .schema
            .replace(ROUTINE_SUBMISSION_SCHEMA.trim(), "");
        assert!(
            ensure_routine_producer_material_ready(&incomplete, &project)
                .unwrap_err()
                .contains("must be introduced together")
        );
    }

    #[test]
    fn historical_migration_schema_bytes_retain_the_original_root() {
        let schema = format!(
            "{AUTOMATIC_POLICY_SCHEMA}\n{}\n",
            HUMAN_AUTHORITY_SCHEMA.trim_end()
        );
        assert_eq!(
            ContentDigest::hash(schema.as_bytes()).as_str(),
            "sha256:4583b841bf5ac65a69b5ca835b6ed76290bcf6a9e13ff612747f3dfa999e7fe0"
        );
    }

    #[test]
    fn historical_migration_bundle_reconstructs_the_retained_quantum_root() {
        let fixture = fixture();
        let mut project = vela_protocol::repo::load_from_path(fixture.path()).unwrap();
        project.frontier_id = Some("vfr_001f148c07eebecb".into());
        let (bundle, _) = initial_policy_bundle_at_with_routine_work(
            fixture.path(),
            &project,
            "local:device-sha256:67fbb8e56377e6868e9f941524e0bf39cfb4fd2a4bfdd25c2edb93fc82f86213|uid:501",
            "2026-07-25T22:24:21Z",
            false,
            AUTHORITY_MIGRATION_ACTION,
        )
        .unwrap();
        assert_eq!(
            bundle.root().unwrap(),
            "sha256:84c2df090f50d84a6036771608ffc8068c676a937628b47382f77ec8bd9f5dfc"
        );
    }
}
