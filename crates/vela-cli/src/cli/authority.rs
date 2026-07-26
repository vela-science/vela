//! Product seams for Era-0 migration and narrow Era-1 administration.
//!
//! Previews are key-free and write-free. Migration requests the one final
//! protected legacy continuity signature. Post-migration work-policy rotation
//! uses fresh platform-owned user presence without reading the human Vela key.
//! Both ask the configured OpenSSH agent to sign only an already-authorized
//! covering repository-authority record.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rand::RngCore;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vela_authority::legacy_translation::translate_legacy_policy;
use vela_authority::runtime_authentication::{
    AuthenticationAdapter, AuthenticationRequest, LocalOsSession, PlatformUserPresenceSession,
    RuntimeSessionState,
};
use vela_authority::{CedarEvaluationInput, CedarPolicyMaterial};
use vela_protocol::authority::{
    AUTHORITY_KEY_ALGORITHM, AUTHORITY_KEY_PURPOSE, AUTHORITY_KEYSET_SCHEMA_V1,
    AuthorityEnvelopeV1, AuthorityEventV1, AuthorityKeyV1, AuthorityKeysetV1, AuthorityRecordV1,
    POLICY_BUNDLE_SCHEMA_V1, PolicyBundleV1, PrincipalSnapshotV1, SemanticApprovalV1,
};
use vela_protocol::authority_history::{
    AUTHORITY_MIGRATION_ACTION, AUTHORITY_MODEL_MIGRATION_SCHEMA_V1, AuthorityHistoryInput,
    AuthorityHistoryVerification, AuthorityModelMigrationV1, POLICY_ROTATE_ACTION,
    verify_authority_history,
};
use vela_protocol::canonical::to_canonical_bytes;
use vela_protocol::events::{
    EVENT_SCHEMA, EventKind, NULL_HASH, StateActor, StateEvent, StateTarget, compute_event_id,
    event_log_hash,
};
use vela_protocol::principal_capability::PrincipalClass;
use vela_protocol::project::Project;

use crate::authority_migration::{
    AuthorityMigrationHistorySnapshot, AuthorityMigrationRequest, execute_authority_migration,
};
use crate::authority_transaction::RepositoryAuthoritySigner;
use crate::authority_transaction::{
    AuthorityEventDraft, AuthorityHistorySnapshot, AuthorityTransactionRequest,
    authority_policy_material_paths, execute_authority_transaction,
};
use crate::frontier_txn::{ContentDigest, FrontierTxn, InputBinding, RepoPath};
use crate::repository_authority_provider::SshAgentRepositoryAuthoritySigner;

use super::{fail_return, print_json};

const PLAN_SCHEMA: &str = "vela.authority-migration-plan.v1";
const PLAN_DOMAIN: &[u8] = b"vela.authority-migration-plan.v1\0";
const WORK_POLICY_PLAN_SCHEMA: &str = "vela.authority-work-policy-plan.v1";
const WORK_POLICY_PLAN_DOMAIN: &[u8] = b"vela.authority-work-policy-plan.v1\0";
const POLICY_HEAD_SCHEMA: &str = "vela.legacy-authority-policy-head.v1";
const POLICY_STORE_SCHEMA: &str = "vela.legacy-authority-policy-store.v1";
const POLICY_SHADOW_TESTS_ROOT: &str =
    "sha256:20edc3cef4390127a3f49daf36583f5ac7fa1fd24d2ebd5f7cc75ca1c6e4e41b";
const MINIMUM_WRITER_VERSION: &str = "0.930.0";
const POLICY_DIRECTORY: &str = ".vela/policies";
const MAX_POLICY_STORE_FILES: usize = 256;
const MAX_POLICY_STORE_BYTES: u64 = 2 * 1024 * 1024;

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

const ROUTINE_WORK_POLICY_TESTS_ROOT: &str =
    "sha256:f4457edafcf11613b0e6a51dd2f999b158af9e471cb8258e3a15b05bde837e1f";

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct PolicyStoreEntry {
    path: String,
    content_root: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct PolicyStoreCommitment {
    schema: String,
    entries: Vec<PolicyStoreEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct PolicyHeadCommitment {
    schema: String,
    mode: String,
    policy_root: Option<String>,
    signature_root: Option<String>,
    policy_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct AuthorityMigrationPlan {
    schema: String,
    ok: bool,
    command: String,
    frontier: String,
    frontier_id: String,
    frontier_name: String,
    git_commit: String,
    git_tree: String,
    vela_version: String,
    vela_binary_sha256: String,
    observed_at: String,
    legacy_actor: String,
    legacy_public_key: String,
    legacy_event_log_root: String,
    legacy_event_count: usize,
    legacy_actor_registry_root: String,
    legacy_active_policy_head_root: String,
    legacy_policy_store_manifest_root: String,
    legacy_policy_mode: String,
    new_principal_id: String,
    repository_key_id: String,
    repository_public_key: String,
    authority_keyset: AuthorityKeysetV1,
    authority_keyset_root: String,
    policy_bundle: PolicyBundleV1,
    policy_bundle_root: String,
    migration_event: StateEvent,
    touched_paths: Vec<String>,
    reason: String,
    writes_now: bool,
    requires_one_legacy_approval: bool,
    plan_root: String,
}

#[derive(Debug)]
struct PreparedPlan {
    plan: AuthorityMigrationPlan,
    project: Project,
    actors_bytes: Vec<u8>,
    authorization_input: CedarEvaluationInput,
    principal: PrincipalSnapshotV1,
    read_set: Vec<InputBinding>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct AuthorityMigrationExecution {
    schema: String,
    ok: bool,
    command: String,
    plan_root: String,
    migration_event_id: String,
    migration_event_root: String,
    authority_keyset_root: String,
    policy_bundle_root: String,
    authority_record_id: String,
    authority_record_root: String,
    before_event_log_root: String,
    after_event_log_root: String,
    operation_id: String,
    transaction_id: String,
    git_publication: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkPolicyPlan {
    schema: String,
    ok: bool,
    command: String,
    frontier: String,
    frontier_id: String,
    frontier_name: String,
    git_commit: String,
    git_tree: String,
    observed_at: String,
    principal_id: String,
    current_policy_bundle_root: String,
    next_policy_bundle: PolicyBundleV1,
    next_policy_bundle_root: String,
    reason: String,
    writes_now: bool,
    requires_user_presence: bool,
    plan_root: String,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_authority_migrate(
    frontier: &Path,
    repository_key_id: &str,
    repository_public_key: &str,
    reason: &str,
    apply: bool,
    confirm_root: Option<&str>,
    confirm_at: Option<&str>,
    json_out: bool,
) {
    crate::ui::set_mode("authority migrate", json_out);
    if apply {
        let confirm_root = confirm_root.unwrap_or_else(|| {
            fail_return("authority migrate --apply requires --confirm-root and --confirm-at")
        });
        let confirm_at = confirm_at.unwrap_or_else(|| {
            fail_return("authority migrate --apply requires --confirm-root and --confirm-at")
        });
        crate::decision_plan::validate_scripted_confirmation_time(confirm_at)
            .unwrap_or_else(|error| fail_return(&format!("{}: {}", error.code, error.message)));
        let initial = prepare_plan(
            frontier,
            repository_key_id,
            repository_public_key,
            reason,
            confirm_at,
        )
        .unwrap_or_else(|error| fail_return(&error));
        if initial.plan.plan_root != confirm_root {
            fail_return::<()>(&format!(
                "authority migration confirmation root mismatch: supplied {confirm_root}, current {}",
                initial.plan.plan_root
            ));
        }
        let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)
            .unwrap_or_else(|error| fail_return(&error));
        let barrier = FrontierTxn::acquire_administrator_write_barrier(frontier, &journal_dir)
            .unwrap_or_else(|error| fail_return(&error.to_string()));
        let locked = prepare_plan(
            frontier,
            repository_key_id,
            repository_public_key,
            reason,
            confirm_at,
        )
        .unwrap_or_else(|error| fail_return(&error));
        if locked.plan.plan_root != initial.plan.plan_root {
            fail_return::<()>(
                "authority migration inputs changed while acquiring the recovery barrier",
            );
        }
        let profile = crate::cli_identity::protected_signer_profile()
            .unwrap_or_else(|error| fail_return(&error));
        let signer_request =
            build_protected_request(&locked, &profile).unwrap_or_else(|error| fail_return(&error));
        let response =
            request_legacy_signature(&signer_request).unwrap_or_else(|error| fail_return(&error));
        let recorded_at =
            canonical_whole_second_time("authority-migration approval", &response.approved_at)
                .unwrap_or_else(|error| fail_return(&error));
        let mut local_session =
            local_session(&recorded_at).unwrap_or_else(|error| fail_return(&error));
        if local_session.principal_id != locked.plan.new_principal_id {
            fail_return::<()>(
                "local operating-system principal changed after protected migration approval",
            );
        }
        let mut repository_signer = SshAgentRepositoryAuthoritySigner::from_environment(
            repository_key_id,
            repository_public_key,
        )
        .unwrap_or_else(|error| fail_return(&error));
        let execution = execute_signed_migration(
            barrier,
            frontier,
            locked,
            response.event_signature,
            recorded_at,
            &mut local_session,
            &mut repository_signer,
        )
        .unwrap_or_else(|error| fail_return(&error));
        if json_out {
            print_json(&execution);
        } else {
            println!("authority migration installed");
            println!("  event: {}", execution.migration_event_id);
            println!("  authority record: {}", execution.authority_record_id);
            println!("  event root: {}", execution.after_event_log_root);
            println!("  next: inspect `git status --short`, then commit the exact canonical delta");
        }
        return;
    }

    if confirm_root.is_some() || confirm_at.is_some() {
        fail_return::<()>(
            "--confirm-root/--confirm-at are valid only with --apply; preview is key-free",
        );
    }
    let observed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let prepared = prepare_plan(
        frontier,
        repository_key_id,
        repository_public_key,
        reason,
        &observed_at,
    )
    .unwrap_or_else(|error| fail_return(&error));
    if json_out {
        print_json(&prepared.plan);
    } else {
        println!(
            "authority migration · key-free preview · {}",
            prepared.plan.frontier_name
        );
        println!("  legacy actor: {}", prepared.plan.legacy_actor);
        println!("  new principal: {}", prepared.plan.new_principal_id);
        println!(
            "  repository keyset: {}",
            prepared.plan.authority_keyset_root
        );
        println!("  policy bundle: {}", prepared.plan.policy_bundle_root);
        println!("  plan root: {}", prepared.plan.plan_root);
        println!("  confirm at: {}", prepared.plan.observed_at);
        println!("  writes now: none");
        println!("  apply requests one final protected legacy approval");
    }
}

pub(crate) fn cmd_authority_enable_work(
    frontier: &Path,
    reason: &str,
    apply: bool,
    confirm_root: Option<&str>,
    confirm_at: Option<&str>,
    json_out: bool,
) {
    crate::ui::set_mode("authority enable-work", json_out);
    if apply {
        let confirm_root = confirm_root.unwrap_or_else(|| {
            fail_return("authority enable-work --apply requires --confirm-root and --confirm-at")
        });
        let confirm_at = confirm_at.unwrap_or_else(|| {
            fail_return("authority enable-work --apply requires --confirm-root and --confirm-at")
        });
        crate::decision_plan::validate_scripted_confirmation_time(confirm_at)
            .unwrap_or_else(|error| fail_return(&format!("{}: {}", error.code, error.message)));
        let initial = prepare_work_policy_plan(frontier, reason, confirm_at)
            .unwrap_or_else(|error| fail_return(&error));
        if initial.0.plan_root != confirm_root {
            fail_return::<()>(&format!(
                "authority work-policy confirmation root mismatch: supplied {confirm_root}, current {}",
                initial.0.plan_root
            ));
        }
        let journal_dir = crate::workflow::frontier_transaction_journal_dir(frontier)
            .unwrap_or_else(|error| fail_return(&error));
        let barrier =
            FrontierTxn::acquire_repository_authority_write_barrier(frontier, &journal_dir)
                .unwrap_or_else(|error| fail_return(&error.to_string()));
        let (plan, project, authority, next_material) =
            prepare_work_policy_plan(frontier, reason, confirm_at)
                .unwrap_or_else(|error| fail_return(&error));
        if plan.plan_root != initial.0.plan_root {
            fail_return::<()>(
                "authority work-policy inputs changed while acquiring the recovery barrier",
            );
        }
        let request =
            build_authority_intent_request(&plan).unwrap_or_else(|error| fail_return(&error));
        let response =
            request_authority_intent(&request).unwrap_or_else(|error| fail_return(&error));
        let recorded_at =
            canonical_whole_second_time("authority-intent approval", &response.approved_at)
                .unwrap_or_else(|error| fail_return(&error));
        let local = local_session(&recorded_at).unwrap_or_else(|error| fail_return(&error));
        if local.principal_id != plan.principal_id {
            fail_return::<()>(
                "local operating-system principal changed after protected policy approval",
            );
        }
        let mut authentication = PlatformUserPresenceSession {
            principal_id: plan.principal_id.clone(),
            issuer: local.issuer,
            subject: local.subject,
            session_root: response.session_root,
            authenticated_at: recorded_at.clone(),
            expires_at: (DateTime::parse_from_rfc3339(&recorded_at)
                .expect("canonical approval time parses")
                .with_timezone(&Utc)
                + Duration::minutes(5))
            .to_rfc3339_opts(SecondsFormat::Secs, true),
        };
        let next_policy = plan.next_policy_bundle.clone();
        let current_policy_root = plan.current_policy_bundle_root.clone();
        let next_policy_root = plan.next_policy_bundle_root.clone();
        let authorization = CedarEvaluationInput {
            schema: authority.policy_material.schema.clone(),
            policies: authority.policy_material.policies.clone(),
            entities: authority.policy_material.entities.clone(),
            principal: format!(
                "Human::{}",
                serde_json::to_string(&plan.principal_id)
                    .expect("serializing the principal cannot fail")
            ),
            principal_class: PrincipalClass::Human,
            action: POLICY_ROTATE_ACTION.into(),
            resource: format!(
                "Frontier::{}",
                serde_json::to_string(&plan.frontier_id)
                    .expect("serializing the frontier cannot fail")
            ),
            context: json!({"exact": true}),
        };
        let (key_id, public_key) =
            active_repository_key(&authority).unwrap_or_else(|error| fail_return(&error));
        let mut signer = SshAgentRepositoryAuthoritySigner::from_environment(key_id, &public_key)
            .unwrap_or_else(|error| fail_return(&error));
        let executable =
            std::env::current_exe().unwrap_or_else(|error| fail_return(&error.to_string()));
        let binary_sha256 = vela_signer::contract::file_sha256(&executable)
            .unwrap_or_else(|error| fail_return(&error));
        let result = execute_authority_transaction(
            barrier,
            frontier,
            AuthorityTransactionRequest {
                history: authority.history,
                intent_digest: plan.plan_root.clone(),
                principal: PrincipalSnapshotV1 {
                    principal_id: plan.principal_id.clone(),
                    principal_class: PrincipalClass::Human,
                    display_name: Some("Repository administrator".into()),
                    affiliation: None,
                    account_links: vec![plan.principal_id.clone()],
                },
                authentication_request: AuthenticationRequest {
                    principal_id: plan.principal_id.clone(),
                    principal_class: PrincipalClass::Human,
                    transaction_at: recorded_at.clone(),
                },
                runtime_session_state: RuntimeSessionState::default(),
                authorization_input: authorization,
                delegation: None,
                semantic_approvals: vec![SemanticApprovalV1 {
                    principal_id: plan.principal_id.clone(),
                    role: "frontier_administrator".into(),
                    action: POLICY_ROTATE_ACTION.into(),
                    reason: reason.into(),
                    approved_at: recorded_at.clone(),
                    intent_digest: plan.plan_root.clone(),
                }],
                event_drafts: vec![AuthorityEventDraft {
                    kind: EventKind::Other("policy.rotated".into()),
                    target: StateTarget {
                        r#type: "frontier".into(),
                        id: plan.frontier_id.clone(),
                    },
                    actor: StateActor {
                        r#type: "human".into(),
                        id: plan.principal_id.clone(),
                    },
                    timestamp: recorded_at.clone(),
                    reason: reason.into(),
                    before_hash: current_policy_root,
                    after_hash: next_policy_root.clone(),
                    payload: json!({
                        "policy_bundle_root": next_policy_root,
                        "change": "enable_signed_agent_work_leases"
                    }),
                    caveats: vec![
                        "This policy grants no review, scientific acceptance, or policy-administration authority to agents.".into(),
                    ],
                }],
                object_drafts: Vec::new(),
                next_authority_keyset: None,
                next_policy_bundle: Some(next_policy),
                next_policy_material: Some(next_material),
                read_set: Vec::new(),
                vela_version: env!("CARGO_PKG_VERSION").into(),
                binary_sha256,
                recorded_at,
            },
            &mut authentication,
            &mut signer,
        )
        .unwrap_or_else(|error| fail_return(&error.to_string()));
        let reloaded = vela_protocol::repo::load_from_path(frontier)
            .unwrap_or_else(|error| fail_return(&error));
        let replayed = load_repository_authority(frontier, &reloaded)
            .unwrap_or_else(|error| fail_return(&error))
            .unwrap_or_else(|| {
                fail_return("repository authority disappeared after policy rotation")
            });
        if replayed
            .history
            .policy_bundle
            .root()
            .map_or(true, |root| root != plan.next_policy_bundle_root)
        {
            fail_return::<()>("policy rotation replay did not activate the exact next bundle");
        }
        if json_out {
            print_json(&json!({
                "ok": true,
                "command": "authority enable-work",
                "frontier": plan.frontier,
                "event_ids": result.event_ids,
                "authority_record_id": result.authority_record_id,
                "authority_record_root": result.authority_record_root,
                "policy_bundle_root": plan.next_policy_bundle_root,
                "after_event_log_root": result.after_event_log_root,
                "writes_now": true
            }));
        } else {
            println!("signed-agent work leases enabled");
            println!("  policy: {}", plan.next_policy_bundle_root);
            println!("  authority record: {}", result.authority_record_id);
            println!("  next: inspect and commit the exact canonical delta");
        }
        let _ = project;
        return;
    }

    if confirm_root.is_some() || confirm_at.is_some() {
        fail_return::<()>(
            "--confirm-root/--confirm-at are valid only with --apply; preview is key-free",
        );
    }
    let observed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let (plan, _, _, _) = prepare_work_policy_plan(frontier, reason, &observed_at)
        .unwrap_or_else(|error| fail_return(&error));
    if json_out {
        print_json(&plan);
    } else {
        println!(
            "authority work policy · key-free preview · {}",
            plan.frontier_name
        );
        println!("  current policy: {}", plan.current_policy_bundle_root);
        println!("  next policy: {}", plan.next_policy_bundle_root);
        println!("  plan root: {}", plan.plan_root);
        println!("  confirm at: {}", plan.observed_at);
        println!("  permits: exact work leases signed by the agent");
        println!("  does not permit: review, acceptance, policy administration, or key rotation");
    }
}

fn prepare_work_policy_plan(
    frontier: &Path,
    reason: &str,
    observed_at: &str,
) -> Result<
    (
        WorkPolicyPlan,
        Project,
        LoadedRepositoryAuthority,
        CedarPolicyMaterial,
    ),
    String,
> {
    if reason.trim().is_empty() || reason != reason.trim() {
        return Err("authority work-policy reason must be non-empty and trimmed".into());
    }
    let frontier = frontier
        .canonicalize()
        .map_err(|error| format!("resolve frontier path: {error}"))?;
    let (git_commit, git_tree) = require_clean_main(&frontier)?;
    let project = vela_protocol::repo::load_from_path(&frontier)?;
    let authority = load_repository_authority(&frontier, &project)?
        .ok_or_else(|| "frontier has not crossed to repository authority".to_string())?;
    if authority
        .policy_material
        .schema
        .contains("action \"work_claim\"")
    {
        return Err("signed-agent work leases are already enabled".into());
    }
    let migration = migration_payload(&authority)?;
    let current_root = authority.history.policy_bundle.root()?;
    let schema = format!(
        "{}\n{}\n",
        authority.policy_material.schema, ROUTINE_WORK_SCHEMA
    );
    let policies = format!(
        "{}\n{}\n",
        authority.policy_material.policies, ROUTINE_WORK_POLICY
    );
    let next_material = CedarPolicyMaterial {
        schema,
        policies,
        entities: authority.policy_material.entities.clone(),
    };
    let next_bundle = PolicyBundleV1 {
        schema: POLICY_BUNDLE_SCHEMA_V1.into(),
        frontier_id: project.frontier_id(),
        cedar_schema_root: ContentDigest::hash(next_material.schema.as_bytes())
            .as_str()
            .into(),
        policies_root: ContentDigest::hash(next_material.policies.as_bytes())
            .as_str()
            .into(),
        entities_root: ContentDigest::hash(to_canonical_bytes(&next_material.entities)?)
            .as_str()
            .into(),
        tests_root: ROUTINE_WORK_POLICY_TESTS_ROOT.into(),
        engine: authority.history.policy_bundle.engine.clone(),
        engine_version: authority.history.policy_bundle.engine_version.clone(),
        restricted_profile: authority.history.policy_bundle.restricted_profile.clone(),
        previous_bundle_root: Some(current_root.clone()),
        authority_summary: "Preserve the exact scientific and human-authority lanes; signed agents may coordinate exact work leases and gain no scientific or governance authority.".into(),
    };
    next_material.validate_against(&next_bundle)?;
    let verification_input = CedarEvaluationInput {
        schema: next_material.schema.clone(),
        policies: next_material.policies.clone(),
        entities: next_material.entities.clone(),
        principal: r#"Human::"verification-only""#.into(),
        principal_class: PrincipalClass::Human,
        action: POLICY_ROTATE_ACTION.into(),
        resource: format!(
            "Frontier::{}",
            serde_json::to_string(&project.frontier_id()).unwrap()
        ),
        context: json!({"exact": true}),
    };
    verify_routine_work_policy(&verification_input, &project)?;
    let next_root = next_bundle.root()?;
    let mut plan = WorkPolicyPlan {
        schema: WORK_POLICY_PLAN_SCHEMA.into(),
        ok: true,
        command: "authority enable-work".into(),
        frontier: frontier.display().to_string(),
        frontier_id: project.frontier_id(),
        frontier_name: project.project.name.clone(),
        git_commit,
        git_tree,
        observed_at: canonical_whole_second_time("work-policy observation", observed_at)?,
        principal_id: migration.new_principal_id,
        current_policy_bundle_root: current_root,
        next_policy_bundle: next_bundle,
        next_policy_bundle_root: next_root,
        reason: reason.into(),
        writes_now: false,
        requires_user_presence: true,
        plan_root: String::new(),
    };
    plan.plan_root = work_policy_plan_root(&plan)?;
    Ok((plan, project, authority, next_material))
}

fn migration_payload(
    authority: &LoadedRepositoryAuthority,
) -> Result<AuthorityModelMigrationV1, String> {
    authority
        .history
        .legacy_events
        .iter()
        .find(|event| event.kind == EventKind::AuthorityModelMigrated)
        .ok_or_else(|| "verified authority history lacks its migration bridge".to_string())
        .and_then(|event| {
            serde_json::from_value(event.payload.clone())
                .map_err(|error| format!("decode migration payload: {error}"))
        })
}

fn active_repository_key(
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

fn work_policy_plan_root(plan: &WorkPolicyPlan) -> Result<String, String> {
    let mut value = serde_json::to_value(plan).map_err(|error| error.to_string())?;
    value
        .as_object_mut()
        .ok_or_else(|| "work-policy plan must be an object".to_string())?
        .insert("plan_root".into(), Value::String(String::new()));
    let mut digest = Sha256::new();
    digest.update(WORK_POLICY_PLAN_DOMAIN);
    digest.update(to_canonical_bytes(&value)?);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn build_authority_intent_request(
    plan: &WorkPolicyPlan,
) -> Result<vela_signer::AuthorityIntentRequest, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let helper = crate::cli_identity::signer_helper_path(&executable)?;
    let helper_sha256 = vela_signer::contract::file_sha256(&helper)?;
    let mut nonce = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let request = vela_signer::AuthorityIntentRequest {
        schema: vela_signer::AUTHORITY_INTENT_REQUEST_SCHEMA.into(),
        nonce: hex::encode(nonce),
        expires_at: (Utc::now()
            + Duration::seconds(vela_signer::AUTHORITY_INTENT_REQUEST_LIFETIME_SECONDS))
        .to_rfc3339_opts(SecondsFormat::Secs, true),
        vela_binary_path: executable.display().to_string(),
        vela_binary_sha256: vela_signer::contract::file_sha256(&executable)?,
        helper_sha256,
        frontier_id: plan.frontier_id.clone(),
        frontier_name: plan.frontier_name.clone(),
        principal_id: plan.principal_id.clone(),
        action: POLICY_ROTATE_ACTION.into(),
        reason: plan.reason.clone(),
        intent_digest: plan.plan_root.clone(),
        current_policy_bundle_root: plan.current_policy_bundle_root.clone(),
        next_policy_bundle_root: plan.next_policy_bundle_root.clone(),
    };
    vela_signer::validate_authority_intent_request(&request, Utc::now())?;
    Ok(request)
}

fn request_authority_intent(
    request: &vela_signer::AuthorityIntentRequest,
) -> Result<vela_signer::AuthorityIntentResponse, String> {
    let helper = PathBuf::from(&request.vela_binary_path)
        .parent()
        .ok_or_else(|| "running Vela binary has no parent directory".to_string())?
        .join(if cfg!(target_os = "windows") {
            "vela-signer.exe"
        } else {
            "vela-signer"
        });
    let bytes = serde_json::to_vec(request)
        .map_err(|error| format!("encode authority-intent request: {error}"))?;
    let mut child = Command::new(&helper)
        .arg("authenticate-authority-intent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start pinned authority-intent helper: {error}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "authority-intent helper stdin is unavailable".to_string())?
        .write_all(&bytes)
        .map_err(|error| format!("write authority-intent request: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for authority-intent helper: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "authority-intent helper declined or failed: {}",
            super::safe_text::inline(String::from_utf8_lossy(&output.stderr).trim())
        ));
    }
    let response: vela_signer::AuthorityIntentResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("decode authority-intent response: {error}"))?;
    vela_signer::validate_authority_intent_response(request, &response)?;
    Ok(response)
}

fn execute_signed_migration<A, S>(
    barrier: crate::frontier_txn::CanonicalWriteBarrier,
    frontier: &Path,
    locked: PreparedPlan,
    event_signature: String,
    recorded_at: String,
    authentication_adapter: &mut A,
    repository_signer: &mut S,
) -> Result<AuthorityMigrationExecution, String>
where
    A: AuthenticationAdapter,
    S: RepositoryAuthoritySigner,
{
    let mut migration_event = locked.plan.migration_event.clone();
    migration_event.signature = Some(event_signature);
    vela_protocol::sign::verify_event_signature(&migration_event, &locked.plan.legacy_public_key)?;
    let migration_event_root = ContentDigest::hash(to_canonical_bytes(&migration_event)?)
        .as_str()
        .to_string();
    let result = execute_authority_migration(
        barrier,
        frontier,
        AuthorityMigrationRequest {
            history: AuthorityMigrationHistorySnapshot {
                frontier_id: locked.plan.frontier_id.clone(),
                legacy_events: locked.project.events.clone(),
                legacy_actor_registry_bytes: locked.actors_bytes,
                legacy_active_policy_head_root: locked.plan.legacy_active_policy_head_root.clone(),
                legacy_policy_store_manifest_root: locked
                    .plan
                    .legacy_policy_store_manifest_root
                    .clone(),
            },
            migration_event: migration_event.clone(),
            authority_keyset: locked.plan.authority_keyset.clone(),
            policy_bundle: locked.plan.policy_bundle.clone(),
            intent_digest: migration_event_root.clone(),
            principal: locked.principal,
            authentication_request: AuthenticationRequest {
                principal_id: locked.plan.new_principal_id.clone(),
                principal_class: PrincipalClass::Human,
                transaction_at: recorded_at.clone(),
            },
            runtime_session_state: RuntimeSessionState::default(),
            authorization_input: locked.authorization_input,
            read_set: locked.read_set,
            vela_version: locked.plan.vela_version.clone(),
            binary_sha256: locked.plan.vela_binary_sha256.clone(),
            recorded_at,
        },
        authentication_adapter,
        repository_signer,
    )
    .map_err(|error| error.to_string())?;
    Ok(AuthorityMigrationExecution {
        schema: "vela.authority-migration-execution.v1".into(),
        ok: true,
        command: "authority migrate".into(),
        plan_root: locked.plan.plan_root,
        migration_event_id: migration_event.id,
        migration_event_root,
        authority_keyset_root: locked.plan.authority_keyset_root,
        policy_bundle_root: locked.plan.policy_bundle_root,
        authority_record_id: result.authority_record_id,
        authority_record_root: result.authority_record_root,
        before_event_log_root: result.before_event_log_root,
        after_event_log_root: result.after_event_log_root,
        operation_id: result.operation_id,
        transaction_id: result.transaction_id,
        git_publication: "uncommitted_exact_canonical_delta".into(),
    })
}

fn prepare_plan(
    frontier: &Path,
    repository_key_id: &str,
    repository_public_key: &str,
    reason: &str,
    observed_at: &str,
) -> Result<PreparedPlan, String> {
    let identity = crate::cli_identity::load_administrative_identity()?;
    if identity.actor_type != "human" {
        return Err("authority migration requires the configured human identity".into());
    }
    let local_session = local_session(observed_at)?;
    prepare_plan_with_context(
        frontier,
        repository_key_id,
        repository_public_key,
        reason,
        observed_at,
        &identity.actor_id,
        &identity.pubkey,
        local_session,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_plan_with_context(
    frontier: &Path,
    repository_key_id: &str,
    repository_public_key: &str,
    reason: &str,
    observed_at: &str,
    identity_actor_id: &str,
    identity_public_key: &str,
    local_session: LocalOsSession,
) -> Result<PreparedPlan, String> {
    if repository_key_id.trim().is_empty() {
        return Err("repository key ID must not be empty".into());
    }
    require_ed25519_public_key(repository_public_key)?;
    if reason.trim().is_empty()
        || reason != reason.trim()
        || reason.len() > 1024
        || reason.chars().any(char::is_control)
    {
        return Err("migration reason must be trimmed, non-empty, and at most 1024 bytes".into());
    }
    DateTime::parse_from_rfc3339(observed_at)
        .map_err(|error| format!("migration observation time is invalid: {error}"))?;
    let frontier = std::fs::canonicalize(frontier)
        .map_err(|error| format!("resolve authority migration Frontier: {error}"))?;
    let (git_commit, git_tree) = require_clean_main(&frontier)?;
    let project = vela_protocol::repo::load_from_path(&frontier)?;
    if project.events.iter().any(|event| {
        event.kind == EventKind::AuthorityModelMigrated
            || event.kind.as_str() == "authority.model_migrated"
    }) {
        return Err("Frontier already crossed the authority-model migration boundary".into());
    }
    for directory in [
        ".vela/authority/events",
        ".vela/authority/records",
        ".vela/authority/keysets",
        ".vela/authority/policies",
    ] {
        let path = frontier.join(directory);
        if path.exists()
            && std::fs::read_dir(&path)
                .map_err(|error| format!("inspect {directory}: {error}"))?
                .next()
                .is_some()
        {
            return Err(format!(
                "pre-migration authority directory {directory} must be empty"
            ));
        }
    }
    let legacy_actor = vela_protocol::proposals::validate_human_reviewer_authority_at(
        &project,
        identity_actor_id,
        observed_at,
    )?;
    if legacy_actor.public_key != identity_public_key {
        return Err(
            "configured protected identity differs from the registered legacy administrator".into(),
        );
    }
    let actors_bytes = std::fs::read(frontier.join(".vela/actors.json"))
        .map_err(|error| format!("read legacy actor registry: {error}"))?;
    let legacy_actor_registry_root = ContentDigest::hash(&actors_bytes).as_str().to_string();
    let principal = PrincipalSnapshotV1 {
        principal_id: local_session.principal_id.clone(),
        principal_class: PrincipalClass::Human,
        display_name: Some("Repository administrator".into()),
        affiliation: None,
        account_links: vec![local_session.principal_id.clone()],
    };
    let (policy_head_root, policy_store_root, policy_mode, mut policy_read_set) =
        legacy_policy_commitments(&frontier)?;
    let (policy_bundle, authorization_input) =
        initial_policy_bundle_at(&frontier, &project, &principal.principal_id, observed_at)?;
    let policy_bundle_root = policy_bundle.root()?;
    let authority_keyset = AuthorityKeysetV1 {
        schema: AUTHORITY_KEYSET_SCHEMA_V1.into(),
        frontier_id: project.frontier_id(),
        generation: 1,
        threshold: 1,
        keys: vec![AuthorityKeyV1 {
            key_id: repository_key_id.into(),
            algorithm: AUTHORITY_KEY_ALGORITHM.into(),
            public_key: repository_public_key.into(),
            valid_from_sequence: 1,
            valid_through_sequence: None,
            purpose: AUTHORITY_KEY_PURPOSE.into(),
        }],
        previous_keyset_root: None,
        activation_record_root: None,
        closed: false,
    };
    let authority_keyset_root = authority_keyset.root()?;
    let migration_payload = AuthorityModelMigrationV1 {
        schema: AUTHORITY_MODEL_MIGRATION_SCHEMA_V1.into(),
        frontier_id: project.frontier_id(),
        legacy_event_log_root: format!("sha256:{}", event_log_hash(&project.events)),
        legacy_actor_registry_root: legacy_actor_registry_root.clone(),
        legacy_active_policy_head_root: policy_head_root.clone(),
        legacy_policy_store_manifest_root: policy_store_root.clone(),
        new_authority_keyset_root: authority_keyset_root.clone(),
        new_policy_bundle_root: policy_bundle_root.clone(),
        new_principal_id: principal.principal_id.clone(),
        minimum_writer_version: MINIMUM_WRITER_VERSION.into(),
        reason: reason.into(),
    };
    migration_payload.validate()?;
    let mut migration_event = StateEvent {
        schema: EVENT_SCHEMA.into(),
        id: String::new(),
        kind: EventKind::AuthorityModelMigrated,
        target: StateTarget {
            r#type: "frontier".into(),
            id: project.frontier_id(),
        },
        actor: StateActor {
            r#type: "human".into(),
            id: legacy_actor.id.clone(),
        },
        timestamp: observed_at.into(),
        reason: reason.into(),
        before_hash: NULL_HASH.into(),
        after_hash: NULL_HASH.into(),
        payload: serde_json::to_value(migration_payload).map_err(|error| error.to_string())?,
        caveats: vec![
            "Historical events remain byte-identical.".into(),
            "This is the final live authority event signed by the legacy personal key.".into(),
        ],
        signature: None,
    };
    migration_event.id = compute_event_id(&migration_event);
    let executable =
        std::env::current_exe().map_err(|error| format!("resolve current Vela binary: {error}"))?;
    let vela_binary_sha256 = vela_signer::contract::file_sha256(&executable)?;
    let mut read_set =
        vec![InputBinding::project_snapshot(&project).map_err(|error| error.to_string())?];
    for path in ["frontier.yaml", "vela.lock"] {
        if frontier.join(path).is_file() {
            read_set.push(
                InputBinding::existing_file(
                    &frontier,
                    RepoPath::parse(path).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?,
            );
        }
    }
    read_set.append(&mut policy_read_set);
    let mut plan = AuthorityMigrationPlan {
        schema: PLAN_SCHEMA.into(),
        ok: true,
        command: "authority migrate".into(),
        frontier: frontier.display().to_string(),
        frontier_id: project.frontier_id(),
        frontier_name: project.project.name.clone(),
        git_commit,
        git_tree,
        vela_version: env!("CARGO_PKG_VERSION").into(),
        vela_binary_sha256,
        observed_at: observed_at.into(),
        legacy_actor: legacy_actor.id,
        legacy_public_key: legacy_actor.public_key,
        legacy_event_log_root: format!("sha256:{}", event_log_hash(&project.events)),
        legacy_event_count: project.events.len(),
        legacy_actor_registry_root,
        legacy_active_policy_head_root: policy_head_root,
        legacy_policy_store_manifest_root: policy_store_root,
        legacy_policy_mode: policy_mode,
        new_principal_id: principal.principal_id.clone(),
        repository_key_id: repository_key_id.into(),
        repository_public_key: repository_public_key.into(),
        authority_keyset,
        authority_keyset_root,
        policy_bundle,
        policy_bundle_root,
        migration_event,
        touched_paths: vec![
            ".vela/events/<migration-event>.json".into(),
            ".vela/authority/keysets/<full-root>.json".into(),
            ".vela/authority/policies/<full-root>.json".into(),
            ".vela/authority/records/<full-root>.dsse.json".into(),
        ],
        reason: reason.into(),
        writes_now: false,
        requires_one_legacy_approval: true,
        plan_root: String::new(),
    };
    plan.plan_root = plan_root(&plan)?;
    Ok(PreparedPlan {
        plan,
        project,
        actors_bytes,
        authorization_input,
        principal,
        read_set,
    })
}

pub(crate) fn initial_policy_bundle_at(
    frontier: &Path,
    project: &Project,
    principal_id: &str,
    observed_at: &str,
) -> Result<(PolicyBundleV1, CedarEvaluationInput), String> {
    initial_policy_bundle_at_with_routine_work(frontier, project, principal_id, observed_at, true)
}

fn initial_policy_bundle_at_with_routine_work(
    frontier: &Path,
    project: &Project,
    principal_id: &str,
    observed_at: &str,
    include_routine_work: bool,
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
    let schema = if include_routine_work {
        format!("{automatic_schema}\n{HUMAN_AUTHORITY_SCHEMA}\n{ROUTINE_WORK_SCHEMA}\n")
    } else {
        // Preserve the exact sequence-1 bytes emitted by Vela 0.930.0-rc.7.
        format!("{automatic_schema}\n{HUMAN_AUTHORITY_SCHEMA}\n")
    };
    let human_policy = human_authority_policy(principal_id)?;
    let policies = if include_routine_work {
        format!("{automatic_policies}\n{human_policy}\n{ROUTINE_WORK_POLICY}\n")
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
        } else {
            if include_routine_work {
                "No automatic scientific admission; signed agents may coordinate exact work leases; one local human principal may decide reviews and administer repository authority.".into()
            } else {
                "No automatic scientific admission; one local human principal may decide reviews and administer repository authority.".into()
            }
        },
    };
    let authorization = CedarEvaluationInput {
        schema,
        policies,
        entities,
        principal: format!("Human::{}", serde_json::to_string(principal_id).unwrap()),
        principal_class: PrincipalClass::Human,
        action: AUTHORITY_MIGRATION_ACTION.into(),
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
    if migrations.is_empty() {
        return Ok(None);
    }
    let [migration_event] = migrations.as_slice() else {
        return Err("frontier contains multiple authority.model_migrated events".into());
    };
    let migration: AuthorityModelMigrationV1 =
        serde_json::from_value(migration_event.payload.clone())
            .map_err(|error| format!("decode authority migration payload: {error}"))?;
    migration.validate()?;

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
    let actors_path = frontier.join(".vela/actors.json");
    let legacy_actor_registry_bytes = std::fs::read(&actors_path)
        .map_err(|error| format!("read {}: {error}", actors_path.display()))?;
    let verification = verify_authority_history(AuthorityHistoryInput {
        frontier_id: &migration.frontier_id,
        legacy_events: &project.events,
        legacy_actor_registry_bytes: &legacy_actor_registry_bytes,
        legacy_active_policy_head_root: &migration.legacy_active_policy_head_root,
        legacy_policy_store_manifest_root: &migration.legacy_policy_store_manifest_root,
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
    let policy_material = load_or_reconstruct_policy_material(
        frontier,
        project,
        migration_event,
        &migration,
        &policy_bundle,
        verification.authority_record_count,
    )?;
    Ok(Some(LoadedRepositoryAuthority {
        history: AuthorityHistorySnapshot {
            frontier_id: migration.frontier_id,
            legacy_events: project.events.clone(),
            legacy_actor_registry_bytes,
            legacy_active_policy_head_root: migration.legacy_active_policy_head_root,
            legacy_policy_store_manifest_root: migration.legacy_policy_store_manifest_root,
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
    let payload = BASE64_STANDARD
        .decode(&envelope.payload)
        .map_err(|error| format!("authority envelope payload is not base64: {error}"))?;
    let record: AuthorityRecordV1 = serde_json::from_slice(&payload)
        .map_err(|error| format!("authority envelope record JSON is invalid: {error}"))?;
    Ok(record.content.sequence)
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

fn human_authority_policy(principal_id: &str) -> Result<String, String> {
    if principal_id.trim().is_empty() || principal_id.chars().any(char::is_control) {
        return Err("repository administrator principal is invalid".into());
    }
    let principal = serde_json::to_string(principal_id)
        .map_err(|error| format!("encode repository administrator principal: {error}"))?;
    Ok(format!(
        r#"permit (
    principal == Human::{principal},
    action in [
        Action::"authority_model_migrate",
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

fn legacy_policy_commitments(
    frontier: &Path,
) -> Result<(String, String, String, Vec<InputBinding>), String> {
    let snapshot = vela_protocol::acceptance_policy::load_active_policy_snapshot(frontier)?;
    let head = PolicyHeadCommitment {
        schema: POLICY_HEAD_SCHEMA.into(),
        mode: snapshot.mode.as_str().into(),
        policy_root: snapshot
            .policy_bytes
            .as_ref()
            .map(|bytes| ContentDigest::hash(bytes).as_str().to_string()),
        signature_root: snapshot
            .signature_bytes
            .as_ref()
            .map(|bytes| ContentDigest::hash(bytes).as_str().to_string()),
        policy_id: snapshot.policy().map(|policy| policy.id.clone()),
    };
    let head_root = canonical_root(&head)?;
    let directory = frontier.join(POLICY_DIRECTORY);
    let mut entries = Vec::new();
    let mut paths = Vec::new();
    let mut read_set = Vec::new();
    if directory.exists() {
        let metadata = std::fs::symlink_metadata(&directory)
            .map_err(|error| format!("inspect legacy policy directory: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("legacy policy store must be a real directory".into());
        }
        let mut total_bytes = 0_u64;
        for entry in std::fs::read_dir(&directory)
            .map_err(|error| format!("read legacy policy directory: {error}"))?
        {
            let entry = entry.map_err(|error| format!("read legacy policy entry: {error}"))?;
            let metadata = std::fs::symlink_metadata(entry.path())
                .map_err(|error| format!("inspect legacy policy entry: {error}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "legacy policy store entry '{}' must be a regular file",
                    entry.path().display()
                ));
            }
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| "legacy policy store byte count overflow".to_string())?;
            if total_bytes > MAX_POLICY_STORE_BYTES {
                return Err("legacy policy store exceeds the 2 MiB migration limit".into());
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "legacy policy filename is not UTF-8".to_string())?;
            let path = format!("{POLICY_DIRECTORY}/{name}");
            let bytes = std::fs::read(entry.path())
                .map_err(|error| format!("read legacy policy {name}: {error}"))?;
            entries.push(PolicyStoreEntry {
                path: path.clone(),
                content_root: ContentDigest::hash(bytes).as_str().to_string(),
            });
            let repo_path = RepoPath::parse(path).map_err(|error| error.to_string())?;
            read_set.push(
                InputBinding::existing_file(frontier, repo_path.clone())
                    .map_err(|error| error.to_string())?,
            );
            paths.push(repo_path);
        }
        if entries.len() > MAX_POLICY_STORE_FILES {
            return Err("legacy policy store exceeds 256 files".into());
        }
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        paths.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        read_set.push(
            InputBinding::exact_directory(
                frontier,
                RepoPath::parse(POLICY_DIRECTORY).map_err(|error| error.to_string())?,
                &paths,
            )
            .map_err(|error| error.to_string())?,
        );
    } else {
        read_set.push(
            InputBinding::absent_file(
                frontier,
                RepoPath::parse(POLICY_DIRECTORY).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?,
        );
    }
    let store = PolicyStoreCommitment {
        schema: POLICY_STORE_SCHEMA.into(),
        entries,
    };
    Ok((
        head_root,
        canonical_root(&store)?,
        snapshot.mode.as_str().into(),
        read_set,
    ))
}

fn build_protected_request(
    prepared: &PreparedPlan,
    profile: &crate::cli_identity::ProtectedSignerProfile,
) -> Result<vela_signer::AuthorityMigrationSignerRequest, String> {
    let vela_binary =
        std::env::current_exe().map_err(|error| format!("resolve running Vela binary: {error}"))?;
    let helper = crate::cli_identity::signer_helper_path(&vela_binary)?;
    let helper_sha256 = vela_signer::contract::file_sha256(&helper)?;
    if helper_sha256 != profile.helper_sha256 {
        return Err(format!(
            "installed signer helper {helper_sha256} does not match protected identity pin {}; rebind the released migration helper once before this final legacy-key operation",
            profile.helper_sha256
        ));
    }
    let mut nonce = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let now = Utc::now();
    let request = vela_signer::AuthorityMigrationSignerRequest {
        schema: vela_signer::AUTHORITY_MIGRATION_REQUEST_SCHEMA.into(),
        nonce: hex::encode(nonce),
        expires_at: (now
            + Duration::seconds(vela_signer::AUTHORITY_MIGRATION_REQUEST_LIFETIME_SECONDS))
        .to_rfc3339_opts(SecondsFormat::Nanos, true),
        vela_binary_path: vela_binary.display().to_string(),
        vela_binary_sha256: prepared.plan.vela_binary_sha256.clone(),
        helper_sha256,
        frontier_id: prepared.plan.frontier_id.clone(),
        frontier_path: prepared.plan.frontier.clone(),
        frontier_name: prepared.plan.frontier_name.clone(),
        reason: prepared.plan.reason.clone(),
        legacy_actor: prepared.plan.legacy_actor.clone(),
        legacy_public_key: prepared.plan.legacy_public_key.clone(),
        observed_at: prepared.plan.observed_at.clone(),
        migration_plan_root: prepared.plan.plan_root.clone(),
        new_principal_id: prepared.plan.new_principal_id.clone(),
        new_authority_keyset_root: prepared.plan.authority_keyset_root.clone(),
        new_policy_bundle_root: prepared.plan.policy_bundle_root.clone(),
        provider: profile.provider.clone(),
        protection_grade: profile.protection_grade.clone(),
        protection_mode: profile.mode,
        event: prepared.plan.migration_event.clone(),
    };
    vela_signer::validate_authority_migration_request(&request, now)?;
    Ok(request)
}

fn request_legacy_signature(
    request: &vela_signer::AuthorityMigrationSignerRequest,
) -> Result<vela_signer::AuthorityMigrationSignerResponse, String> {
    let helper = PathBuf::from(&request.vela_binary_path)
        .parent()
        .ok_or_else(|| "running Vela binary has no parent directory".to_string())?
        .join(if cfg!(target_os = "windows") {
            "vela-signer.exe"
        } else {
            "vela-signer"
        });
    let bytes = serde_json::to_vec(request)
        .map_err(|error| format!("encode authority-migration signer request: {error}"))?;
    let mut child = Command::new(&helper)
        .arg("approve-authority-migration")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!(
                "start pinned migration helper {}: {error}",
                helper.display()
            )
        })?;
    child
        .stdin
        .take()
        .ok_or_else(|| "migration helper stdin is unavailable".to_string())?
        .write_all(&bytes)
        .map_err(|error| format!("write migration helper request: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for migration helper: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "migration helper declined or failed: {}",
            super::safe_text::inline(String::from_utf8_lossy(&output.stderr).trim())
        ));
    }
    let response: vela_signer::AuthorityMigrationSignerResponse =
        serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("decode migration helper response: {error}"))?;
    vela_signer::validate_authority_migration_response(request, &response)?;
    Ok(response)
}

fn canonical_whole_second_time(name: &str, value: &str) -> Result<String, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| {
            value
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        })
        .map_err(|error| format!("{name} is not RFC3339: {error}"))
}

fn local_session(observed_at: &str) -> Result<LocalOsSession, String> {
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

fn require_clean_main(frontier: &Path) -> Result<(String, String), String> {
    let branch = git(frontier, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    if branch != "main" {
        return Err(format!(
            "authority migration requires main, found {branch:?}"
        ));
    }
    let status = git(
        frontier,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    if !status.is_empty() {
        return Err("authority migration requires a clean tracked and untracked worktree".into());
    }
    let commit = git(frontier, &["rev-parse", "--verify", "HEAD"])?;
    let tree = git(frontier, &["rev-parse", "--verify", "HEAD^{tree}"])?;
    Ok((commit, tree))
}

fn git(frontier: &Path, args: &[&str]) -> Result<String, String> {
    let output = crate::git_hardened::output(frontier, args)?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|_| format!("git {} output is not UTF-8", args.join(" ")))
}

fn require_ed25519_public_key(value: &str) -> Result<(), String> {
    let bytes = hex::decode(value)
        .map_err(|error| format!("decode repository authority public key: {error}"))?;
    if bytes.len() != 32 || value.len() != 64 || value.to_ascii_lowercase() != value {
        return Err(
            "repository authority public key must be 64 lowercase Ed25519 hex characters".into(),
        );
    }
    Ok(())
}

fn canonical_root(value: &impl Serialize) -> Result<String, String> {
    Ok(ContentDigest::hash(to_canonical_bytes(value)?)
        .as_str()
        .to_string())
}

fn plan_root(plan: &AuthorityMigrationPlan) -> Result<String, String> {
    let mut value = serde_json::to_value(plan).map_err(|error| error.to_string())?;
    value
        .as_object_mut()
        .ok_or_else(|| "authority migration plan must be an object".to_string())?
        .insert("plan_root".into(), Value::String(String::new()));
    let bytes = to_canonical_bytes(&value)?;
    let mut digest = Sha256::new();
    digest.update(PLAN_DOMAIN);
    digest.update(bytes);
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use ed25519_dalek::Signer as _;
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;
    use vela_protocol::authority::{
        AUTHORITY_PAYLOAD_TYPE_V1, AuthorityEnvelopeV1, DsseSignatureV1, dsse_pae,
    };
    use vela_protocol::authority_history::{AuthorityHistoryInput, verify_authority_history};
    use vela_protocol::sign::ActorRecord;

    use crate::authority_transaction::{
        authority_keyset_path, authority_policy_path, authority_record_path,
    };

    const OBSERVED_AT: &str = "2026-07-24T12:00:00Z";
    const PRINCIPAL_ID: &str = "local:device-fixture|uid:501";

    struct Fixture {
        temporary: TempDir,
        actor_key: SigningKey,
        repository_key: SigningKey,
        actor_public_key: String,
        repository_public_key: String,
    }

    impl Fixture {
        fn root(&self) -> &Path {
            self.temporary.path()
        }

        fn session(&self) -> LocalOsSession {
            LocalOsSession {
                principal_id: PRINCIPAL_ID.into(),
                issuer: "device-fixture".into(),
                subject: "uid:501".into(),
                session_root: format!("sha256:{}", "8".repeat(64)),
                authenticated_at: OBSERVED_AT.into(),
                expires_at: "2026-07-24T13:00:00Z".into(),
                recovery_recent: false,
            }
        }

        fn prepare(&self, reason: &str) -> Result<PreparedPlan, String> {
            prepare_plan_with_context(
                self.root(),
                "repository-key-fixture",
                &self.repository_public_key,
                reason,
                OBSERVED_AT,
                "reviewer:fixture",
                &self.actor_public_key,
                self.session(),
            )
        }
    }

    fn fixture() -> Fixture {
        let temporary = TempDir::new().unwrap();
        let actor_key = SigningKey::from_bytes(&[41; 32]);
        let repository_key = SigningKey::from_bytes(&[42; 32]);
        let actor_public_key = hex::encode(actor_key.verifying_key().to_bytes());
        let repository_public_key = hex::encode(repository_key.verifying_key().to_bytes());
        let mut project = vela_protocol::project::assemble(
            "migration-fixture",
            Vec::new(),
            0,
            0,
            "Disposable authority-migration fixture.",
        );
        project.actors.push(ActorRecord {
            id: "reviewer:fixture".into(),
            public_key: actor_public_key.clone(),
            algorithm: "ed25519".into(),
            created_at: "2026-07-01T00:00:00Z".into(),
            tier: None,
            orcid: None,
            access_clearance: None,
            revoked_at: None,
            revoked_reason: None,
        });
        vela_protocol::repo::init_repo(temporary.path(), &project).unwrap();
        for arguments in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "Vela Fixture"],
            vec!["config", "user.email", "fixture@vela.invalid"],
            vec!["add", "-A"],
            vec!["commit", "-qm", "fixture baseline"],
        ] {
            let output = Command::new("git")
                .arg("-C")
                .arg(temporary.path())
                .args(&arguments)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {} failed: {}",
                arguments.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Fixture {
            temporary,
            actor_key,
            repository_key,
            actor_public_key,
            repository_public_key,
        }
    }

    struct TestRepositorySigner {
        key: SigningKey,
    }

    impl RepositoryAuthoritySigner for TestRepositorySigner {
        fn sign(
            &mut self,
            payload_type: &str,
            canonical_payload: &[u8],
        ) -> Result<Vec<DsseSignatureV1>, String> {
            if payload_type != AUTHORITY_PAYLOAD_TYPE_V1 {
                return Err("unexpected repository-authority payload type".into());
            }
            Ok(vec![DsseSignatureV1 {
                keyid: "repository-key-fixture".into(),
                sig: BASE64_STANDARD.encode(
                    self.key
                        .sign(&dsse_pae(payload_type, canonical_payload))
                        .to_bytes(),
                ),
            }])
        }
    }

    #[test]
    fn key_free_plan_is_stable_and_binds_reason_and_repository_key() {
        let fixture = fixture();
        let first = fixture
            .prepare("Move the disposable fixture to repository authority.")
            .unwrap();
        let repeated = fixture
            .prepare("Move the disposable fixture to repository authority.")
            .unwrap();
        assert_eq!(first.plan.plan_root, repeated.plan.plan_root);
        assert_eq!(first.plan.migration_event.signature, None);
        assert!(!first.plan.writes_now);
        assert!(first.plan.requires_one_legacy_approval);

        let changed_reason = fixture
            .prepare("Move this exact fixture to repository authority.")
            .unwrap();
        assert_ne!(first.plan.plan_root, changed_reason.plan.plan_root);

        let other_key = SigningKey::from_bytes(&[43; 32]);
        let changed_key = prepare_plan_with_context(
            fixture.root(),
            "repository-key-fixture-2",
            &hex::encode(other_key.verifying_key().to_bytes()),
            "Move the disposable fixture to repository authority.",
            OBSERVED_AT,
            "reviewer:fixture",
            &fixture.actor_public_key,
            fixture.session(),
        )
        .unwrap();
        assert_ne!(first.plan.plan_root, changed_key.plan.plan_root);
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
        let request = AuthenticationRequest {
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
    fn preview_rejects_dirty_or_existing_authority_state() {
        let fixture = fixture();
        std::fs::write(fixture.root().join("untracked.txt"), b"drift\n").unwrap();
        assert!(
            fixture
                .prepare("Move the disposable fixture to repository authority.")
                .unwrap_err()
                .contains("clean tracked and untracked worktree")
        );
        std::fs::remove_file(fixture.root().join("untracked.txt")).unwrap();

        let authority_directory = fixture.root().join(".vela/authority/records");
        std::fs::create_dir_all(&authority_directory).unwrap();
        std::fs::write(authority_directory.join("unexpected.json"), b"{}\n").unwrap();
        let output = Command::new("git")
            .arg("-C")
            .arg(fixture.root())
            .args(["add", "-A"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let output = Command::new("git")
            .arg("-C")
            .arg(fixture.root())
            .args(["commit", "-qm", "inject authority state"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(
            fixture
                .prepare("Move the disposable fixture to repository authority.")
                .unwrap_err()
                .contains("pre-migration authority directory")
        );
    }

    #[test]
    fn composed_disposable_migration_replays_from_a_clean_git_clone() {
        let fixture = fixture();
        let mut prepared = fixture
            .prepare("Move the disposable fixture to repository authority.")
            .unwrap();
        prepared.plan.vela_version = "0.930.0-rc.1".into();
        prepared.plan.plan_root = plan_root(&prepared.plan).unwrap();
        let legacy_events = prepared.project.events.clone();
        let legacy_file = fixture
            .root()
            .join(".vela/events")
            .join(format!("{}.json", legacy_events[0].id));
        let legacy_bytes_before = std::fs::read(&legacy_file).unwrap();
        assert!(
            !legacy_events
                .iter()
                .any(|event| event.id == prepared.plan.migration_event.id),
            "migration event unexpectedly collides with legacy history"
        );
        assert!(
            !fixture
                .root()
                .join(".vela/events")
                .join(format!("{}.json", prepared.plan.migration_event.id))
                .exists(),
            "migration event path must be absent before execution"
        );
        let actors_bytes = prepared.actors_bytes.clone();
        let policy_head_root = prepared.plan.legacy_active_policy_head_root.clone();
        let policy_store_root = prepared.plan.legacy_policy_store_manifest_root.clone();
        let signature =
            vela_protocol::sign::sign_event(&prepared.plan.migration_event, &fixture.actor_key)
                .unwrap();
        let journal_dir = fixture.root().join(".vela/operation-journals");
        let barrier =
            FrontierTxn::acquire_write_barrier_for_test(fixture.root(), &journal_dir).unwrap();
        let mut adapter = fixture.session();
        let mut signer = TestRepositorySigner {
            key: fixture.repository_key.clone(),
        };
        let execution_result = execute_signed_migration(
            barrier,
            fixture.root(),
            prepared,
            signature,
            OBSERVED_AT.into(),
            &mut adapter,
            &mut signer,
        );
        if let Err(error) = &execution_result {
            let legacy_bytes_after = std::fs::read(&legacy_file).unwrap();
            eprintln!(
                "migration failed: {error}; before={} after={} canonical={}",
                ContentDigest::hash(&legacy_bytes_before).as_str(),
                ContentDigest::hash(&legacy_bytes_after).as_str(),
                ContentDigest::hash(to_canonical_bytes(&legacy_events[0]).unwrap()).as_str()
            );
        }
        let execution = execution_result.unwrap();

        let migrated = vela_protocol::repo::load_from_path(fixture.root()).unwrap();
        assert_eq!(migrated.events.len(), legacy_events.len() + 1);
        assert!(
            migrated
                .events
                .iter()
                .any(|event| event.id == execution.migration_event_id)
        );

        for arguments in [
            vec!["add", ".vela/events", ".vela/authority"],
            vec!["commit", "-qm", "migrate disposable authority"],
        ] {
            let output = Command::new("git")
                .arg("-C")
                .arg(fixture.root())
                .args(&arguments)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {} failed: {}",
                arguments.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let clone_parent = TempDir::new().unwrap();
        let clone = clone_parent.path().join("clone");
        let output = Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--no-local",
                fixture.root().to_str().unwrap(),
                clone.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "clean clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let cloned = vela_protocol::repo::load_from_path(&clone).unwrap();
        let migration_event = cloned
            .events
            .iter()
            .find(|event| event.id == execution.migration_event_id)
            .unwrap()
            .clone();
        let keyset: AuthorityKeysetV1 = serde_json::from_slice(
            &std::fs::read(
                clone.join(authority_keyset_path(&execution.authority_keyset_root).unwrap()),
            )
            .unwrap(),
        )
        .unwrap();
        let bundle: PolicyBundleV1 = serde_json::from_slice(
            &std::fs::read(
                clone.join(authority_policy_path(&execution.policy_bundle_root).unwrap()),
            )
            .unwrap(),
        )
        .unwrap();
        let envelope: AuthorityEnvelopeV1 = serde_json::from_slice(
            &std::fs::read(clone.join(authority_record_path(&execution.authority_record_id)))
                .unwrap(),
        )
        .unwrap();
        let mut history = legacy_events;
        history.push(migration_event);
        let replay = verify_authority_history(AuthorityHistoryInput {
            frontier_id: &cloned.frontier_id(),
            legacy_events: &history,
            legacy_actor_registry_bytes: &actors_bytes,
            legacy_active_policy_head_root: &policy_head_root,
            legacy_policy_store_manifest_root: &policy_store_root,
            authority_keysets: std::slice::from_ref(&keyset),
            policy_bundles: std::slice::from_ref(&bundle),
            authority_events: &[],
            authority_envelopes: std::slice::from_ref(&envelope),
        })
        .unwrap();
        assert_eq!(replay.authority_record_count, 1);
        assert_eq!(
            replay.final_authority_record_root,
            Some(execution.authority_record_root)
        );
    }
}
